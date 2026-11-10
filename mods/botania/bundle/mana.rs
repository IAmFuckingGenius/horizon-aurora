// Botania for Horizon Aurora: the mana items, added to `crates/horizon-client/src/game.rs` as
// `mod botania_mana` by horizon-mixins, beside `mod botania` (runtime.rs). A modified port of
// Vazkii's Botania (Botania License, see LICENSE and NOTICE.md in this bundle): the items that
// keep mana (Mana Tablet, Band of Mana, Greater Band of Mana, Mana Mirror, Terra Shatterer),
// `ManaItemHandler` over a player's inventory, the mana Botania's tools, bows and armour pay
// instead of durability and to mend themselves, the armour sets' discounts and the Terrasteel
// set's bonus, and a mana pool charging or draining the mana items lying in it. The numbers live
// in `botania_core::{mana_item, mana_handler, mana_gear}`.
//
// Inner attributes and `//!` comments are not allowed in a horizon-mixins module payload.

use super::Game;
use botania_core::consts::PoolVariant;
use botania_core::fx::Record;
use botania_core::mana_gear::{self, ArmorSet};
use botania_core::mana_handler::{self, Asker, DamagePayer};
use botania_core::mana_item::{self, Bar, Kind, ManaStack, Nbt, PoolMode};
use botania_core::pool::Pool;
use botania_core::tools;
use horizon_client::containers::BlockEntity;
use horizon_client::effects::EffectAction;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_core::random::RandomSource;
use horizon_world::nbt::{Compound, JavaString, Tag};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

const POOL: &str = "botania:mana_pool";
const MIRROR: &str = "botania:mana_mirror";
const UNBREAKING: &str = "minecraft:unbreaking";
const INFINITY: &str = "minecraft:infinity";
// `SoundSource.PLAYERS`'s ordinal.
const PLAYERS: usize = 7;
// `ChatFormatting.GRAY` and `RED`.
const GRAY: u32 = 0xAAAAAA;
const RED: u32 = 0xFF5555;
// The tooltip's mana bar: a 3 pixel bar in a 1 pixel black frame (`ManaBarTooltipComponent`).
const BAR_HEIGHT: f32 = 5.0;
// The Terra Shatterer's rank letters, a line of text over its bar.
const RANK_ROW: f32 = 10.0;

// ---- state ------------------------------------------------------------------------------------

struct State {
    // The widest line of the mana item tooltip last built, which the mana bar spans.
    tooltip_width: f32,
}

static STATE: Mutex<State> = Mutex::new(State { tooltip_width: 0.0 });

// When the bar's pulse started (`ClientTickHandler.ticksInGame`).
static START: LazyLock<Instant> = LazyLock::new(Instant::now);

// Never held across a call into the game.
fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- the stack's NBT (`ItemNBTHelper` keys in `minecraft:custom_data`) -------------------------

fn key(k: &str) -> JavaString {
    JavaString::from(k)
}

fn get_int(nbt: &Compound, k: &str) -> Option<i32> {
    match nbt.get(&key(k))? {
        Tag::Int(v) => Some(*v),
        Tag::Short(v) => Some(i32::from(*v)),
        Tag::Byte(v) => Some(i32::from(*v)),
        Tag::Long(v) => i32::try_from(*v).ok(),
        _ => None,
    }
}

fn read_nbt(stack: &ItemStack) -> Nbt {
    let data = stack.custom_data();
    let int = |k: &str| data.and_then(|c| get_int(c, k));
    Nbt {
        mana: int("mana").unwrap_or(0),
        creative: int("creative").is_some_and(|v| v != 0),
        one_use: int("oneUse").is_some_and(|v| v != 0),
        max_mana: int("maxMana"),
        backlog: int("manaBacklog"),
    }
}

// The stack's `ManaItem` (`XplatAbstractions.findManaItem`), if it has one.
fn mana_stack(stack: &ItemStack) -> Option<ManaStack> {
    if stack.count == 0 {
        return None;
    }
    let kind = Kind::of_item(&stack.item)?;
    Some(ManaStack::new(kind, stack.count as i32, read_nbt(stack)))
}

fn set_custom_data(stack: &mut ItemStack, c: Compound) {
    let value = (!c.is_empty()).then_some(Tag::Compound(c));
    stack.data_mut().set_other("minecraft:custom_data", value);
    stack.tidy();
}

// Writes what changed between `old` and `new`: `setMana` keeps the tag only above 0, the mirror's
// `setMaxMana` and `setManaBacklog` always write theirs.
fn write_nbt(stack: &mut ItemStack, old: &Nbt, new: &Nbt) {
    if old == new {
        return;
    }
    let mut c = stack.custom_data().cloned().unwrap_or_default();
    if new.mana != old.mana {
        if new.mana > 0 {
            c.insert(key("mana"), Tag::Int(new.mana));
        } else {
            c.remove(&key("mana"));
        }
    }
    if new.max_mana != old.max_mana
        && let Some(m) = new.max_mana
    {
        c.insert(key("maxMana"), Tag::Int(m));
    }
    if new.backlog != old.backlog
        && let Some(b) = new.backlog
    {
        c.insert(key("manaBacklog"), Tag::Int(b));
    }
    set_custom_data(stack, c);
}

// ---- ManaItemHandler over the acting player's inventory ----------------------------------------

#[derive(Clone, Copy)]
enum Slot {
    Main(usize),
    Off,
}

fn terra_blacklisted(game: &Game<'_>, stack: &ItemStack) -> bool {
    game.items()
        .item_in_tag(mana_item::TERRA_PICK_BLACKLIST, &stack.item)
}

fn asker_of(game: &Game<'_>, stack: &ItemStack) -> Asker {
    Asker {
        mana: mana_stack(stack),
        terra_blacklisted: terra_blacklisted(game, stack),
    }
}

// `getManaItems`: the mana items of `Inventory.items` then the off hand. The asking stack is
// skipped (`stackInSlot == stack`): by value, the first slot holding it, which only matters when
// it is a mana item itself.
fn sources(game: &Game<'_>, skip: Option<&ItemStack>) -> (Vec<Slot>, Vec<ManaStack>) {
    let mut slots = Vec::new();
    let mut items = Vec::new();
    let mut skip = skip;
    let main = game
        .inventory
        .iter()
        .take(36)
        .enumerate()
        .map(|(i, s)| (Slot::Main(i), s));
    for (slot, s) in main.chain(std::iter::once((Slot::Off, &game.offhand))) {
        let Some(s) = s else {
            continue;
        };
        if skip.is_some_and(|k| k == s) {
            skip = None;
            continue;
        }
        if let Some(m) = mana_stack(s) {
            slots.push(slot);
            items.push(m);
        }
    }
    (slots, items)
}

// Runs one of `ManaItemHandler`'s loops for `stack` over the acting player's mana items, and
// writes the mana that moved back into their stacks.
fn with_sources<R>(
    game: &mut Game<'_>,
    stack: &ItemStack,
    run: impl FnOnce(&mut [ManaStack], &Asker) -> R,
) -> R {
    let asker = asker_of(game, stack);
    let skip = asker.mana.is_some().then_some(stack);
    let (slots, mut items) = sources(game, skip);
    let before = items.clone();
    let out = run(&mut items, &asker);
    for ((slot, b), a) in slots.iter().zip(&before).zip(&items) {
        if b.nbt == a.nbt {
            continue;
        }
        let target = match slot {
            Slot::Main(i) => game.inventory.get_mut(*i).and_then(Option::as_mut),
            Slot::Off => game.offhand.as_mut(),
        };
        if let Some(s) = target {
            write_nbt(s, &b.nbt, &a.nbt);
        }
    }
    out
}

fn worn<'a>(game: &'a Game<'_>) -> [Option<&'a str>; 4] {
    std::array::from_fn(|i| {
        game.armor[i]
            .as_ref()
            .filter(|s| s.count > 0)
            .map(|s| &*s.item)
    })
}

// `discountManaForTool`: the worn helmets' discounts and 5% per Unbreaking level of `tool`.
fn tool_cost(game: &Game<'_>, tool: &ItemStack, cost: i32) -> i32 {
    let discounts = mana_gear::armor_discounts(worn(game));
    let unbreaking = tool.enchantment_level(UNBREAKING) as i32;
    mana_handler::discount_cost(cost, mana_handler::full_discount(&discounts, unbreaking))
}

fn request_here(game: &mut Game<'_>, stack: &ItemStack, mana: i32, remove: bool) -> i32 {
    if stack.count == 0 {
        return 0;
    }
    with_sources(game, stack, |items, asker| {
        mana_handler::request_mana(items, asker, mana, remove)
    })
}

fn request_exact_here(game: &mut Game<'_>, stack: &ItemStack, mana: i32, remove: bool) -> bool {
    if stack.count == 0 {
        return false;
    }
    with_sources(game, stack, |items, asker| {
        mana_handler::request_mana_exact(items, asker, mana, remove)
    })
}

fn dispatch_here(game: &mut Game<'_>, stack: &ItemStack, mana: i32, add: bool) -> i32 {
    if stack.count == 0 {
        return 0;
    }
    with_sources(game, stack, |items, asker| {
        mana_handler::dispatch_mana(items, asker, mana, add)
    })
}

fn dispatch_exact_here(game: &mut Game<'_>, stack: &ItemStack, mana: i32, add: bool) -> bool {
    if stack.count == 0 {
        return false;
    }
    with_sources(game, stack, |items, asker| {
        mana_handler::dispatch_mana_exact(items, asker, mana, add)
    })
}

fn request_exact_for_tool_here(
    game: &mut Game<'_>,
    stack: &ItemStack,
    mana: i32,
    remove: bool,
) -> bool {
    let cost = tool_cost(game, stack, mana);
    request_exact_here(game, stack, cost, remove)
}

// Runs `run` with player `player`'s state in place (theirs already inside their own hook);
// the default answer when they aren't reached.
fn as_player<R: Default>(
    game: &mut Game<'_>,
    player: u128,
    run: impl FnOnce(&mut Game<'_>) -> R,
) -> R {
    let mut out = None;
    game.native_as_player(player, |g| out = Some(run(g)));
    out.unwrap_or_default()
}

// `ManaItemHandler.requestMana(stack, player, manaToGet, remove)`: up to `mana` from the player's
// mana items; how much they had.
#[allow(dead_code, reason = "shared API for the rest of the bundle")]
pub(super) fn request_mana(
    game: &mut Game<'_>,
    player: u128,
    stack: &ItemStack,
    mana: i32,
    remove: bool,
) -> i32 {
    as_player(game, player, |g| request_here(g, stack, mana, remove))
}

// `ManaItemHandler.requestManaExact`: exactly `mana`, or nothing.
#[allow(dead_code, reason = "shared API for the rest of the bundle")]
pub(super) fn request_mana_exact(
    game: &mut Game<'_>,
    player: u128,
    stack: &ItemStack,
    mana: i32,
    remove: bool,
) -> bool {
    as_player(game, player, |g| request_exact_here(g, stack, mana, remove))
}

// `ManaItemHandler.requestManaForTool`: `requestMana` of `cost` less the armour's and Unbreaking's
// discount (`discountManaForTool`).
#[allow(dead_code, reason = "shared API for the rest of the bundle")]
pub(super) fn request_mana_for_tool(
    game: &mut Game<'_>,
    player: u128,
    stack: &ItemStack,
    cost: i32,
    remove: bool,
) -> i32 {
    as_player(game, player, |g| {
        let cost = tool_cost(g, stack, cost);
        request_here(g, stack, cost, remove)
    })
}

// `ManaItemHandler.requestManaExactForTool`.
#[allow(dead_code, reason = "shared API for the rest of the bundle")]
pub(super) fn request_mana_exact_for_tool(
    game: &mut Game<'_>,
    player: u128,
    stack: &ItemStack,
    cost: i32,
    remove: bool,
) -> bool {
    as_player(game, player, |g| {
        request_exact_for_tool_here(g, stack, cost, remove)
    })
}

// `ManaItemHandler.dispatchMana`: gives `mana` to the first of the player's mana items that takes
// it; how much fitted.
#[allow(dead_code, reason = "shared API for the rest of the bundle")]
pub(super) fn dispatch_mana(
    game: &mut Game<'_>,
    player: u128,
    stack: &ItemStack,
    mana: i32,
    add: bool,
) -> i32 {
    as_player(game, player, |g| dispatch_here(g, stack, mana, add))
}

// `ManaItemHandler.dispatchManaExact`.
#[allow(dead_code, reason = "shared API for the rest of the bundle")]
pub(super) fn dispatch_mana_exact(
    game: &mut Game<'_>,
    player: u128,
    stack: &ItemStack,
    mana: i32,
    add: bool,
) -> bool {
    as_player(game, player, |g| dispatch_exact_here(g, stack, mana, add))
}

struct Payer<'a, 'g> {
    game: &'a mut Game<'g>,
    stack: &'a ItemStack,
    cost: i32,
}

impl DamagePayer for Payer<'_, '_> {
    fn pay(&mut self, remove: bool) -> bool {
        request_exact_for_tool_here(self.game, self.stack, self.cost, remove)
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        self.game.world.random.next_int_bound(bound).unwrap_or(0)
    }
}

// `ToolCommons.damageItemIfPossible(stack, amount, player, manaPerDamage)`: each point of `amount`
// paid with `mana_per_damage` mana (less the tool discount) while the mana lasts, Unbreaking
// making a paid point free with a chance of level / (level + 1); the damage left to the item.
// The wear Horizon's game deals to Botania's own tools, bows and armour is already paid by
// `item_damaged`: this is for wear a mod deals itself.
#[allow(dead_code, reason = "shared API for the rest of the bundle")]
pub(super) fn damage_item_if_possible(
    game: &mut Game<'_>,
    player: u128,
    stack: &ItemStack,
    amount: i32,
    mana_per_damage: i32,
) -> i32 {
    let mut left = amount;
    game.native_as_player(player, |g| {
        let unbreaking = stack.enchantment_level(UNBREAKING) as i32;
        let mut payer = Payer {
            game: g,
            stack,
            cost: mana_per_damage,
        };
        left = mana_handler::damage_item_if_possible(amount, unbreaking, &mut payer);
    });
    left
}

// ---- the hooks: wear, inventory ticks -----------------------------------------------------------

// The stack of `item` a player's wear lands on: the held one, the off hand's, then the armour.
fn worn_or_held(game: &Game<'_>, item: &str) -> Option<ItemStack> {
    let held = game.inventory.get(game.selected).and_then(Option::as_ref);
    held.into_iter()
        .chain(game.offhand.as_ref())
        .chain(game.armor.iter().flatten())
        .find(|s| &*s.item == item && s.count > 0)
        .cloned()
}

// `native_item_damaged`: Botania's tools, bows and armour pay their wear with mana
// (`damageItem` → `ToolCommons.damageItemIfPossible`). The hook hears the wear vanilla's
// Unbreaking left, so each of those points is paid in full (Botania's own Unbreaking roll would
// count the enchantment twice).
pub(super) fn item_damaged(
    game: &mut Game<'_>,
    player: u128,
    item: &str,
    amount: &mut i32,
    cancel: &mut bool,
) {
    if *cancel || *amount <= 0 || !item.starts_with("botania:") {
        return;
    }
    let Some(stack) = worn_or_held(game, item) else {
        return;
    };
    let infinity = stack.enchantment_level(INFINITY) > 0;
    let per_damage = match mana_gear::tool(item, infinity) {
        Some(t) => t.per_damage,
        None => match mana_gear::armor(item) {
            Some(a) => a.per_damage,
            None => return,
        },
    };
    let mut left = *amount;
    game.native_as_player(player, |g| {
        while left > 0 && request_exact_for_tool_here(g, &stack, per_damage, true) {
            left -= 1;
        }
    });
    *amount = left;
}

fn mend(game: &mut Game<'_>, slot: i32) {
    if let Some(s) = game.native_carried_stack(slot)
        && s.damage > 0
    {
        s.damage -= 1;
    }
}

// `native_inventory_tick`: Botania's `inventoryTick`s, on the server.
pub(super) fn inventory_tick(game: &mut Game<'_>, slot: i32, _selected: bool) {
    let Some(stack) = game.native_carried_stack(slot) else {
        return;
    };
    if !stack.item.starts_with("botania:") {
        return;
    }
    let stack = stack.clone();
    if !game.native_authoritative() {
        return;
    }
    let item = &*stack.item;
    // The tools and bows mend a point a tick for `requestManaExactForTool(stack, player, cost)`.
    if let Some(t) = mana_gear::tool(item, false) {
        if let Some(cost) = t.repair
            && stack.damage > 0
            && request_exact_for_tool_here(game, &stack, cost, true)
        {
            mend(game, slot);
        }
        return;
    }
    // The armour mends for `requestManaExact(stack, player, 2 * manaPerDamage)`, no discount.
    if let Some(a) = mana_gear::armor(item) {
        if stack.damage > 0 && request_exact_here(game, &stack, a.repair, true) {
            mend(game, slot);
        }
        if a.set == ArmorSet::Terrasteel && a.helmet && (100..=103).contains(&slot) {
            terrasteel_tick(game, &stack);
        }
        return;
    }
    if item == MIRROR {
        mirror_tick(game, slot, &stack);
    }
}

// `TerrasteelHelmItem.inventoryTick` with the helmet worn and the whole set on: a heart every 80
// ticks while hurt with food between 1 and 17, and 10 mana every 10 ticks to the mana items.
fn terrasteel_tick(game: &mut Game<'_>, helm: &ItemStack) {
    if mana_gear::worn_set(worn(game)) != Some(ArmorSet::Terrasteel) {
        return;
    }
    let Some(p) = game.player.as_mut() else {
        return;
    };
    let tick = p.health.tick_count;
    let food = p.health.food.food;
    let health = p.health.health();
    let max = p.attribute("minecraft:max_health").unwrap_or(20.0) as f32;
    let hurt = health > 0.0 && health < max;
    if mana_gear::terrasteel_heals(food, hurt, tick) {
        game.apply_effect_action(EffectAction::Heal(1.0));
    }
    if mana_gear::terrasteel_gives_mana(tick) {
        dispatch_exact_here(game, helm, mana_gear::TERRASTEEL_MANA, true);
    }
}

fn pool_of(nbt: &Compound, block: &str) -> Pool {
    let variant = PoolVariant::of_block(block).unwrap_or(PoolVariant::Default);
    let mut p = Pool::new(variant);
    p.mana = get_int(nbt, "mana").unwrap_or(0);
    if let Some(cap) = get_int(nbt, "manaCap")
        && cap > 0
    {
        p.cap = cap;
    }
    p.mana = p.mana.clamp(0, p.cap);
    p
}

// The mirror's bound pool (`GlobalPos` under `pos`: `{dimension, pos: [I; x, y, z]}`).
fn mirror_binding(stack: &ItemStack) -> Option<(String, [i32; 3])> {
    let Tag::Compound(g) = stack.custom_data()?.get(&key("pos"))? else {
        return None;
    };
    let Tag::String(dim) = g.get(&key("dimension"))? else {
        return None;
    };
    let pos = match g.get(&key("pos"))? {
        Tag::IntArray(v) if v.len() == 3 => [v[0], v[1], v[2]],
        Tag::List(v) if v.len() == 3 => {
            let int = |t: &Tag| match t {
                Tag::Int(i) => Some(*i),
                _ => None,
            };
            [int(&v[0])?, int(&v[1])?, int(&v[2])?]
        }
        _ => return None,
    };
    (pos[1] != i32::MIN).then(|| (dim.to_string_lossy(), pos))
}

// `ManaMirrorItem.inventoryTick`: a bound pool takes the mana spent from the mirror (its
// backlog) and the mirror shows the pool's mana; a pool gone leaves it empty. A pool in another
// dimension or an unloaded chunk is out of reach (Botania loads it): the mirror is left as it is.
fn mirror_tick(game: &mut Game<'_>, slot: i32, stack: &ItemStack) {
    let Some((dim, pos)) = mirror_binding(stack) else {
        return;
    };
    if dim != game.dim.id() || !game.world.loaded(pos[0], pos[2]) {
        return;
    }
    let before = read_nbt(stack);
    let mut nbt = before;
    let pool = match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(m)) if m.id == POOL => Some(m.nbt.clone()),
        _ => None,
    };
    match pool {
        None => nbt.mana = 0,
        Some(pool_nbt) => {
            let mut p = pool_of(&pool_nbt, &game.native_block(pos));
            let backlog = nbt.backlog.unwrap_or(0);
            if backlog != 0 && p.receive(backlog) {
                let mana = p.mana;
                game.native_with_mod_block_entity(pos, |m| {
                    m.nbt.insert(key("mana"), Tag::Int(mana));
                });
            }
            nbt.backlog = Some(0);
            nbt.mana = p.current().max(0);
            nbt.max_mana = Some(p.cap);
        }
    }
    if nbt != before
        && let Some(s) = game.native_carried_stack(slot)
    {
        write_nbt(s, &before, &nbt);
    }
}

// `native_use_item_on`: `ManaMirrorItem.useOn`, a sneak-click on a pool binds the mirror to it.
pub(super) fn use_on(
    game: &mut Game<'_>,
    stack: &ItemStack,
    pos: [i32; 3],
    _face: [i32; 3],
    claimed: &mut bool,
) {
    if *claimed || &*stack.item != MIRROR || !game.native_authoritative() {
        return;
    }
    if !game.native_sneaking() {
        return;
    }
    let is_pool = matches!(
        game.world.block_entities.get(&(pos[0], pos[1], pos[2])),
        Some(BlockEntity::Mod(m)) if m.id == POOL
    );
    if !is_pool {
        return;
    }
    let dim = game.dim.id();
    if let Some(held) = game.held_mut().as_mut()
        && &*held.item == MIRROR
    {
        let mut g = Compound::new();
        g.insert(key("dimension"), Tag::String(JavaString::from(dim)));
        g.insert(key("pos"), Tag::IntArray(vec![pos[0], pos[1], pos[2]]));
        let mut c = held.custom_data().cloned().unwrap_or_default();
        c.insert(key("pos"), Tag::Compound(g));
        set_custom_data(held, c);
    }
    if let Some(at) = game
        .player
        .as_ref()
        .map(|p| [p.position.x, p.position.y, p.position.z])
    {
        game.native_play_sound_in_place("botania:ding", at, PLAYERS, 1.0, 1.0);
    }
    *claimed = true;
}

// ---- the pool and the mana items in it --------------------------------------------------------

// `ManaPoolBlockEntity.serverTick`'s loop over the item entities in the pool's cell: an outputting
// pool charges the mana items, any other drains them, 1000 a tick each (no bellows are ported).
// Every 10 of the pool's ticks a moving item starts the charging animation's wisps on the clients
// near it (`CHARGE_EFFECT_EVENT`). Called by `botania::pool_tick` after the infusions.
pub(super) fn pool_items(
    game: &mut Game<'_>,
    pos: [i32; 3],
    pool: &mut Pool,
    nbt: &Compound,
    ticks: u32,
) {
    let dim = game.dim.id();
    let items = game.native_item_entities(
        dim,
        [f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])],
        [
            f64::from(pos[0] + 1),
            f64::from(pos[1] + 1),
            f64::from(pos[2] + 1),
        ],
    );
    if items.is_empty() {
        return;
    }
    let mode = PoolMode {
        outputting: get_int(nbt, "outputting").is_some_and(|v| v != 0),
        can_accept: get_int(nbt, "canAccept").is_none_or(|v| v != 0),
        can_spare: get_int(nbt, "canSpare").is_none_or(|v| v != 0),
        mana_void_below: game.native_block([pos[0], pos[1] - 1, pos[2]]) == "botania:mana_void",
        bellows: 0,
    };
    let mut paths = Vec::new();
    for e in &items {
        let Some(mut m) = mana_stack(&e.stack) else {
            continue;
        };
        let before = m;
        let did = mana_item::pool_transfer(pool, mode, &mut m);
        if m.nbt != before.nbt {
            let mut s = e.stack.clone();
            write_nbt(&mut s, &before.nbt, &m.nbt);
            game.native_set_item_entity(dim, e.id, s);
        }
        if did && ticks % mana_item::POOL_EFFECT_INTERVAL == 0 {
            paths.push(mana_item::encode_item_position(pos, e.position));
        }
    }
    if paths.is_empty() {
        return;
    }
    let relative = if pool.cap > 0 {
        pool.current() as f32 / pool.cap as f32
    } else {
        0.0
    };
    // The block event's parameter is the item's place, which fits in a byte.
    let charging = mode.outputting;
    super::botania_particles::send(
        game,
        paths.into_iter().map(|param| Record::Flow {
            pool: pos,
            param: param as u8,
            relative,
            charging,
        }),
    );
}

// ---- the client: bar, tooltip, the tooltip's mana bar ------------------------------------------

// `native_item_bar`: `getBarWidth`/`getBarColor` of the mana items (the creative tablet shows
// none, the Terra Shatterer its durability).
pub(super) fn item_bar(_game: &Game<'_>, stack: &ItemStack, bar: &mut Option<(i32, u32)>) {
    let Some(m) = mana_stack(stack) else {
        return;
    };
    match m.bar() {
        Bar::Vanilla => {}
        Bar::Hidden => *bar = None,
        Bar::Shown(width, color) => *bar = Some((width, color)),
    }
}

// `native_tooltip`: `appendHoverText` of the creative tablet ("Creative") and the Terra Shatterer
// (its rank, and "Get a Life" when full), under the name.
pub(super) fn tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    let Some(m) = mana_stack(stack) else {
        return;
    };
    let mut extra = Vec::new();
    match m.kind {
        Kind::Tablet if m.nbt.creative => {
            extra.push((
                Text::translate("botaniamisc.creative", &[]).with_color(GRAY),
                u32::MAX,
            ));
        }
        Kind::TerraPick => {
            let level = mana_item::terra_pick_level(m.nbt.mana);
            let rank = Text::translate(&format!("botania.rank{level}"), &[]);
            extra.push((Text::translate("botaniamisc.toolRank", &[rank]), u32::MAX));
            if m.mana() == i32::MAX {
                extra.push((
                    Text::translate("botaniamisc.getALife", &[]).with_color(RED),
                    u32::MAX,
                ));
            }
        }
        _ => {}
    }
    let at = lines.len().min(1);
    lines.splice(at..at, extra);
    let width = lines
        .iter()
        .map(|(t, _)| game.native_text_width(&t.resolve_legacy().plain()))
        .fold(0.0, f32::max);
    state().tooltip_width = width;
}

// `native_tooltip_component`: `ManaBarTooltip`, a bar as wide as the tooltip; the Terra
// Shatterer's with its rank letters over it.
pub(super) fn tooltip_component(_game: &Game<'_>, stack: &ItemStack, size: &mut Option<[f32; 2]>) {
    let Some(m) = mana_stack(stack) else {
        return;
    };
    let width = state().tooltip_width.max(24.0);
    let height = if m.kind == Kind::TerraPick {
        RANK_ROW + BAR_HEIGHT
    } else {
        BAR_HEIGHT
    };
    *size = Some([width, height]);
}

// `ManaBarTooltipComponent.renderImage`: the fill in a pulsing light blue
// (`hsvToRgb(0.528, (sin(ticks * 0.2) + 1) * 0.3 + 0.4, 1)`), the rest dark grey.
pub(super) fn tooltip_component_draw(
    game: &Game<'_>,
    stack: &ItemStack,
    batch: &mut HudBatch,
    origin: [f32; 2],
    size: [f32; 2],
) {
    let Some(m) = mana_stack(stack) else {
        return;
    };
    if m.kind == Kind::TerraPick {
        rank_bar(game, &m, batch, origin, size);
        return;
    }
    let [x, y] = origin;
    let w = size[0];
    let fraction = m.tooltip_fraction().clamp(0.0, 1.0);
    let inner = (w - 2.0).max(0.0);
    let filled = (inner * fraction).floor();
    let ticks = START.elapsed().as_secs_f64() * 20.0;
    let saturation = (((ticks * 0.2).sin() as f32) + 1.0) * 0.3 + 0.4;
    let color = botania_core::wand::hsv_to_rgb(0.528, saturation, 1.0);
    game.native_fill(batch, [x, y, w, BAR_HEIGHT], 0xFF00_0000);
    if filled > 0.0 {
        game.native_fill(batch, [x + 1.0, y + 1.0, filled, 3.0], 0xFF00_0000 | color);
    }
    if inner - filled > 0.0 {
        game.native_fill(
            batch,
            [x + 1.0 + filled, y + 1.0, inner - filled, 3.0],
            0xFF55_5555,
        );
    }
}

// A rank's name as the lang file has it (`botania.rank0` is `§cD`), in its runs of colour.
fn rank_runs(level: usize) -> Vec<(String, u32)> {
    let name = horizon_client::lang::get(&format!("botania.rank{level}"));
    tools::legacy_runs(&name, 0xFFFFFF)
}

fn runs_width(game: &Game<'_>, runs: &[(String, u32)]) -> f32 {
    runs.iter().map(|(t, _)| game.native_text_width(t)).sum()
}

fn draw_runs(game: &Game<'_>, batch: &mut HudBatch, runs: &[(String, u32)], at: [f32; 2]) {
    let [mut x, y] = at;
    for (text, rgb) in runs {
        game.native_text(batch, text, [x, y], *rgb, true);
        x += game.native_text_width(text);
    }
}

// `ManaBarTooltipComponent` for the Terra Shatterer: its rank's letter over the start of the bar
// and the next rank's over its end (none at SS), and the bar filled toward the next rank with a
// rainbow drifting along it (`hsvToRgb((ticks * 0.01 + column / width) % 1, 1, 1)`), one column
// short of full below SS.
fn rank_bar(
    game: &Game<'_>,
    m: &ManaStack,
    batch: &mut HudBatch,
    origin: [f32; 2],
    size: [f32; 2],
) {
    let [x, y] = origin;
    let w = size[0];
    let level = mana_item::terra_pick_level(m.nbt.mana);
    let last = tools::rank_is_last(level);
    draw_runs(game, batch, &rank_runs(level), [x, y]);
    if !last {
        let next = rank_runs(level + 1);
        let right = x + w - runs_width(game, &next);
        draw_runs(game, batch, &next, [right, y]);
    }
    let bar_y = y + RANK_ROW;
    let total = (w - 2.0).max(0.0) as i32;
    let filled = tools::rank_bar_width(total, m.tooltip_fraction(), last).max(0);
    let ticks = (START.elapsed().as_secs_f64() * 20.0) as f32;
    game.native_fill(batch, [x, bar_y, w, BAR_HEIGHT], 0xFF00_0000);
    for i in 0..filled {
        let hue = tools::rank_bar_hue(i, total, ticks);
        let rgb = botania_core::wand::hsv_to_rgb(hue, 1.0, 1.0);
        let at = [x + 1.0 + i as f32, bar_y + 1.0, 1.0, 3.0];
        game.native_fill(batch, at, 0xFF00_0000 | rgb);
    }
    let rest = total - filled;
    if rest > 0 {
        let at = [x + 1.0 + filled as f32, bar_y + 1.0, rest as f32, 3.0];
        game.native_fill(batch, at, 0xFF55_5555);
    }
}
