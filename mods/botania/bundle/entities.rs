// Botania's Ender Air and pixies for Horizon Aurora: a modified port of Vazkii's Botania
// (Botania License, see LICENSE and NOTICE.md), added to `crates/horizon-client/src/game.rs` as
// `mod botania_entities` by horizon-mixins. The rules are in `botania_core::entities`.
//
// - Ender Air (`EnderAirEntity`): Horizon has no plain entity type a content mod can declare, so
//   a cloud is a `minecraft:marker` tagged `botania.ender_air`, which the content's
//   `botania:ender_air_release` function summons where the Pure Daisy turns end stone. This
//   module ages it (its `data.Age`), sends its particles and discards it after 60 ticks. A glass
//   bottle used near one, or anywhere in the End away from dragon's breath, fills with it
//   (`EnderAirItem.onPlayerInteract`); dispensers do the same (`EnderAirBottlingBehavior`).
// - The thrown Ender Air Bottle (`EnderAirBottleEntity`) is the content's thrown type
//   `botania:ender_air_bottle`: this module throws it with Botania's sound, dispenses it, and on
//   its impact turns the stone around into end stone, or makes an Overworld ghast cry.
// - Pixies (`PixieEntity`, `PixieHandler`) are the content's living type `botania:pixie` (a
//   wandering trader's class) with Botania's 1 x 1 box, no goals, and the flight and strike of
//   `customServerAiStep`, set at each tick's start. They are drawn with `PixieModel`; their
//   sparkles are Botania's (`botania_particles`), sent from the server as the pixie ticks.
//
// Inner attributes and `//!` comments are not allowed in a horizon-mixins module payload.

use super::Game;
use super::native_block_entity_draw::BlockEntityDraw;
use super::native_mob_draw::NativeMobRender;
use botania_core::entities::{self as rules, Aabb, PixieStep};
use botania_core::fx::Record;
use horizon_client::items::ItemStack;
use horizon_client::mobs::model_layers::{
    CubeDeformation, CubeListBuilder, LayerDefinition, MeshDefinition, ModelPart, PartPose,
};
use horizon_client::mobs::{Mob, Who};
use horizon_client::mod_api::NativeDispensed;
use horizon_client::particles::{ParticleOptions, ParticleType};
use horizon_client::projectiles::{Projectile, ProjectileKind};
use horizon_core::geometry::{Direction, Vec3};
use horizon_core::random::RandomSource;
use horizon_world::nbt::{Compound, JavaString, Tag};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

// `SoundSource` ordinals.
const NEUTRAL: usize = 6;
const PLAYERS: usize = 7;
const THE_END: &str = "minecraft:the_end";
const PLAYER: &str = "minecraft:player";
// The dispenser's level events: the click, a launch, the smoke.
const CLICK: i32 = 1000;
const LAUNCH: i32 = 1002;
const SMOKE: i32 = 2000;
// Level events: a splash potion's particles (2002) and its sound (1053, its own event since
// 1.21), a block's breaking particles (2001).
const SPLASH: i32 = 2002;
const SPLASH_SOUND: i32 = 1053;
const DESTROY_BLOCK: i32 = 2001;
// The damage types only a player (or what a player threw or shot) deals. The game names no
// causing entity when a player hurts a mob, so these stand for `source.getEntity() instanceof
// Player` where a pixie asks it.
const PLAYER_DAMAGE: [&str; 5] = [
    "minecraft:player_attack",
    "minecraft:arrow",
    "minecraft:trident",
    "minecraft:thrown",
    "minecraft:player_explosion",
];
// Hits on players kept for their tick at most.
const MAX_HURTS: usize = 64;

// ---- state ---------------------------------------------------------------------------------------

// The server's side. Never held across a call into the game: a game helper can come back into
// this module (a summon's join, a hit's damage hook), and the lock is not reentrant.
#[derive(Default)]
struct Server {
    clouds: Vec<Cloud>,
    // Ender Air Bottles in flight: their level and projectile id.
    bottles: Vec<(&'static str, u32)>,
    pixies: HashMap<u32, Pixie>,
    // Pixies nobody summoned through `spawn_pixie` (a `/summon`, a save): without a target they
    // are discarded, as Botania's `baseTick` discards them.
    strays: Vec<(&'static str, u32)>,
    // `spawn_pixie` is summoning: the pixie that joins is its own.
    spawning: bool,
    // Players hurt by an entity since their last tick (`PixieHandler.onDamageTaken`).
    hurts: Vec<Hurt>,
    // The entity ids of the players seen ticking, for the damage hook, which has no game.
    players: HashSet<i32>,
    rng: u64,
}

// An Ender Air cloud: its marker.
struct Cloud {
    dim: &'static str,
    id: u32,
    age: i32,
}

// `PixieEntity`'s fields: `summoner`, `damage`, `effect`, the target and `PIXIE_TYPE`.
struct Pixie {
    dim: &'static str,
    summoner: Option<Summoner>,
    target: u32,
    // Where the target last stood and its width and height: a target gone from the level is
    // still flown at, as Botania keeps the entity it was given.
    aim: Option<([f64; 3], f32, f32)>,
    dark: bool,
    damage: f32,
    effect: Option<(&'static str, i32)>,
    ticks: i32,
    // Where it was at its last tick, for its parting sparkles.
    at: [f64; 3],
    // It reached its aim this tick: it strikes once the level has ticked.
    strike: bool,
}

#[derive(Clone, Copy)]
struct Summoner {
    id: u32,
    player: bool,
}

struct Hurt {
    victim: i32,
    causing: i32,
    waited: u8,
}

static SERVER: LazyLock<Mutex<Server>> = LazyLock::new(Mutex::default);

fn server() -> MutexGuard<'static, Server> {
    SERVER.lock().unwrap_or_else(PoisonError::into_inner)
}

fn random_bits() -> u64 {
    let mut s = server();
    let mut x = if s.rng == 0 {
        0x2545_F491_4F6C_DD1D
    } else {
        s.rng
    };
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    s.rng = x;
    x
}

// `Math.random()`.
fn random_unit() -> f64 {
    (random_bits() >> 11) as f64 / (1u64 << 53) as f64
}

// `Random.nextInt(n)` of `Collections.shuffle`'s own random.
fn random_below(n: i32) -> i32 {
    (random_bits() % n.max(1) as u64) as i32
}

// ---- small things --------------------------------------------------------------------------------

fn aabb(b: horizon_core::geometry::Aabb) -> Aabb {
    let (lo, hi) = (b.min_position(), b.max_position());
    [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z]
}

fn step(d: Direction) -> [i32; 3] {
    match d {
        Direction::Down => [0, -1, 0],
        Direction::Up => [0, 1, 0],
        Direction::North => [0, 0, -1],
        Direction::South => [0, 0, 1],
        Direction::West => [-1, 0, 0],
        Direction::East => [1, 0, 0],
    }
}

// `Direction.get3DDataValue`: the smoke's direction.
fn data_value(d: Direction) -> i32 {
    match d {
        Direction::Down => 0,
        Direction::Up => 1,
        Direction::North => 2,
        Direction::South => 3,
        Direction::West => 4,
        Direction::East => 5,
    }
}

fn feet(game: &Game<'_>) -> Option<[f64; 3]> {
    let p = game.player.as_ref()?;
    Some([p.position.x, p.position.y, p.position.z])
}

fn block_of(at: [f64; 3]) -> [i32; 3] {
    at.map(|v| v.floor() as i32)
}

fn is_air(game: &Game<'_>, pos: [i32; 3]) -> bool {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    game.res.facts().is_air(state)
}

// The acting hand loses one item (`stack.shrink(1)`).
fn shrink_held(game: &mut Game<'_>) {
    if let Some(s) = game.held_mut().as_mut() {
        s.count = s.count.saturating_sub(1);
        if s.count == 0 {
            *game.held_mut() = None;
        }
    }
}

// `Inventory.placeItemBackInInventory`: what doesn't fit is dropped from the eyes.
fn give_or_drop(game: &mut Game<'_>, stack: ItemStack) {
    if let Some(rest) = game.give(stack)
        && let Some(eye) = game.eye_position()
    {
        game.throw_from_eyes(rest, eye, false);
    }
}

// ---- Ender Air -----------------------------------------------------------------------------------

// The marker's tags name it a cloud: its age, from `data.Age` (`readAdditionalSaveData`).
fn cloud_age(game: &Game<'_>, id: u32) -> Option<i32> {
    let raw = game.persist.raw.get(&id)?;
    let Some(Tag::List(tags)) = raw.get("Tags") else {
        return None;
    };
    let tagged = tags
        .iter()
        .any(|t| matches!(t, Tag::String(s) if s.to_string_lossy() == rules::ENDER_AIR_TAG));
    if !tagged {
        return None;
    }
    Some(match raw.get("data").and_then(|d| d.get("Age")) {
        Some(Tag::Int(age)) => *age,
        _ => 0,
    })
}

// `addAdditionalSaveData`: the age goes into the marker's `data`, which its save keeps.
fn save_cloud_age(game: &mut Game<'_>, id: u32, age: i32) {
    let Some(Tag::Compound(c)) = game.persist.raw.get_mut(&id) else {
        return;
    };
    let data = c
        .entry(JavaString::from("data"))
        .or_insert_with(|| Tag::Compound(Compound::new()));
    if let Tag::Compound(d) = data {
        d.insert(JavaString::from("Age"), Tag::Int(age));
    }
}

fn cloud_feet(game: &Game<'_>, id: u32) -> Option<[f64; 3]> {
    let o = game.others.iter().find(|o| o.id == id)?;
    Some([o.position.x, o.position.y, o.position.z])
}

// `EnderAirItem.isClearFromDragonBreath`: no cloud of dragon's breath (one with no potion) meets
// `area`. A cloud's box is its radius across each way and half a block tall.
fn breath_clear(game: &Game<'_>, area: Aabb) -> bool {
    !game.breath_clouds.iter().any(|b| {
        let r = f64::from(b.radius);
        let cloud = [
            b.at[0] - r,
            b.at[1],
            b.at[2] - r,
            b.at[0] + r,
            b.at[1] + 0.5,
            b.at[2] + r,
        ];
        b.potion.is_none() && rules::intersects(cloud, area)
    })
}

// `EnderAirItem.pickupFromEntity`: the first cloud of this level meeting `area` is used up.
fn take_cloud(game: &mut Game<'_>, dim: &'static str, area: Aabb) -> bool {
    let ids: Vec<u32> = server()
        .clouds
        .iter()
        .filter(|c| c.dim == dim)
        .map(|c| c.id)
        .collect();
    let Some(id) = ids.into_iter().find(|id| {
        cloud_feet(game, *id).is_some_and(|f| rules::intersects(rules::ender_air_box(f), area))
    }) else {
        return false;
    };
    server().clouds.retain(|c| c.id != id);
    game.native_remove_entity(dim, id);
    true
}

// `notAimingAtFluid`: the look (5 blocks, fluids included) ends on a block holding no fluid, or
// on nothing.
fn aiming_at_fluid(game: &Game<'_>) -> bool {
    let Some(eye) = game.eye_position() else {
        return false;
    };
    let dir = super::view_vector(game.yaw, game.pitch);
    let Some(hit) = game.clip_ray_fluids(eye, dir, rules::POV_REACH, true) else {
        return false;
    };
    let state = game.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
    let f = game.res.facts().get(state);
    f.fluid != horizon_client::blocks::Fluid::None || f.waterlogged
}

// `EnderAirItem.onPlayerInteract` with a glass bottle in the acting hand.
fn bottle_by_hand(game: &mut Game<'_>, claimed: &mut bool) {
    let Some(body) = game.player.as_ref().map(|p| aabb(p.bounds())) else {
        return;
    };
    let Some(at) = feet(game) else {
        return;
    };
    let dim = game.dim.id();
    let from_end = dim == THE_END
        && rules::bottles_by_hand(
            true,
            breath_clear(game, rules::inflate(body, rules::HAND_BREATH_RANGE)),
            aiming_at_fluid(game),
        );
    if !from_end && !take_cloud(game, dim, rules::inflate(body, rules::HAND_REACH)) {
        return;
    }
    *claimed = true;
    give_or_drop(game, ItemStack::new(rules::ENDER_AIR_BOTTLE, 1));
    shrink_held(game);
    // `playSound(null, player.blockPosition(), ...)`: the middle of the block at the feet.
    let block = block_of(at).map(|v| f64::from(v) + 0.5);
    game.native_play_sound_in_place("minecraft:entity.item.pickup", block, NEUTRAL, 0.5, 1.0);
    let ctx = game.who_context(Some(Who::Player), None);
    game.post_game_event("minecraft:fluid_pickup", at, ctx);
}

// After a level's tick: its clouds age, sparkle half the ticks, and go after 60.
fn tick_clouds(game: &mut Game<'_>, dim: &'static str) {
    let mut clouds: Vec<Cloud> = {
        let mut s = server();
        if s.clouds.is_empty() {
            return;
        }
        let (mine, rest): (Vec<Cloud>, Vec<Cloud>) = std::mem::take(&mut s.clouds)
            .into_iter()
            .partition(|c| c.dim == dim);
        s.clouds = rest;
        mine
    };
    let rgb = rules::ENDER_AIR_COLOR;
    let options = ParticleOptions::colored(
        ParticleType::EntityEffect,
        [
            ((rgb >> 16) & 0xFF) as f32 / 255.0,
            ((rgb >> 8) & 0xFF) as f32 / 255.0,
            (rgb & 0xFF) as f32 / 255.0,
            1.0,
        ],
    );
    let mut fx = Vec::new();
    clouds.retain_mut(|c| {
        let Some(at) = cloud_feet(game, c.id) else {
            return false;
        };
        c.age += 1;
        if rules::ender_air_gone(c.age) {
            game.native_remove_entity(dim, c.id);
            return false;
        }
        save_cloud_age(game, c.id, c.age);
        // `random.nextBoolean()`, then five at `getX() + random.nextDouble()`...
        if random_unit() < 0.5 {
            for _ in 0..5 {
                fx.push([
                    at[0] + random_unit(),
                    at[1] + random_unit(),
                    at[2] + random_unit(),
                ]);
            }
        }
        true
    });
    if !fx.is_empty() {
        game.with_level(|_, level, _| {
            for at in &fx {
                level.native_send_particles(&options, None, *at, [0.0; 3], 0.0, 1);
            }
        });
    }
    server().clouds.extend(clouds);
}

// `PureDaisyRecipe.set`: once the block is set, the recipe's function runs as the server at the
// block's bottom centre, with its output suppressed. An unknown function does nothing.
pub(super) fn run_success_function(game: &mut Game<'_>, name: &str, at: [i32; 3]) {
    let Ok(functions) = game.functions_named(name) else {
        return;
    };
    let mut source = game.server_source();
    source.pos = [
        f64::from(at[0]) + 0.5,
        f64::from(at[1]),
        f64::from(at[2]) + 0.5,
    ];
    source.silent = true;
    // `ServerFunctionManager.execute` starts a fresh execution: its command count from zero.
    game.exec.commands = 0;
    for function in &functions {
        let _ = game.run_function(function, None, source);
    }
}

// ---- the Ender Air Bottle ------------------------------------------------------------------------

fn track_bottle(dim: &'static str, id: u32) {
    let mut s = server();
    if !s.bottles.iter().any(|b| b.1 == id) {
        s.bottles.push((dim, id));
    }
}

// `EnderAirItem.use`: thrown as `shootFromRotation(player, xRot, yRot, 0, 1.5, 1)` from the eyes
// less 0.1, one used up outside creative, with Botania's sound.
fn throw_bottle(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    let Some(kind) = horizon_client::projectiles::thrown_type(rules::ENDER_AIR_BOTTLE) else {
        return;
    };
    let (Some(eye), Some(at)) = (game.eye_position(), feet(game)) else {
        return;
    };
    *claimed = true;
    // The mods, before the bottle exists, as for any thrown item: a canceled throw is a failed
    // use, and the guest's client gets back the item it took.
    if let (Some(player), Some(name)) = (game.lan_acting_uuid, game.lan_acting_name.clone()) {
        let mut cancel = false;
        game.native_player_launch_projectile_v2(player, &name, stack, &mut cancel);
        if cancel {
            game.use_failed = true;
            game.guest_inventory_resend = true;
            return;
        }
    }
    if !game.creative {
        shrink_held(game);
    }
    let pitch = rules::throw_pitch(game.world.random.next_float());
    game.native_play_sound_in_place("botania:ender_air_throw", at, PLAYERS, 1.0, pitch);
    let (velocity, on_ground) = game.player.as_ref().map_or((Vec3::ZERO, true), |p| {
        (game.known_player_movement(), p.ground.on_ground)
    });
    let start = Vec3::new(eye[0], eye[1] - 0.1, eye[2]);
    let mut bottle = Projectile::new(ProjectileKind::Thrown(kind), start, Some(Who::Player));
    let (pitch, yaw) = (game.pitch, game.yaw);
    bottle.shoot_from_rotation(
        &mut game.world.random,
        pitch,
        yaw,
        0.0,
        rules::THROW_SPEED,
        rules::THROW_UNCERTAINTY,
        velocity,
        on_ground,
    );
    game.next_entity += 1;
    bottle.id = game.next_entity;
    let id = bottle.id;
    game.add_projectile(bottle);
    track_bottle(game.dim.id(), id);
}

// `getCoordsToPut` and `convertBlock` around `center`: up to 64 of the stone blocks within 4
// turn into end stone, a tenth of them with breaking particles; the splash is at `splash`.
fn convert_blocks(game: &mut Game<'_>, dim: &'static str, center: [i32; 3], splash: [i32; 3]) {
    game.level_event_at(SPLASH, splash, 0xFF00_0008_u32 as i32);
    game.level_event_at(SPLASH_SOUND, splash, 0);
    let Some(end_stone) = game.native_default_state("minecraft:end_stone") else {
        return;
    };
    let candidates: Vec<[i32; 3]> = {
        let items = game.items();
        rules::conversion_area(center)
            .filter(|p| {
                if !game.world.loaded(p[0], p[2]) {
                    return false;
                }
                let state = game.world.block(p[0], p[1], p[2]);
                items.block_in_tag(rules::CONVERTABLE, &game.res.facts().get(state).name)
            })
            .collect()
    };
    for pos in rules::pick_converted(candidates, random_below) {
        game.native_set_block(dim, pos, end_stone, horizon_world::level::flags::UPDATE_ALL);
        if random_unit() < rules::DESTROY_PARTICLE_CHANCE {
            game.level_event_at(DESTROY_BLOCK, pos, end_stone.0 as i32);
        }
    }
}

// `onHitEntity` on an Overworld ghast: a hurt of nothing, and `botania:ghast_ender_air_crying`
// dropped two blocks up, pushed the way it faces. (Its tears' item particles can't be sent from a
// server mod yet.)
fn ghast_cries(game: &mut Game<'_>, dim: &'static str, ghast: u32, owner: u32, splash: [i32; 3]) {
    let Some((at, yaw, pitch)) = game
        .mobs
        .iter()
        .find(|m| m.id == ghast && !m.removed)
        .map(|m| (m.position, m.yaw, m.head_pitch))
    else {
        return;
    };
    game.level_event_at(SPLASH, splash, 0xFF00_0008_u32 as i32);
    game.level_event_at(SPLASH_SOUND, splash, 0);
    let by = (owner != 0).then_some(owner);
    if !game.native_hurt_entity(dim, ghast, 0.0, "minecraft:thrown", by) && by.is_some() {
        game.native_hurt_entity(dim, ghast, 0.0, "minecraft:thrown", None);
    }
    // `new Vec3(look.x, 0, look.z).normalize()`.
    let look = super::view_vector(yaw, pitch);
    let len = (look[0] * look[0] + look[2] * look[2]).sqrt();
    let push = if len < 1.0e-4 {
        [0.0, 0.0]
    } else {
        [look[0] / len * 0.4, look[2] / len * 0.4]
    };
    let table = rules::GHAST_LOOT_TABLE;
    for stack in game.sequence_loot(table, table, None) {
        game.spawn_at_location(at, 2.0, stack);
        if let Some(item) = game.just_spawned_item() {
            let v = item.velocity;
            item.velocity = Vec3::new(v.x + push[0], v.y, v.z + push[1]);
        }
    }
}

// ---- pixies --------------------------------------------------------------------------------------

// `BotaniaEntities.PIXIE` over the wandering trader the content type copies: a 1 x 1 box, the
// sounds of a `FlyingMob` (none ambient, the generic hurt and death), and no goals.
fn patch_pixie(m: &mut Mob, server_side: bool) {
    if m.kind.width == rules::PIXIE_SIZE && m.kind.height == rules::PIXIE_SIZE {
        return;
    }
    let mut kind = (*m.kind).clone();
    kind.width = rules::PIXIE_SIZE;
    kind.height = rules::PIXIE_SIZE;
    kind.eye_height = rules::PIXIE_EYE_HEIGHT;
    kind.sounds = ["", "entity.generic.hurt", "entity.generic.death"];
    kind.step_sound = None;
    m.kind = Arc::new(kind);
    if server_side {
        m.ai.goals.clear();
        m.ai.targets.clear();
    }
}

/// Summons a pixie at `at` (Botania's `new PixieEntity`, `setProps`, `addFreshEntity`) in the
/// level in place: it flies at the entity `target` and strikes it for `damage`, as the player or
/// mob `summoner` (none: as itself), then adding `effect` (an effect id and its ticks, amplifier
/// 0) to a target that isn't a player; it is gone after 200 ticks. `dark` is the Gaia Guardian's
/// slow pixie (type 1), hurt only by players. Its id, or None off a server or where the summon
/// fails. A Great Fairy Ring's pixies come from [`player_hurt`] already, through
/// [`pixie_ring_worn`].
pub(super) fn spawn_pixie(
    game: &mut Game<'_>,
    at: [f64; 3],
    summoner: Option<u32>,
    target: u32,
    dark: bool,
    damage: f32,
    effect: Option<(&'static str, i32)>,
) -> Option<u32> {
    if !game.native_authoritative() {
        return None;
    }
    let dim = game.dim.id();
    let summoner = summoner.map(|id| Summoner {
        id,
        player: !game.mobs.iter().any(|m| m.id == id),
    });
    server().spawning = true;
    let id = game.native_summon(dim, rules::PIXIE, at, Compound::new());
    server().spawning = false;
    let id = id?;
    if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
        patch_pixie(m, true);
    }
    server().pixies.insert(
        id,
        Pixie {
            dim,
            summoner,
            target,
            aim: None,
            dark,
            damage,
            effect,
            ticks: 0,
            at,
            strike: false,
        },
    );
    Some(id)
}

/// Whether the acting player wears a Great Fairy Ring (`botania:pixie_ring`), which adds 0.25 to
/// their pixie chance. Horizon has no accessory slots, so a ring anywhere in the inventory or
/// the off hand is worn, as the mana bands are.
pub(super) fn pixie_ring_worn(game: &Game<'_>) -> bool {
    game.inventory
        .iter()
        .take(36)
        .chain(std::iter::once(&game.offhand))
        .flatten()
        .any(|s| s.count > 0 && &*s.item == rules::PIXIE_RING)
}

fn stack_item(stack: Option<&ItemStack>) -> Option<&str> {
    stack.filter(|s| s.count > 0).map(|s| &*s.item)
}

// `PixieHandler.onDamageTaken` for the acting player, hurt by the entity `causing`.
fn player_hurt(game: &mut Game<'_>, me: i32, at: [f64; 3], causing: i32) {
    let Ok(attacker) = u32::try_from(causing) else {
        return;
    };
    // `source.getEntity() instanceof LivingEntity`: a mob or a player.
    let living = game.mobs.iter().any(|m| m.id == attacker && !m.removed)
        || game
            .native_entity(attacker)
            .and_then(|e| game.native_entity_kind(e))
            .is_some_and(|k| k == PLAYER);
    if !living {
        return;
    }
    let (chance, full_set, sword) = {
        let worn: [Option<&str>; 4] = std::array::from_fn(|i| stack_item(game.armor[i].as_ref()));
        let main = stack_item(game.inventory.get(game.selected).and_then(Option::as_ref));
        let off = stack_item(game.offhand.as_ref());
        (
            rules::spawn_chance(worn, main, pixie_ring_worn(game)),
            rules::has_elementium_set(worn),
            main == Some(rules::ELEMENTIUM_SWORD) || off == Some(rules::ELEMENTIUM_SWORD),
        )
    };
    if random_unit() >= chance {
        return;
    }
    let effect = full_set.then(|| {
        let i = game.world.random.next_int_bound(4).unwrap_or(0) as usize;
        rules::PIXIE_EFFECTS[i.min(3)]
    });
    let from = [at[0], at[1] + rules::PIXIE_SPAWN_HEIGHT, at[2]];
    spawn_pixie(
        game,
        from,
        Some(me as u32),
        attacker,
        false,
        rules::pixie_damage(sword),
        effect,
    );
}

// Where the entity `target` stands, and its width and height (`getBbWidth`, `getBbHeight`).
fn target_aim(game: &Game<'_>, target: u32) -> Option<([f64; 3], f32, f32)> {
    if let Some(m) = game.mobs.iter().find(|m| m.id == target && !m.removed) {
        let (w, h, _) = m.dims();
        return Some((
            [m.position.x, m.position.y, m.position.z],
            w as f32,
            h as f32,
        ));
    }
    let e = game.native_entity(target)?;
    let [lo, hi] = game.native_entity_bounding_box(e)?;
    Some((
        [(lo[0] + hi[0]) / 2.0, lo[1], (lo[2] + hi[2]) / 2.0],
        (hi[0] - lo[0]) as f32,
        (hi[1] - lo[1]) as f32,
    ))
}

// After a level's tick: the pixies that reached their aim strike (`customServerAiStep`'s hurt
// and `addEffect`) and are discarded.
fn strike(game: &mut Game<'_>, dim: &'static str) {
    let strikes: Vec<(u32, Pixie)> = {
        let mut s = server();
        let ids: Vec<u32> = s
            .pixies
            .iter()
            .filter(|(_, p)| p.dim == dim && p.strike)
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter()
            .filter_map(|id| s.pixies.remove(&id).map(|p| (id, p)))
            .collect()
    };
    for (id, p) in strikes {
        // `playerAttack(summoner)`, `mobAttack(summoner)`, or `mobAttack(this)`; a summoner
        // gone from the level leaves the pixie to strike as itself.
        match p.summoner.filter(|s| game.native_entity(s.id).is_some()) {
            Some(s) => {
                let kind = if s.player {
                    "minecraft:player_attack"
                } else {
                    "minecraft:mob_attack"
                };
                game.native_hurt_entity(dim, p.target, p.damage, kind, Some(s.id));
            }
            None => {
                game.native_hurt_entity(dim, p.target, p.damage, "minecraft:mob_attack", Some(id));
            }
        }
        if let Some((effect, ticks)) = p.effect
            && let Some(m) = game
                .mobs
                .iter_mut()
                .find(|m| m.id == p.target && !m.removed)
        {
            m.add_effect(effect, ticks, 0);
        }
        discard_pixie(game, dim, id, p.dark);
    }
}

// `discard()` of the pixie `id`: a summoned one parts in sparkles.
fn discard_pixie(game: &mut Game<'_>, dim: &'static str, id: u32, dark: bool) {
    let at = game
        .mobs
        .iter()
        .find(|m| m.id == id && !m.removed)
        .map(|m| [m.position.x, m.position.y, m.position.z]);
    if game.native_remove_mob(dim, id)
        && !dark
        && let Some(at) = at
    {
        pixie_sparkles(game, at, false, true);
    }
}

// A pixie's sparkles of this tick (`baseTick` on a client), or the burst where a summoned one
// goes (`remove`), sent to the players near it.
fn pixie_sparkles(game: &Game<'_>, feet: [f64; 3], dark: bool, parting: bool) {
    let n = if parting {
        rules::SPARKLES_ON_REMOVAL
    } else {
        rules::SPARKLES_PER_TICK
    };
    let records: Vec<Record> = (0..n)
        .map(|_| {
            let r = [random_unit(), random_unit(), random_unit(), random_unit()];
            Record::Sparkle(rules::pixie_sparkle(feet, dark, parting, r))
        })
        .collect();
    super::botania_particles::send(game, records);
}

// ---- the window's side: the pixies' box and model ----------------------------------------------

// Each tick of the window: pixies in view get Botania's box, which picking and their outline use.
fn client_tick(game: &mut Game<'_>) {
    for m in game.mobs.iter_mut() {
        if m.kind.id == rules::PIXIE {
            patch_pixie(m, false);
        }
    }
}

// `PixieModel` baked (`BotaniaLayerDefinitions`, 32 x 32).
fn pixie_model() -> &'static ModelPart {
    static MODEL: OnceLock<ModelPart> = OnceLock::new();
    MODEL.get_or_init(|| {
        let mut mesh = MeshDefinition::new();
        let root = mesh.root();
        for p in rules::PIXIE_PARTS {
            let mut cubes = CubeListBuilder::create();
            for c in p.cubes {
                cubes = cubes
                    .tex_offs(c.tex[0], c.tex[1])
                    .set_mirror(c.mirror)
                    .add_box_with(c.origin, c.size, CubeDeformation::new(c.grow));
            }
            let [x, y, z, x_rot, y_rot, z_rot] = p.pose;
            let pose = PartPose::offset_and_rotation(x, y, z, x_rot, y_rot, z_rot);
            mesh.add_or_replace_child(root, p.name, cubes, pose);
        }
        let [width, height] = rules::PIXIE_TEXTURE_SIZE;
        LayerDefinition::create(mesh, width, height).bake_root()
    })
}

// `ClientProxy.dootDoot`, settled once at start as Botania settles it (by the UTC date).
fn halloween() -> bool {
    static HALLOWEEN: OnceLock<bool> = OnceLock::new();
    *HALLOWEEN.get_or_init(|| {
        let days = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() / 86_400);
        rules::doot_doot(i64::try_from(days).unwrap_or(0))
    })
}

// ---- hooks ---------------------------------------------------------------------------------------

// `native_use_item`: a glass bottle filled with Ender Air, an Ender Air Bottle thrown.
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if *claimed || stack.count == 0 || !game.native_authoritative() {
        return;
    }
    match &*stack.item {
        rules::GLASS_BOTTLE => bottle_by_hand(game, claimed),
        rules::ENDER_AIR_BOTTLE => throw_bottle(game, stack, claimed),
        _ => {}
    }
}

// `native_dispense_item`: `EnderAirBottlingBehavior` for glass bottles (vanilla's behaviour when
// there is no Ender Air), and the bottle thrown as `AbstractProjectileDispenseBehavior` throws.
pub(super) fn dispense_item(
    game: &mut Game<'_>,
    dimension: &str,
    pos: [i32; 3],
    facing: Direction,
    stack: &ItemStack,
    answer: &mut Option<NativeDispensed>,
) {
    if answer.is_some() || stack.count == 0 || dimension != game.dim.id() {
        return;
    }
    let dim = game.dim.id();
    let s = step(facing);
    let front = [pos[0] + s[0], pos[1] + s[1], pos[2] + s[2]];
    let key = (pos[0], pos[1], pos[2]);
    match &*stack.item {
        rules::GLASS_BOTTLE => {
            let from_end = dim == THE_END
                && rules::bottles_in_end(
                    true,
                    is_air(game, front),
                    is_air(game, [front[0], front[1] + 1, front[2]]),
                    breath_clear(
                        game,
                        rules::inflate(rules::block_box(front), rules::DISPENSER_BREATH_RANGE),
                    ),
                );
            if !from_end && !take_cloud(game, dim, rules::block_box(front)) {
                return;
            }
            // `fillBottle`: the bottle takes the slot of the last glass bottle, else the first
            // empty slot (`DispenserBlockEntity.addItem`), else it is dropped out of the front.
            let filled = ItemStack::new(rules::ENDER_AIR_BOTTLE, 1);
            let mut left = stack.clone();
            left.count -= 1;
            if left.count == 0 {
                *answer = Some((Some(filled), CLICK));
                return;
            }
            let mut placed = false;
            if let Some(entity) = game.world.block_entities.get_mut(&key)
                && let Some(cell) = entity.slots_mut().iter_mut().find(|c| c.is_none())
            {
                *cell = Some(filled.clone());
                placed = true;
            }
            if !placed {
                game.spawn_dispensed(key, facing, filled, 6);
                game.dispenser_event(CLICK, key, 0);
                game.dispenser_event(SMOKE, key, data_value(facing));
            }
            *answer = Some((Some(left), CLICK));
        }
        rules::ENDER_AIR_BOTTLE => {
            let Some(kind) = horizon_client::projectiles::thrown_type(rules::ENDER_AIR_BOTTLE)
            else {
                return;
            };
            let out = rules::DISPENSE_OUT;
            let at = Vec3::new(
                f64::from(pos[0]) + 0.5 + out * f64::from(s[0]),
                f64::from(pos[1]) + 0.5 + out * f64::from(s[1]),
                f64::from(pos[2]) + 0.5 + out * f64::from(s[2]),
            );
            let mut bottle = Projectile::new(ProjectileKind::Thrown(kind), at, None);
            // `shoot(stepX, stepY + 0.1F, stepZ, ...)`, spread by the bottle's own random.
            let dir = [
                f64::from(s[0]),
                f64::from(s[1] as f32 + rules::DISPENSE_RAISE as f32),
                f64::from(s[2]),
            ];
            let mut own = horizon_client::random_source::entity();
            bottle.shoot(
                &mut own,
                dir,
                rules::DISPENSE_POWER,
                rules::DISPENSE_UNCERTAINTY,
            );
            game.next_entity += 1;
            bottle.id = game.next_entity;
            let id = bottle.id;
            game.add_projectile(bottle);
            track_bottle(dim, id);
            let mut left = stack.clone();
            left.count -= 1;
            *answer = Some(((left.count > 0).then_some(left), LAUNCH));
        }
        _ => {}
    }
}

// `native_entity_join_level`: an Ender Air marker starts ageing; a bottle in flight is heard
// when it lands; a pixie that `spawn_pixie` didn't make is a stray.
pub(super) fn join_level(
    game: &mut Game<'_>,
    dimension: &str,
    entity: u32,
    kind: &str,
    canceled: bool,
) {
    if canceled || dimension != game.dim.id() {
        return;
    }
    let dim = game.dim.id();
    match kind {
        "minecraft:marker" => {
            if let Some(age) = cloud_age(game, entity) {
                let mut s = server();
                s.clouds.retain(|c| c.id != entity);
                s.clouds.push(Cloud {
                    dim,
                    id: entity,
                    age,
                });
            }
        }
        rules::ENDER_AIR_BOTTLE => track_bottle(dim, entity),
        rules::PIXIE => {
            let mut s = server();
            if !s.spawning {
                s.strays.push((dim, entity));
            }
        }
        _ => {}
    }
}

// `native_level_tick_post`: clouds age, pixies strike, and what left the level is forgotten.
pub(super) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    if !game.native_authoritative() || dimension != game.dim.id() {
        return;
    }
    let dim = game.dim.id();
    tick_clouds(game, dim);
    strike(game, dim);
    let (strays, bottles, pixies) = {
        let mut s = server();
        let (strays, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.strays)
            .into_iter()
            .partition(|p| p.0 == dim);
        s.strays = rest;
        let bottles: Vec<u32> = s
            .bottles
            .iter()
            .filter(|b| b.0 == dim)
            .map(|b| b.1)
            .collect();
        let pixies: Vec<(u32, [f64; 3], bool)> = s
            .pixies
            .iter()
            .filter(|(_, p)| p.dim == dim)
            .map(|(id, p)| (*id, p.at, p.dark))
            .collect();
        (strays, bottles, pixies)
    };
    // Strays whose tick never came (outside the ticking chunks).
    for (_, id) in strays {
        discard_pixie(game, dim, id, false);
    }
    let gone_bottles: Vec<u32> = bottles
        .into_iter()
        .filter(|id| game.native_entity(*id).is_none())
        .collect();
    // Pixies killed, or gone with their chunk: the window sees them go too.
    let gone_pixies: Vec<(u32, [f64; 3], bool)> = pixies
        .into_iter()
        .filter(|(id, ..)| !game.mobs.iter().any(|m| m.id == *id && !m.removed))
        .collect();
    if gone_bottles.is_empty() && gone_pixies.is_empty() {
        return;
    }
    {
        let mut s = server();
        s.bottles.retain(|b| !gone_bottles.contains(&b.1));
        for (id, ..) in &gone_pixies {
            s.pixies.remove(id);
        }
    }
    for (_, at, dark) in gone_pixies {
        if !dark {
            pixie_sparkles(game, at, false, true);
        }
    }
}

// `native_hears_projectile_impacts`: while bottles fly or pixies live.
pub(super) fn hears_impacts(hears: &mut bool) {
    let s = server();
    if !s.bottles.is_empty() || !s.pixies.is_empty() {
        *hears = true;
    }
}

// `native_projectile_impact`: an Ender Air Bottle lands (`onHitBlock`, `onHitEntity`); a pixie
// takes nothing from its summoner's shot (`isInvulnerableTo`), which flies on.
#[allow(clippy::too_many_arguments)]
pub(super) fn projectile_impact(
    game: &mut Game<'_>,
    dimension: &str,
    projectile: u32,
    kind: &str,
    owner: u32,
    entity: Option<u32>,
    entity_kind: &str,
    block: [i32; 3],
    at: [f64; 3],
    canceled: &mut bool,
) {
    if *canceled || dimension != game.dim.id() || !game.native_authoritative() {
        return;
    }
    let dim = game.dim.id();
    if kind != rules::ENDER_AIR_BOTTLE {
        if entity_kind == rules::PIXIE
            && let Some(id) = entity
        {
            let s = server();
            let from_summoner = s.pixies.get(&id).is_some_and(|p| {
                !p.dark
                    && p.summoner
                        .is_some_and(|m| m.player && owner != 0 && m.id == owner)
            });
            if from_summoner {
                *canceled = true;
            }
        }
        return;
    }
    *canceled = true;
    server().bottles.retain(|b| b.1 != projectile);
    let splash = block_of(at);
    match entity {
        None => convert_blocks(game, dim, block, splash),
        Some(id) if rules::ghast_cries(entity_kind, dim) => {
            ghast_cries(game, dim, id, owner, splash);
        }
        // `BlockPos.containing(result.getLocation())`: an entity's hit is at its feet.
        Some(id) => {
            let feet = game
                .native_entity(id)
                .and_then(|e| game.native_entity_position(e))
                .unwrap_or(at);
            convert_blocks(game, dim, block_of(feet), splash);
        }
    }
    game.native_remove_entity(dim, projectile);
}

// `native_living_incoming_damage`, with no game: a player hurt by an entity is noted for their
// tick (`PixieHandler.onDamageTaken`), and a pixie takes nothing it is invulnerable to.
pub(super) fn incoming_damage(
    entity: i32,
    kind: &str,
    damage_type: &str,
    causing: i32,
    canceled: &mut bool,
) {
    if *canceled {
        return;
    }
    if kind == PLAYER {
        if causing > 0 && causing != entity {
            let mut s = server();
            if s.hurts.len() < MAX_HURTS {
                s.hurts.push(Hurt {
                    victim: entity,
                    causing,
                    waited: 0,
                });
            }
        }
        return;
    }
    if kind != rules::PIXIE {
        return;
    }
    let Ok(id) = u32::try_from(entity) else {
        return;
    };
    let s = server();
    let Some(p) = s.pixies.get(&id) else {
        return;
    };
    // A player's hit names no causing entity here: its damage type tells it.
    let from_player =
        s.players.contains(&causing) || causing == 0 && PLAYER_DAMAGE.contains(&damage_type);
    let from_summoner = match p.summoner {
        Some(m) if !m.player => causing > 0 && m.id == causing as u32,
        // A player summoner's own blows and shots are turned away where they start.
        Some(_) => false,
        None => causing == 0 && !from_player,
    };
    if rules::pixie_invulnerable(p.dark, from_summoner, from_player) {
        *canceled = true;
    }
}

// `native_attack_entity`: a summoner's blow on their own pixie does nothing (`isInvulnerableTo`).
pub(super) fn attack_entity(game: &mut Game<'_>, entity: i32, kind: &str, canceled: &mut bool) {
    if *canceled || kind != rules::PIXIE || !game.native_authoritative() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let Ok(id) = u32::try_from(entity) else {
        return;
    };
    let s = server();
    if s.pixies.get(&id).is_some_and(|p| {
        !p.dark
            && p.summoner
                .is_some_and(|m| m.player && m.id as i32 == player.entity_id)
    }) {
        *canceled = true;
    }
}

// `native_player_tick`: the acting player's hits since their last tick may each bring a pixie.
pub(super) fn player_tick(game: &mut Game<'_>) {
    if !game.native_authoritative() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let me = player.entity_id;
    let causing: Vec<i32> = {
        let mut s = server();
        s.players.insert(me);
        if s.hurts.is_empty() {
            return;
        }
        let (mine, rest): (Vec<Hurt>, Vec<Hurt>) = std::mem::take(&mut s.hurts)
            .into_iter()
            .partition(|h| h.victim == me);
        s.hurts = rest;
        mine.into_iter().map(|h| h.causing).collect()
    };
    for c in causing {
        player_hurt(game, me, player.position, c);
    }
}

// `native_entity_tick_listened`: while there are pixies to fly.
pub(super) fn tick_listened(listened: &mut bool) {
    let s = server();
    if !s.pixies.is_empty() || !s.strays.is_empty() {
        *listened = true;
    }
}

// `native_entity_tick_pre`: `PixieEntity.baseTick` and `customServerAiStep` for a pixie, before
// it moves: a stray or a pixie past 200 ticks is discarded; otherwise it heads for its target
// (or where the target was last seen), and strikes once within a block of its aim.
pub(super) fn tick_pre(
    game: &mut Game<'_>,
    dimension: &str,
    entity: u32,
    kind: &str,
    canceled: bool,
) {
    if kind != rules::PIXIE
        || canceled
        || dimension != game.dim.id()
        || !game.native_authoritative()
    {
        return;
    }
    let dim = game.dim.id();
    let Some(i) = game.mobs.iter().position(|m| m.id == entity) else {
        return;
    };
    let known = {
        let mut s = server();
        match s.pixies.get_mut(&entity) {
            Some(p) => {
                p.ticks += 1;
                Some((p.ticks, p.target, p.dark, p.aim))
            }
            None => None,
        }
    };
    let Some((ticks, target, dark, last_aim)) = known else {
        server().strays.retain(|s| s.1 != entity);
        discard_pixie(game, dim, entity, false);
        return;
    };
    patch_pixie(&mut game.mobs[i], true);
    let here = {
        let p = game.mobs[i].position;
        [p.x, p.y, p.z]
    };
    // `baseTick` runs for a dying pixie too: it sparkles until it is gone.
    pixie_sparkles(game, here, dark, false);
    if let Some(p) = server().pixies.get_mut(&entity) {
        p.at = here;
    }
    if !game.mobs[i].alive() {
        return;
    }
    let aim = target_aim(game, target).or(last_aim);
    let Some((feet, width, height)) = aim.filter(|_| !rules::pixie_gone(ticks, true)) else {
        server().pixies.remove(&entity);
        discard_pixie(game, dim, entity, dark);
        return;
    };
    let m = &mut game.mobs[i];
    let PixieStep {
        velocity,
        yaw,
        reached,
    } = rules::pixie_step(
        [m.position.x, m.position.y, m.position.z],
        feet,
        width,
        height,
        dark,
    );
    m.velocity = Vec3::new(velocity[0], velocity[1], velocity[2]);
    m.yaw = yaw;
    m.body_yaw = yaw;
    if let Some(p) = server().pixies.get_mut(&entity) {
        p.aim = Some((feet, width, height));
        p.strike |= reached;
    }
}

// `native_interact_entity`: a pixie has no trades; the click is taken so the trader's screen
// doesn't open.
pub(super) fn interact_entity(kind: &str, claimed: &mut bool) {
    if kind == rules::PIXIE {
        *claimed = true;
    }
}

// `native_tick`: on a server, hits waiting for their player's tick expire after two ticks; in
// the window, the pixies get their box.
pub(super) fn tick(game: &mut Game<'_>) {
    if game.native_window() {
        client_tick(game);
        return;
    }
    let mut s = server();
    if !s.hurts.is_empty() {
        for h in &mut s.hurts {
            h.waited += 1;
        }
        s.hurts.retain(|h| h.waited <= 2);
    }
}

// `native_mob_submit`: `PixieRenderer` with `PixieModel` (`entityCutoutNoCull`), its wings
// flapping by the age, tipping over as it dies and red while hurt, in October Botania's
// Halloween look.
pub(super) fn mob_submit(
    game: &Game<'_>,
    mob: &NativeMobRender<'_>,
    draw: &mut BlockEntityDraw,
    drawn: &mut bool,
) {
    if *drawn || mob.kind != rules::PIXIE {
        return;
    }
    let mut model = pixie_model().clone();
    for (name, turn) in rules::wing_turns(mob.age) {
        if let Some(part) = model.get_child_mut(name) {
            part.y_rot = turn;
        }
    }
    draw.push();
    // `LivingEntityRenderer.setupRotations`: the death roll about z, then the model's frame
    // (`scale(-1, -1, 1)` after the body's turn, which this frame has turned the other way).
    if mob.death_time > 0 {
        let f = ((mob.death_time as f32 + mob.partial - 1.0) / 20.0 * 1.6)
            .sqrt()
            .min(1.0);
        draw.rotate_about([0.0, 0.0, 1.0], -(f * 90.0), [0.0; 3]);
    }
    draw.scale([1.0, -1.0, -1.0]);
    draw.translate([0.0, -1.501, 0.0]);
    let start = draw.vertices_mut().len();
    let ok =
        game.native_draw_model_part(draw, &model, rules::pixie_texture(halloween()), 0xFF_FFFF)
            || game.native_draw_model_part(draw, &model, rules::pixie_texture(false), 0xFF_FFFF);
    if ok {
        *drawn = true;
        // `getOverlayCoords`: the hurt red while hurt or dying.
        if mob.hurt_time > 0 || mob.death_time > 0 {
            let red = horizon_client::overlay::living(0.0, true);
            for v in &mut draw.vertices_mut()[start..] {
                v.layer = red;
            }
        }
    }
    draw.pop();
}
