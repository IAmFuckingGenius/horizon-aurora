// Botania for Horizon Aurora: the game side of the port, added to `crates/horizon-client/src/game.rs`
// as `mod botania` by horizon-mixins. A modified port of Vazkii's Botania (Botania License, see
// LICENSE and NOTICE.md in this bundle). The mechanics live in the `botania-core` crate; this
// module reads and writes the game through Horizon's native helpers and keeps the little state
// Botania keeps outside its block entities (a pool's tick counter, the mana network's pools).
//
// Inner attributes and `//!` comments are not allowed in a horizon-mixins module payload.

use super::{Game, GameLevel};
use botania_core::apothecary::{Apothecary, Collide, Fluid, Thrown};
use botania_core::burst::{self, Aabb, Burst, BurstWorld};
use botania_core::consts::{PoolVariant, SpreaderVariant};
use botania_core::daisy::PureDaisy;
use botania_core::flower::{self, Collectors, Endoflame, Fuel, Generating};
use botania_core::pool::{self, Pool};
use botania_core::recipe::{
    BlockState as RecipeState, InfusionRecipe, Ingredient, PetalRecipe, PureDaisyRecipe,
    infusion_pick,
};
use horizon_client::containers::{BlockEntity, ModBlockEntity, ModContainer};
use horizon_client::items::ItemStack;
use horizon_world::nbt::{Compound, JavaString, Tag};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};

pub(super) const ID: &str = "botania";
pub(super) const NAME: &str = "Botania";
pub(super) const VERSION: &str = "0.1.0";
pub(super) const DESCRIPTION: &str = "A port of Vazkii's Botania 1.20.1 mechanics (work in progress): mana pools, \
     mana infusion, the Endoflame, the Petal Apothecary, the Pure Daisy and mana spreaders \
     shooting bursts. Needs the Botania content mod. Botania License; not made or endorsed by Vazkii.";

// Block entity ids, as Botania names its block entity types.
const POOL: &str = "botania:mana_pool";
const ENDOFLAME: &str = "botania:endoflame";
const ALTAR: &str = "botania:altar";
const DAISY: &str = "botania:pure_daisy";
const SPREADER: &str = "botania:mana_spreader";

// ---- state kept beside the block entities -----------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct PoolTrack {
    // Botania's `ticks`: counts from 0 when the pool loads, not saved.
    ticks: u32,
    // The mana clients were last sent.
    sent: i32,
    // The comparator level last told (`native_update_output_neighbors`).
    level: i32,
    // `soundTicks`: the craft sound plays at most every 6 ticks.
    sound: i32,
}

#[derive(Default)]
struct State {
    // The mana network's pools, by dimension and position (`ManaNetworkHandler`); checked
    // against the world before use, so a broken pool's entry is harmless.
    pools: HashMap<(&'static str, [i32; 3]), PoolTrack>,
    // Comparators to tell once the level's block entities are back in the world.
    outputs: Vec<(&'static str, [i32; 3])>,
    // Item entities an apothecary made or emptied (`ItemFlagsComponent.apothecarySpawned`).
    spawned: HashSet<u32>,
    // Item entities a pool made (`ItemFlagsComponent.manaInfusionSpawned`).
    infused: HashSet<u32>,
    infusion_recipes: Option<((usize, usize), Arc<Vec<InfusionRecipe>>)>,
    // What each apothecary remembers of its last craft (`lastRecipe`, `lastReagent`,
    // `recipeKeepTicks`), which Botania doesn't save either.
    apothecaries: HashMap<(&'static str, [i32; 3]), Memory>,
    // Parsed recipes, by the recipe table they came from (its address and length).
    petal_recipes: Option<((usize, usize), Arc<Vec<PetalRecipe>>)>,
    daisy_recipes: Option<((usize, usize, usize, usize), Arc<Vec<PureDaisyRecipe>>)>,
    spreaders: HashMap<(&'static str, [i32; 3]), SpreaderTrack>,
    bursts: Vec<Flying>,
    // Rotations of spreaders just placed, for their first tick.
    placed: HashMap<(&'static str, [i32; 3]), (f32, f32)>,
}

// What a spreader keeps outside its NBT: its receiver (the scan's result; Botania doesn't save
// it either), the aim it was scanned for, its tick count and last redstone signal.
#[derive(Clone, Copy, Debug)]
struct SpreaderTrack {
    receiver: Option<[i32; 3]>,
    rotation: (f32, f32),
    ticks: u32,
    powered: bool,
}

// A burst in flight: a virtual one, not an entity (design 3.6, variant A).
#[derive(Clone, Debug)]
struct Flying {
    dim: &'static str,
    burst: Burst,
    // Shot during this tick's block entity ticks: it first moves next tick.
    fresh: bool,
}

#[derive(Clone, Debug, Default)]
struct Memory {
    last_recipe: Option<Vec<String>>,
    last_reagent: Option<Ingredient>,
    keep_ticks: i32,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

// Never held across a call into the game: a game helper can come back into this module (a
// comparator reading a pool), and the lock is not reentrant.
fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- NBT ----------------------------------------------------------------------------------------

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

// Sets an int, touching the compound (and the block entity's revision) only when it changes.
fn set_int(entity: &mut ModBlockEntity, k: &str, v: i32) {
    if get_int(&entity.nbt, k) != Some(v) {
        entity.nbt.insert(key(k), Tag::Int(v));
    }
}

// `NbtUtils.readBlockPos`: `{X, Y, Z}`.
fn get_pos(nbt: &Compound, k: &str) -> Option<[i32; 3]> {
    let Tag::Compound(c) = nbt.get(&key(k))? else {
        return None;
    };
    Some([get_int(c, "X")?, get_int(c, "Y")?, get_int(c, "Z")?])
}

fn set_pos(entity: &mut ModBlockEntity, k: &str, pos: Option<[i32; 3]>) {
    if get_pos(&entity.nbt, k) == pos {
        return;
    }
    match pos {
        Some(p) => {
            let mut c = Compound::new();
            c.insert(key("X"), Tag::Int(p[0]));
            c.insert(key("Y"), Tag::Int(p[1]));
            c.insert(key("Z"), Tag::Int(p[2]));
            entity.nbt.insert(key(k), Tag::Compound(c));
        }
        None => {
            entity.nbt.remove(&key(k));
        }
    }
}

fn copy_int(from: &Compound, to: &mut Compound, k: &str) {
    if let Some(v) = get_int(from, k) {
        to.insert(key(k), Tag::Int(v));
    }
}

// ---- the mods list ----------------------------------------------------------------------------

pub(super) fn mods(mods: &mut Vec<super::native_hooks::NativeMod>) {
    mods.push(super::native_hooks::NativeMod {
        id: ID.into(),
        title: format!("{NAME} {VERSION}"),
        description: DESCRIPTION.into(),
        keys: Vec::new(),
    });
}

// ---- block entities -------------------------------------------------------------------------

fn entity_id(block: &str) -> Option<&'static str> {
    if PoolVariant::of_block(block).is_some() {
        Some(POOL)
    } else if block == "botania:endoflame" {
        Some(ENDOFLAME)
    } else if is_apothecary(block) {
        Some(ALTAR)
    } else if block == "botania:pure_daisy" {
        Some(DAISY)
    } else if SpreaderVariant::of_block(block).is_some() {
        Some(SPREADER)
    } else {
        None
    }
}

fn is_apothecary(block: &str) -> bool {
    block.starts_with("botania:apothecary_")
}

// `native_block_entity_container`: an apothecary's 16 slots (`SimpleInventoryBlockEntity`),
// saved and dropped by the game; no face reaches them and no menu opens.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_none() && is_apothecary(block) {
        *container = Some(ModContainer {
            slots: botania_core::consts::APOTHECARY_SLOTS,
            ..ModContainer::default()
        });
    }
}

// `native_block_entity`: Botania's blocks get their block entities.
pub(super) fn block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_some() {
        return;
    }
    let Some(id) = entity_id(block) else { return };
    let mut nbt = Compound::new();
    if let Some(variant) = PoolVariant::of_block(block) {
        nbt.insert(key("mana"), Tag::Int(0));
        nbt.insert(key("manaCap"), Tag::Int(variant.capacity()));
    }
    *entity = Some((id.to_owned(), nbt));
}

// `native_block_entity_ticks`: they tick (`getTicker`).
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if entity_id(block).is_some() {
        *ticks = true;
    }
}

// `native_block_entity_tick`: a server tick of one of them.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if entity.id == POOL {
        pool_tick(game, pos, entity);
    } else if entity.id == ENDOFLAME {
        endoflame_tick(game, pos, entity);
    } else if entity.id == ALTAR {
        apothecary_tick(game, pos, entity);
    } else if entity.id == DAISY {
        daisy_tick(game, pos, entity);
    } else if entity.id == SPREADER {
        spreader_tick(game, pos, entity);
    }
}

// `native_block_entity_update_tag`: what clients get (`writePacketNBT`).
pub(super) fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    let mut out = Compound::new();
    if entity.id == POOL {
        copy_int(&entity.nbt, &mut out, "mana");
        copy_int(&entity.nbt, &mut out, "manaCap");
    } else if entity.id == SPREADER {
        copy_int(&entity.nbt, &mut out, "mana");
        for k in ["rotationX", "rotationY"] {
            if let Some(v) = entity.nbt.get(&key(k)) {
                out.insert(key(k), v.clone());
            }
        }
    } else if entity.id == ENDOFLAME {
        copy_int(&entity.nbt, &mut out, "mana");
        copy_int(&entity.nbt, &mut out, "burnTime");
        if let Some(Tag::Compound(b)) = entity.nbt.get(&key("binding")) {
            out.insert(key("binding"), Tag::Compound(b.clone()));
        }
    } else {
        return;
    }
    *tag = Some(out);
}

// `native_level_tick_post`: tell the comparators of the pools whose level changed.
pub(super) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    move_bursts(game, dimension);
    let due: Vec<[i32; 3]> = {
        let mut s = state();
        let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut s.outputs)
            .into_iter()
            .partition(|(d, _)| *d == dimension);
        s.outputs = later;
        now.into_iter().map(|(_, p)| p).collect()
    };
    for pos in due {
        game.native_update_output_neighbors(dimension, pos);
    }
}

// ---- the mana pool ------------------------------------------------------------------------------

fn pool_of(nbt: &Compound, block: &str) -> Pool {
    let variant = PoolVariant::of_block(block).unwrap_or(PoolVariant::Default);
    let mut p = Pool::new(variant);
    p.mana = get_int(nbt, "mana").unwrap_or(0);
    // `initManaCapAndNetwork`: -1 (never set) becomes the variant's capacity.
    match get_int(nbt, "manaCap") {
        Some(cap) if cap > 0 => p.cap = cap,
        _ => {}
    }
    p.mana = p.mana.clamp(0, p.cap);
    p
}

fn is_pool(game: &Game<'_>, pos: [i32; 3]) -> bool {
    game.world.loaded(pos[0], pos[2])
        && game
            .native_mod_block_entity(pos)
            .is_some_and(|m| m.id == POOL)
}

fn mana_void_below(game: &Game<'_>, pos: [i32; 3]) -> bool {
    game.native_block([pos[0], pos[1] - 1, pos[2]]) == "botania:mana_void"
}

// `ManaPoolBlockEntity.serverTick` without mana items (phase 2): join the network, keep the
// capacity, send clients the mana at most every 10 ticks, and tell comparators.
fn pool_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let dim = game.dim.id();
    let block = game.native_block(pos);
    let mut p = pool_of(&entity.nbt, &block);
    let crafted = infuse(game, pos, &mut p);
    set_int(entity, "mana", p.mana);
    set_int(entity, "manaCap", p.cap);
    let level = p.comparator();
    let (sync, sound) = {
        let mut s = state();
        let track = s.pools.entry((dim, pos)).or_insert(PoolTrack {
            ticks: 0,
            sent: p.mana,
            level,
            sound: 0,
        });
        // `soundTicks--`, then `craftingEffect`: the sound when it has run down.
        if track.sound > 0 {
            track.sound -= 1;
        }
        let sound = crafted && track.sound == 0;
        if sound {
            track.sound = botania_core::consts::POOL_CRAFT_SOUND_TICKS;
        }
        let sync = track.sent != p.mana && pool::pool_sync_tick(track.ticks);
        if sync {
            track.sent = p.mana;
        }
        let tell = track.level != level;
        track.level = level;
        track.ticks = track.ticks.wrapping_add(1);
        if tell {
            s.outputs.push((dim, pos));
        }
        (sync, sound)
    };
    if sound {
        let at = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + 0.5,
            f64::from(pos[2]) + 0.5,
        ];
        game.native_play_sound_in_place("botania:mana_pool_craft", at, 4, 1.0, 1.0);
    }
    if sync {
        game.native_block_entity_sync(pos);
    }
}

fn infusion_recipes(game: &Game<'_>) -> Arc<Vec<InfusionRecipe>> {
    let table = game.native_recipes_of_type("botania:mana_infusion");
    let at = (table.as_ptr() as usize, table.len());
    if let Some((k, r)) = &state().infusion_recipes
        && *k == at
    {
        return r.clone();
    }
    let parsed: Arc<Vec<InfusionRecipe>> = Arc::new(
        table
            .iter()
            .filter_map(|r| InfusionRecipe::parse(&r.id, &core_json(&r.json)?))
            .collect(),
    );
    state().infusion_recipes = Some((at, parsed.clone()));
    parsed
}

// Forgets flagged item entities that are gone, so the sets stay small.
fn prune_flags(game: &Game<'_>) {
    let mut s = state();
    if s.spawned.len() + s.infused.len() < 4096 {
        return;
    }
    let alive: HashSet<u32> = game.dropped.iter().map(|e| e.id).collect();
    s.spawned.retain(|id| alive.contains(id));
    s.infused.retain(|id| alive.contains(id));
}

// `ManaPoolBlockEntity.collideEntityItem` for the item entities in the pool's cell (what
// `entityInside` hands it in 1.20.1): the recipe for the block under the pool, paid with mana;
// one item of each entity a tick; the output flagged so it isn't infused again. Whether any
// item was crafted (`craftingEffect`).
fn infuse(game: &mut Game<'_>, pos: [i32; 3], pool: &mut Pool) -> bool {
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
        return false;
    }
    let Some(below) = recipe_state(game, [pos[0], pos[1] - 1, pos[2]]) else {
        return false;
    };
    let recipes = infusion_recipes(game);
    let infused = state().infused.clone();
    let mut crafts = Vec::new();
    {
        let tags = game.items();
        let item_tag = |t: &str, i: &str| tags.item_in_tag(t, i);
        let block_tag = |t: &str, b: &str| tags.block_in_tag(t, b);
        for e in &items {
            if e.stack.count == 0 || infused.contains(&e.id) {
                continue;
            }
            if let Some(r) = infusion_pick(&recipes, &e.stack.item, &below, &item_tag, &block_tag)
                && pool.current() >= r.mana
            {
                pool.receive(-r.mana);
                crafts.push((e.id, e.stack.clone(), r.output.clone()));
            }
        }
    }
    let at = [
        f64::from(pos[0]) + 0.5,
        f64::from(pos[1]) + 1.5,
        f64::from(pos[2]) + 0.5,
    ];
    let crafted = !crafts.is_empty();
    for (id, stack, output) in crafts {
        let mut rest = stack;
        rest.count -= 1;
        game.native_set_item_entity(dim, id, rest);
        if let Some(out) = game.native_summon(
            dim,
            "minecraft:item",
            at,
            item_nbt(&output.item, output.count),
        ) {
            state().infused.insert(out);
        }
    }
    if crafted {
        prune_flags(game);
    }
    crafted
}

// The comparator reads a pool (`ManaPoolBlock.hasAnalogOutputSignal`).
pub(super) fn has_output(
    level: &GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    output: &mut bool,
) {
    if level
        .native_state_block(state)
        .is_some_and(|b| PoolVariant::of_block(b).is_some() || is_apothecary(b))
    {
        *output = true;
    }
}

// `ManaPoolBlock.getAnalogOutputSignal`.
pub(super) fn output(
    level: &GameLevel<'_>,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    output: &mut i32,
) {
    let Some(block) = level.native_state_block(state) else {
        return;
    };
    if is_apothecary(block) {
        // `PetalApothecaryBlock.getAnalogOutputSignal`: 15 with water.
        let fluid = level
            .registry
            .state(state)
            .and_then(|s| s.properties.get("fluid"))
            .and_then(|f| Fluid::of_name(f));
        let at = [pos.x, pos.y, pos.z];
        let fluid =
            fluid.or_else(
                || match level.world.block_entities.get(&(at[0], at[1], at[2])) {
                    Some(BlockEntity::Mod(m)) if m.id == ALTAR => nbt_fluid(&m.nbt),
                    _ => None,
                },
            );
        *output = if fluid == Some(Fluid::Water) { 15 } else { 0 };
        return;
    }
    if PoolVariant::of_block(block).is_none() {
        return;
    }
    let at = [pos.x, pos.y, pos.z];
    *output = match level.world.block_entities.get(&(at[0], at[1], at[2])) {
        Some(BlockEntity::Mod(m)) if m.id == POOL => pool_of(&m.nbt, block).comparator(),
        // Out of the world while it ticks: the level it last had.
        _ => state_level(at).unwrap_or(0),
    };
}

fn state_level(at: [i32; 3]) -> Option<i32> {
    state()
        .pools
        .iter()
        .find(|((_, p), _)| *p == at)
        .map(|(_, t)| t.level)
}

// ---- generating flowers -------------------------------------------------------------------------

// What a generating flower binds to. Botania binds generating flowers to mana collectors (the
// spreaders); until spreaders and bursts are ported, this port binds them to the nearest pool
// in the same range (6), so their mana has somewhere to go. Marked in STATUS.md.
struct PoolTargets<'a, 'g> {
    game: &'a mut Game<'g>,
    dim: &'static str,
}

impl Collectors for PoolTargets<'_, '_> {
    fn is_target(&self, pos: [i32; 3]) -> bool {
        is_pool(self.game, pos)
    }

    fn closest(&self, pos: [i32; 3], radius: i32) -> Option<[i32; 3]> {
        let mut known: Vec<[i32; 3]> = state()
            .pools
            .keys()
            .filter(|(d, _)| *d == self.dim)
            .map(|(_, p)| *p)
            .collect();
        known.sort();
        let live: Vec<[i32; 3]> = known.into_iter().filter(|p| self.is_target(*p)).collect();
        flower::closest(live, pos, radius)
    }

    fn status(&mut self, pos: [i32; 3]) -> Option<(i32, i32, bool)> {
        let block = self.game.native_block(pos);
        let void = mana_void_below(self.game, pos);
        let m = self.game.native_mod_block_entity(pos)?;
        if m.id != POOL {
            return None;
        }
        let p = pool_of(&m.nbt, &block);
        Some((p.current(), p.cap, p.is_full(void)))
    }

    fn give(&mut self, pos: [i32; 3], mana: i32) {
        let block = self.game.native_block(pos);
        self.game.native_with_mod_block_entity(pos, |m| {
            let mut p = pool_of(&m.nbt, &block);
            if p.receive(mana) {
                set_int(m, "mana", p.mana);
            }
        });
    }
}

fn generating_of(nbt: &Compound) -> Generating {
    Generating {
        ticks_existed: get_int(nbt, "ticksExisted").unwrap_or(0),
        mana: get_int(nbt, "mana").unwrap_or(0),
        binding: get_pos(nbt, "binding"),
    }
}

fn save_generating(entity: &mut ModBlockEntity, f: &Generating) {
    set_int(entity, "ticksExisted", f.ticks_existed);
    set_int(entity, "mana", f.mana);
    set_pos(entity, "binding", f.binding);
}

// An item's furnace burn time (`getSmeltingBurnTime`), 0 for a spreader
// (`EndoflameBlockEntity.getBurnTime`).
fn burn_time(game: &Game<'_>, stack: &ItemStack) -> i32 {
    if SpreaderVariant::of_block(&stack.item).is_some() {
        return 0;
    }
    game.res.providers().burn_time(game.items(), stack, false)
}

// `hasCraftingRemainingItem`: a lava bucket leaves its bucket.
fn has_remainder(game: &Game<'_>, item: &str) -> bool {
    game.items()
        .get(item)
        .is_some_and(|i| i.crafting_remainder.is_some())
}

// The item entity's age (`ItemFlagsComponent.timeCounter` counts the same ticks).
fn item_age(game: &Game<'_>, id: u32) -> i32 {
    game.dropped
        .iter()
        .find(|e| e.id == id)
        .map_or(0, |e| e.age)
}

// `EndoflameBlockEntity.tickFlower` on the server.
fn endoflame_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let dim = game.dim.id();
    let mut e = Endoflame {
        flower: generating_of(&entity.nbt),
        burn_time: get_int(&entity.nbt, "burnTime").unwrap_or(0),
    };
    let soil = flower::soil_delay(&game.native_block([pos[0], pos[1] - 1, pos[2]]));
    // The 7×7×7 box around it (`AABB(pos - 3, pos + 4)`), only on ticks it may light something.
    let r = botania_core::consts::ENDOFLAME_RANGE;
    let looks = e.burn_time <= 1;
    let items = if looks {
        game.native_item_entities(
            dim,
            [
                f64::from(pos[0] - r),
                f64::from(pos[1] - r),
                f64::from(pos[2] - r),
            ],
            [
                f64::from(pos[0] + r + 1),
                f64::from(pos[1] + r + 1),
                f64::from(pos[2] + r + 1),
            ],
        )
    } else {
        Vec::new()
    };
    let fuels: Vec<Fuel> = items
        .iter()
        .map(|i| Fuel {
            age: item_age(game, i.id),
            burn: burn_time(game, &i.stack),
            remainder: has_remainder(game, &i.stack.item),
            count: i.stack.count,
        })
        .collect();
    let was = (e.flower.binding, e.burn_time > 0);
    let result = {
        let mut targets = PoolTargets { game, dim };
        e.tick(pos, &mut targets, &fuels, soil)
    };
    save_generating(entity, &e.flower);
    set_int(entity, "burnTime", e.burn_time);
    if let Some(i) = result.lit {
        let item = &items[i];
        let mut stack = item.stack.clone();
        stack.count -= 1;
        game.native_set_item_entity(dim, item.id, stack);
        let at = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + 0.5,
            f64::from(pos[2]) + 0.5,
        ];
        game.native_play_sound_in_place("botania:endoflame", at, 4, 1.0, 1.0);
    }
    if result.lit.is_some() || result.rebound || was != (e.flower.binding, e.burn_time > 0) {
        game.native_block_entity_sync(pos);
    }
}

// ---- the petal apothecary -------------------------------------------------------------------

fn get_str(nbt: &Compound, k: &str) -> Option<String> {
    match nbt.get(&key(k))? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}

// The fluid kept in the block entity, for an apothecary whose block has no `fluid` property
// (a content mod that declares it on a plain template).
fn nbt_fluid(nbt: &Compound) -> Option<Fluid> {
    get_str(nbt, "fluid").and_then(|f| Fluid::of_name(&f))
}

fn is_air(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

// The fluid an item gives up into an empty apothecary (`extractFluidFromItemEntity`), and
// whether it is a fluid container at all (`isFluidContainer`). Fish buckets count as water;
// their fish isn't released yet.
fn bucket_fluid(item: &str) -> (Option<Fluid>, bool) {
    match item {
        "minecraft:water_bucket" => (Some(Fluid::Water), true),
        "minecraft:lava_bucket" => (Some(Fluid::Lava), true),
        _ if item.ends_with("_bucket") && item.starts_with("minecraft:") => {
            let fish = matches!(
                item,
                "minecraft:cod_bucket"
                    | "minecraft:salmon_bucket"
                    | "minecraft:pufferfish_bucket"
                    | "minecraft:tropical_fish_bucket"
                    | "minecraft:axolotl_bucket"
                    | "minecraft:tadpole_bucket"
            );
            (fish.then_some(Fluid::Water), true)
        }
        "minecraft:bucket" => (None, true),
        _ => (None, false),
    }
}

// A recipe file as the content mod's data has it, read by `botania-core`.
fn core_json(v: &serde_json::Value) -> Option<botania_core::json::Value> {
    botania_core::json::parse(&serde_json::to_string(v).ok()?).ok()
}

fn petal_recipes(game: &Game<'_>) -> Arc<Vec<PetalRecipe>> {
    let table = game.native_recipes_of_type("botania:petal_apothecary");
    let at = (table.as_ptr() as usize, table.len());
    if let Some((k, r)) = &state().petal_recipes
        && *k == at
    {
        return r.clone();
    }
    let parsed: Arc<Vec<PetalRecipe>> = Arc::new(
        table
            .iter()
            .filter_map(|r| PetalRecipe::parse(&r.id, &core_json(&r.json)?))
            .collect(),
    );
    state().petal_recipes = Some((at, parsed.clone()));
    parsed
}

fn set_fluid(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, fluid: Fluid) {
    if get_str(&entity.nbt, "fluid").as_deref() != Some(fluid.name()) {
        entity
            .nbt
            .insert(key("fluid"), Tag::String(JavaString::from(fluid.name())));
    }
    game.native_set_block_property(pos, "fluid", fluid.name());
}

// An item stack entity's compound as `/summon minecraft:item ~ ~ ~ {Item: ...}` takes it.
fn item_nbt(item: &str, count: i32) -> Compound {
    let mut stack = Compound::new();
    stack.insert(key("id"), Tag::String(JavaString::from(item)));
    stack.insert(key("count"), Tag::Int(count));
    let mut nbt = Compound::new();
    nbt.insert(key("Item"), Tag::Compound(stack));
    nbt
}

// `PetalApothecaryBlockEntity.serverTick`, with `entityInside`: every item entity in the block's
// cell or the one above it goes through `collideEntityItem`, then the remembered recipe ages.
fn apothecary_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let dim = game.dim.id();
    let state_id = game.world.block(pos[0], pos[1], pos[2]);
    let has_property = game
        .native_block_state_property(state_id, "fluid")
        .is_some();
    let fluid = game
        .native_block_state_property(state_id, "fluid")
        .and_then(Fluid::of_name)
        .or_else(|| nbt_fluid(&entity.nbt))
        .unwrap_or_default();
    // The slots, read up to the first empty one as Botania's matching reads them.
    let slots: Vec<ItemStack> = entity
        .items
        .iter()
        .map_while(|s| s.clone().filter(|s| s.count > 0))
        .collect();
    let memory = state()
        .apothecaries
        .get(&(dim, pos))
        .cloned()
        .unwrap_or_default();
    let mut a = Apothecary {
        fluid,
        slots: slots.iter().map(|s| s.item.to_string()).collect(),
        last_recipe: memory.last_recipe,
        last_reagent: memory.last_reagent,
        keep_ticks: memory.keep_ticks,
    };
    let mut stacks = slots;
    let items = game.native_item_entities(
        dim,
        [f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])],
        [
            f64::from(pos[0] + 1),
            f64::from(pos[1] + 2),
            f64::from(pos[2] + 1),
        ],
    );
    let mut actions = Vec::new();
    if !items.is_empty() {
        let recipes = petal_recipes(game);
        let spawned = state().spawned.clone();
        let tags = game.items();
        let in_tag = |t: &str, i: &str| tags.item_in_tag(t, i);
        for (n, e) in items.iter().enumerate() {
            let (holds, container) = bucket_fluid(&e.stack.item);
            let thrown = Thrown {
                item: &e.stack.item,
                count: e.stack.count,
                spawned: spawned.contains(&e.id),
                holds,
                container,
            };
            let c = a.collide(&thrown, &recipes, &in_tag);
            if c != Collide::Nothing {
                actions.push((n, c));
            }
        }
    }
    let center = [
        f64::from(pos[0]) + 0.5,
        f64::from(pos[1]) + 0.5,
        f64::from(pos[2]) + 0.5,
    ];
    let mut changed = false;
    for (n, c) in actions {
        let e = &items[n];
        match c {
            Collide::Nothing | Collide::Ignite => {}
            Collide::Fill(f) => {
                game.native_set_item_entity(dim, e.id, ItemStack::new("minecraft:bucket", 1));
                state().spawned.insert(e.id);
                set_fluid(game, pos, entity, f);
                changed = true;
            }
            Collide::Craft { output, .. } => {
                let mut rest = e.stack.clone();
                rest.count -= 1;
                game.native_set_item_entity(dim, e.id, rest);
                stacks.clear();
                let at = [center[0], f64::from(pos[1]) + 1.5, center[2]];
                if let Some(id) = game.native_summon(
                    dim,
                    "minecraft:item",
                    at,
                    item_nbt(&output.item, output.count),
                ) {
                    state().spawned.insert(id);
                }
                set_fluid(game, pos, entity, Fluid::Empty);
                game.native_play_sound_in_place("botania:altar_craft", center, 4, 1.0, 1.0);
                changed = true;
            }
            Collide::Take => {
                let mut rest = e.stack.clone();
                rest.count -= 1;
                let mut one = e.stack.clone();
                one.count = 1;
                game.native_set_item_entity(dim, e.id, rest);
                stacks.push(one);
                // GENERIC_SPLASH at 0.1, pitch 10 (clamped to 2 as the client clamps it).
                game.native_play_sound_in_place(
                    "minecraft:entity.generic.splash",
                    center,
                    4,
                    0.1,
                    2.0,
                );
                changed = true;
            }
        }
    }
    a.tick_keep();
    if !has_property {
        let f = a.fluid;
        if get_str(&entity.nbt, "fluid").as_deref() != Some(f.name()) {
            entity
                .nbt
                .insert(key("fluid"), Tag::String(JavaString::from(f.name())));
        }
    }
    if changed {
        let size = entity.items.len();
        let mut slots: Vec<Option<ItemStack>> = stacks.into_iter().map(Some).collect();
        slots.resize(size, None);
        slots.truncate(size);
        if entity.items != slots {
            entity.items = slots;
        }
        game.native_block_entity_sync(pos);
    }
    state().apothecaries.insert(
        (dim, pos),
        Memory {
            last_recipe: a.last_recipe,
            last_reagent: a.last_reagent,
            keep_ticks: a.keep_ticks,
        },
    );
}

// ---- the Pure Daisy -----------------------------------------------------------------------------

fn daisy_recipes(game: &Game<'_>) -> Arc<Vec<PureDaisyRecipe>> {
    let plain = game.native_recipes_of_type("botania:pure_daisy");
    let copying = game.native_recipes_of_type("botania:state_copying_pure_daisy");
    let at = (
        plain.as_ptr() as usize,
        plain.len(),
        copying.as_ptr() as usize,
        copying.len(),
    );
    if let Some((k, r)) = &state().daisy_recipes
        && *k == at
    {
        return r.clone();
    }
    let parsed: Arc<Vec<PureDaisyRecipe>> = Arc::new(
        plain
            .iter()
            .chain(copying)
            .filter_map(|r| PureDaisyRecipe::parse(&r.id, &core_json(&r.json)?))
            .collect(),
    );
    state().daisy_recipes = Some((at, parsed.clone()));
    parsed
}

// The block at `pos` as recipes see it.
fn recipe_state(game: &Game<'_>, pos: [i32; 3]) -> Option<RecipeState> {
    let id = game.world.block(pos[0], pos[1], pos[2]);
    let s = game.pack.assets.registry.state(id)?;
    let mut properties: Vec<(String, String)> = s
        .properties
        .keys()
        .zip(s.properties.values())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    properties.sort();
    Some(RecipeState {
        name: s.name.clone(),
        properties,
    })
}

// `PureDaisyBlockEntity.tickFlower` on the server.
fn daisy_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let dim = game.dim.id();
    let mut daisy = PureDaisy {
        position: get_int(&entity.nbt, "position").map_or(0, |p| p.clamp(0, 7) as usize),
        ticks_remaining: std::array::from_fn(|i| {
            get_int(&entity.nbt, &format!("ticksRemaining{i}")).unwrap_or(-1)
        }),
    };
    let ticks_existed = get_int(&entity.nbt, "ticksExisted").unwrap_or(0) + 1;
    let recipes = daisy_recipes(game);
    let mut found: Option<(usize, RecipeState)> = None;
    let step = {
        let g: &Game<'_> = game;
        let tags = g.items();
        let in_tag = |t: &str, b: &str| tags.block_in_tag(t, b);
        daisy.tick(pos, |p| {
            let s = recipe_state(g, p)?;
            if is_air(&s.name) {
                return None;
            }
            let i = recipes.iter().position(|r| r.matches(&s, &in_tag))?;
            let time = recipes[i].time;
            found = Some((i, s));
            Some(time)
        })
    };
    if let Some((_, at)) = step.convert
        && let Some((i, s)) = found
    {
        let result = recipes[i].result(&s);
        let registry = game.pack.assets.registry;
        if let Some(mut id) = game.native_default_state(&result.name) {
            for (k, v) in &result.properties {
                if let Some(with) = super::native_world_edit::with_property(registry, id, k, v) {
                    id = with;
                }
            }
            game.native_set_block(dim, at, id, horizon_world::level::flags::UPDATE_ALL);
        }
    }
    set_int(entity, "ticksExisted", ticks_existed);
    set_int(entity, "position", daisy.position as i32);
    for (i, t) in daisy.ticks_remaining.iter().enumerate() {
        set_int(entity, &format!("ticksRemaining{i}"), *t);
    }
}

// ---- the mana spreader and its bursts -------------------------------------------------------

fn get_float(nbt: &Compound, k: &str) -> Option<f32> {
    match nbt.get(&key(k))? {
        Tag::Float(bits) => Some(f32::from_bits(*bits)),
        Tag::Double(bits) => Some(f64::from_bits(*bits) as f32),
        Tag::Int(v) => Some(*v as f32),
        _ => None,
    }
}

fn set_float(entity: &mut ModBlockEntity, k: &str, v: f32) {
    if get_float(&entity.nbt, k) != Some(v) {
        entity.nbt.insert(key(k), Tag::Float(v.to_bits()));
    }
}

fn is_spreader(game: &Game<'_>, pos: [i32; 3]) -> bool {
    game.native_mod_block_entity(pos)
        .is_some_and(|m| m.id == SPREADER)
}

// What a burst meets in the level: a pool is the slab a burst hits (`*_SHAPE_INTERACT`), any
// other block its collision shape; leaves are passed; pools take the mana.
struct GameBursts<'a, 'g> {
    game: &'a Game<'g>,
}

impl BurstWorld for GameBursts<'_, '_> {
    fn boxes(&self, pos: [i32; 3]) -> Vec<Aabb> {
        let block = self.game.native_block(pos);
        match PoolVariant::of_block(&block) {
            Some(v) => vec![[0.0, 0.0, 0.0, 1.0, v.burst_slab_height() / 16.0, 1.0]],
            None => self.game.native_block_collision(pos),
        }
    }
    fn passes(&self, pos: [i32; 3]) -> bool {
        let block = self.game.native_block(pos);
        self.game.items().block_in_tag("minecraft:leaves", &block)
    }
    fn receiver(&self, pos: [i32; 3]) -> bool {
        is_pool(self.game, pos)
    }
}

// `ManaSpreaderBlockEntity.commonTick` on the server, for the plain, redstone, elven and Gaia
// spreaders without lenses: draw from the pools beside it, find its receiver, and shoot.
fn spreader_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let dim = game.dim.id();
    let block = game.native_block(pos);
    let Some(variant) = SpreaderVariant::of_block(&block) else {
        return;
    };
    let stats = variant.stats();
    let redstone = variant == SpreaderVariant::Redstone;
    let mut mana = get_int(&entity.nbt, "mana")
        .unwrap_or(0)
        .clamp(0, stats.capacity);
    // A rotation the placement hook left for it, else its own.
    let placed = state().placed.remove(&(dim, pos));
    let (rx, ry) = placed.unwrap_or((
        get_float(&entity.nbt, "rotationX").unwrap_or(0.0),
        get_float(&entity.nbt, "rotationY").unwrap_or(0.0),
    ));
    let mut can_shoot = get_int(&entity.nbt, "canShootBurst").unwrap_or(1) != 0;
    let track = state().spreaders.get(&(dim, pos)).copied();
    let in_network = track.is_some();
    let receiver = track.and_then(|t| t.receiver);
    // Draw from the pools beside it, all but its own receiver (any for a redstone spreader).
    if in_network {
        for d in botania_core::mth::Direction::ALL {
            let at = d.offset(pos);
            if (Some(at) == receiver && !redstone) || !is_pool(game, at) {
                continue;
            }
            let pool_block = game.native_block(at);
            let have = game
                .native_mod_block_entity(at)
                .map_or(0, |m| pool_of(&m.nbt, &pool_block).current());
            let draw = pool::spreader_draw(have, mana, stats.capacity);
            if draw > 0 {
                game.native_with_mod_block_entity(at, |m| {
                    let mut p = pool_of(&m.nbt, &pool_block);
                    if p.receive(-draw) {
                        set_int(m, "mana", p.mana);
                    }
                });
                mana += draw;
            }
        }
    }
    let powered = game.native_neighbor_signal(dim, pos) > 0;
    // The receiver scan: when the aim changed and every 10 ticks (Botania rescans whenever a
    // block on the path changes).
    let ticks = track.map_or(0, |t| t.ticks);
    let rescan = track.is_none_or(|t| t.rotation != (rx, ry)) || ticks % 10 == 0;
    let receiver = if rescan {
        burst::scan_receiver(&GameBursts { game }, pos, rx, ry, &stats).0
    } else {
        receiver
    };
    // `getEntitiesOfClass` for its own burst, as the ping watchdog does: none left, it may shoot.
    let flying = state()
        .bursts
        .iter()
        .any(|b| b.dim == dim && b.burst.source == pos && b.burst.alive);
    if !flying {
        can_shoot = true;
    }
    let powered_last = track.is_some_and(|t| t.powered);
    let should = if redstone {
        powered && !powered_last
    } else {
        !powered
    };
    let receiver_full = receiver.is_some_and(|r| {
        let b = game.native_block(r);
        let void = mana_void_below(game, r);
        game.native_mod_block_entity(r)
            .is_some_and(|m| pool_of(&m.nbt, &b).is_full(void))
    });
    if should
        && (receiver.is_some() || redstone)
        && can_shoot
        && (redstone || !receiver_full)
        && mana >= stats.burst_mana
    {
        mana -= stats.burst_mana;
        can_shoot = false;
        state().bursts.push(Flying {
            dim,
            burst: Burst::from_spreader(pos, rx, ry, &stats, false),
            fresh: true,
        });
        let at = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + 0.5,
            f64::from(pos[2]) + 0.5,
        ];
        let pitch = 0.7 + 0.3 * ((ticks % 7) as f32 / 7.0);
        game.native_play_sound_in_place("botania:spreaderfire", at, 4, 0.05, pitch);
    }
    set_int(entity, "mana", mana);
    set_float(entity, "rotationX", rx);
    set_float(entity, "rotationY", ry);
    set_int(entity, "canShootBurst", i32::from(can_shoot));
    let changed_receiver = track.is_none_or(|t| t.receiver != receiver || t.rotation != (rx, ry));
    state().spreaders.insert(
        (dim, pos),
        SpreaderTrack {
            receiver,
            rotation: (rx, ry),
            ticks: ticks.wrapping_add(1),
            powered,
        },
    );
    if changed_receiver {
        game.native_block_entity_sync(pos);
    }
}

// The bursts of `dimension` shot before this tick move: `ManaBurstEntity.tick` (an entity first
// moves in the tick after it is added).
fn move_bursts(game: &mut Game<'_>, dimension: &str) {
    let mut mine: Vec<Flying> = {
        let mut s = state();
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.bursts)
            .into_iter()
            .partition(|b| b.dim == dimension);
        s.bursts = rest;
        mine
    };
    let mut deliveries = Vec::new();
    let mut ended = Vec::new();
    {
        let world = GameBursts { game: &*game };
        for b in mine.iter_mut() {
            if b.fresh {
                b.fresh = false;
                continue;
            }
            // A burst about to fly into an unloaded chunk is lost.
            let next = botania_core::mth::block_of([
                b.burst.pos[0] + b.burst.vel[0],
                b.burst.pos[1] + b.burst.vel[1],
                b.burst.pos[2] + b.burst.vel[2],
            ]);
            if !game.world.loaded(next[0], next[2]) {
                b.burst.alive = false;
            } else {
                let r = b.burst.tick(&world);
                if let Some(d) = r.delivered {
                    deliveries.push(d);
                }
            }
            if !b.burst.alive {
                ended.push(b.burst.source);
            }
        }
    }
    for (at, mana) in deliveries {
        let block = game.native_block(at);
        game.native_with_mod_block_entity(at, |m| {
            let mut p = pool_of(&m.nbt, &block);
            if p.receive(mana) {
                set_int(m, "mana", p.mana);
            }
        });
    }
    // `remove`: the spreader that shot it may shoot again.
    for source in ended {
        game.native_with_mod_block_entity(source, |m| {
            if m.id == SPREADER {
                set_int(m, "canShootBurst", 1);
            }
        });
    }
    mine.retain(|b| b.burst.alive);
    state().bursts.extend(mine);
}

// `ManaSpreaderBlock.setPlacedBy`: a placed spreader faces the player who placed it.
pub(super) fn placed(
    game: &mut Game<'_>,
    pos: [i32; 3],
    state_id: horizon_world::registry::BlockStateId,
) {
    let Some(block) = game.native_state_block(state_id).map(str::to_owned) else {
        return;
    };
    if SpreaderVariant::of_block(&block).is_none() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let rotation = burst::placed_rotation(player.yaw, player.pitch);
    let dim = game.dim.id();
    state().placed.insert((dim, pos), rotation);
}

// ---- data -----------------------------------------------------------------------------------

// Botania's crafting recipes the game's table needs for phase 1, in Botania's 1.20 form (the
// content mod carries them so, which 26.3 can't read), and the wrapper recipes it doesn't know.
// They are given in 26.3's form under `botania:horizon/<name>`: the content mod's own file at the
// same path would win over a mod's.
const RECIPES: &[(&str, &[u8])] = &[
    (
        "apothecary_default",
        horizon_client::native_data!("botania", "recipe/apothecary_default.json"),
    ),
    (
        "mana_pool",
        horizon_client::native_data!("botania", "recipe/mana_pool.json"),
    ),
    (
        "diluted_pool",
        horizon_client::native_data!("botania", "recipe/diluted_pool.json"),
    ),
    (
        "livingrock_slab",
        horizon_client::native_data!("botania", "recipe/livingrock_slab.json"),
    ),
    (
        "mana_spreader",
        horizon_client::native_data!("botania", "recipe/mana_spreader.json"),
    ),
    (
        "twig_wand",
        horizon_client::native_data!("botania", "recipe/twig_wand.json"),
    ),
    (
        "livingwood_twig",
        horizon_client::native_data!("botania", "recipe/livingwood_twig.json"),
    ),
    (
        "livingwood_planks",
        horizon_client::native_data!("botania", "recipe/livingwood_planks.json"),
    ),
    (
        "livingwood",
        horizon_client::native_data!("botania", "recipe/livingwood.json"),
    ),
    (
        "petal_black",
        horizon_client::native_data!("botania", "recipe/petal_black.json"),
    ),
    (
        "petal_blue",
        horizon_client::native_data!("botania", "recipe/petal_blue.json"),
    ),
    (
        "petal_brown",
        horizon_client::native_data!("botania", "recipe/petal_brown.json"),
    ),
    (
        "petal_cyan",
        horizon_client::native_data!("botania", "recipe/petal_cyan.json"),
    ),
    (
        "petal_gray",
        horizon_client::native_data!("botania", "recipe/petal_gray.json"),
    ),
    (
        "petal_green",
        horizon_client::native_data!("botania", "recipe/petal_green.json"),
    ),
    (
        "petal_light_blue",
        horizon_client::native_data!("botania", "recipe/petal_light_blue.json"),
    ),
    (
        "petal_light_gray",
        horizon_client::native_data!("botania", "recipe/petal_light_gray.json"),
    ),
    (
        "petal_lime",
        horizon_client::native_data!("botania", "recipe/petal_lime.json"),
    ),
    (
        "petal_magenta",
        horizon_client::native_data!("botania", "recipe/petal_magenta.json"),
    ),
    (
        "petal_orange",
        horizon_client::native_data!("botania", "recipe/petal_orange.json"),
    ),
    (
        "petal_pink",
        horizon_client::native_data!("botania", "recipe/petal_pink.json"),
    ),
    (
        "petal_purple",
        horizon_client::native_data!("botania", "recipe/petal_purple.json"),
    ),
    (
        "petal_red",
        horizon_client::native_data!("botania", "recipe/petal_red.json"),
    ),
    (
        "petal_white",
        horizon_client::native_data!("botania", "recipe/petal_white.json"),
    ),
    (
        "petal_yellow",
        horizon_client::native_data!("botania", "recipe/petal_yellow.json"),
    ),
];

// `native_data_pack_files`.
pub(super) fn data(files: &mut Vec<(String, Vec<u8>)>) {
    for (name, bytes) in RECIPES {
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };
        let Ok(v) = botania_core::json::parse(text) else {
            continue;
        };
        if let Some(recipe) = botania_core::recipe::unwrap(&v) {
            files.push((
                format!("recipe/botania:horizon/{name}"),
                recipe.to_json().into_bytes(),
            ));
        }
    }
}
