// Botania for Horizon Aurora: the game side of the port, added to `crates/horizon-client/src/game.rs`
// as `mod botania` by horizon-mixins. A modified port of Vazkii's Botania (Botania License, see
// LICENSE and NOTICE.md in this bundle). The mechanics live in the `botania-core` crate; this
// module reads and writes the game through Horizon's native helpers and keeps the little state
// Botania keeps outside its block entities (a pool's tick counter, the mana network's pools).
//
// Inner attributes and `//!` comments are not allowed in a horizon-mixins module payload.

use super::{Game, GameLevel};
use botania_core::consts::{PoolVariant, SpreaderVariant};
use botania_core::flower::{self, Collectors, Endoflame, Fuel, Generating};
use botania_core::pool::{self, Pool};
use horizon_client::containers::{BlockEntity, ModBlockEntity};
use horizon_client::items::ItemStack;
use horizon_world::nbt::{Compound, JavaString, Tag};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

pub(super) const ID: &str = "botania";
pub(super) const NAME: &str = "Botania";
pub(super) const VERSION: &str = "0.1.0";
pub(super) const DESCRIPTION: &str = "A port of Vazkii's Botania 1.20.1 mechanics (work in progress): mana pools \
     and the Endoflame. Needs the Botania content mod. Botania License; not made or endorsed by Vazkii.";

// Block entity ids, as Botania names its block entity types.
const POOL: &str = "botania:mana_pool";
const ENDOFLAME: &str = "botania:endoflame";

// ---- state kept beside the block entities -----------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct PoolTrack {
    // Botania's `ticks`: counts from 0 when the pool loads, not saved.
    ticks: u32,
    // The mana clients were last sent.
    sent: i32,
    // The comparator level last told (`native_update_output_neighbors`).
    level: i32,
}

#[derive(Default)]
struct State {
    // The mana network's pools, by dimension and position (`ManaNetworkHandler`); checked
    // against the world before use, so a broken pool's entry is harmless.
    pools: HashMap<(&'static str, [i32; 3]), PoolTrack>,
    // Comparators to tell once the level's block entities are back in the world.
    outputs: Vec<(&'static str, [i32; 3])>,
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
    } else {
        None
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
    }
}

// `native_block_entity_update_tag`: what clients get (`writePacketNBT`).
pub(super) fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    let mut out = Compound::new();
    if entity.id == POOL {
        copy_int(&entity.nbt, &mut out, "mana");
        copy_int(&entity.nbt, &mut out, "manaCap");
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
    let p = pool_of(&entity.nbt, &block);
    set_int(entity, "mana", p.mana);
    set_int(entity, "manaCap", p.cap);
    let level = p.comparator();
    let sync = {
        let mut s = state();
        let track = s.pools.entry((dim, pos)).or_insert(PoolTrack {
            ticks: 0,
            sent: p.mana,
            level,
        });
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
        sync
    };
    if sync {
        game.native_block_entity_sync(pos);
    }
}

// The comparator reads a pool (`ManaPoolBlock.hasAnalogOutputSignal`).
pub(super) fn has_output(
    level: &GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    output: &mut bool,
) {
    if level
        .native_state_block(state)
        .is_some_and(|b| PoolVariant::of_block(b).is_some())
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
