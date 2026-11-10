// Botania's remaining tools for Horizon Aurora: a modified port of Vazkii's Botania (Botania
// License, see LICENSE and NOTICE.md). The Terra Truncator felling trees, the elementium axe's
// beheading, the elementium shears shearing around their user, the elementium hoe's wet farmland,
// the Terra Shatterer's and Terra Truncator's item models, and the break particles Botania sends
// the player who breaks with an area tool. The rules are in `botania_core::tools`; this module
// wires them to the hooks (`botania_tools` in game.rs).
//
// Inner attributes and `//!` comments are not allowed in a horizon-mixins module payload.

use super::Game;
use botania_core::burst::Aabb;
use botania_core::tools::{self, AreaRings, Growth, Swapper, TreeWorld};
use botania_core::weapons;
use horizon_client::first_person::Hand;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::{NativeLootDrops, NativeUseTarget};
use horizon_protocol::packets::play;
use horizon_world::consumable::UseAnimation;
use horizon_world::nbt::{Compound, JavaString, Tag};
use horizon_world::registry::BlockStateId;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

// The component the item models read the tools' state from (`minecraft:condition` on
// `minecraft:custom_model_data` flags): 26.3 has no model property a mod's code answers in a
// horizon-mixins build, so the server writes Botania's `botania:active` and `botania:tipped`
// there.
const MODEL_DATA: &str = "minecraft:custom_model_data";
// `LevelEvent.PARTICLES_DESTROY_BLOCK`.
const BREAK_EVENT: i32 = 2001;

#[derive(Default)]
struct State {
    // `TerraTruncatorItem.blockSwappers`: the fellings under way, in the order they began.
    fellings: Vec<Felling>,
    // `tickingSwappers`: a felling's own breaks don't start another.
    felling: bool,
    // Shears' uses that found a mob to shear this tick, sheared after the level's tick.
    shearings: Vec<Shearing>,
    // Blocks an area tool broke for a player, whose own client the break event leaves out.
    seen: Vec<Seen>,
    // The module's own random numbers, for the beheading roll.
    rng: u64,
}

// A felling (`BlockSwapper`): the level, the player it breaks as and the inventory slot the
// Terra Truncator was in.
struct Felling {
    dim: &'static str,
    player: u128,
    slot: usize,
    swapper: Swapper,
}

// A mob the elementium shears of `player` shear: where the player's eyes were and how they
// looked, and the hand that held the shears.
struct Shearing {
    dim: &'static str,
    player: u128,
    mob: u32,
    eye: [f64; 3],
    look: (f32, f32),
    hand: Hand,
}

// A block broken for `player`: where, and its state before.
struct Seen {
    dim: &'static str,
    player: u128,
    pos: [i32; 3],
    state: u32,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

// Never held across a call into the game: a break calls the hooks again.
fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

// `Random.nextFloat()` of the module's own generator, seeded from the clock on first use.
fn random_float() -> f32 {
    let mut s = state();
    let mut x = if s.rng == 0 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        now | 1
    } else {
        s.rng
    };
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    s.rng = x;
    (x >> 40) as f32 / (1u32 << 24) as f32
}

fn key(k: &str) -> JavaString {
    JavaString::from(k)
}

fn aabb(b: horizon_core::geometry::Aabb) -> Aabb {
    let (lo, hi) = (b.min_position(), b.max_position());
    [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z]
}

fn feet(game: &Game<'_>) -> Option<[f64; 3]> {
    let p = game.player.as_ref()?;
    Some([p.position.x, p.position.y, p.position.z])
}

// The stack in the main hand.
fn main_stack(game: &Game<'_>) -> Option<ItemStack> {
    let stack = game.inventory.get(game.selected)?.as_ref()?;
    (stack.count > 0).then(|| stack.clone())
}

fn custom_flag(stack: &ItemStack, k: &str) -> bool {
    match stack.custom_data().and_then(|c| c.get(&key(k))) {
        Some(Tag::Byte(v)) => *v != 0,
        Some(Tag::Short(v)) => *v != 0,
        Some(Tag::Int(v)) => *v != 0,
        Some(Tag::Long(v)) => *v != 0,
        _ => false,
    }
}

// ---- the rings --------------------------------------------------------------------------------

// What the player in place carries that changes an area tool: the Ring of Thor and an active
// Stone of Temperance. Neither is ported yet: none. The rings' module answers here once they
// are (Thor widens the Terra Shatterer's area, Temperance holds it to rank B and stops the Terra
// Truncator felling).
pub(super) fn area_rings(_game: &Game<'_>) -> AreaRings {
    AreaRings::default()
}

// ---- break particles for the breaker -----------------------------------------------------------

// `ToolCommons.removeBlockWithDrops` sends the player the break event of each block it breaks for
// them (`ClientboundLevelEventPacket` 2001), since the break itself leaves their client out
// (`Block.playerWillDestroy`). Noted here and sent after the level's tick, where the server's
// players can be reached; the Terra Shatterer's and the elementium shovel's areas note theirs too.
pub(super) fn seen_breaking(game: &Game<'_>, player: u128, pos: [i32; 3], state: BlockStateId) {
    let dim = game.dim.id();
    state_seen(dim, player, pos, state.0);
}

fn state_seen(dim: &'static str, player: u128, pos: [i32; 3], block: u32) {
    state().seen.push(Seen {
        dim,
        player,
        pos,
        state: block,
    });
}

fn send_seen(game: &Game<'_>, seen: &[Seen]) {
    let Some(host) = game.lan.as_ref() else {
        return;
    };
    for s in seen {
        let Some(guest) = host.hub.guests.iter().find(|g| g.uuid == s.player) else {
            continue;
        };
        let event = play::LevelEvent {
            kind: BREAK_EVENT,
            pos: s.pos,
            data: s.state as i32,
            global: false,
        };
        host.hub
            .send_to(guest.conn, &play::Clientbound::LevelEvent(event));
    }
}

// ---- the Terra Truncator ----------------------------------------------------------------------

// `TerraTruncatorItem.onBlockStartBreak`: a player mining with the Terra Truncator (a block in
// reach of their look) starts a felling from the block mined.
pub(super) fn destroy_block(
    game: &mut Game<'_>,
    pos: [i32; 3],
    _block_state: BlockStateId,
    canceled: bool,
) {
    if canceled || !game.native_authoritative() {
        return;
    }
    let Some(held) = main_stack(game) else {
        return;
    };
    if &*held.item != tools::TERRA_AXE || state().felling {
        return;
    }
    let Some(eye) = game.eye_position() else {
        return;
    };
    let dir = super::view_vector(game.yaw, game.pitch);
    if game.clip_ray(eye, dir, tools::AXE_REACH).is_none() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let slot = game.selected;
    terra_axe_break_other(game, player.uuid, slot, pos, tools::BLOCK_RANGE);
}

// `TerraTruncatorItem.breakOtherBlock`: a felling from `pos` with `range` steps
// (`tools::BLOCK_RANGE`), as player `player` with the Terra Truncator in inventory slot `slot`,
// unless they sneak, hold a Stone of Temperance or the break is a felling's own. The Ring of
// Loki's cursors start theirs here too.
pub(super) fn terra_axe_break_other(
    game: &mut Game<'_>,
    player: u128,
    slot: usize,
    pos: [i32; 3],
    range: i32,
) {
    let rings = area_rings(game);
    if !tools::truncator_fells(game.native_sneaking(), rings.temperance) {
        return;
    }
    let dim = game.dim.id();
    let mut s = state();
    if s.felling {
        return;
    }
    s.fellings.push(Felling {
        dim,
        player,
        slot,
        swapper: Swapper::new(pos, range),
    });
}

// What a felling sees of the level, as the player it breaks for.
struct Fell<'a, 'g> {
    game: &'a mut Game<'g>,
    player: u128,
    seen: &'a mut Vec<([i32; 3], u32)>,
}

impl TreeWorld for Fell<'_, '_> {
    fn growth(&self, pos: [i32; 3]) -> Option<Growth> {
        if !self.game.world.loaded(pos[0], pos[2]) {
            return None;
        }
        let s = self.game.world.block(pos[0], pos[1], pos[2]);
        let name = &*self.game.res.facts().get(s).name;
        let items = self.game.items();
        if items.block_in_tag("minecraft:leaves", name) {
            Some(Growth::Leaves)
        } else if items.block_in_tag("minecraft:logs", name) || name == "minecraft:mangrove_roots" {
            Some(Growth::Wood)
        } else {
            None
        }
    }

    // `removeBlockWithDrops`: a loaded block that isn't air, can be mined (`getDestroyProgress`
    // above 0) and an axe mines or is leaves, broken as the player (`ServerPlayerGameMode
    // .destroyBlock`), its break event sent to them.
    fn remove(&mut self, pos: [i32; 3]) {
        let g = &mut *self.game;
        if !g.world.loaded(pos[0], pos[2]) {
            return;
        }
        let s = g.world.block(pos[0], pos[1], pos[2]);
        let facts = g.res.facts().get(s);
        if g.res.behaviours().is_air(s) || facts.hardness < 0.0 {
            return;
        }
        let name = facts.name.clone();
        let items = g.items();
        if !items.block_in_tag("minecraft:mineable/axe", &name)
            && !items.block_in_tag("minecraft:leaves", &name)
        {
            return;
        }
        self.seen.push((pos, s.0));
        g.native_player_break_block(self.player, pos, false);
    }
}

// One tick of a felling, as its player: the Terra Truncator still in the slot it was in breaks
// (`BlockSwapper` keeps the stack it began with; one that broke leaves an empty hand). False once
// the felling is over, or its player isn't in its level any more.
fn tick_felling(game: &mut Game<'_>, f: &mut Felling) -> bool {
    let mut alive = false;
    let mut seen = Vec::new();
    let (slot, player) = (f.slot, f.player);
    let swapper = &mut f.swapper;
    game.native_as_player(player, |g| {
        g.with_hand(Hand::Main, |g| {
            let prior = g.selected;
            let own = g
                .inventory
                .get(slot)
                .and_then(Option::as_ref)
                .is_some_and(|s| &*s.item == tools::TERRA_AXE);
            let mut emptied = None;
            if own {
                g.selected = slot;
            } else if let Some(cell) = g.inventory.get_mut(prior) {
                emptied = Some(cell.take());
            }
            let mut world = Fell {
                game: &mut *g,
                player,
                seen: &mut seen,
            };
            alive = swapper.tick(&mut world);
            g.selected = prior;
            if let (Some(back), Some(cell)) = (emptied, g.inventory.get_mut(prior)) {
                *cell = back;
            }
        });
    });
    let dim = f.dim;
    for (pos, block) in seen {
        state_seen(dim, player, pos, block);
    }
    alive
}

// ---- the elementium shears -------------------------------------------------------------------

// `ElementiumShearsItem.getUseDuration`: used until let go.
pub(super) fn use_duration(stack: &ItemStack, duration: &mut i32) {
    if &*stack.item == tools::ELEMENTIUM_SHEARS {
        *duration = tools::SHEARS_USE_DURATION;
    }
}

// `ElementiumShearsItem.getUseAnimation`: the bow's pose.
pub(super) fn use_animation(stack: &ItemStack, animation: &mut UseAnimation) {
    if &*stack.item == tools::ELEMENTIUM_SHEARS {
        *animation = UseAnimation::Bow;
    }
}

// `ElementiumShearsItem.onUseTick`: every fifth tick of the use, the first `Shearable` within 12
// blocks that is ready is sheared, and the shears take a point of wear (mana pays it).
pub(super) fn use_tick(
    game: &mut Game<'_>,
    stack: &ItemStack,
    entity: Option<i32>,
    remaining: i32,
) {
    if entity.is_some()
        || &*stack.item != tools::ELEMENTIUM_SHEARS
        || !tools::shears_shear_now(remaining)
        || !game.native_authoritative()
    {
        return;
    }
    let (Some(player), Some(at), Some(eye)) =
        (game.native_acting_player(), feet(game), game.eye_position())
    else {
        return;
    };
    let area = tools::shears_area(at);
    let mob = game.mobs.iter().enumerate().find_map(|(i, m)| {
        if m.removed || !weapons::intersects(aabb(m.bounds()), area) {
            return None;
        }
        let ready =
            match tools::ready_for_shearing(&m.kind.id, m.alive(), m.is_baby(), m.extra.sheared) {
                Some(ready) => ready,
                None if &*m.kind.id == "minecraft:copper_golem" => {
                    m.alive() && game.golem_shearable(i)
                }
                None => return None,
            };
        // A leashed mob is left: the shears the server shears with would cut its lead instead.
        (ready && m.extra.leash.is_none()).then_some(m.id)
    });
    let Some(mob) = mob else {
        return;
    };
    let hand = game.used_hand().unwrap_or(game.hand);
    let look = (game.yaw, game.pitch);
    state().shearings.push(Shearing {
        dim: game.dim.id(),
        player: player.uuid,
        mob,
        eye,
        look,
        hand,
    });
}

// `Shearable.shear`, after the level's tick: the mob is sheared as vanilla's shears shear it (its
// drops, sound and shearing game event), then the player's elementium shears wear by a point.
fn shear(game: &mut Game<'_>, s: &Shearing) {
    if !game.mobs.iter().any(|m| m.id == s.mob && !m.removed) {
        return;
    }
    let target = NativeUseTarget::Entity {
        id: s.mob as i32,
        location: [0.0; 3],
    };
    let shears = ItemStack::new("minecraft:shears", 1);
    let used = game.native_use_as_player(shears, s.eye, s.look.0, s.look.1, target, false);
    if !used.is_some_and(|u| u.consumed) {
        return;
    }
    let hand = s.hand;
    game.native_as_player(s.player, |g| {
        let held = match hand {
            Hand::Main => g.inventory.get(g.selected).and_then(Option::as_ref),
            Hand::Off => g.offhand.as_ref(),
        };
        if held.is_some_and(|h| &*h.item == tools::ELEMENTIUM_SHEARS) {
            g.with_hand(hand, |g| g.damage_held(1));
        }
    });
}

// ---- the elementium hoe -----------------------------------------------------------------------

// `ElementiumHoeItem.useOn`: a till that worked (`HoeItem.useOn`, 26.3's block transformer) leaves
// farmland at its wettest.
pub(super) fn use_on(
    game: &mut Game<'_>,
    stack: &ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    if *claimed || &*stack.item != tools::ELEMENTIUM_HOE || !game.native_authoritative() {
        return;
    }
    if !matches!(game.use_block_transformer(stack, pos, face), Ok(true)) {
        return;
    }
    game.native_set_block_property(pos, "moisture", tools::TILLED_MOISTURE);
    *claimed = true;
}

// ---- the elementium axe -----------------------------------------------------------------------

// `elementium_axe_beheading`, the pool `LootHandler` adds to every entity's loot table: a mob
// killed by a player holding an elementium axe in the main hand may drop its head, more likely
// with the axe's Looting. Only when the mob's own table rolled, as the pool is part of it. A
// player's head is `botania_tool_uses`': its killer's hand is read after the level's tick.
pub(super) fn loot_drops(
    game: &mut Game<'_>,
    context: &NativeLootDrops<'_>,
    drops: &mut Vec<ItemStack>,
) {
    if context.block.is_some() || context.used || context.table.is_empty() {
        return;
    }
    let (Some((_, kind)), Some(killer)) = (context.entity, context.player) else {
        return;
    };
    if kind == "minecraft:player" {
        return;
    }
    let skeleton = game.res.mob_data().entity_has("minecraft:skeletons", kind);
    let Some(head) = tools::beheading(kind, skeleton) else {
        return;
    };
    let mut held = None;
    game.native_as_player(killer, |g| held = main_stack(g));
    let Some(axe) = held.filter(|s| &*s.item == tools::ELEMENTIUM_AXE) else {
        return;
    };
    let looting = axe.enchantment_level("minecraft:looting") as i32;
    if tools::beheads(head, looting, random_float()) {
        drops.push(ItemStack::new(head.item, 1));
    }
}

// ---- the item models --------------------------------------------------------------------------

// Sets the stack's model flags, the component gone when none is on; nothing written when they
// are already so.
fn set_model_flags(stack: &mut ItemStack, flags: &[bool]) {
    let want = flags.iter().any(|f| *f).then(|| {
        let mut c = Compound::new();
        let list = flags.iter().map(|f| Tag::Byte(i8::from(*f))).collect();
        c.insert(key("flags"), Tag::List(list));
        Tag::Compound(c)
    });
    if stack.data().and_then(|d| d.other(MODEL_DATA)) == want.as_ref() {
        return;
    }
    stack.data_mut().set_other(MODEL_DATA, want);
    stack.tidy();
}

// Each tick of a carried Terra Shatterer or Terra Truncator, on the server: the flags its item
// model reads. The Terra Shatterer's `botania:active` is its `enabled` and `botania:tipped` its
// `tipped`; the Terra Truncator's model rests (flag 0, Botania's `botania:active` 0) while its
// holder sneaks or carries an active Stone of Temperance (`shouldBreak`).
pub(super) fn inventory_tick(game: &mut Game<'_>, slot: i32) {
    let Some(item) = game.native_carried_stack(slot).map(|s| s.item.clone()) else {
        return;
    };
    let (pick, axe) = (&*item == weapons::TERRA_PICK, &*item == tools::TERRA_AXE);
    if (!pick && !axe) || !game.native_authoritative() {
        return;
    }
    let resting =
        axe && !tools::truncator_fells(game.native_sneaking(), area_rings(game).temperance);
    let Some(stack) = game.native_carried_stack(slot) else {
        return;
    };
    if pick {
        let flags = [custom_flag(stack, "enabled"), custom_flag(stack, "tipped")];
        set_model_flags(stack, &flags);
    } else {
        set_model_flags(stack, &[resting]);
    }
}

// ---- after a level's tick ---------------------------------------------------------------------

// After a level's tick (`TerraTruncatorItem.onTickEnd`): its fellings take their blocks, the
// shears shear what they found, and the players who broke blocks with an area tool are sent
// those blocks' break events.
pub(super) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    if !game.native_authoritative() {
        return;
    }
    let (mut fellings, shearings) = {
        let mut s = state();
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.fellings)
            .into_iter()
            .partition(|f| f.dim == dimension);
        s.fellings = rest;
        let (cut, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.shearings)
            .into_iter()
            .partition(|c| c.dim == dimension);
        s.shearings = rest;
        (mine, cut)
    };
    if !fellings.is_empty() {
        state().felling = true;
        fellings.retain_mut(|f| tick_felling(game, f));
        let mut s = state();
        s.felling = false;
        // Fellings a felling's breaks started (none: they are refused) keep their order after.
        let started = std::mem::take(&mut s.fellings);
        s.fellings = fellings;
        s.fellings.extend(started);
    }
    for s in &shearings {
        shear(game, s);
    }
    let seen = {
        let mut s = state();
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.seen)
            .into_iter()
            .partition(|e| e.dim == dimension);
        s.seen = rest;
        mine
    };
    send_seen(game, &seen);
}
