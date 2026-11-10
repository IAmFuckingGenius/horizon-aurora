//! Botania's bows, weapons and tools, without the game: the livingwood and crystal bows
//! (`LivingwoodBowItem`, `CrystalBowItem`), the Terra Blade's burst (`TerraBladeItem`), the
//! Thundercaller's chain (`ThundercallerItem`), the Starcaller's falling stars (`StarcallerItem`,
//! `FallingStarEntity` on `ThrowableCopyEntity`), the Terra Shatterer's ranks and area
//! (`TerraShattererItem`, `ToolCommons.removeBlocksInIteration`), the elementium shovel and
//! pickaxe, and what the mana tools pay in mana (`getManaPerDamage` and the mending in their
//! `inventoryTick`).

use crate::burst::{self, Aabb, Burst, BurstWorld};
use crate::mth;

pub const LIVINGWOOD_BOW: &str = "botania:livingwood_bow";
pub const CRYSTAL_BOW: &str = "botania:crystal_bow";
pub const TERRA_SWORD: &str = "botania:terra_sword";
pub const THUNDER_SWORD: &str = "botania:thunder_sword";
pub const STAR_SWORD: &str = "botania:star_sword";
pub const TERRA_PICK: &str = "botania:terra_pick";
pub const ELEMENTIUM_PICK: &str = "botania:elementium_pickaxe";
pub const ELEMENTIUM_SHOVEL: &str = "botania:elementium_shovel";

// ---- the bows -------------------------------------------------------------------------------

/// `BowItem.getUseDuration`: a bow is drawn until it is let go.
pub const BOW_USE_DURATION: i32 = 72000;

/// A Botania bow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bow {
    Livingwood,
    Crystal,
}

impl Bow {
    pub fn of_item(item: &str) -> Option<Bow> {
        match item {
            LIVINGWOOD_BOW => Some(Bow::Livingwood),
            CRYSTAL_BOW => Some(Bow::Crystal),
            _ => None,
        }
    }

    /// `chargeVelocityMultiplier`: the crystal bow draws twice as fast.
    pub fn charge_multiplier(self) -> f32 {
        match self {
            Bow::Livingwood => 1.0,
            Bow::Crystal => 2.0,
        }
    }

    /// `(int) ((getUseDuration(stack) - timeLeft) * chargeVelocityMultiplier())`: the charge a
    /// release with `time_left` ticks of the use left shoots with. Below 0 shoots nothing.
    pub fn charge(self, time_left: i32) -> i32 {
        ((BOW_USE_DURATION - time_left) as f32 * self.charge_multiplier()) as i32
    }
}

/// `BowItem.getPowerForTime`: an arrow leaves at three times this, and only from 0.1.
pub fn bow_power(charge: i32) -> f32 {
    let f = charge as f32 / 20.0;
    ((f * f + f * 2.0) / 3.0).min(1.0)
}

/// `CrystalBowItem.ARROW_COST`, halved with Infinity: the mana an arrow made of mana costs, which
/// is also the crystal bow's mana for a point of wear.
pub fn crystal_arrow_cost(infinity: bool) -> i32 {
    200 / if infinity { 2 } else { 1 }
}

// ---- mana for wear ----------------------------------------------------------------------------

/// `getManaPerDamage` (`CustomDamageItem.damageItem`): the mana that pays for a point of a tool's
/// wear instead of its durability.
pub fn mana_per_damage(item: &str, infinity: bool) -> Option<i32> {
    Some(match item {
        "botania:manasteel_sword"
        | "botania:manasteel_pick"
        | "botania:manasteel_axe"
        | "botania:manasteel_shovel"
        | "botania:manasteel_hoe"
        | "botania:elementium_sword"
        | ELEMENTIUM_PICK
        | "botania:elementium_axe"
        | ELEMENTIUM_SHOVEL
        | "botania:elementium_hoe"
        | THUNDER_SWORD => 60,
        "botania:manasteel_shears" | "botania:elementium_shears" => 30,
        TERRA_SWORD | TERRA_PICK | "botania:terra_axe" => 100,
        STAR_SWORD => 120,
        "botania:glass_pickaxe" => 160,
        LIVINGWOOD_BOW => 40,
        CRYSTAL_BOW => crystal_arrow_cost(infinity),
        _ => return None,
    })
}

/// The mana `inventoryTick` spends to mend a point of wear each tick: twice the mana per damage,
/// except the pickaxes (`ManasteelPickaxeItem`'s own 60 twice, for all of them) and the bows
/// (`LivingwoodBowItem`'s 40 twice, the crystal bow's too).
pub fn repair_cost(item: &str) -> Option<i32> {
    match item {
        "botania:manasteel_pick" | ELEMENTIUM_PICK | TERRA_PICK | "botania:glass_pickaxe" => {
            Some(120)
        }
        LIVINGWOOD_BOW | CRYSTAL_BOW => Some(80),
        _ => mana_per_damage(item, false).map(|m| m * 2),
    }
}

// ---- boxes ------------------------------------------------------------------------------------

/// `new AABB(a, b).inflate(grow)`: the box between two points, grown on every side.
pub fn swept_box(a: [f64; 3], b: [f64; 3], grow: f64) -> Aabb {
    [
        a[0].min(b[0]) - grow,
        a[1].min(b[1]) - grow,
        a[2].min(b[2]) - grow,
        a[0].max(b[0]) + grow,
        a[1].max(b[1]) + grow,
        a[2].max(b[2]) + grow,
    ]
}

/// `AABB.intersects`: the boxes overlap (touching faces don't).
pub fn intersects(a: Aabb, b: Aabb) -> bool {
    (0..3).all(|i| a[i] < b[i + 3] && a[i + 3] > b[i])
}

/// Whether the ray from `from` along the unit `dir` meets the box within `reach`, a start inside
/// it included (`ProjectileUtil.getEntityHitResult`: `contains`, then `AABB.clip`).
pub fn ray_meets_box(from: [f64; 3], dir: [f64; 3], reach: f64, b: Aabb) -> bool {
    let (mut near, mut far) = (0.0_f64, reach);
    for i in 0..3 {
        let (lo, hi) = (b[i], b[i + 3]);
        if dir[i].abs() < 1.0e-12 {
            if from[i] < lo || from[i] > hi {
                return false;
            }
            continue;
        }
        let (a, c) = ((lo - from[i]) / dir[i], (hi - from[i]) / dir[i]);
        near = near.max(a.min(c));
        far = far.min(a.max(c));
        if near > far {
            return false;
        }
    }
    true
}

// ---- the Terra Blade --------------------------------------------------------------------------

/// `ManaBurst.NO_SOURCE`: a burst no spreader shot.
pub const NO_SOURCE: [i32; 3] = [0, i32::MIN, 0];
/// The Terra Blade's burst: `MANA_PER_DAMAGE` of mana, green.
pub const BLADE_MANA: i32 = 100;
pub const BLADE_COLOR: u32 = 0x20FF20;
/// `MANA_PER_DAMAGE / 3`: what a strike takes of the burst's mana.
pub const BLADE_STRIKE_COST: i32 = BLADE_MANA / 3;
/// `4F + getTerrasteelItemTier().getAttackDamageBonus()` (4).
pub const BLADE_DAMAGE: f32 = 8.0;

/// `TerraBladeItem.getBurst`: the burst `ManaBurstEntity(Player)` makes at the player's eyes less
/// `0.1F` (`ThrowableProjectile(type, owner, level)`), turned with `setRot(yRot + 180, -xRot)`,
/// seven times as fast, with 100 mana that starts to fade after 40 ticks by 4 a tick, and no
/// gravity.
pub fn blade_burst(x: f64, eye_y: f64, z: f64, yaw: f32, pitch: f32) -> Burst {
    let (y_rot, x_rot) = ((yaw + 180.0) % 360.0, (-pitch) % 360.0);
    let v = burst::velocity(x_rot, y_rot);
    Burst {
        pos: [x, eye_y - f64::from(0.1_f32), z],
        vel: [v[0] * 7.0, v[1] * 7.0, v[2] * 7.0],
        mana: BLADE_MANA,
        starting_mana: BLADE_MANA,
        min_mana_loss: 40,
        mana_loss_per_tick: 4.0,
        accumulated_loss: 0.0,
        ticks_existed: 0,
        source: NO_SOURCE,
        left_source: false,
        last_collision: None,
        alive: true,
        fake: false,
        collided: None,
    }
}

/// `TerraBladeItem.updateBurst`: the box a blade burst strikes in, from where it was to where it
/// is, grown by a block.
pub fn blade_reach(pos: [f64; 3], old: [f64; 3]) -> Aabb {
    swept_box(pos, old, 1.0)
}

/// A strike on a living entity not hurt lately: the burst's mana after paying for it, when it has
/// enough (and then it hurts for `BLADE_DAMAGE` and is gone).
pub fn blade_strike(mana: i32) -> Option<i32> {
    (mana >= BLADE_STRIKE_COST).then_some(mana - BLADE_STRIKE_COST)
}

// ---- the Thundercaller ------------------------------------------------------------------------

/// How far from the last one struck the lightning looks for the next.
pub const THUNDER_RANGE: f64 = 8.0;
/// The lightning's colours (`THUNDERCALLER_EFFECT`): outer and inner.
pub const THUNDER_OUTER: u32 = 0xAADFFF;
pub const THUNDER_INNER: u32 = 0x0179C4;

/// Jumps of the lightning: 10 while it thunders (`Level.isThundering`), else 4.
pub fn thunder_hops(thundering: bool) -> usize {
    if thundering { 10 } else { 4 }
}

/// A living entity the lightning may jump to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Link {
    pub id: u32,
    /// Where it stands (`getX`, `getY`, `getZ`).
    pub pos: [f64; 3],
    pub bounds: Aabb,
    /// `instanceof Enemy` and not a player: the lightning's selector.
    pub enemy: bool,
}

/// `ThundercallerItem.hurtEnemy`: from `first` (the entity hit, and where it stands), up to `hops`
/// jumps, each to an enemy picked at random (`pick(n)` for `nextInt(n)`) among those whose box
/// meets the box 8 blocks around the last one struck, but not that one nor one already struck
/// (the entity hit first isn't kept out after the first jump). Damage starts at `hops + 1` and
/// falls by one a jump. The struck, in order, with their damage.
pub fn thunder_chain(
    first: (u32, [f64; 3]),
    entities: &[Link],
    hops: usize,
    mut pick: impl FnMut(usize) -> usize,
) -> Vec<(u32, f32)> {
    let mut struck: Vec<(u32, f32)> = Vec::new();
    let (mut last, mut at) = first;
    let mut damage = hops as i32 + 1;
    for _ in 0..hops {
        let r = THUNDER_RANGE;
        let area = [
            at[0] - r,
            at[1] - r,
            at[2] - r,
            at[0] + r,
            at[1] + r,
            at[2] + r,
        ];
        let near: Vec<&Link> = entities
            .iter()
            .filter(|e| {
                e.id != last
                    && e.enemy
                    && !struck.iter().any(|s| s.0 == e.id)
                    && intersects(e.bounds, area)
            })
            .collect();
        if near.is_empty() {
            break;
        }
        let target = near[pick(near.len()).min(near.len() - 1)];
        struck.push((target.id, damage as f32));
        (last, at) = (target.id, target.pos);
        damage -= 1;
    }
    struck
}

// ---- the Starcaller ---------------------------------------------------------------------------

/// `StarcallerItem.INTERVAL`: ticks between two stars (more than).
pub const STAR_INTERVAL: i64 = 12;
/// How far the player's look reaches for the block a star falls on.
pub const STAR_REACH: f64 = 48.0;
/// `ThrowableCopyEntity.getGravity`.
pub const STAR_GRAVITY: f64 = 0.03_f32 as f64;
/// The star's sparkles (`SparkleParticleData.sparkle(2F, 1F, 0.4F, 1F, 6)`).
pub const STAR_RGB: [f32; 3] = [1.0, 0.4, 1.0];

/// Whether a swing summons a star: more than 12 ticks since the last one.
pub fn star_ready(now: i64, last: i64) -> bool {
    now - last > STAR_INTERVAL
}

/// `StarcallerItem.summonFallingStar`: where a star starts and how it flies, from the block the
/// player looks at and two `Math.random()` draws: up to 4.5 blocks aside and 24 up, falling
/// straight at the block's corner at 1.5 blocks a tick.
pub fn star_spawn(block: [i32; 3], rx: f64, rz: f64) -> ([f64; 3], [f64; 3]) {
    let mot = [(0.5 * rx - 0.25) * 18.0, 24.0, (0.5 * rz - 0.25) * 18.0];
    let pos = [
        f64::from(block[0]) + mot[0],
        f64::from(block[1]) + mot[1],
        f64::from(block[2]) + mot[2],
    ];
    // `normalize().reverse().scale(1.5)`.
    let len = (mot[0] * mot[0] + mot[1] * mot[1] + mot[2] * mot[2]).sqrt();
    let vel = mot.map(|v| -(v / len) * 1.5);
    (pos, vel)
}

/// A second star with the first, at night (only the world's first: Botania reads the day time as
/// it counts past 24000) out of the rain, one time in eight.
pub fn bonus_star(raining: bool, day_time: i64, r: f64) -> bool {
    !raining && (day_time - 18000).abs() < 1800 && r < 0.125
}

/// `FallingStarEntity.onHitEntity`: 10 one time in four, else 5.
pub fn star_damage(r: f64) -> f32 {
    if r < 0.25 { 10.0 } else { 5.0 }
}

/// The box a star strikes in: from where it was to where it is, grown by 2.
pub fn star_reach(pos: [f64; 3], old: [f64; 3]) -> Aabb {
    swept_box(pos, old, 2.0)
}

/// What a falling star needs of the level.
pub trait StarWorld {
    /// The collision boxes at `pos`, relative to the block (`ClipContext.Block.COLLIDER`).
    fn boxes(&self, pos: [i32; 3]) -> Vec<Aabb>;
    fn air(&self, pos: [i32; 3]) -> bool;
    fn water(&self, pos: [i32; 3]) -> bool;
    fn lava(&self, pos: [i32; 3]) -> bool;
}

struct Solid<'a, W>(&'a W);

impl<W: StarWorld> BurstWorld for Solid<'_, W> {
    fn boxes(&self, pos: [i32; 3]) -> Vec<Aabb> {
        self.0.boxes(pos)
    }
    fn passes(&self, _pos: [i32; 3]) -> bool {
        false
    }
    fn receiver(&self, _pos: [i32; 3]) -> bool {
        false
    }
}

/// A falling star (`FallingStarEntity`).
#[derive(Clone, Debug, PartialEq)]
pub struct Star {
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    /// `tickCount`, which the level counts up before each tick.
    pub age: i32,
    /// `hasBeenInAir`: until then a block it hits doesn't stop it.
    pub been_in_air: bool,
    pub alive: bool,
}

/// What a star's move did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StarStep {
    /// Where it was (`xOld`, `yOld`, `zOld`).
    pub old: [f64; 3],
    /// The block it hit and died on, unless that was air (its break particles,
    /// `LevelEvent.PARTICLES_DESTROY_BLOCK`).
    pub broke: Option<[i32; 3]>,
}

impl Star {
    pub fn new(pos: [f64; 3], vel: [f64; 3]) -> Star {
        Star {
            pos,
            vel,
            age: 0,
            been_in_air: false,
            alive: true,
        }
    }

    /// `FallingStarEntity.tick` up to its look for entities: `ThrowableCopyEntity.tick` casts the
    /// ray to where it moves (a block hit kills a star that has been in the air), moves it, slows
    /// it (0.99, 0.8 in water) and pulls it down by 0.03; then whether its feet are in air, water
    /// or lava.
    pub fn step(&mut self, world: &impl StarWorld) -> StarStep {
        self.age += 1;
        let old = self.pos;
        let here = mth::block_of(self.pos);
        // `isInWater`/`isInLava` as `baseTick` left them, before the move.
        let (in_water, in_lava) = (world.water(here), world.lava(here));
        let to = [
            self.pos[0] + self.vel[0],
            self.pos[1] + self.vel[1],
            self.pos[2] + self.vel[2],
        ];
        let mut broke = None;
        if let Some(hit) = burst::clip(&Solid(world), self.pos, to)
            && self.been_in_air
        {
            if !world.air(hit.pos) {
                broke = Some(hit.pos);
            }
            self.alive = false;
        }
        let drag = f64::from(if in_water { 0.8_f32 } else { 0.99_f32 });
        self.vel = [
            self.vel[0] * drag,
            self.vel[1] * drag - STAR_GRAVITY,
            self.vel[2] * drag,
        ];
        self.pos = to;
        if !self.been_in_air {
            self.been_in_air = world.air(mth::block_of(to)) || in_water || in_lava;
        }
        StarStep { old, broke }
    }

    /// `tickCount > 200`: it is gone at the end of this tick.
    pub fn expired(&self) -> bool {
        self.age > 200
    }
}

// ---- the Terra Shatterer ----------------------------------------------------------------------

/// `TerraShattererItem.LEVELS`: the mana each rank starts at (D, C, B, A, S, SS).
pub const PICK_LEVELS: [i32; 6] = [0, 10000, 1000000, 10000000, 100000000, 1000000000];
/// How far the player's look reaches for the face the area spreads from.
pub const PICK_REACH: f64 = 10.0;

/// `TerraShattererItem.getLevel`: the highest rank its mana reaches.
pub fn pick_level(mana: i32) -> i32 {
    (1..PICK_LEVELS.len())
        .rev()
        .find(|&i| mana >= PICK_LEVELS[i])
        .unwrap_or(0) as i32
}

/// `TerraShattererItem.breakOtherBlock`'s `canMine`: a tool right for the drops (or a block that
/// needs none), and fast on it, or a block a shovel or hoe mines.
pub fn pick_can_mine(
    requires_tool: bool,
    correct_for_drops: bool,
    speed: f32,
    shovel_or_hoe: bool,
) -> bool {
    (!requires_tool || correct_for_drops) && (speed > 1.0 || shovel_or_hoe)
}

/// `TerraShattererItem.breakOtherBlock`: the corners of the area around the block mined at rank
/// `level`, on the `face` the player looks at (a normal): `level - 1` blocks to each side along
/// the face, from one below up to `2 * max(1, level - 1) - 1` above (not along the look).
pub fn pick_area(level: i32, face: [i32; 3]) -> Option<([i32; 3], [i32; 3])> {
    let (do_x, do_y, do_z) = (face[0] == 0, face[1] == 0, face[2] == 0);
    let range = level - 1;
    let range_y = range.max(1);
    if range == 0 && level != 1 {
        return None;
    }
    let pick = |on: bool, v: i32| if on { v } else { 0 };
    Some((
        [pick(do_x, -range), pick(do_y, -1), pick(do_z, -range)],
        [
            pick(do_x, range),
            pick(do_y, range_y * 2 - 1),
            pick(do_z, range),
        ],
    ))
}

/// `ToolCommons.removeBlocksInIteration`'s blocks: `BlockPos.betweenClosed` from `center + begin`
/// to `center + end` (x fastest, then y, then z), without the centre, which the game breaks.
pub fn between_closed(center: [i32; 3], begin: [i32; 3], end: [i32; 3]) -> Vec<[i32; 3]> {
    let a: [i32; 3] = std::array::from_fn(|i| center[i] + begin[i]);
    let b: [i32; 3] = std::array::from_fn(|i| center[i] + end[i]);
    let (lo, hi): ([i32; 3], [i32; 3]) = (
        std::array::from_fn(|i| a[i].min(b[i])),
        std::array::from_fn(|i| a[i].max(b[i])),
    );
    let mut out = Vec::new();
    for z in lo[2]..=hi[2] {
        for y in lo[1]..=hi[1] {
            for x in lo[0]..=hi[0] {
                if [x, y, z] != center {
                    out.push([x, y, z]);
                }
            }
        }
    }
    out
}

// ---- elementium -------------------------------------------------------------------------------

/// `ElementiumShovelItem.onBlockStartBreak`: the column of the same falling block it breaks with
/// the one mined.
pub const SHOVEL_AREA: ([i32; 3], [i32; 3]) = ([0, -12, 0], [1, 12, 1]);

/// A `FallingBlock`: sand, gravel, concrete powder, anvils, the dragon egg.
pub fn is_falling_block(block: &str) -> bool {
    matches!(
        block,
        "minecraft:sand"
            | "minecraft:red_sand"
            | "minecraft:gravel"
            | "minecraft:anvil"
            | "minecraft:chipped_anvil"
            | "minecraft:damaged_anvil"
            | "minecraft:dragon_egg"
    ) || (block.starts_with("minecraft:") && block.ends_with("_concrete_powder"))
}

/// `ElementiumPickaxeItem.shouldFilterOut`'s tool: the elementium pickaxe, or a tipped Terra
/// Shatterer.
pub fn filters_drops(tool: &str, tipped: bool) -> bool {
    tool == ELEMENTIUM_PICK || (tool == TERRA_PICK && tipped)
}

/// `shouldFilterOut`'s drop: `botania:disposable` always, `botania:semi_disposable` unless the
/// player sneaks.
pub fn filter_out(disposable: bool, semi_disposable: bool, sneaking: bool) -> bool {
    disposable || (semi_disposable && !sneaking)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const FULL: Aabb = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];

    #[derive(Default)]
    struct Level {
        solid: HashMap<[i32; 3], Aabb>,
        water: Vec<[i32; 3]>,
    }

    impl BurstWorld for Level {
        fn boxes(&self, pos: [i32; 3]) -> Vec<Aabb> {
            self.solid.get(&pos).copied().into_iter().collect()
        }
        fn passes(&self, _pos: [i32; 3]) -> bool {
            false
        }
        fn receiver(&self, _pos: [i32; 3]) -> bool {
            false
        }
    }

    impl StarWorld for Level {
        fn boxes(&self, pos: [i32; 3]) -> Vec<Aabb> {
            self.solid.get(&pos).copied().into_iter().collect()
        }
        fn air(&self, pos: [i32; 3]) -> bool {
            !self.solid.contains_key(&pos) && !self.water.contains(&pos)
        }
        fn water(&self, pos: [i32; 3]) -> bool {
            self.water.contains(&pos)
        }
        fn lava(&self, _pos: [i32; 3]) -> bool {
            false
        }
    }

    #[test]
    fn the_crystal_bow_draws_twice_as_fast() {
        // getPowerForTime: full power after 20 ticks of charge; 0.1 is the least that shoots.
        assert_eq!(Bow::Livingwood.charge(BOW_USE_DURATION - 20), 20);
        assert_eq!(Bow::Crystal.charge(BOW_USE_DURATION - 10), 20);
        assert_eq!(bow_power(20), 1.0);
        assert_eq!(bow_power(60), 1.0);
        // Held 2 ticks: f = 0.1, (0.01 + 0.2) / 3 = 0.07; 3 ticks: 0.1075.
        assert!(bow_power(Bow::Livingwood.charge(BOW_USE_DURATION - 2)) < 0.1);
        assert!(bow_power(Bow::Livingwood.charge(BOW_USE_DURATION - 3)) >= 0.1);
        assert!(bow_power(Bow::Crystal.charge(BOW_USE_DURATION - 1)) < 0.1);
        assert!(bow_power(Bow::Crystal.charge(BOW_USE_DURATION - 2)) >= 0.1);
        assert!((bow_power(6) - 0.23).abs() < 1e-6);
        assert!(Bow::Crystal.charge(BOW_USE_DURATION + 1) < 0);
        assert_eq!(Bow::of_item("minecraft:bow"), None);
    }

    #[test]
    fn mana_pays_for_wear_at_botanias_rates() {
        assert_eq!(crystal_arrow_cost(false), 200);
        assert_eq!(crystal_arrow_cost(true), 100);
        assert_eq!(mana_per_damage(CRYSTAL_BOW, true), Some(100));
        assert_eq!(mana_per_damage(LIVINGWOOD_BOW, true), Some(40));
        assert_eq!(mana_per_damage("botania:manasteel_pick", false), Some(60));
        assert_eq!(
            mana_per_damage("botania:elementium_shears", false),
            Some(30)
        );
        assert_eq!(mana_per_damage(TERRA_SWORD, false), Some(100));
        assert_eq!(mana_per_damage(THUNDER_SWORD, false), Some(60));
        assert_eq!(mana_per_damage(STAR_SWORD, false), Some(120));
        assert_eq!(mana_per_damage("botania:glass_pickaxe", false), Some(160));
        assert_eq!(mana_per_damage("minecraft:diamond_sword", false), None);
        // Mending: twice the rate, but every pickaxe uses ManasteelPickaxeItem's 60 and both
        // bows LivingwoodBowItem's 40.
        assert_eq!(repair_cost("botania:manasteel_sword"), Some(120));
        assert_eq!(repair_cost("botania:manasteel_shears"), Some(60));
        assert_eq!(repair_cost(STAR_SWORD), Some(240));
        assert_eq!(repair_cost(TERRA_SWORD), Some(200));
        assert_eq!(repair_cost("botania:terra_axe"), Some(200));
        assert_eq!(repair_cost(TERRA_PICK), Some(120));
        assert_eq!(repair_cost("botania:glass_pickaxe"), Some(120));
        assert_eq!(repair_cost(CRYSTAL_BOW), Some(80));
    }

    #[test]
    fn the_blade_burst_flies_where_the_player_looks() {
        // Yaw 0 looks south (+Z): the burst turns to yRot 180, 0.2 a tick times 7.
        let b = blade_burst(0.5, 65.62, 0.5, 0.0, 0.0);
        assert_eq!(b.pos, [0.5, 65.62 - f64::from(0.1_f32), 0.5]);
        assert!((b.vel[2] - 1.4).abs() < 1e-6, "{:?}", b.vel);
        assert!(b.vel[0].abs() < 1e-6 && b.vel[1].abs() < 1e-6);
        // Looking straight up (pitch -90): xRot 90, up.
        let up = blade_burst(0.0, 0.0, 0.0, 0.0, -90.0);
        assert!((up.vel[1] - 1.4).abs() < 1e-6, "{:?}", up.vel);
        // Yaw 90 looks west (-X).
        let west = blade_burst(0.0, 0.0, 0.0, 90.0, 0.0);
        assert!((west.vel[0] + 1.4).abs() < 1e-6, "{:?}", west.vel);
    }

    /// Flies a blade burst south through an empty level past a mob whose box is `mob`: the tick
    /// it struck, if any, and the tick it died.
    fn fly_past(mob: Aabb) -> (Option<i32>, i32) {
        let level = Level::default();
        let mut b = blade_burst(0.5, 65.62, 0.5, 0.0, 0.0);
        let mut struck = None;
        for tick in 1..=200 {
            let mut hit = false;
            b.tick_with(&level, |burst, old| {
                if intersects(blade_reach(burst.pos, old), mob)
                    && let Some(left) = blade_strike(burst.mana)
                {
                    burst.mana = left;
                    burst.alive = false;
                    hit = true;
                }
            });
            if hit {
                struck = Some(tick);
            }
            if !b.alive {
                return (struck, tick);
            }
        }
        (struck, 200)
    }

    #[test]
    fn the_blade_burst_strikes_until_tick_56_and_fades_on_tick_64() {
        // 100 mana, from tick 40 four a tick: 0 on tick 64. A strike needs 33 when the lens
        // looks, before that tick's loss: 36 on tick 56, 32 on tick 57.
        let nothing = [100.0, 0.0, 100.0, 101.0, 1.0, 101.0];
        assert_eq!(fly_past(nothing), (None, 64));
        // A mob 10 blocks ahead: the swept box, grown by 1, reaches z 10.2 on tick 7.
        let near = [0.2, 64.0, 10.2, 0.8, 66.0, 10.8];
        assert_eq!(fly_past(near), (Some(7), 7));
        // At z 78.6 the burst reaches it on tick 56, with 36 mana: struck.
        let far = [0.2, 64.0, 78.6, 0.8, 66.0, 79.2];
        assert_eq!(fly_past(far), (Some(56), 56));
        // At z 80 it reaches it on tick 57 with 32: no strike, and it flies on to fade.
        let beyond = [0.2, 64.0, 80.0, 0.8, 66.0, 80.6];
        assert_eq!(fly_past(beyond), (None, 64));
    }

    fn link(id: u32, x: f64, enemy: bool) -> Link {
        Link {
            id,
            pos: [x, 64.0, 0.0],
            bounds: [x - 0.3, 64.0, -0.3, x + 0.3, 65.95, 0.3],
            enemy,
        }
    }

    #[test]
    fn the_lightning_jumps_to_enemies_within_eight_blocks() {
        // The zombie hit (1) at x 0; an enemy (2) at 5, another (3) at 12, a cow (4) at 3 and an
        // enemy (5) far off at 30.
        let all = [
            link(1, 0.0, true),
            link(2, 5.0, true),
            link(3, 12.0, true),
            link(4, 3.0, false),
            link(5, 30.0, true),
        ];
        // First jump: only 2 is near enough (3's box starts at 11.7). Then from 2, both 1 (the
        // first one isn't kept out) and 3. Picking the first each time: 1, then nothing is left
        // near 1 but 2, already struck.
        let first = thunder_chain((1, [0.0, 64.0, 0.0]), &all, thunder_hops(false), |_| 0);
        assert_eq!(first, vec![(2, 5.0), (1, 4.0)]);
        // Picking the last: 2, 3, then nothing within 8 of 3 that wasn't struck.
        let last = thunder_chain((1, [0.0, 64.0, 0.0]), &all, 4, |n| n - 1);
        assert_eq!(last, vec![(2, 5.0), (3, 4.0)]);
        // In a thunderstorm 10 jumps, the first for 11.
        let storm = thunder_chain((1, [0.0, 64.0, 0.0]), &all, thunder_hops(true), |n| n - 1);
        assert_eq!(storm[0], (2, 11.0));
        // A cow hit starts a chain too; no enemy near, nothing.
        let alone = [link(4, 3.0, false), link(5, 30.0, true)];
        assert!(thunder_chain((4, [3.0, 64.0, 0.0]), &alone, 4, |_| 0).is_empty());
    }

    #[test]
    fn a_star_falls_from_24_blocks_up_at_the_block() {
        assert!(!star_ready(112, 100));
        assert!(star_ready(113, 100));
        // Both draws 0.5: straight above the block's corner, straight down.
        let (pos, vel) = star_spawn([10, 64, -5], 0.5, 0.5);
        assert_eq!(pos, [10.0, 88.0, -5.0]);
        assert_eq!(vel[1], -1.5);
        // Draws 1 and 0: 4.5 east and north of it, aimed back at it.
        let (pos, vel) = star_spawn([10, 64, -5], 1.0, 0.0);
        assert_eq!(pos, [14.5, 88.0, -9.5]);
        let len = 616.5_f64.sqrt();
        let expect = [-4.5 / len * 1.5, -24.0 / len * 1.5, 4.5 / len * 1.5];
        assert!(
            (0..3).all(|i| (vel[i] - expect[i]).abs() < 1e-12),
            "{vel:?}"
        );
        // A bonus star only on the first night (Botania reads the day time as it counts).
        assert!(bonus_star(false, 18000, 0.1));
        assert!(bonus_star(false, 16201, 0.1));
        assert!(!bonus_star(false, 16200, 0.1));
        assert!(!bonus_star(false, 42000, 0.1));
        assert!(!bonus_star(true, 18000, 0.1));
        assert!(!bonus_star(false, 18000, 0.125));
        assert_eq!(star_damage(0.24), 10.0);
        assert_eq!(star_damage(0.25), 5.0);
    }

    #[test]
    fn a_star_slows_falls_and_breaks_on_the_ground() {
        // ThrowableCopyEntity: drag 0.99F, gravity 0.03F, both in double.
        let mut level = Level::default();
        let mut star = Star::new([0.5, 100.5, 0.5], [0.0, -1.5, 0.0]);
        star.step(&level);
        assert_eq!(star.pos[1], 99.0);
        let vy = -1.5 * 0.9900000095367432 - 0.029999999329447746;
        assert!((star.vel[1] - vy).abs() < 1e-12);
        assert!(star.been_in_air);
        star.step(&level);
        assert!((star.pos[1] - (99.0 + vy)).abs() < 1e-12);
        // In water: 0.8F.
        level.water.push([0, 50, 0]);
        let mut wet = Star::new([0.5, 50.5, 0.5], [0.0, -1.0, 0.0]);
        wet.step(&level);
        let vy = -0.800000011920929 - 0.029999999329447746;
        assert!((wet.vel[1] - vy).abs() < 1e-12);

        // The ground: a full block at y 64. A star in the air dies on it.
        level.solid.insert([0, 64, 0], FULL);
        let mut star = Star::new([0.5, 70.5, 0.5], [0.0, -1.5, 0.0]);
        let mut end = None;
        for _ in 0..10 {
            let step = star.step(&level);
            if !star.alive {
                end = Some(step);
                break;
            }
        }
        let end = end.expect("it lands");
        assert_eq!(end.broke, Some([0, 64, 0]));
        assert!(end.old[1] > 65.0);

        // A star that starts inside a block flies through it until it has been in the air.
        let mut buried = Star::new([0.5, 64.5, 0.5], [0.0, -1.5, 0.0]);
        let step = buried.step(&level);
        assert!(buried.alive && step.broke.is_none() && buried.been_in_air);

        let mut old = Star::new([0.0, 300.0, 0.0], [0.0; 3]);
        old.age = 200;
        assert!(!old.expired());
        old.age = 201;
        assert!(old.expired());
    }

    #[test]
    fn the_terra_shatterer_ranks_by_mana() {
        assert_eq!(pick_level(0), 0);
        assert_eq!(pick_level(9999), 0);
        assert_eq!(pick_level(10000), 1);
        assert_eq!(pick_level(999_999), 1);
        assert_eq!(pick_level(1_000_000), 2);
        assert_eq!(pick_level(10_000_000), 3);
        assert_eq!(pick_level(100_000_000), 4);
        assert_eq!(pick_level(1_000_000_000), 5);
        assert_eq!(pick_level(i32::MAX), 5);
        assert!(pick_can_mine(true, true, 9.0, false));
        assert!(!pick_can_mine(true, false, 9.0, false));
        assert!(pick_can_mine(false, false, 1.0, true));
        assert!(!pick_can_mine(false, false, 1.0, false));
    }

    #[test]
    fn the_terra_shatterer_breaks_an_area_on_the_face() {
        let c = [10, 64, 10];
        // Rank C (2) on the top face: the 3 by 3 around it, in betweenClosed's order.
        let (b, e) = pick_area(2, [0, 1, 0]).unwrap();
        assert_eq!((b, e), ([-1, 0, -1], [1, 0, 1]));
        assert_eq!(
            between_closed(c, b, e),
            vec![
                [9, 64, 9],
                [10, 64, 9],
                [11, 64, 9],
                [9, 64, 10],
                [11, 64, 10],
                [9, 64, 11],
                [10, 64, 11],
                [11, 64, 11],
            ]
        );
        // Rank B (1) on a side: one wide, from one below to one above.
        let (b, e) = pick_area(1, [1, 0, 0]).unwrap();
        assert_eq!(between_closed(c, b, e), vec![[10, 63, 10], [10, 65, 10]]);
        // Rank A (3) facing north: 5 wide, from one below to three above.
        let (b, e) = pick_area(3, [0, 0, -1]).unwrap();
        assert_eq!((b, e), ([-2, -1, 0], [2, 3, 0]));
        let area = between_closed(c, b, e);
        assert_eq!(area.len(), 24);
        assert_eq!(area[0], [8, 63, 10]);
        assert_eq!(area[23], [12, 67, 10]);
        // SS (5): 9 by 9.
        let (b, e) = pick_area(5, [0, -1, 0]).unwrap();
        assert_eq!(between_closed(c, b, e).len(), 80);
    }

    #[test]
    fn elementium_tools() {
        assert!(filters_drops(ELEMENTIUM_PICK, false));
        assert!(!filters_drops(TERRA_PICK, false));
        assert!(filters_drops(TERRA_PICK, true));
        assert!(!filters_drops("botania:manasteel_pick", true));
        assert!(filter_out(true, false, true));
        assert!(filter_out(false, true, false));
        assert!(!filter_out(false, true, true));
        assert!(!filter_out(false, false, false));
        assert!(is_falling_block("minecraft:sand"));
        assert!(is_falling_block("minecraft:red_concrete_powder"));
        assert!(!is_falling_block("minecraft:suspicious_sand"));
        assert!(!is_falling_block("minecraft:dirt"));
        // Two by two columns, 12 down and 12 up.
        let (b, e) = SHOVEL_AREA;
        let column = between_closed([0, 64, 0], b, e);
        assert_eq!(column.len(), 2 * 25 * 2 - 1);
        assert_eq!(column[0], [0, 52, 0]);
    }

    #[test]
    fn a_look_meets_a_box_ahead_within_reach() {
        let mob = [-0.3, 64.0, 2.7, 0.3, 65.95, 3.3];
        let eye = [0.0, 65.0, 0.0];
        assert!(ray_meets_box(eye, [0.0, 0.0, 1.0], 3.0, mob));
        assert!(!ray_meets_box(eye, [0.0, 0.0, 1.0], 2.5, mob));
        assert!(!ray_meets_box(eye, [0.0, 0.0, -1.0], 3.0, mob));
        assert!(!ray_meets_box(eye, [1.0, 0.0, 0.0], 3.0, mob));
        assert!(ray_meets_box([0.0, 64.5, 3.0], [1.0, 0.0, 0.0], 0.1, mob));
        assert!(!intersects(swept_box([0.0; 3], [1.0, 0.0, 0.0], 0.5), mob));
    }
}
