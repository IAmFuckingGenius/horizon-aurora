// Botania's tools placing from the inventory, a player's head under the elementium axe and the
// advancements Botania's code grants, for Horizon Aurora: a modified port of Vazkii's Botania
// (Botania License, see LICENSE and NOTICE.md). The rules are in `botania_core::tool_uses`; this
// module wires them to the hooks (`botania_tool_uses` in game.rs).
//
// Inner attributes and `//!` comments are not allowed in a horizon-mixins module payload.

use super::{Game, Hit};
use botania_core::tool_uses::{self, Placer};
use botania_core::{tools, weapons};
use horizon_client::first_person::Hand;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeLootDrops;
use horizon_client::particles::{ParticleOptions, ParticleType};
use horizon_world::nbt::{JavaString, Tag};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

// `SoundSource.PLAYERS`.
const PLAYERS: usize = 7;
// What a dying player's own loot table is called (`EntityType.PLAYER.getDefaultLootTable`).
const PLAYER_TABLE: &str = "minecraft:entities/player";

#[derive(Default)]
struct State {
    // Players killed by a player this tick, whose killer's main hand is read after the level's
    // tick (inside the dying player's turn the killer can't be reached).
    deaths: Vec<Death>,
    rng: u64,
}

struct Death {
    dim: &'static str,
    victim: u128,
    killer: u128,
    at: [f64; 3],
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

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

fn is_block_item(game: &Game<'_>, stack: &ItemStack) -> bool {
    game.items()
        .get(&stack.item)
        .is_some_and(|i| i.block.is_some())
}

// The Terra Shatterer's rank (`TerraShattererItem.getLevel`) from its mana.
fn shatterer_level(stack: &ItemStack) -> i32 {
    let mana = match stack
        .custom_data()
        .and_then(|c| c.get(&JavaString::from("mana")))
    {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    };
    weapons::pick_level(mana)
}

// ---- placing from the inventory -----------------------------------------------------------------

// `ManasteelPickaxeItem.useOn` (with the Terra Shatterer's guard before it) and
// `ManasteelAxeItem.useOn`: the first stack in the inventory named a torch (a sapling for the
// axes) is used on the clicked face as if it were in the tool's hand (`PlayerHelper.substituteUse`:
// placed, used up, its sound and statistic). A pickaxe that placed one waits 5 ticks. An axe that
// placed nothing goes on to strip as an axe does.
pub(super) fn use_on(
    game: &mut Game<'_>,
    stack: &ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    if *claimed || !game.native_authoritative() || game.native_acting_player().is_none() {
        return;
    }
    let Some(placer) = Placer::of(&stack.item) else {
        return;
    };
    let main_hand = game.hand == Hand::Main;
    let off_hand = game.offhand.clone().filter(|s| s.count > 0);
    let places = if &*stack.item == weapons::TERRA_PICK {
        let level = shatterer_level(stack);
        tool_uses::shatterer_places(main_hand, off_hand.is_some(), game.native_sneaking(), level)
            && tool_uses::tool_places(main_hand, false)
    } else {
        let block = off_hand.as_ref().is_some_and(|s| is_block_item(game, s));
        tool_uses::tool_places(main_hand, block)
    };
    if !places {
        return;
    }
    // The click as the player made it: the point on the face their look met.
    let hit = game
        .pick()
        .filter(|h| h.pos == pos && h.face == face)
        .unwrap_or_else(|| Hit {
            pos,
            face,
            point: std::array::from_fn(|i| f64::from(pos[i]) + 0.5 + f64::from(face[i]) * 0.5),
        });
    for slot in 0..game.inventory.len().min(36) {
        let found = game.inventory[slot].as_ref().is_some_and(|s| {
            s.count > 0
                && placer.places(&tool_uses::description_id(&s.item, is_block_item(game, s)))
        });
        if !found {
            continue;
        }
        // The found stack in the tool's hand for the use, then back in its slot.
        let substitute = game.inventory[slot].take();
        let tool = std::mem::replace(game.held_mut(), substitute);
        let used = matches!(game.use_hand(Some(&hit)), Ok(true)) && !game.use_failed;
        let rest = std::mem::replace(game.held_mut(), tool);
        if game.inventory[slot].is_none() {
            game.inventory[slot] = rest.filter(|s| s.count > 0);
        }
        game.use_failed = false;
        if used {
            if placer == Placer::Torch {
                let group = horizon_client::cooldowns::group(stack, None);
                game.add_cooldown(group, tool_uses::TORCH_COOLDOWN);
            }
            *claimed = true;
            return;
        }
    }
}

// ---- advancements -------------------------------------------------------------------------------

// `PlayerHelper.grantCriterion(player, advancement, "code_triggered")` for the acting player: the
// criterion of an advancement Botania grants from its code, unless the advancement is done.
fn grant_code_triggered(game: &mut Game<'_>, advancement: &str) {
    if !game.native_authoritative() || game.native_acting_player().is_none() {
        return;
    }
    let tree = game.advancement_tree();
    let Some(index) = tree.find_id(advancement) else {
        return;
    };
    if game.adv.progress.done(&tree.list[index]) {
        return;
    }
    game.grant_advancement(index, Some(tool_uses::CODE_TRIGGERED));
}

// `TerraShattererItem.breakOtherBlock`: an area broken at rank SS grants `challenge/rank_ss_pick`.
// Called by `botania_weapons::pick_break_other` once its area is broken, with the Shatterer's own
// rank (before the rings change it).
pub(super) fn rank_ss_pick(game: &mut Game<'_>, level: i32) {
    if level == tool_uses::RANK_SS {
        grant_code_triggered(game, tool_uses::RANK_SS_PICK);
    }
}

// `LexicaBotaniaItem.use`: `UseItemSuccessTrigger` with the book (`main/lexicon_use`) and the
// opening sound; the book itself (Patchouli) has no screen here.
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if *claimed || &*stack.item != tool_uses::LEXICON || !game.native_authoritative() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let book = stack.clone();
    game.native_trigger_criterion(player.uuid, tool_uses::USE_ITEM_SUCCESS, |c| {
        c.item("item", &book)
    });
    let pitch = 0.7 + random_float() * 0.4;
    game.native_play_sound_in_place("botania:lexicon_open", player.position, PLAYERS, 1.0, pitch);
    *claimed = true;
}

// `TinyPotatoBlock.use` with an empty hand: the potato is petted (`main/tiny_potato_pet`) and a
// heart rises from it (`spawnHearts`). Its items, jump and names aren't ported: a hand holding
// something passes.
pub(super) fn right_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if *canceled || !game.native_authoritative() || game.hand != Hand::Main {
        return;
    }
    if game.native_block(pos) != tool_uses::TINY_POTATO {
        return;
    }
    let empty = |s: &Option<ItemStack>| s.as_ref().is_none_or(|s| s.count == 0);
    let main_empty = game.inventory.get(game.selected).is_none_or(empty);
    if !main_empty || game.native_acting_player().is_none() {
        return;
    }
    grant_code_triggered(game, tool_uses::TINY_POTATO_PET);
    // `TinyPotatoBlock.SHAPE.bounds()`: x and z 6/16 to 10/16, top 6/16.
    let (lo, hi) = (6.0 / 16.0, 10.0 / 16.0);
    let at = [
        f64::from(pos[0]) + lo + f64::from(random_float()) * (hi - lo),
        f64::from(pos[1]) + 6.0 / 16.0,
        f64::from(pos[2]) + lo + f64::from(random_float()) * (hi - lo),
    ];
    let heart = ParticleOptions::simple(ParticleType::Heart);
    game.native_send_particles_in_place(&heart, None, at, [0.0; 3], 0.0, 1);
    *canceled = true;
}

// ---- the elementium axe on players --------------------------------------------------------------

// A dying player's own loot table (`elementium_axe_beheading` is a pool of every entity's): with
// a player credited with the kill, the killer's hand is read after the level's tick.
pub(super) fn loot_drops(
    game: &mut Game<'_>,
    context: &NativeLootDrops<'_>,
    _drops: &mut Vec<ItemStack>,
) {
    if context.used || context.table != PLAYER_TABLE || !game.native_authoritative() {
        return;
    }
    let (Some((_, "minecraft:player")), Some(killer)) = (context.entity, context.player) else {
        return;
    };
    // The drops are heard in the dying player's turn.
    let Some(victim) = game.native_acting_player().map(|p| p.uuid) else {
        return;
    };
    if victim == killer {
        return;
    }
    let dim = game.dim.id();
    state().deaths.push(Death {
        dim,
        victim,
        killer,
        at: context.origin,
    });
}

// After a level's tick: a player killed by a player holding an elementium axe drops their head
// (`fill_player_head`: their profile) at Botania's chance, 9.09% and as much again per Looting
// level.
pub(super) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    if !game.native_authoritative() {
        return;
    }
    let deaths = {
        let mut s = state();
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.deaths)
            .into_iter()
            .partition(|d| d.dim == dimension);
        s.deaths = rest;
        mine
    };
    let Some(head) = tools::beheading("minecraft:player", false) else {
        return;
    };
    for d in deaths {
        let mut held = None;
        game.native_as_player(d.killer, |g| {
            held = g.inventory.get(g.selected).cloned().flatten();
        });
        let Some(axe) = held.filter(|s| s.count > 0 && &*s.item == tools::ELEMENTIUM_AXE) else {
            continue;
        };
        let looting = axe.enchantment_level("minecraft:looting") as i32;
        if tools::beheads(head, looting, random_float()) {
            let stack = game.native_player_head(d.victim);
            game.spawn_item_own(stack, d.at, 10);
        }
    }
}
