// Botania's bows, weapons and tools for Horizon Aurora: a modified port of Vazkii's Botania
// (Botania License, see LICENSE and NOTICE.md). The livingwood and crystal bows, the Terra
// Blade's beam, the Thundercaller's chain lightning, the Starcaller's falling stars, the Terra
// Shatterer's ranks and area mining, the elementium shovel and pickaxe, and the mana tools that
// pay their wear and mend with mana. The rules are in `botania_core::weapons`; this module wires
// them to the hooks. The beams and stars are virtual, as the spreaders' bursts are: the server
// moves them after each level tick and draws them with particles.

use super::Game;
use botania_core::burst::{self, Burst};
use botania_core::fx::Record;
use botania_core::mth;
use botania_core::weapons::{self, Bow, Link, Star, StarWorld};
use horizon_client::camera::{FovPlayer, UseItem};
use horizon_client::first_person::Hand;
use horizon_client::items::ItemStack;
use horizon_core::geometry::Vec3;
use horizon_core::random::RandomSource;
use horizon_world::consumable::UseAnimation;
use horizon_world::nbt::{Compound, JavaString, Tag};
use horizon_world::registry::BlockStateId;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};

// The `SoundSource` ordinal of `PLAYERS`.
const PLAYERS: usize = 7;
// `StarcallerItem.TAG_LAST_TRIGGER`.
const LAST_TRIGGER: &str = "lastTriggerTime";

#[derive(Default)]
struct State {
    // Main hand swings since each player's last tick (`native_player_arm_swing`).
    swings: HashSet<u128>,
    // When each player last attacked an entity (game time).
    attacked: HashMap<u128, i64>,
    // Each player's attack strength at the end of their last tick: what a swing had before it
    // started the strength again (`LeftClickPacket` reads it before the swing).
    strength: HashMap<u128, f32>,
    // Thundercaller hits for their player's tick: (player, entity hit).
    chains: Vec<(u128, u32)>,
    blades: Vec<Blade>,
    stars: Vec<Falling>,
    // An area break is running: the breaks it makes don't start another (`ToolCommons.recCall`).
    breaking: bool,
    // The module's own random numbers (`Math.random()` and the effects).
    rng: u64,
}

// A Terra Blade's beam in flight, and the entity id of the player who swung it.
struct Blade {
    dim: &'static str,
    burst: Burst,
    owner: u32,
    // Its trail's own part of the size (`botania_particles::wobble`).
    wobble: f32,
}

// A falling star in flight, and the entity id of the player who called it.
struct Falling {
    dim: &'static str,
    star: Star,
    owner: u32,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

// Never held across a call into the game: a game helper can come back into this module (a tool's
// wear while it breaks blocks), and the lock is not reentrant.
fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn random_bits() -> u64 {
    let mut s = state();
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

// `Math.random()`: 0 to 1.
fn random_unit() -> f64 {
    (random_bits() >> 11) as f64 / (1u64 << 53) as f64
}

// `Random.nextInt(n)`.
fn random_below(n: usize) -> usize {
    (random_bits() % n.max(1) as u64) as usize
}

// ---- mana -------------------------------------------------------------------------------------

// `ManaItemHandler.requestManaExactForTool`, from the mana items module (`botania_mana`): the
// player's mana tablets, rings and mirrors, with the armour's discount.
fn request_mana_exact_for_tool(
    game: &mut Game<'_>,
    player: u128,
    stack: &ItemStack,
    cost: i32,
    remove: bool,
) -> bool {
    super::botania_mana::request_mana_exact_for_tool(game, player, stack, cost, remove)
}

// ---- small things -----------------------------------------------------------------------------

fn aabb(b: horizon_core::geometry::Aabb) -> burst::Aabb {
    let (lo, hi) = (b.min_position(), b.max_position());
    [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z]
}

fn key(k: &str) -> JavaString {
    JavaString::from(k)
}

// A long in the stack's `minecraft:custom_data` (`ItemNBTHelper.getLong`), if it has one.
fn custom_long(stack: &ItemStack, k: &str) -> Option<i64> {
    match stack.custom_data()?.get(&key(k))? {
        Tag::Long(v) => Some(*v),
        Tag::Int(v) => Some(i64::from(*v)),
        Tag::Short(v) => Some(i64::from(*v)),
        Tag::Byte(v) => Some(i64::from(*v)),
        _ => None,
    }
}

fn custom_int(stack: &ItemStack, k: &str) -> i32 {
    match stack.custom_data().and_then(|c| c.get(&key(k))) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Short(v)) => i32::from(*v),
        Some(Tag::Byte(v)) => i32::from(*v),
        Some(Tag::Long(v)) => (*v).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        _ => 0,
    }
}

// Changes the stack's `minecraft:custom_data`, where Botania keeps its item NBT.
fn edit_custom(stack: &mut ItemStack, f: impl FnOnce(&mut Compound)) {
    let mut c = stack.custom_data().cloned().unwrap_or_default();
    f(&mut c);
    let value = (!c.is_empty()).then_some(Tag::Compound(c));
    stack.data_mut().set_other("minecraft:custom_data", value);
    stack.tidy();
}

// The item in the main hand.
fn main_item(game: &Game<'_>) -> Option<Arc<str>> {
    let stack = game.inventory.get(game.selected)?.as_ref()?;
    (stack.count > 0).then(|| stack.item.clone())
}

fn feet(game: &Game<'_>) -> Option<[f64; 3]> {
    let p = game.player.as_ref()?;
    Some([p.position.x, p.position.y, p.position.z])
}

// `Player.getAttackStrengthScale(0F)`.
fn attack_strength(game: &Game<'_>) -> f32 {
    let delay = horizon_client::attack_strength::delay(game.main_hand_attack_speed());
    (game.attack_ticks as f32 / delay).clamp(0.0, 1.0)
}

// ---- the bows -----------------------------------------------------------------------------------

// `BowItem.getUseDuration`: Botania's bows are bows, drawn until they are let go.
pub(super) fn use_duration(stack: &ItemStack, duration: &mut i32) {
    if Bow::of_item(&stack.item).is_some() {
        *duration = weapons::BOW_USE_DURATION;
    }
}

// `BowItem.getUseAnimation`: the drawing pose.
pub(super) fn use_animation(stack: &ItemStack, animation: &mut UseAnimation) {
    if Bow::of_item(&stack.item).is_some() {
        *animation = UseAnimation::Bow;
    }
}

fn arrow_cost(stack: &ItemStack) -> i32 {
    weapons::crystal_arrow_cost(stack.enchantment_level("minecraft:infinity") > 0)
}

// `BowItem.use` and `CrystalBowItem.use`: a survival player draws the livingwood bow with an
// arrow to shoot, the crystal bow with the mana for one (`canFire`). Both sides alike.
pub(super) fn use_start(
    game: &mut Game<'_>,
    stack: &ItemStack,
    entity: Option<i32>,
    canceled: &mut bool,
) {
    if *canceled || entity.is_some() || game.creative {
        return;
    }
    let Some(bow) = Bow::of_item(&stack.item) else {
        return;
    };
    let can = match bow {
        Bow::Livingwood => game.get_projectile(stack).is_some(),
        Bow::Crystal => {
            let player = game.native_acting_id();
            request_mana_exact_for_tool(game, player, stack, arrow_cost(stack), false)
        }
    };
    if !can {
        *canceled = true;
    }
}

// `releaseUsing`: the arrow, on the server.
pub(super) fn release(
    game: &mut Game<'_>,
    stack: &ItemStack,
    entity: Option<i32>,
    time_left: i32,
    used: &mut bool,
) {
    if entity.is_some() || !game.native_authoritative() {
        return;
    }
    let Some(bow) = Bow::of_item(&stack.item) else {
        return;
    };
    let hand = game.used_hand().unwrap_or(game.hand);
    let shot = game.with_hand(hand, |g| match bow {
        Bow::Livingwood => release_livingwood(g, stack, time_left),
        Bow::Crystal => release_crystal(g, stack, time_left),
    });
    if shot {
        *used = true;
    }
}

// `LivingwoodBowItem` is a `BowItem` (charge multiplier 1): the game's own bow, as `release_bow`
// shoots it (26.3's rules: Infinity and creative keep plain arrows, `arrow_loose` hears it).
fn release_livingwood(game: &mut Game<'_>, stack: &ItemStack, time_left: i32) -> bool {
    let ammo = game.get_projectile(stack);
    let mut charge = Bow::Livingwood.charge(time_left);
    let mut canceled = false;
    game.native_arrow_loose(stack, ammo.is_some(), &mut charge, &mut canceled);
    let power = super::item_use::bow_release_power(ammo.is_some(), charge, canceled);
    let (Some(power), Some(ammo)) = (power, ammo) else {
        return false;
    };
    let (bonus, infinity) = game.bow_enchantments(&ammo.stack.item);
    let free = game.creative || infinity;
    shoot(game, stack, &ammo.stack, power, bonus, free);
    if !free {
        use_ammo(game, ammo.slot);
    }
    game.damage_held(1);
    true
}

// `CrystalBowItem.releaseUsing`: twice the charge; with no arrow, one made of mana when the
// player has it; plain arrows (and any in creative) are never used up and can't be picked up;
// the mana is paid through the bow's wear (200 a point, 100 with Infinity).
fn release_crystal(game: &mut Game<'_>, stack: &ItemStack, time_left: i32) -> bool {
    let creative = game.creative;
    let player = game.native_acting_id();
    let can_materialize =
        creative || request_mana_exact_for_tool(game, player, stack, arrow_cost(stack), false);
    let ammo = game.get_projectile(stack);
    let charge = Bow::Crystal.charge(time_left);
    if charge < 0 {
        return false;
    }
    let (arrow, slot) = match ammo {
        Some(a) => (a.stack, a.slot),
        None if can_materialize => (ItemStack::new("minecraft:arrow", 1), None),
        None => return false,
    };
    let power = weapons::bow_power(charge);
    if f64::from(power) < 0.1 {
        return false;
    }
    let unpickable = creative || &*arrow.item == "minecraft:arrow";
    let (bonus, _) = game.bow_enchantments(&arrow.item);
    shoot(game, stack, &arrow, power, bonus, unpickable);
    if !unpickable {
        use_ammo(game, slot);
    }
    game.damage_held(1);
    true
}

// The arrow leaving the bow, as the game's `release_bow` makes it: from the eyes less 0.1 at
// three times the power, critical at full power, Power's damage, Punch, Flame, a tipped arrow's
// effect, its sound, and the bow's use statistic.
fn shoot(
    game: &mut Game<'_>,
    bow: &ItemStack,
    ammo: &ItemStack,
    power: f32,
    bonus: f64,
    intangible: bool,
) {
    let Some(eye) = game.eye_position() else {
        return;
    };
    let p = Vec3::new(eye[0], eye[1] - 0.1, eye[2]);
    let effect = super::dispenser::tipped_effect(ammo).or_else(|| {
        (&*ammo.item == "minecraft:spectral_arrow").then_some(("minecraft:glowing", 200))
    });
    let kind = horizon_client::projectiles::ProjectileKind::Arrow {
        base_damage: 2.0 + bonus,
        critical: power >= 1.0,
        effect,
    };
    let mut arrow = horizon_client::projectiles::Projectile::new(
        kind,
        p,
        Some(horizon_client::mobs::Who::Player),
    );
    if let Some(a) = arrow.arrow.as_mut() {
        a.set_item(&horizon_client::arrows::drawn_ammo(ammo, intangible));
        a.set_owner(Some(horizon_client::mobs::Who::Player));
        a.fired_from_weapon = Some(bow.clone());
    }
    game.ignite_spawned_arrow(bow, &mut arrow);
    let (velocity, on_ground) = game.player.as_ref().map_or((Vec3::ZERO, true), |pl| {
        (game.known_player_movement(), pl.ground.on_ground)
    });
    let (pitch, yaw) = (game.pitch, game.yaw);
    arrow.shoot_from_rotation(
        &mut game.world.random,
        pitch,
        yaw,
        0.0,
        power * 3.0,
        1.0,
        velocity,
        on_ground,
    );
    game.add_projectile(arrow);
    let player = horizon_client::lan::playerdata::ints_uuid(game.acting_player_uuid());
    let name = game.acting_player_name().to_owned();
    let projectile = game.next_entity;
    game.native_entity_shoot_bow(player, &name, projectile, power * 3.0);
    let pitch = 1.0 / (game.world.random.next_float() * 0.4 + 1.2) + power * 0.5;
    let at = feet(game).unwrap_or(eye);
    game.audio
        .play("minecraft:entity.arrow.shoot", Some(at), 1.0, pitch);
    game.award_stat("used", &bow.item, 1);
}

fn use_ammo(game: &mut Game<'_>, slot: Option<usize>) {
    let Some(i) = slot else {
        return;
    };
    let cell = game.stack_in_mut(i);
    if let Some(s) = cell.as_mut() {
        s.count -= 1;
        if s.count == 0 {
            *cell = None;
        }
    }
}

// The zoom of a drawn bow (`AbstractClientPlayer.getFieldOfViewModifier`, which zooms for any
// `BowItem`): the game gives it to `minecraft:bow` alone.
pub(super) fn fov(game: &Game<'_>, player: &FovPlayer, base: f32, modifier: &mut f32) {
    let Some((UseItem::Other, ticks)) = player.using else {
        return;
    };
    let Some(stack) = game.using.and_then(|(slot, _)| game.stack_in(slot)) else {
        return;
    };
    if Bow::of_item(&stack.item).is_none() || base == 0.0 {
        return;
    }
    let drawn = FovPlayer {
        using: Some((UseItem::Bow, ticks)),
        ..*player
    };
    let first_person = game.camera_type.first_person();
    let bow = horizon_client::camera::field_of_view_modifier(
        &drawn,
        first_person,
        game.fov_effect_scale(),
    );
    *modifier *= bow / base;
}

// Each tick of a carried stack: a Starcaller without a timer starts one (`lastTriggerTime`, the
// game time, in its own NBT), the Terra Shatterer spends its mana while its area is on. Mending
// with mana is the mana module's (`botania_mana`).
pub(super) fn inventory_tick(game: &mut Game<'_>, slot: i32) {
    let Some(item) = game.native_carried_stack(slot).map(|s| s.item.clone()) else {
        return;
    };
    let star = &*item == weapons::STAR_SWORD;
    if !star && &*item != weapons::TERRA_PICK {
        return;
    }
    if !game.native_authoritative() {
        return;
    }
    if game.native_acting_player().is_none() {
        return;
    }
    if star {
        let now = game.world.game_time;
        if let Some(stack) = game.native_carried_stack(slot)
            && custom_long(stack, LAST_TRIGGER).is_none()
        {
            edit_custom(stack, |c| {
                c.insert(key(LAST_TRIGGER), Tag::Long(now));
            });
        }
    }
    if &*item == weapons::TERRA_PICK {
        pick_tick(game, slot);
    }
}

// ---- melee ----------------------------------------------------------------------------------------

// The Terra Blade's beam: a burst from the eyes, the sword's wear, its sound.
fn spawn_blade(game: &mut Game<'_>, owner: u32) {
    let (Some(eye), Some(at)) = (game.eye_position(), feet(game)) else {
        return;
    };
    let burst = weapons::blade_burst(at[0], eye[1], at[2], game.yaw, game.pitch);
    let dim = game.dim.id();
    let wobble = super::botania_particles::wobble();
    state().blades.push(Blade {
        dim,
        burst,
        owner,
        wobble,
    });
    game.with_hand(Hand::Main, |g| g.damage_held(1));
    game.native_play_sound_in_place("botania:terrablade", at, PLAYERS, 1.0, 1.0);
}

// `TerraBladeItem.attackEntity`: an attack at full strength shoots the beam.
pub(super) fn attack_entity(game: &mut Game<'_>, canceled: bool) {
    if canceled || !game.native_authoritative() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let now = game.world.game_time;
    state().attacked.insert(player.uuid, now);
    if main_item(game).as_deref() == Some(weapons::TERRA_SWORD)
        && attack_strength(game) >= 1.0
        && !game.spectator()
    {
        spawn_blade(game, player.entity_id as u32);
    }
}

// A main hand swing (`ServerGamePacketListenerImpl.handlePunch`); outside the player's turn, so
// it is only noted for their tick.
pub(super) fn arm_swing(player: u128, hand: &str) {
    if hand == "mainhand" {
        state().swings.insert(player);
    }
}

// The swing met nothing in reach: Botania's beam leaves on a click at the air (the `MISS` branch
// of `Minecraft.startAttack`, `LeftClickPacket`), not on a block or an entity.
fn swung_at_nothing(game: &Game<'_>) -> bool {
    let Some(eye) = game.eye_position() else {
        return false;
    };
    let dir = super::view_vector(game.yaw, game.pitch);
    if game.clip_ray(eye, dir, game.block_reach).is_some() {
        return false;
    }
    let reach = game.entity_reach;
    !game
        .mobs
        .iter()
        .any(|m| !m.removed && weapons::ray_meets_box(eye, dir, reach, aabb(m.bounds())))
}

// `hurtEnemy`: the Thundercaller's hit, for the attacker's tick.
pub(super) fn hurt_enemy(game: &mut Game<'_>, stack: &ItemStack, entity: u32) {
    if &*stack.item != weapons::THUNDER_SWORD || !game.native_authoritative() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    state().chains.push((player.uuid, entity));
}

// `ThundercallerItem.hurtEnemy`: the lightning jumps from the entity hit to the enemies near it,
// and its bolts are drawn from the attacker's middle through each one struck.
fn thunder(game: &mut Game<'_>, attacker: u32, first: u32) {
    let Some(hit) = game
        .mobs
        .iter()
        .find(|m| m.id == first)
        .map(|m| [m.position.x, m.position.y, m.position.z])
    else {
        return;
    };
    let links: Vec<Link> = game
        .mobs
        .iter()
        .filter(|m| !m.removed)
        .map(|m| Link {
            id: m.id,
            pos: [m.position.x, m.position.y, m.position.z],
            bounds: aabb(m.bounds()),
            enemy: m.is_enemy(),
        })
        .collect();
    let hops = weapons::thunder_hops(game.native_weather().1 > 0.9);
    let struck = weapons::thunder_chain((first, hit), &links, hops, random_below);
    if struck.is_empty() {
        return;
    }
    let centres: Vec<[f64; 3]> = struck
        .iter()
        .filter_map(|(id, _)| links.iter().find(|l| l.id == *id))
        .map(|l| {
            let b = l.bounds;
            [
                (b[0] + b[3]) / 2.0,
                (b[1] + b[4]) / 2.0,
                (b[2] + b[5]) / 2.0,
            ]
        })
        .collect();
    let dim = game.dim.id();
    for (id, damage) in &struck {
        game.native_hurt_entity(dim, *id, *damage, "minecraft:player_attack", Some(attacker));
    }
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let mut from = [
        p.position.x,
        p.position.y + f64::from(p.dimensions.height) / 2.0,
        p.position.z,
    ];
    // `THUNDERCALLER_EFFECT`: Botania's bolt from one to the next (`botania_particles`).
    let mut fx = Vec::new();
    for to in centres {
        fx.push(Record::Bolt { from, to });
        from = to;
    }
    super::botania_particles::send(game, fx);
}

// `StarcallerItem.inventoryTick` on a swing: more than 12 ticks after the sword's last
// (`lastTriggerTime` in its NBT, now when it has none), a star falls on the block the player looks
// at within 48 blocks, two on some nights.
fn starcaller(game: &mut Game<'_>, owner: u32, now: i64) {
    {
        let Some(sword) = game.held_mut().as_mut() else {
            return;
        };
        let last = custom_long(sword, LAST_TRIGGER).unwrap_or(now);
        if !weapons::star_ready(now, last) {
            return;
        }
        edit_custom(sword, |c| {
            c.insert(key(LAST_TRIGGER), Tag::Long(now));
        });
    }
    let Some(eye) = game.eye_position() else {
        return;
    };
    let dir = super::view_vector(game.yaw, game.pitch);
    let Some(hit) = game.clip_ray(eye, dir, weapons::STAR_REACH) else {
        return;
    };
    let (pos, vel) = weapons::star_spawn(hit.pos, random_unit(), random_unit());
    let mut stars = vec![Star::new(pos, vel)];
    let raining = game.native_weather().0 > 0.2;
    let day_time = game.world.day_time;
    // Botania draws the one in eight only at night out of the rain.
    if weapons::bonus_star(raining, day_time, 0.0)
        && weapons::bonus_star(raining, day_time, random_unit())
    {
        let bonus = [
            vel[0] + random_unit() - 0.5,
            vel[1] + random_unit() - 0.5,
            vel[2] + random_unit() - 0.5,
        ];
        stars.push(Star::new(pos, bonus));
    }
    let dim = game.dim.id();
    {
        let mut s = state();
        for star in stars {
            s.stars.push(Falling { dim, star, owner });
        }
    }
    game.with_hand(Hand::Main, |g| g.damage_held(1));
    if let Some(at) = feet(game) {
        game.native_play_sound_in_place("botania:starcaller", at, PLAYERS, 1.0, 1.0);
    }
}

// The end of a player's tick: their Thundercaller hits, their swing's beam or star, and their
// attack strength for the next swing.
pub(super) fn player_tick(game: &mut Game<'_>) {
    if !game.native_authoritative() {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let (uuid, me) = (player.uuid, player.entity_id as u32);
    let now = game.world.game_time;
    let (swung, attacked, before, chains) = {
        let mut s = state();
        let swung = s.swings.remove(&uuid);
        let attacked = s
            .attacked
            .get(&uuid)
            .is_some_and(|&t| (0..=1).contains(&(now - t)));
        let before = s.strength.get(&uuid).copied().unwrap_or(0.0);
        let mut chains = Vec::new();
        if !s.chains.is_empty() {
            let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.chains)
                .into_iter()
                .partition(|c| c.0 == uuid);
            s.chains = rest;
            chains = mine.into_iter().map(|c| c.1).collect();
        }
        (swung, attacked, before, chains)
    };
    for first in chains {
        thunder(game, me, first);
    }
    if swung {
        match main_item(game).as_deref() {
            Some(weapons::TERRA_SWORD)
                if !attacked && before >= 1.0 && !game.spectator() && swung_at_nothing(game) =>
            {
                spawn_blade(game, me);
            }
            Some(weapons::STAR_SWORD) => game.with_hand(Hand::Main, |g| starcaller(g, me, now)),
            _ => {}
        }
    }
    let strength = attack_strength(game);
    state().strength.insert(uuid, strength);
}

// ---- the beams and stars in flight --------------------------------------------------------------

// What a star sees of the level in place.
struct StarBlocks<'a, 'g> {
    game: &'a Game<'g>,
}

impl StarBlocks<'_, '_> {
    fn fluid(&self, pos: [i32; 3]) -> horizon_client::blocks::Fluid {
        let s = self.game.world.block(pos[0], pos[1], pos[2]);
        self.game.res.facts().get(s).fluid
    }
}

impl StarWorld for StarBlocks<'_, '_> {
    fn boxes(&self, pos: [i32; 3]) -> Vec<burst::Aabb> {
        self.game.native_block_collision(pos)
    }
    fn air(&self, pos: [i32; 3]) -> bool {
        let s = self.game.world.block(pos[0], pos[1], pos[2]);
        self.game.res.behaviours().is_air(s)
    }
    fn water(&self, pos: [i32; 3]) -> bool {
        matches!(self.fluid(pos), horizon_client::blocks::Fluid::Water)
    }
    fn lava(&self, pos: [i32; 3]) -> bool {
        matches!(self.fluid(pos), horizon_client::blocks::Fluid::Lava)
    }
}

// After a level's tick: its beams and stars fly a tick.
pub(super) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    if !game.native_authoritative() {
        return;
    }
    let (mut blades, mut stars) = {
        let mut s = state();
        if s.blades.is_empty() && s.stars.is_empty() {
            return;
        }
        let (blades, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.blades)
            .into_iter()
            .partition(|b| b.dim == dimension);
        s.blades = rest;
        let (stars, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.stars)
            .into_iter()
            .partition(|f| f.dim == dimension);
        s.stars = rest;
        (blades, stars)
    };
    move_blades(game, dimension, &mut blades);
    move_stars(game, dimension, &mut stars);
    blades.retain(|b| b.burst.alive);
    stars.retain(|f| f.star.alive);
    let mut s = state();
    s.blades.extend(blades);
    s.stars.extend(stars);
}

// `ManaBurstEntity.tick` with `TerraBladeItem.updateBurst`: the beam flies, gives its mana to a
// pool or spreader it hits, and strikes the first living entity in its way that wasn't hurt
// lately, paying 33 of its mana for 8 damage.
fn move_blades(game: &mut Game<'_>, dimension: &str, blades: &mut [Blade]) {
    if blades.is_empty() {
        return;
    }
    let mut mobs: Vec<(u32, burst::Aabb, i32)> = game
        .mobs
        .iter()
        .filter(|m| !m.removed)
        .map(|m| (m.id, aabb(m.bounds()), m.hurt_time))
        .collect();
    let mut strikes: Vec<(u32, u32)> = Vec::new();
    let mut deliveries = Vec::new();
    let mut fx = Vec::new();
    {
        let world = super::botania::GameBursts { game: &*game };
        for b in blades.iter_mut() {
            let p = b.burst.pos;
            let v = b.burst.vel;
            let next = mth::block_of([p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
            if !game.world.loaded(next[0], next[2]) {
                b.burst.alive = false;
                continue;
            }
            let owner = b.owner;
            let r = b.burst.tick_with(&world, |beam, old| {
                let reach = weapons::blade_reach(beam.pos, old);
                for (id, bounds, hurt) in mobs.iter_mut() {
                    if *id == owner || *hurt != 0 || !weapons::intersects(*bounds, reach) {
                        continue;
                    }
                    if let Some(left) = weapons::blade_strike(beam.mana) {
                        beam.mana = left;
                        beam.alive = false;
                        *hurt = 10;
                        strikes.push((*id, owner));
                        break;
                    }
                }
            });
            if let Some(d) = r.delivered {
                deliveries.push(d);
            }
            // A mana burst's particles (`ManaBurstEntity.particles`, its death): the trail along
            // this tick's flight, as big as the mana left.
            let size = b.burst.mana.max(0) as f32 / b.burst.starting_mana.max(1) as f32;
            if b.burst.alive {
                fx.push(Record::Burst {
                    pos: b.burst.pos,
                    prev: p,
                    motion: v.map(|c| c as f32),
                    color: weapons::BLADE_COLOR,
                    size,
                    wobble: b.wobble,
                });
            } else {
                fx.push(Record::BurstDeath {
                    pos: b.burst.pos,
                    color: weapons::BLADE_COLOR,
                    size,
                });
            }
        }
    }
    for (at, mana) in deliveries {
        super::botania::receive_mana(game, at, mana);
    }
    for (target, owner) in strikes {
        game.native_hurt_entity(
            dimension,
            target,
            weapons::BLADE_DAMAGE,
            "minecraft:player_attack",
            Some(owner),
        );
    }
    super::botania_particles::send(game, fx);
}

// `FallingStarEntity.tick`: the star falls; on the ground (once it has been in the air) it breaks
// apart; the first living entity within 2 blocks of its flight that wasn't hurt lately takes 5
// or 10 (a villager is passed over, the star flies on); after 200 ticks it is gone.
fn move_stars(game: &mut Game<'_>, dimension: &str, stars: &mut [Falling]) {
    if stars.is_empty() {
        return;
    }
    let mut mobs: Vec<(u32, burst::Aabb, i32, bool, bool)> = game
        .mobs
        .iter()
        .filter(|m| !m.removed)
        .map(|m| {
            let villager = m.kind.id == "minecraft:villager";
            (m.id, aabb(m.bounds()), m.hurt_time, villager, m.alive())
        })
        .collect();
    let mut hurts: Vec<(u32, f32, u32)> = Vec::new();
    let mut broken = Vec::new();
    let mut fx = Vec::new();
    {
        let world = StarBlocks { game: &*game };
        for f in stars.iter_mut() {
            let p = f.star.pos;
            let v = f.star.vel;
            let next = mth::block_of([p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
            if !game.world.loaded(next[0], next[2]) {
                f.star.alive = false;
                continue;
            }
            let step = f.star.step(&world);
            if let Some(at) = step.broke {
                broken.push(at);
            }
            fx.push(Record::Star { pos: f.star.pos });
            let reach = weapons::star_reach(f.star.pos, step.old);
            let mut met = false;
            for (id, bounds, hurt, villager, alive) in mobs.iter_mut() {
                if *hurt != 0 || !weapons::intersects(*bounds, reach) {
                    continue;
                }
                met = true;
                if !*villager {
                    if *alive {
                        hurts.push((*id, weapons::star_damage(random_unit()), f.owner));
                        *hurt = 10;
                    }
                    f.star.alive = false;
                }
                break;
            }
            if !met && f.star.expired() {
                f.star.alive = false;
            }
        }
    }
    for at in broken {
        let s = game.world.block(at[0], at[1], at[2]);
        game.level_event_at(2001, at, s.0 as i32);
    }
    for (target, damage, owner) in hurts {
        game.native_hurt_entity(
            dimension,
            target,
            damage,
            "minecraft:player_attack",
            Some(owner),
        );
    }
    super::botania_particles::send(game, fx);
}

// ---- the Terra Shatterer ------------------------------------------------------------------------

// What the Terra Shatterer keeps in its `minecraft:custom_data`, under Botania's NBT names.
struct PickData {
    mana: i32,
    enabled: bool,
    tipped: bool,
}

fn pick_data(stack: &ItemStack) -> PickData {
    PickData {
        mana: custom_int(stack, "mana"),
        enabled: custom_int(stack, "enabled") != 0,
        tipped: custom_int(stack, "tipped") != 0,
    }
}

// `TerraShattererItem.inventoryTick`: an area turned on without a rank turns off; else, while the
// player isn't swinging, it spends its rank in mana each tick.
fn pick_tick(game: &mut Game<'_>, slot: i32) {
    let swinging = game.swing.current.is_some();
    let Some(stack) = game.native_carried_stack(slot) else {
        return;
    };
    let data = pick_data(stack);
    if !data.enabled {
        return;
    }
    let level = weapons::pick_level(data.mana);
    if level == 0 {
        edit_custom(stack, |c| {
            c.insert(key("enabled"), Tag::Byte(0));
        });
    } else if !swinging {
        let mana = data.mana.saturating_sub(level);
        edit_custom(stack, |c| {
            if mana > 0 {
                c.insert(key("mana"), Tag::Int(mana));
            } else {
                c.remove(&key("mana"));
            }
        });
    }
}

// `TerraShattererItem.use`: a sneaking right click with a ranked Terra Shatterer turns its area
// on or off (in the main hand with something in the other, not while it looks at a block).
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if *claimed || &*stack.item != weapons::TERRA_PICK || !game.native_authoritative() {
        return;
    }
    let data = pick_data(stack);
    if !game.native_sneaking() || weapons::pick_level(data.mana) == 0 {
        return;
    }
    let other_hand = game.offhand.as_ref().is_some_and(|s| s.count > 0);
    if game.hand == Hand::Main && other_hand {
        // `getPlayerPOVHitResult`: 5 blocks in 1.20.1.
        let dir = super::view_vector(game.yaw, game.pitch);
        if let Some(eye) = game.eye_position()
            && game.clip_ray(eye, dir, 5.0).is_some()
        {
            return;
        }
    }
    let Some(held) = game.held_mut().as_mut() else {
        return;
    };
    if &*held.item != weapons::TERRA_PICK {
        return;
    }
    edit_custom(held, |c| {
        c.insert(key("enabled"), Tag::Byte(i8::from(!data.enabled)));
    });
    if let Some(at) = feet(game) {
        game.native_play_sound_in_place("botania:terra_pick_mode", at, PLAYERS, 1.0, 1.0);
    }
    *claimed = true;
}

// `breakOtherBlock`'s `canMine`.
fn pick_can_mine(game: &Game<'_>, tool: &ItemStack, s: BlockStateId) -> bool {
    let facts = game.res.facts().get(s);
    let name = &*facts.name;
    let items = game.items();
    let shovel_or_hoe = items.block_in_tag("minecraft:mineable/shovel", name)
        || items.block_in_tag("minecraft:mineable/hoe", name);
    weapons::pick_can_mine(
        facts.requires_tool,
        items.correct_for_drops(Some(tool), name),
        items.mining_speed(Some(tool), name),
        shovel_or_hoe,
    )
}

// `ToolCommons.removeBlocksInIteration`: breaks `targets` as the player, each one loaded, not
// air, breakable (`getDestroyProgress` above 0) and taken by `filter`, with the drops and wear
// of the held tool (`removeBlockWithDrops`, `ServerPlayerGameMode.destroyBlock`).
fn break_area(
    game: &mut Game<'_>,
    targets: &[[i32; 3]],
    filter: impl Fn(&Game<'_>, BlockStateId) -> bool,
) {
    let Some(player) = game.native_acting_player() else {
        return;
    };
    state().breaking = true;
    for &p in targets {
        if !game.world.loaded(p[0], p[2]) {
            continue;
        }
        let s = game.world.block(p[0], p[1], p[2]);
        if game.res.behaviours().is_air(s)
            || game.res.facts().get(s).hardness < 0.0
            || !filter(game, s)
        {
            continue;
        }
        super::botania_tools::seen_breaking(game, player.uuid, p, s);
        game.native_player_break_block(player.uuid, p, false);
    }
    state().breaking = false;
}

// `TerraShattererItem.onBlockStartBreak`: the face the player looks at within 10 blocks, then
// `breakOtherBlock` on it.
fn pick_break(game: &mut Game<'_>, tool: &ItemStack, pos: [i32; 3]) {
    let Some(eye) = game.eye_position() else {
        return;
    };
    let dir = super::view_vector(game.yaw, game.pitch);
    let Some(hit) = game.clip_ray(eye, dir, weapons::PICK_REACH) else {
        return;
    };
    pick_break_other(game, tool, pos, hit.face);
}

// `TerraShattererItem.breakOtherBlock`: with its area on, the blocks around the one at `pos` (not
// yet broken), on `face`, as wide as its rank, which the rings change (`botania_tools::area_rings`:
// Thor every axis and a rank more, Temperance rank B at most). The Ring of Loki's cursors break
// theirs here too.
pub(super) fn pick_break_other(
    game: &mut Game<'_>,
    tool: &ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
) {
    let data = pick_data(tool);
    if !data.enabled {
        return;
    }
    let block_state = game.world.block(pos[0], pos[1], pos[2]);
    if !pick_can_mine(game, tool, block_state) || game.res.behaviours().is_air(block_state) {
        return;
    }
    let rings = super::botania_tools::area_rings(game);
    let level = weapons::pick_level(data.mana);
    let Some((begin, end)) = botania_core::tools::pick_area_with(level, face, rings) else {
        return;
    };
    let targets = weapons::between_closed(pos, begin, end);
    break_area(game, &targets, |g, s| pick_can_mine(g, tool, s));
    super::botania_tool_uses::rank_ss_pick(game, level);
}

// `ElementiumShovelItem.onBlockStartBreak`: a falling block it mines fast takes the same blocks
// above and below with it.
fn shovel_break(game: &mut Game<'_>, tool: &ItemStack, pos: [i32; 3], block_state: BlockStateId) {
    let name = game.res.facts().get(block_state).name.clone();
    if game.items().mining_speed(Some(tool), &name) <= 1.0 || !weapons::is_falling_block(&name) {
        return;
    }
    let (begin, end) = weapons::SHOVEL_AREA;
    let targets = weapons::between_closed(pos, begin, end);
    break_area(game, &targets, |g, s| g.res.facts().get(s).name == name);
}

// A player breaks a block: the Terra Shatterer's area, the elementium shovel's column.
pub(super) fn destroy_block(
    game: &mut Game<'_>,
    pos: [i32; 3],
    block_state: BlockStateId,
    canceled: bool,
) {
    if canceled || !game.native_authoritative() {
        return;
    }
    let Some(Some(held)) = game.inventory.get(game.selected) else {
        return;
    };
    let (pick, shovel) = (
        &*held.item == weapons::TERRA_PICK,
        &*held.item == weapons::ELEMENTIUM_SHOVEL,
    );
    if !pick && !shovel {
        return;
    }
    let tool = held.clone();
    if state().breaking {
        return;
    }
    if pick {
        pick_break(game, &tool, pos);
    } else {
        shovel_break(game, &tool, pos, block_state);
    }
}

// `ElementiumPickaxeItem.shouldFilterOut`: the elementium pickaxe (and a tipped Terra Shatterer)
// leaves the junk a block drops: always the disposable blocks, the semi-disposable ones unless
// the player sneaks.
pub(super) fn loot_drops(
    game: &mut Game<'_>,
    context: &horizon_client::mod_api::NativeLootDrops<'_>,
    drops: &mut Vec<ItemStack>,
) {
    if context.block.is_none() || drops.is_empty() {
        return;
    }
    let Some(tool) = context.tool else {
        return;
    };
    let tipped = &*tool.item == weapons::TERRA_PICK && pick_data(tool).tipped;
    if !weapons::filters_drops(&tool.item, tipped) {
        return;
    }
    let sneaking = context.player.is_some_and(|p| {
        game.native_acting_player().is_some_and(|a| a.uuid == p) && game.native_sneaking()
    });
    let items = game.items();
    drops.retain(|d| {
        d.count == 0
            || !weapons::filter_out(
                items.item_in_tag("botania:disposable", &d.item),
                items.item_in_tag("botania:semi_disposable", &d.item),
                sneaking,
            )
    });
}
