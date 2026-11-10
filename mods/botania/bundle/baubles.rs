// Botania for Horizon Aurora: the trinkets ("baubles") and the relic rings, added to
// `crates/horizon-client/src/game.rs` as `mod botania_baubles` by horizon-mixins, beside `mod botania`
// (runtime.rs). A modified port of Vazkii's Botania (Botania License, see LICENSE and NOTICE.md in
// this bundle): the rings (Aura, Magnetization, Chordata, Correction, Dexterous Motion, the
// Mantle, the Great Fairy Ring, Far Reach, Thor, Odin, Loki), the Stone of Temperance, the sashes,
// the Tectonic Girdle, the pendants, the amulets, the cloaks and the Benevolent Goddess' Charm,
// and the relics' binding (`RelicImpl`). The numbers live in `botania_core::baubles`.
//
// Horizon has no accessory slots (Curios, Trinkets): a trinket works while it is carried in the
// main inventory (hotbar first) or the off hand, the first stack of each kind, and a relic only
// for the player it is bound to (`canEquip`). Botania's own fallback (`InventoryEquipmentHandler`)
// reads the hotbar alone.
//
// Inner attributes and `//!` comments are not allowed in a horizon-mixins module payload.

use super::{Game, Hit, NativeEffect};
use botania_core::baubles::{self as b, Cloak, RelicLine, RelicTick};
use botania_core::mana_handler::{self, Asker};
use botania_core::mana_item::{Kind, ManaStack, Nbt};
use horizon_client::first_person::Hand;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeBlockRim;
use horizon_client::text::Text;
use horizon_core::geometry::Vec3;
use horizon_world::nbt::{Compound, JavaString, Tag};
use horizon_world::registry::BlockStateId;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

// `SoundSource` ordinals.
const NEUTRAL: usize = 6;
const PLAYERS: usize = 7;
// `ChatFormatting.GRAY`.
const GRAY: u32 = 0xAAAAAA;
// The trinkets' own attribute modifiers (the bauble UUIDs Botania gives them).
const ODIN_MODIFIER: &str = "botania:odin_ring";
const REACH_MODIFIER: &str = "botania:reach_ring";
const GIRDLE_MODIFIER: &str = "botania:knockback_belt";
const STEP_MODIFIER: &str = "botania:travel_belt";
const MAX_HEALTH: &str = "minecraft:max_health";
const KNOCKBACK_RESISTANCE: &str = "minecraft:knockback_resistance";
const STEP_HEIGHT: &str = "minecraft:step_height";
const JUMP_STRENGTH: &str = "minecraft:jump_strength";
// Mod messages: a client's dodge and air jump (`DodgePacket`, `JumpPacket`).
const DODGE_CHANNEL: &str = "botania:dodge";
const JUMP_CHANNEL: &str = "botania:jump";
// The Terra Shatterer and the Terra Truncator (`SequentialBreaker`s that break on Loki's cursors).
const TERRA_PICK: &str = "botania:terra_pick";
const TERRA_AXE: &str = "botania:terra_axe";
// `ToolCommons.raytraceFromEntity(player, 10, false)` in `onBlockStartBreak`.
const BREAKER_REACH: f64 = 10.0;

// The static effects a ring gave, to take back when it is no longer worn (`onUnequipped`).
const GAVE_WATER: u8 = 1;
const GAVE_HASTE: u8 = 2;
const GAVE_INVISIBILITY: u8 = 4;

// ---- state ------------------------------------------------------------------------------------

// What the damage hooks, which have no game, need of a player: what they carried at their last
// tick.
#[derive(Clone, Default)]
struct Guard {
    uuid: u128,
    odin: bool,
    crimson: bool,
    // The first cloak carried, and whether it is cooled down.
    cloak: Option<(Cloak, bool)>,
    health: f32,
    sash_buffer: Option<f32>,
    amulet_jumps: Option<i32>,
}

// A cloak that acted on a hit, for its wearer's next tick: the cooldown, the sound and what it does
// to the others.
struct CloakHit {
    player: u128,
    cloak: Cloak,
    amount: f32,
    attacker: i32,
}

#[derive(Default)]
struct Client {
    // `ClientTickHandler.ticksInGame`.
    ticks: i32,
    // The movement keys as the last movement input had them: strafe (left +), forward, jump.
    strafe: f32,
    forward: f32,
    jump: bool,
    dodge: b::DodgeKeys,
    amulet: b::AmuletJumps,
    // The Planestrider's Sash's speed and where the player was (`TAG_SPEED`, `oldX`...), kept on
    // the client that alone uses them.
    planestrider: f32,
    old_pos: [f64; 3],
    // The jump the sash already raised (the ground jump's `noJumpDelay` of 10 lasts a tick).
    jump_raised: bool,
    // This player's profile name, for the relics' tooltip.
    own: Option<(u128, String)>,
    profile_retry: i32,
}

#[derive(Default)]
struct State {
    // Each player's ticks since they joined (`tickCount`).
    ticks: HashMap<u128, i32>,
    // The static effects given to each player.
    gave: HashMap<u128, u8>,
    // By entity id: what the damage hooks read.
    guards: HashMap<i32, Guard>,
    cloak_hits: Vec<CloakHit>,
    // Entity ids of the players whose last jump was an amulet's (`JUMPING_PLAYERS`).
    jumping: HashSet<i32>,
    // A Loki ring is placing or breaking its copies (`recCall`).
    loki_busy: bool,
    client: Client,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

// Never held across a call into the game: the game's helpers come back into the hooks.
fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- the stack's NBT (`ItemNBTHelper` keys in `minecraft:custom_data`) -------------------------

fn key(k: &str) -> JavaString {
    JavaString::from(k)
}

fn tag_int(tag: Option<&Tag>) -> Option<i32> {
    match tag? {
        Tag::Int(v) => Some(*v),
        Tag::Short(v) => Some(i32::from(*v)),
        Tag::Byte(v) => Some(i32::from(*v)),
        Tag::Long(v) => Some((*v).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32),
        _ => None,
    }
}

fn custom_int_or(stack: &ItemStack, k: &str, default: i32) -> i32 {
    tag_int(stack.custom_data().and_then(|c| c.get(&key(k)))).unwrap_or(default)
}

fn custom_int(stack: &ItemStack, k: &str) -> i32 {
    custom_int_or(stack, k, 0)
}

fn custom_bool(stack: &ItemStack, k: &str) -> bool {
    custom_int(stack, k) != 0
}

// Changes the stack's `minecraft:custom_data`, where Botania keeps its item NBT.
fn edit_custom(stack: &mut ItemStack, f: impl FnOnce(&mut Compound)) {
    let mut c = stack.custom_data().cloned().unwrap_or_default();
    f(&mut c);
    let value = (!c.is_empty()).then_some(Tag::Compound(c));
    stack.data_mut().set_other("minecraft:custom_data", value);
    stack.tidy();
}

fn set_int(stack: &mut ItemStack, k: &str, v: i32) {
    if custom_int(stack, k) == v && stack.custom_data().is_some_and(|c| c.contains_key(&key(k))) {
        return;
    }
    edit_custom(stack, |c| {
        c.insert(key(k), Tag::Int(v));
    });
}

// `RelicImpl.getSoulbindUUID`: None when unbound or not a well-formed UUID.
fn soulbind(stack: &ItemStack) -> Option<u128> {
    match stack.custom_data()?.get(&key(b::TAG_SOULBIND))? {
        Tag::String(s) => b::uuid_from_string(&s.to_string().ok()?),
        _ => None,
    }
}

// ---- where the trinkets are -------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Main(usize),
    Off,
}

// The carried stacks in the order a trinket is looked for: the main inventory (hotbar first), then
// the off hand.
fn carried<'a>(game: &'a Game<'_>) -> impl Iterator<Item = (Slot, &'a ItemStack)> + 'a {
    let main = game
        .inventory
        .iter()
        .take(36)
        .enumerate()
        .filter_map(|(i, s)| Some((Slot::Main(i), s.as_ref()?)));
    main.chain(game.offhand.as_ref().map(|s| (Slot::Off, s)))
        .filter(|(_, s)| s.count > 0)
}

fn stack_mut<'a>(game: &'a mut Game<'_>, slot: Slot) -> Option<&'a mut ItemStack> {
    match slot {
        Slot::Main(i) => game.inventory.get_mut(i).and_then(Option::as_mut),
        Slot::Off => game.offhand.as_mut(),
    }
}

// `canEquip`: a relic only for the player it is bound to.
fn can_equip(stack: &ItemStack, player: u128) -> bool {
    !b::is_relic(&stack.item) || soulbind(stack) == Some(player)
}

// `EquipmentHandler.findOrEmpty(pred, player)`: the first carried stack the test takes.
fn find(game: &Game<'_>, player: u128, test: impl Fn(&str) -> bool) -> Option<(Slot, ItemStack)> {
    carried(game)
        .find(|(_, s)| test(&s.item) && can_equip(s, player))
        .map(|(slot, s)| (slot, s.clone()))
}

fn find_item(game: &Game<'_>, player: u128, item: &str) -> Option<(Slot, ItemStack)> {
    find(game, player, |i| i == item)
}

// `StoneOfTemperanceItem.hasTemperanceActive`: an active Stone of Temperance anywhere carried.
pub(super) fn temperance_active(game: &Game<'_>) -> bool {
    carried(game)
        .any(|(_, s)| &*s.item == b::TEMPERANCE_STONE && custom_bool(s, b::TEMPERANCE_TAG_ACTIVE))
}

// `RingOfThorItem.getThorRing`: the acting player carries a Ring of Thor bound to them. For the
// Terra Shatterer's and the Terra Truncator's areas (the tools' module).
pub(super) fn thor_ring(game: &Game<'_>) -> bool {
    let Some(player) = game.native_acting_player() else {
        return false;
    };
    find_item(game, player.uuid, b::THOR_RING).is_some()
}

// ---- mana -------------------------------------------------------------------------------------

fn request_mana_exact(
    game: &mut Game<'_>,
    player: u128,
    stack: &ItemStack,
    mana: i32,
    remove: bool,
) -> bool {
    super::botania_mana::request_mana_exact(game, player, stack, mana, remove)
}

// `requestManaExact(stack, player, mana, false)` on a client, from the mana items it sees: its own
// copy of the inventory (a mirror's pool is the server's alone).
fn client_has_mana(game: &Game<'_>, mana: i32) -> bool {
    let mut items: Vec<ManaStack> = carried(game)
        .filter_map(|(_, s)| {
            let kind = Kind::of_item(&s.item)?;
            let data = s.custom_data();
            let int = |k: &str| tag_int(data.and_then(|c| c.get(&key(k))));
            let nbt = Nbt {
                mana: int("mana").unwrap_or(0),
                creative: int("creative").is_some_and(|v| v != 0),
                one_use: int("oneUse").is_some_and(|v| v != 0),
                max_mana: int("maxMana"),
                backlog: int("manaBacklog"),
            };
            Some(ManaStack::new(kind, s.count as i32, nbt))
        })
        .collect();
    let asker = Asker {
        mana: None,
        terra_blacklisted: false,
    };
    mana_handler::request_mana_exact(&mut items, &asker, mana, false)
}

// ---- effects and attributes -------------------------------------------------------------------

fn effect_of(game: &Game<'_>, player: u128, id: &str) -> Option<b::Effect> {
    game.native_effects(player)?
        .into_iter()
        .find(|e| e.id == id)
        .map(|e| b::Effect {
            amplifier: i32::from(e.amplifier),
            infinite: e.duration == -1,
        })
}

fn give_effect(game: &mut Game<'_>, player: u128, id: &str, amplifier: i32, duration: i32) {
    let effect = NativeEffect {
        id: id.to_owned(),
        amplifier: amplifier.clamp(0, 255) as u8,
        duration,
        ambient: false,
        particles: true,
    };
    game.native_add_effect(player, &effect);
}

// `EntityHelper.addStaticEffect`.
fn add_static_effect(game: &mut Game<'_>, player: u128, id: &str, amplifier: i32) {
    if b::needs_static_effect(effect_of(game, player, id), amplifier) {
        give_effect(game, player, id, amplifier, -1);
    }
}

// `EntityHelper.removeStaticEffect`.
fn remove_static_effect(game: &mut Game<'_>, player: u128, id: &str, amplifier: i32) {
    if b::is_static_effect(effect_of(game, player, id), amplifier) {
        game.native_remove_effect(player, id);
    }
}

// `EntityHelper.convertStaticEffectToFinite`.
fn static_effect_to_finite(
    game: &mut Game<'_>,
    player: u128,
    id: &str,
    amplifier: i32,
    duration: i32,
) {
    if b::is_static_effect(effect_of(game, player, id), amplifier) {
        game.native_remove_effect(player, id);
        give_effect(game, player, id, amplifier, duration);
    }
}

// A trinket's attribute modifier on the acting player, there or not (`addTransientModifier`,
// `removeModifier`), as `update_player_attributes` keeps the creative reach.
fn set_modifier(game: &mut Game<'_>, attribute: &str, id: &str, amount: f64, on: bool) {
    let Some(p) = game.player.as_mut() else {
        return;
    };
    let Some(k) = p.attributes.key(attribute) else {
        return;
    };
    if !matches!(p.attributes.get_instance(Some(k.clone())), Ok(Some(_))) {
        return;
    }
    let has = p.attributes.has_modifier(Some(k), id).unwrap_or(false);
    if has != on {
        let _ = p.set_modifier(attribute, id, amount, "add_value", on);
    }
}

fn clear_fire(game: &mut Game<'_>) {
    if let Some(p) = game.player.as_mut()
        && p.fire_ticks > 0
    {
        p.fire_ticks = 0;
    }
}

fn feet(game: &Game<'_>) -> Option<[f64; 3]> {
    let p = game.player.as_ref()?;
    Some([p.position.x, p.position.y, p.position.z])
}

fn block_name(game: &Game<'_>, pos: [i32; 3]) -> std::sync::Arc<str> {
    let s = game.world.block(pos[0], pos[1], pos[2]);
    game.res.facts().get(s).name.clone()
}

// `isInWaterOrBubble`.
fn in_water_or_bubble(game: &Game<'_>) -> bool {
    let Some(p) = game.player.as_ref() else {
        return false;
    };
    if p.fluids.was_touching_water {
        return true;
    }
    let at = [
        p.position.x.floor() as i32,
        p.position.y.floor() as i32,
        p.position.z.floor() as i32,
    ];
    &*block_name(game, at) == "minecraft:bubble_column"
}

// ---- the server's tick of a player's trinkets (`onWornTick`) ------------------------------------

// The end of a player's tick (Botania's `InventoryEquipmentHandler.onPlayerTick`, a mixin at the
// end of `Player.tick`).
pub(super) fn player_tick(game: &mut Game<'_>) {
    if !game.native_authoritative() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let (uuid, entity) = (player.uuid, player.entity_id);
    let tick = {
        let mut s = state();
        let t = s.ticks.entry(uuid).or_insert(0);
        *t = t.wrapping_add(1);
        *t
    };
    let mut gave = state().gave.get(&uuid).copied().unwrap_or(0);

    aura_tick(game, uuid, tick);
    magnet_tick(game, uuid);
    water_tick(game, uuid, &mut gave);
    correction_tick(game, uuid);
    dodge_tick(game, uuid);
    mantle_tick(game, uuid, &mut gave);
    invisibility_tick(game, uuid, &mut gave);
    cloak_cooldowns(game, uuid);

    let reach = find_item(game, uuid, b::REACH_RING).is_some();
    set_modifier(
        game,
        b::REACH_ATTRIBUTE,
        REACH_MODIFIER,
        b::REACH_BONUS,
        reach,
    );
    let odin = find_item(game, uuid, b::ODIN_RING).is_some();
    set_modifier(game, MAX_HEALTH, ODIN_MODIFIER, b::ODIN_HEALTH, odin);
    let girdle = find_item(game, uuid, b::KNOCKBACK_BELT).is_some();
    set_modifier(
        game,
        KNOCKBACK_RESISTANCE,
        GIRDLE_MODIFIER,
        b::GIRDLE_KNOCKBACK_RESISTANCE,
        girdle,
    );
    let sash = sash_tick(game, uuid);
    // `RingOfOdinItem.onValidPlayerWornTick`, `PyroclastPendantItem` and `CrimsonPendantItem`.
    let crimson = find_item(game, uuid, b::SUPER_LAVA_PENDANT).is_some();
    if odin || crimson || find_item(game, uuid, b::LAVA_PENDANT).is_some() {
        clear_fire(game);
    }

    let (cloak, amulet) = (
        find(game, uuid, |i| Cloak::of_item(i).is_some()).and_then(|(_, s)| {
            Some((
                Cloak::of_item(&s.item)?,
                custom_int(&s, b::CLOAK_TAG_COOLDOWN) == 0,
            ))
        }),
        find(game, uuid, |i| b::amulet_jumps(i).is_some())
            .and_then(|(_, s)| b::amulet_jumps(&s.item)),
    );
    let (health, on_ground) = game
        .player
        .as_ref()
        .map_or((0.0, false), |p| (p.health.health(), p.ground.on_ground));
    let hits = {
        let mut s = state();
        s.gave.insert(uuid, gave);
        s.guards.insert(
            entity,
            Guard {
                uuid,
                odin,
                crimson,
                cloak,
                health,
                sash_buffer: sash.map(|s| s.fall_buffer),
                amulet_jumps: amulet,
            },
        );
        // A landing after an amulet jump that hurt nobody ends the jump.
        if on_ground {
            s.jumping.remove(&entity);
        }
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.cloak_hits)
            .into_iter()
            .partition(|h| h.player == uuid);
        s.cloak_hits = rest;
        mine
    };
    for hit in hits {
        cloak_acted(game, uuid, entity, &hit);
    }
}

// `BandOfAuraItem.onWornTick`: 5 mana to the mana items every 50 ticks (10 for the greater).
fn aura_tick(game: &mut Game<'_>, uuid: u128, tick: i32) {
    for item in [b::AURA_RING, b::AURA_RING_GREATER] {
        let Some(interval) = b::aura_interval(item) else {
            continue;
        };
        if !b::aura_due(tick, interval) {
            continue;
        }
        if let Some((_, stack)) = find_item(game, uuid, item) {
            super::botania_mana::dispatch_mana_exact(game, uuid, &stack, b::AURA_MANA, true);
        }
    }
}

// `RingOfMagnetizationItem.onWornTick` on the server: the items around fly to the player's middle.
// No Solegnolia stops it (`hasSolegnoliaAround`: the flower isn't in the port yet).
fn magnet_tick(game: &mut Game<'_>, uuid: u128) {
    if game.spectator() {
        return;
    }
    for item in [b::MAGNET_RING, b::MAGNET_RING_GREATER] {
        let Some((slot, stack)) = find_item(game, uuid, item) else {
            continue;
        };
        let Some(range) = b::magnet_range(item) else {
            continue;
        };
        let cooldown = custom_int(&stack, b::MAGNET_TAG_COOLDOWN);
        let (next, pull) = b::magnet_tick(cooldown, game.native_sneaking(), false);
        if next != cooldown
            && let Some(s) = stack_mut(game, slot)
        {
            set_int(s, b::MAGNET_TAG_COOLDOWN, next);
        }
        if pull && let Some(at) = feet(game) {
            pull_items(game, at, range);
        }
    }
}

fn pull_items(game: &mut Game<'_>, at: [f64; 3], range: f64) {
    let (target, lo, hi) = b::magnet_box(at, range);
    let items = game.items();
    let mut moves = Vec::new();
    for (i, e) in game.dropped.iter().enumerate() {
        if moves.len() >= b::MAGNET_MAX_PULLED {
            break;
        }
        let bounds = e.bounds();
        let (min, max) = (bounds.min_position(), bounds.max_position());
        if max.x < lo[0]
            || min.x > hi[0]
            || max.y < lo[1]
            || min.y > hi[1]
            || max.z < lo[2]
            || min.z > hi[2]
        {
            continue;
        }
        let block = [
            e.position.x.floor() as i32,
            e.position.y.floor() as i32,
            e.position.z.floor() as i32,
        ];
        let below = [block[0], block[1] - 1, block[2]];
        let check = b::PullCheck {
            alive: e.stack.count > 0 && e.health > 0,
            pickup_delay: e.pickup_delay,
            mana_item: Kind::of_item(&e.stack.item).is_some(),
            relic: b::is_relic(&e.stack.item),
            blacklisted_item: items.item_in_tag(b::MAGNET_BLACKLIST, &e.stack.item),
            blacklisted_block_here: items
                .block_in_tag(b::MAGNET_BLACKLIST, &block_name(game, block)),
            blacklisted_block_below: items
                .block_in_tag(b::MAGNET_BLACKLIST, &block_name(game, below)),
        };
        if !b::can_pull(&check) {
            continue;
        }
        let height = max.y - min.y;
        let centre = [e.position.x, e.position.y + height / 2.0, e.position.z];
        moves.push((i, b::pull_velocity(target, centre, b::MAGNET_PULL_SPEED)));
    }
    for (i, v) in moves {
        if let Some(e) = game.dropped.get_mut(i) {
            e.velocity = Vec3::new(v[0], v[1], v[2]);
            e.needs_sync = true;
        }
    }
}

// A player throws an item (`Player.drop`): the Ring of Magnetization rests for 100 ticks.
pub(super) fn item_dropped(game: &mut Game<'_>, player: u128) {
    if !game.native_authoritative() {
        return;
    }
    game.native_as_player(player, |g| {
        let Some((slot, _)) = find(g, player, |i| b::magnet_range(i).is_some()) else {
            return;
        };
        if let Some(s) = stack_mut(g, slot) {
            set_int(s, b::MAGNET_TAG_COOLDOWN, b::MAGNET_TOSS_COOLDOWN);
        }
    });
}

// `RingOfChordataItem.onWornTick`: in water, Conduit Power and Dolphin's Grace (not with an active
// Stone of Temperance) for 3 mana a tick; out of mana they run out in 3 seconds, out of water
// they go.
fn water_tick(game: &mut Game<'_>, uuid: u128, gave: &mut u8) {
    let worn = find_item(game, uuid, b::WATER_RING);
    let Some((_, stack)) = worn else {
        if *gave & GAVE_WATER != 0 {
            remove_static_effect(game, uuid, b::CONDUIT_POWER, 0);
            remove_static_effect(game, uuid, b::DOLPHINS_GRACE, 0);
            *gave &= !GAVE_WATER;
        }
        return;
    };
    if !in_water_or_bubble(game) {
        remove_static_effect(game, uuid, b::CONDUIT_POWER, 0);
        remove_static_effect(game, uuid, b::DOLPHINS_GRACE, 0);
        *gave &= !GAVE_WATER;
        return;
    }
    if request_mana_exact(game, uuid, &stack, b::WATER_RING_MANA, false) {
        add_static_effect(game, uuid, b::CONDUIT_POWER, 0);
        if temperance_active(game) {
            remove_static_effect(game, uuid, b::DOLPHINS_GRACE, 0);
        } else {
            add_static_effect(game, uuid, b::DOLPHINS_GRACE, 0);
        }
        request_mana_exact(game, uuid, &stack, b::WATER_RING_MANA, true);
        *gave |= GAVE_WATER;
    } else {
        let out = b::WATER_RING_OUT_OF_MANA;
        static_effect_to_finite(game, uuid, b::CONDUIT_POWER, 0, out);
        static_effect_to_finite(game, uuid, b::DOLPHINS_GRACE, 0, out);
    }
}

// The block's place among Botania's tools (`SortableTool.getSortingPriority`), None for another
// tool.
fn sorting_priority(stack: &ItemStack, glass: bool) -> Option<i32> {
    let efficiency = stack.enchantment_level("minecraft:efficiency") as i32;
    let rank = if &*stack.item == TERRA_PICK {
        botania_core::weapons::pick_level(custom_int(stack, "mana"))
    } else {
        0
    };
    b::sorting_priority(&stack.item, efficiency, rank, glass)
}

// `RingOfCorrectionItem.onWornTick`: while the player digs with one of Botania's tools, the best of
// their tools for the block they look at goes into the main hand.
fn correction_tick(game: &mut Game<'_>, uuid: u128) {
    if game.mining.is_none() || find_item(game, uuid, b::SWAP_RING).is_none() {
        return;
    }
    let selected = game.selected;
    let Some(held) = game
        .inventory
        .get(selected)
        .and_then(Option::as_ref)
        .filter(|s| s.count > 0)
        .cloned()
    else {
        return;
    };
    let Some(eye) = game.eye_position() else {
        return;
    };
    let dir = super::view_vector(game.yaw, game.pitch);
    let Some(hit) = game.clip_ray(eye, dir, b::SWAP_RING_REACH) else {
        return;
    };
    let name = block_name(game, hit.pos);
    let items = game.items();
    let glass = items.block_in_tag("c:glass_blocks", &name)
        || items.block_in_tag("minecraft:impermeable", &name);
    let Some(held_priority) = sorting_priority(&held, glass) else {
        return;
    };
    let fast = |s: &ItemStack| items.mining_speed(Some(s), &name) > 1.0;
    let mut carried_tools = Vec::new();
    let mut slots = Vec::new();
    for (slot, s) in carried(game) {
        if slot == Slot::Main(selected) {
            continue;
        }
        if let Some(priority) = sorting_priority(s, glass) {
            carried_tools.push(b::ToolChoice {
                slot: slots.len(),
                priority,
                fast: fast(s),
            });
            slots.push(slot);
        }
    }
    let Some(best) = b::best_tool((held_priority, fast(&held)), &carried_tools) else {
        return;
    };
    match slots[best] {
        Slot::Main(i) => game.inventory.swap(selected, i),
        Slot::Off => {
            if let Some(main) = game.inventory.get_mut(selected) {
                std::mem::swap(main, &mut game.offhand);
            }
        }
    }
}

// `RingOfDexterousMotionItem.onWornTick`: the dodge's cooldown runs down.
fn dodge_tick(game: &mut Game<'_>, uuid: u128) {
    let Some((slot, stack)) = find_item(game, uuid, b::DODGE_RING) else {
        return;
    };
    let cooldown = custom_int(&stack, b::DODGE_TAG_COOLDOWN);
    if cooldown > 0
        && let Some(s) = stack_mut(game, slot)
    {
        set_int(s, b::DODGE_TAG_COOLDOWN, cooldown - 1);
    }
}

// `RingOfTheMantleItem.onWornTick`: Haste II while there are 5 mana, paid once a swing.
fn mantle_tick(game: &mut Game<'_>, uuid: u128, gave: &mut u8) {
    let Some((_, stack)) = find_item(game, uuid, b::MINING_RING) else {
        if *gave & GAVE_HASTE != 0 {
            remove_static_effect(game, uuid, b::HASTE, b::MINING_RING_HASTE);
            *gave &= !GAVE_HASTE;
        }
        return;
    };
    if request_mana_exact(game, uuid, &stack, b::MINING_RING_MANA, false) {
        add_static_effect(game, uuid, b::HASTE, b::MINING_RING_HASTE);
        *gave |= GAVE_HASTE;
    } else {
        remove_static_effect(game, uuid, b::HASTE, b::MINING_RING_HASTE);
        *gave &= !GAVE_HASTE;
    }
    if b::mining_ring_pays(game.swing.animation) {
        request_mana_exact(game, uuid, &stack, b::MINING_RING_MANA, true);
    }
}

// `InvisibilityCloakItem.onWornTick`: invisible for 2 mana a tick.
fn invisibility_tick(game: &mut Game<'_>, uuid: u128, gave: &mut u8) {
    let paid = match find_item(game, uuid, b::INVISIBILITY_CLOAK) {
        Some((_, stack)) => request_mana_exact(game, uuid, &stack, b::INVISIBILITY_MANA, true),
        None => false,
    };
    if paid {
        if *gave & GAVE_INVISIBILITY == 0
            || !b::is_static_effect(effect_of(game, uuid, b::INVISIBILITY), 0)
        {
            let effect = NativeEffect {
                id: b::INVISIBILITY.to_owned(),
                amplifier: 0,
                duration: -1,
                ambient: true,
                particles: true,
            };
            game.native_remove_effect(uuid, b::INVISIBILITY);
            game.native_add_effect(uuid, &effect);
        }
        *gave |= GAVE_INVISIBILITY;
    } else if *gave & GAVE_INVISIBILITY != 0 {
        // `onUnequipped`: only the cloak's own invisibility (Botania's amplifier -42).
        remove_static_effect(game, uuid, b::INVISIBILITY, 0);
        *gave &= !GAVE_INVISIBILITY;
    }
}

// `CloakOfVirtueItem.onWornTick`: the cooldown runs down.
fn cloak_cooldowns(game: &mut Game<'_>, uuid: u128) {
    for item in [b::HOLY_CLOAK, b::UNHOLY_CLOAK, b::BALANCE_CLOAK] {
        let Some((slot, stack)) = find_item(game, uuid, item) else {
            continue;
        };
        let cooldown = custom_int(&stack, b::CLOAK_TAG_COOLDOWN);
        if cooldown > 0
            && let Some(s) = stack_mut(game, slot)
        {
            set_int(s, b::CLOAK_TAG_COOLDOWN, cooldown - 1);
        }
    }
}

// What a cloak did to a hit (`effectOnDamage`), in its wearer's next tick: the cooldown, the sound,
// the Cloak of Sin's blow to the enemies around and the Cloak of Balance's to the attacker.
fn cloak_acted(game: &mut Game<'_>, uuid: u128, entity: i32, hit: &CloakHit) {
    let item = match hit.cloak {
        Cloak::Virtue => b::HOLY_CLOAK,
        Cloak::Sin => b::UNHOLY_CLOAK,
        Cloak::Balance => b::BALANCE_CLOAK,
    };
    if let Some((slot, _)) = find_item(game, uuid, item)
        && let Some(s) = stack_mut(game, slot)
    {
        set_int(s, b::CLOAK_TAG_COOLDOWN, b::CLOAK_COOLDOWN);
    }
    let Some(at) = feet(game) else {
        return;
    };
    let sound = match hit.cloak {
        Cloak::Sin => "botania:unholy_cloak",
        Cloak::Virtue | Cloak::Balance => "botania:holy_cloak",
    };
    game.native_play_sound_in_place(sound, at, PLAYERS, 1.0, 1.0);
    let dim = game.dim.id();
    match hit.cloak {
        Cloak::Virtue => {}
        Cloak::Sin => {
            let r = b::SIN_RANGE;
            let enemies: Vec<u32> = game
                .mobs
                .iter()
                .filter(|m| !m.removed && m.alive() && m.is_enemy())
                .filter(|m| {
                    let bx = m.bounds();
                    let (lo, hi) = (bx.min_position(), bx.max_position());
                    hi.x >= at[0] - r
                        && lo.x <= at[0] + r
                        && hi.y >= at[1] - r
                        && lo.y <= at[1] + r
                        && hi.z >= at[2] - r
                        && lo.z <= at[2] + r
                })
                .map(|m| m.id)
                .collect();
            for id in enemies {
                game.native_hurt_entity(
                    dim,
                    id,
                    hit.amount,
                    "minecraft:player_attack",
                    Some(entity as u32),
                );
            }
        }
        Cloak::Balance => {
            if hit.attacker > 0 && hit.attacker != entity {
                game.native_hurt_entity(
                    dim,
                    hit.attacker as u32,
                    hit.amount,
                    "minecraft:indirect_magic",
                    Some(entity as u32),
                );
            }
        }
    }
}

// `SojournersSashItem.tickBelt` on the server: the step boost while the sash has mana and the
// player doesn't sneak. The sash worn, for the falls.
fn sash_tick(game: &mut Game<'_>, uuid: u128) -> Option<b::Sash> {
    let found = find(game, uuid, |i| b::sash(i).is_some());
    let sash = found.as_ref().and_then(|(_, s)| b::sash(&s.item));
    let boost = match &found {
        Some((_, stack)) => {
            request_mana_exact(game, uuid, stack, b::SASH_COST, false) && !game.native_sneaking()
        }
        None => false,
    };
    set_modifier(game, STEP_HEIGHT, STEP_MODIFIER, b::SASH_STEP_BOOST, boost);
    sash
}

// ---- relics -----------------------------------------------------------------------------------

// A carried stack's tick (`inventoryTick`) on the server: a relic binds to the first player who
// carries it and hurts anyone else (`RelicImpl.tickBinding`); a Ring of Loki other than the one in
// use drops its selection (`RingOfLokiItem.inventoryTick` leaves binding mode in the inventory).
pub(super) fn inventory_tick(game: &mut Game<'_>, slot: i32) {
    let Some(stack) = game.native_carried_stack(slot) else {
        return;
    };
    if !b::is_relic(&stack.item) {
        return;
    }
    let item = stack.item.clone();
    let bound = soulbind(stack);
    if !game.native_authoritative() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let tick = state().ticks.get(&player.uuid).copied().unwrap_or(0);
    match b::relic_tick(bound, player.uuid, tick) {
        RelicTick::Bind => {
            if let Some(s) = game.native_carried_stack(slot) {
                let text = b::uuid_to_string(player.uuid);
                edit_custom(s, |c| {
                    c.insert(
                        key(b::TAG_SOULBIND),
                        Tag::String(JavaString::from(text.as_str())),
                    );
                });
            }
        }
        RelicTick::Hurt => {
            let dim = game.dim.id();
            let me = player.entity_id as u32;
            if !game.native_hurt_entity(
                dim,
                me,
                b::RELIC_WRONG_PLAYER_DAMAGE,
                "botania:relic_damage",
                None,
            ) {
                game.native_hurt_entity(
                    dim,
                    me,
                    b::RELIC_WRONG_PLAYER_DAMAGE,
                    "minecraft:magic",
                    None,
                );
            }
        }
        RelicTick::Nothing => {}
    }
    if &*item == b::LOKI_RING {
        let worn = find_item(game, player.uuid, b::LOKI_RING).map(|(s, _)| s);
        let this = match slot {
            0..=35 => Some(Slot::Main(slot as usize)),
            -106 => Some(Slot::Off),
            _ => None,
        };
        if this != worn
            && let Some(s) = game.native_carried_stack(slot)
            && loki_origin(s)[1] != i32::MIN
        {
            set_loki_origin(s, b::LOKI_UNBOUND);
        }
    }
}

// ---- Ring of Loki -------------------------------------------------------------------------------

fn loki_origin(stack: &ItemStack) -> [i32; 3] {
    [
        custom_int_or(stack, b::LOKI_TAG_X_ORIGIN, 0),
        custom_int_or(stack, b::LOKI_TAG_Y_ORIGIN, i32::MIN),
        custom_int_or(stack, b::LOKI_TAG_Z_ORIGIN, 0),
    ]
}

fn set_loki_origin(stack: &mut ItemStack, pos: [i32; 3]) {
    edit_custom(stack, |c| {
        c.insert(key(b::LOKI_TAG_X_ORIGIN), Tag::Int(pos[0]));
        c.insert(key(b::LOKI_TAG_Y_ORIGIN), Tag::Int(pos[1]));
        c.insert(key(b::LOKI_TAG_Z_ORIGIN), Tag::Int(pos[2]));
    });
}

// `getCursorList`.
fn loki_cursors(stack: &ItemStack) -> Vec<[i32; 3]> {
    let Some(Tag::Compound(list)) = stack
        .custom_data()
        .and_then(|c| c.get(&key(b::LOKI_TAG_CURSOR_LIST)))
    else {
        return Vec::new();
    };
    let count = tag_int(list.get(&key(b::LOKI_TAG_CURSOR_COUNT))).unwrap_or(0);
    (0..count.max(0))
        .map(|i| {
            let name = format!("{}{i}", b::LOKI_TAG_CURSOR_PREFIX);
            let cursor = match list.get(&key(&name)) {
                Some(Tag::Compound(c)) => Some(c),
                _ => None,
            };
            let int = |k: &str| cursor.and_then(|c| tag_int(c.get(&key(k)))).unwrap_or(0);
            [
                int(b::LOKI_TAG_X_OFFSET),
                int(b::LOKI_TAG_Y_OFFSET),
                int(b::LOKI_TAG_Z_OFFSET),
            ]
        })
        .collect()
}

// `setCursorList`.
fn set_loki_cursors(stack: &mut ItemStack, cursors: &[[i32; 3]]) {
    let mut list = Compound::new();
    for (i, c) in cursors.iter().enumerate() {
        let mut cursor = Compound::new();
        cursor.insert(key(b::LOKI_TAG_X_OFFSET), Tag::Int(c[0]));
        cursor.insert(key(b::LOKI_TAG_Y_OFFSET), Tag::Int(c[1]));
        cursor.insert(key(b::LOKI_TAG_Z_OFFSET), Tag::Int(c[2]));
        list.insert(
            key(&format!("{}{i}", b::LOKI_TAG_CURSOR_PREFIX)),
            Tag::Compound(cursor),
        );
    }
    list.insert(
        key(b::LOKI_TAG_CURSOR_COUNT),
        Tag::Int(cursors.len() as i32),
    );
    edit_custom(stack, |c| {
        c.insert(key(b::LOKI_TAG_CURSOR_LIST), Tag::Compound(list));
    });
}

// `RingOfLokiItem.onPlayerInteract`, where a right click on a block starts: sneaking with an empty
// main hand selects the blocks (the click is spent), sneaking with something in the hand uses it on
// every cursor too, for mana each.
pub(super) fn right_click_block(
    game: &mut Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    if *canceled || !game.native_authoritative() || state().loki_busy || !game.native_sneaking() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let uuid = player.uuid;
    let Some((slot, ring)) = find_item(game, uuid, b::LOKI_RING) else {
        return;
    };
    let held = game.held().filter(|s| s.count > 0).cloned();
    match held {
        None if game.hand == Hand::Main => {
            let mut cursors = loki_cursors(&ring);
            let (origin, what) = b::loki_select(loki_origin(&ring), &mut cursors, pos);
            if let Some(s) = stack_mut(game, slot) {
                set_loki_origin(s, origin);
                match what {
                    b::LokiSelect::Started => set_loki_cursors(s, &[]),
                    b::LokiSelect::Added | b::LokiSelect::Removed => set_loki_cursors(s, &cursors),
                    b::LokiSelect::Finished | b::LokiSelect::Full => {}
                }
            }
            if what == b::LokiSelect::Full {
                game.native_action_bar(
                    uuid,
                    Text::translate("botaniamisc.lokiRingLimitReached", &[]),
                );
            }
            *canceled = true;
        }
        None => {}
        Some(_) => loki_place(game, uuid, &ring, pos, face),
    }
}

// The copies: the item used on each cursor around the block clicked, as if clicked at the same
// place of that block (`getUseOnContext`), each one paid when it acts (`consumesAction`).
fn loki_place(game: &mut Game<'_>, uuid: u128, ring: &ItemStack, pos: [i32; 3], face: [i32; 3]) {
    let cursors = loki_cursors(ring);
    if cursors.is_empty() {
        return;
    }
    let cost = b::loki_cost(cursors.len());
    let look = game
        .forced_hit
        .filter(|h| h.pos == pos)
        .map(|h| h.point)
        .unwrap_or_else(|| {
            std::array::from_fn(|i| f64::from(pos[i]) + 0.5 + f64::from(face[i]) * 0.5)
        });
    state().loki_busy = true;
    for target in b::loki_targets(pos, &cursors) {
        if !request_mana_exact(game, uuid, ring, cost, false) {
            break;
        }
        let hit = Hit {
            pos: target,
            face,
            point: b::loki_hit_point(target, look),
        };
        let saved_hit = game.forced_hit.replace(hit);
        let saved_failed = std::mem::replace(&mut game.use_failed, false);
        let used = matches!(game.use_hand(Some(&hit)), Ok(true)) && !game.use_failed;
        game.forced_hit = saved_hit;
        game.use_failed = saved_failed;
        if used {
            request_mana_exact(game, uuid, ring, cost, true);
        }
    }
    state().loki_busy = false;
}

// `RingOfLokiItem.breakOnAllCursors`: a sneaking player who breaks the block they look at with the
// Terra Shatterer or the Terra Truncator breaks the same on every cursor around it. Botania calls
// the tool's `breakOtherBlock` on each cursor, then breaks the cursor's block: here breaking it
// as the player runs the tools' own `destroy_block` hooks on it, which are that call
// (`botania_weapons::pick_break_other` with the origin's face,
// `botania_tools::terra_axe_break_other`, which a sneaking player never fells with), so calling
// them here too would break each area twice.
pub(super) fn destroy_block(
    game: &mut Game<'_>,
    pos: [i32; 3],
    _block_state: BlockStateId,
    canceled: bool,
) {
    if canceled || !game.native_authoritative() || state().loki_busy || !game.native_sneaking() {
        return;
    }
    let Some(held) = game.inventory.get(game.selected).and_then(Option::as_ref) else {
        return;
    };
    if &*held.item != TERRA_PICK && &*held.item != TERRA_AXE {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let Some((_, ring)) = find_item(game, player.uuid, b::LOKI_RING) else {
        return;
    };
    let Some(eye) = game.eye_position() else {
        return;
    };
    let dir = super::view_vector(game.yaw, game.pitch);
    if game
        .clip_ray(eye, dir, BREAKER_REACH)
        .is_none_or(|h| h.pos != pos)
    {
        return;
    }
    let cursors = loki_cursors(&ring);
    if cursors.is_empty() {
        return;
    }
    state().loki_busy = true;
    for target in b::loki_targets(pos, &cursors) {
        if game.world.loaded(target[0], target[2]) {
            game.native_player_break_block(player.uuid, target, false);
        }
    }
    state().loki_busy = false;
}

// ---- Stone of Temperance ----------------------------------------------------------------------

// `StoneOfTemperanceItem.use`: a right click turns it on or off.
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if *claimed || &*stack.item != b::TEMPERANCE_STONE || !game.native_authoritative() {
        return;
    }
    let Some(held) = game.held_mut().as_mut() else {
        return;
    };
    if &*held.item != b::TEMPERANCE_STONE {
        return;
    }
    let active = custom_bool(held, b::TEMPERANCE_TAG_ACTIVE);
    edit_custom(held, |c| {
        c.insert(key(b::TEMPERANCE_TAG_ACTIVE), Tag::Byte(i8::from(!active)));
    });
    if let Some(at) = feet(game) {
        game.native_play_sound_in_place(
            "botania:temperance_stone_configure",
            at,
            NEUTRAL,
            1.0,
            1.0,
        );
    }
    *claimed = true;
}

// ---- the damage hooks (no game: what the players carried at their last tick) --------------------

// A player is about to be hurt (`Player.isInvulnerableTo`, then `Player.hurt`'s amount): the Ring of
// Odin's and the Crimson Pendant's immunities, the sashes' cushioned falls and the cloaks. The
// pixies are the entities module's (`botania_entities`).
pub(super) fn incoming_damage(
    entity: i32,
    kind: &str,
    damage_type: &str,
    causing: i32,
    amount: &mut f32,
    canceled: &mut bool,
) {
    if *canceled || kind != "minecraft:player" {
        return;
    }
    let mut s = state();
    let Some(guard) = s.guards.get(&entity).cloned() else {
        return;
    };
    if (guard.odin && b::odin_immune(damage_type)) || (guard.crimson && b::is_fire(damage_type)) {
        *canceled = true;
        return;
    }
    if damage_type == "minecraft:fall" {
        let jumped = s.jumping.remove(&entity);
        if let Some(buffer) = guard.sash_buffer {
            // The sash shortens the fall: for a whole number of blocks, the damage drops by as many.
            let jumps = if jumped { guard.amulet_jumps } else { None };
            *amount = b::sash_fall(*amount, buffer, jumps);
            if *amount <= 0.0 {
                *canceled = true;
                return;
            }
        }
    }
    if let Some((cloak, true)) = guard.cloak
        && cloak.acts_on(damage_type)
    {
        let hit = CloakHit {
            player: guard.uuid,
            cloak,
            amount: if cloak == Cloak::Balance {
                *amount / 2.0
            } else {
                *amount
            },
            attacker: causing,
        };
        *amount = cloak.amount(*amount, guard.health);
        s.cloak_hits.push(hit);
        if let Some(g) = s.guards.get_mut(&entity) {
            g.cloak = Some((cloak, false));
        }
    }
}

// ---- explosions -------------------------------------------------------------------------------

// `BenevolentGoddessCharmItem.shouldProtectExplosion`: a player within 8 blocks with a charm and
// 1000 mana keeps the blocks whole.
pub(super) fn explosion_detonate(
    game: &mut Game<'_>,
    dimension: &str,
    center: [f64; 3],
    blocks: &mut Vec<[i32; 3]>,
) {
    if blocks.is_empty() || !game.native_authoritative() {
        return;
    }
    let r = b::GODDESS_CHARM_RANGE;
    let near: Vec<u128> = game
        .native_server_players()
        .into_iter()
        .filter(|p| p.dimension == dimension)
        .filter(|p| {
            let [x, y, z] = p.position;
            // The player's box (0.6 wide, 1.8 high) meets the blast's, grown by 8.
            x + 0.3 >= center[0] - r
                && x - 0.3 <= center[0] + r
                && y + 1.8 >= center[1] - r
                && y <= center[1] + r
                && z + 0.3 >= center[2] - r
                && z - 0.3 <= center[2] + r
        })
        .map(|p| p.uuid)
        .collect();
    for uuid in near {
        let mut paid = false;
        game.native_as_player(uuid, |g| {
            if let Some((_, charm)) = find_item(g, uuid, b::GODDESS_CHARM) {
                paid = request_mana_exact(g, uuid, &charm, b::GODDESS_CHARM_MANA, true);
            }
        });
        if paid {
            blocks.clear();
            return;
        }
    }
}

// `TectonicGirdleItem.negateExplosionKnockback`: a blast doesn't push a player wearing the girdle.
pub(super) fn explosion_knockback(
    game: &mut Game<'_>,
    entity: Option<u32>,
    knockback_velocity: &mut [f64; 3],
) {
    if entity.is_some() || !game.native_authoritative() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    if find_item(game, player.uuid, b::KNOCKBACK_BELT).is_some() {
        *knockback_velocity = [0.0; 3];
    }
}

// ---- messages from the clients ----------------------------------------------------------------

// `DodgePacket` and `JumpPacket`.
pub(super) fn net_from_client(game: &mut Game<'_>, player: u128, channel: &str) {
    if channel != DODGE_CHANNEL && channel != JUMP_CHANNEL {
        return;
    }
    let dodge = channel == DODGE_CHANNEL;
    game.native_as_player(player, |g| {
        if dodge {
            if let Some(at) = feet(g) {
                g.native_play_sound_in_place("botania:dash", at, PLAYERS, 1.0, 1.0);
            }
            let Some((slot, _)) = find_item(g, player, b::DODGE_RING) else {
                return;
            };
            if let Some(p) = g.player.as_mut() {
                p.cause_food_exhaustion(b::DODGE_EXHAUSTION);
            }
            if let Some(s) = stack_mut(g, slot) {
                set_int(s, b::DODGE_TAG_COOLDOWN, b::DODGE_COOLDOWN);
            }
        } else {
            if find(g, player, |i| b::amulet_jumps(i).is_some()).is_none() {
                return;
            }
            let entity = g.native_acting_player().map(|p| p.entity_id);
            if let Some(p) = g.player.as_mut() {
                p.cause_food_exhaustion(b::AMULET_EXHAUSTION);
                p.fall_distance = 0.0;
            }
            if let Some(entity) = entity {
                state().jumping.insert(entity);
            }
        }
    });
}

// ---- the client: dodges, air jumps, the sashes' push, the HUD, Loki's wireframes ----------------

fn client_window(game: &Game<'_>) -> bool {
    game.native_window() && game.remote.is_some()
}

// The movement keys of this tick (`player.input`).
pub(super) fn movement_input(game: &Game<'_>, forward: f32, strafe: f32, jump: bool) {
    if !client_window(game) {
        return;
    }
    let mut s = state();
    s.client.forward = forward;
    s.client.strafe = strafe;
    s.client.jump = jump;
}

// A client tick, after the player moved.
pub(super) fn client_tick(game: &mut Game<'_>) {
    if !client_window(game) || game.player.is_none() {
        return;
    }
    let me = game.own_uuid();
    let (ticks, strafe, forward, jump, need_name) = {
        let mut s = state();
        s.client.ticks = s.client.ticks.wrapping_add(1);
        let c = &mut s.client;
        let need = c.own.as_ref().is_none_or(|(u, _)| *u != me) && {
            c.profile_retry -= 1;
            c.profile_retry <= 0
        };
        (c.ticks, c.strafe, c.forward, c.jump, need)
    };
    if need_name {
        let name = game
            .native_client_profiles()
            .into_iter()
            .find(|p| p.uuid == me)
            .map(|p| p.name);
        let mut s = state();
        match name {
            Some(n) => s.client.own = Some((me, n)),
            None => s.client.profile_retry = 40,
        }
    }
    dodge(game, me, ticks, strafe, forward);
    let sash = find(game, me, |i| b::sash(i).is_some()).map(|(_, s)| s);
    let sash = sash.filter(|_| client_has_mana(game, b::SASH_COST));
    air_jump(game, me, jump, sash.as_ref());
    sash_push(game, forward, sash.as_ref());
}

// `RingOfDexterousMotionItem.ClientLogic`: a movement key pressed twice within 5 ticks throws the
// player 1.25 blocks a tick that way, on the ground and not flying.
fn dodge(game: &mut Game<'_>, me: u128, ticks: i32, strafe: f32, forward: f32) {
    let Some((_, ring)) = find_item(game, me, b::DODGE_RING) else {
        return;
    };
    if custom_int(&ring, b::DODGE_TAG_COOLDOWN) > 0 {
        return;
    }
    let keys = [strafe > 0.0, strafe < 0.0, forward > 0.0, forward < 0.0];
    let Some(dir) = state().client.dodge.press(ticks, keys) else {
        return;
    };
    let yaw = game.yaw;
    let Some(p) = game.player.as_mut() else {
        return;
    };
    if p.flying || !p.ground.on_ground {
        return;
    }
    let v = b::dodge_velocity(yaw, dir);
    p.velocity = Vec3::new(v[0], v[1], v[2]);
    let _ = game.native_net_send_to_server(DODGE_CHANNEL, &[1]);
}

// `CirrusAmuletItem.onWornTick` on the client: jumps in the air (one more with the Nimbus Amulet),
// each told to the server, which takes the food and forgets the fall.
fn air_jump(game: &mut Game<'_>, me: u128, jumping: bool, sash: Option<&ItemStack>) {
    let Some(max) = find(game, me, |i| b::amulet_jumps(i).is_some())
        .and_then(|(_, s)| b::amulet_jumps(&s.item))
    else {
        return;
    };
    let on_ground = game.player.as_ref().is_some_and(|p| p.ground.on_ground);
    let jump = state().client.amulet.tick(on_ground, jumping, max);
    if !jump {
        return;
    }
    let yaw = game.yaw;
    let sash_jump = sash.and_then(|s| b::sash(&s.item)).map(|s| s.jump);
    let Some(p) = game.player.as_mut() else {
        return;
    };
    // `jumpFromGround`, and the sash's jump (`onPlayerJump`, which hears every jump).
    let strength = p.attribute(JUMP_STRENGTH).unwrap_or(0.42);
    let power = f64::from(b::jump_power(strength, p.status.jump_boost));
    let mut v = p.velocity;
    v.y = power.max(v.y);
    if p.sprinting {
        let r = f64::from(yaw).to_radians();
        v.x += -r.sin() * 0.2;
        v.z += r.cos() * 0.2;
    }
    if let Some(extra) = sash_jump {
        v.y += f64::from(extra);
    }
    p.velocity = v;
    let _ = game.native_net_send_to_server(JUMP_CHANNEL, &[1]);
}

// `SojournersSashItem.tickBelt` on the client: walking forward on the ground (or flying), out of
// water, the sash pushes the player ahead; a jump from the ground goes higher.
fn sash_push(game: &mut Game<'_>, forward: f32, sash: Option<&ItemStack>) {
    let Some(stack) = sash else {
        return;
    };
    let Some(numbers) = b::sash(&stack.item) else {
        return;
    };
    let planestrider = &*stack.item == b::SPEED_UP_BELT;
    let in_water = in_water_or_bubble(game);
    let yaw = game.yaw;
    let Some(p) = game.player.as_mut() else {
        return;
    };
    let pos = [p.position.x, p.position.y, p.position.z];
    let mut s = state();
    let c = &mut s.client;
    // The ground jump this tick (`noJumpDelay` is 10 right after `jumpFromGround`).
    let jumped = p.no_jump_delay == 10;
    if jumped && !c.jump_raised {
        p.velocity.y += f64::from(numbers.jump);
    }
    c.jump_raised = jumped;
    let moved = (0..3).any(|i| (c.old_pos[i] - pos[i]).abs() > 0.001);
    if (p.ground.on_ground || p.flying) && forward > 0.0 && !in_water {
        let speed = if planestrider {
            c.planestrider
        } else {
            numbers.speed
        };
        let push = b::sash_push(yaw, speed);
        p.velocity = Vec3::new(p.velocity.x + push[0], p.velocity.y, p.velocity.z + push[2]);
        if planestrider {
            c.planestrider = b::planestrider_speed(c.planestrider);
        }
    } else if planestrider && !moved {
        c.planestrider = 0.0;
    }
    c.old_pos = pos;
}

// `RingOfDexterousMotionItem.ClientLogic.renderHUD`: the cooldown as a bar under the crosshair.
pub(super) fn hud(game: &Game<'_>, batch: &mut HudBatch) {
    if !client_window(game) {
        return;
    }
    let Some(p) = game.player.as_ref() else {
        return;
    };
    if p.flying {
        return;
    }
    let Some((_, ring)) = find_item(game, game.own_uuid(), b::DODGE_RING) else {
        return;
    };
    let width = b::dodge_bar_width(custom_int(&ring, b::DODGE_TAG_COOLDOWN), 0.0);
    if width <= 0 {
        return;
    }
    let x = (batch.width / 2.0).floor() - 20.0;
    let y = (batch.height / 2.0).floor() + 20.0;
    game.native_fill(batch, [x, y - 2.0, 40.0, 1.0], 0x8800_0000);
    game.native_fill(batch, [x, y - 2.0, width as f32, 1.0], 0xFFFF_FFFF);
}

// `BoundBlockRenderer` for the Ring of Loki (`getWireframesToDraw`, `getSourceWireframe`): the
// cursors around the block looked at (around the origin while selecting) in a colour that cycles
// every 10 seconds, the origin thicker.
pub(super) fn visuals(game: &Game<'_>, rims: &mut Vec<NativeBlockRim>) {
    if !client_window(game) {
        return;
    }
    let Some((_, ring)) = find_item(game, game.own_uuid(), b::LOKI_RING) else {
        return;
    };
    let looked = game.pick().map(|h| h.pos).filter(|pos| {
        let s = game.world.block(pos[0], pos[1], pos[2]);
        !game.res.behaviours().is_air(s)
    });
    let origin = loki_origin(&ring);
    let cursors = loki_cursors(&ring);
    let ticks = state().client.ticks;
    let hue = (ticks.rem_euclid(200)) as f32 / 200.0;
    let argb = 0xFF00_0000 | botania_core::wand::hsv_to_rgb(hue, 0.6, 1.0);
    for pos in b::loki_wireframes(origin, &cursors, looked) {
        rims.push(NativeBlockRim {
            pos,
            argb,
            width: 1.0,
        });
    }
    let source = if origin[1] != i32::MIN {
        Some(origin)
    } else {
        looked.filter(|_| !cursors.is_empty())
    };
    if let Some(pos) = source {
        rims.push(NativeBlockRim {
            pos,
            argb,
            width: 2.5,
        });
    }
}

// ---- tooltips and the items' defaults ---------------------------------------------------------

// `appendHoverText`: the relics' owner (`RelicImpl.addDefaultTooltip`), the Stone of Temperance's
// state, the cosmetic baubles' line.
pub(super) fn tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    let item = &*stack.item;
    let mut extra = Vec::new();
    if item == b::TEMPERANCE_STONE {
        let k = if custom_bool(stack, b::TEMPERANCE_TAG_ACTIVE) {
            "botaniamisc.active"
        } else {
            "botaniamisc.inactive"
        };
        extra.push((Text::translate(k, &[]), u32::MAX));
    } else if item.starts_with("botania:cosmetic_") {
        let k = if item == "botania:cosmetic_thinking_hand" {
            "botaniamisc.cosmeticThinking"
        } else {
            "botaniamisc.cosmeticBauble"
        };
        extra.push((Text::translate(k, &[]).with_color(GRAY).italic(), u32::MAX));
    }
    if b::is_relic(item) {
        let me = client_window(game).then(|| game.own_uuid());
        let line = match b::relic_line(soulbind(stack), me) {
            RelicLine::Unbound => Text::translate("botaniamisc.relicUnbound", &[]),
            RelicLine::NotYours => Text::translate("botaniamisc.notYourSagittarius", &[]),
            RelicLine::Yours => {
                let name = state()
                    .client
                    .own
                    .as_ref()
                    .map(|(_, n)| n.clone())
                    .unwrap_or_default();
                Text::translate("botaniamisc.relicSoulbound", &[Text::literal(name)])
            }
        };
        extra.push((line, u32::MAX));
    }
    if extra.is_empty() {
        return;
    }
    let at = lines.len().min(1);
    lines.splice(at..at, extra);
}
