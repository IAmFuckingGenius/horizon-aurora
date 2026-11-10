//! Botania's ender air and pixies, without the game: the Ender Air cloud (`EnderAirEntity`), how a
//! glass bottle or a dispenser bottles it (`EnderAirItem.onPlayerInteract`,
//! `EnderAirBottlingBehavior`), the thrown Ender Air Bottle (`EnderAirItem.use`,
//! `EnderAirBottleEntity`), and pixies (`PixieEntity`, `PixieHandler`, `PixieModel`): when one is
//! summoned, how it flies and strikes, how long it lives and how it is drawn.

use crate::armor_model::{self, Cube, Part};
use crate::fx::Sparkle;
use crate::mth;

pub const ENDER_AIR_BOTTLE: &str = "botania:ender_air_bottle";
pub const GLASS_BOTTLE: &str = "minecraft:glass_bottle";
pub const PIXIE: &str = "botania:pixie";
pub const PIXIE_RING: &str = "botania:pixie_ring";
pub const ELEMENTIUM_SWORD: &str = "botania:elementium_sword";
pub const ELEMENTIUM_ARMOR: [&str; 4] = [
    "botania:elementium_helmet",
    "botania:elementium_chestplate",
    "botania:elementium_leggings",
    "botania:elementium_boots",
];

// ---- ender air ----------------------------------------------------------------------------------

/// The tag of the marker that stands for an Ender Air cloud: the content's
/// `botania:ender_air_release` function summons `minecraft:marker` with it, as Horizon has no
/// plain entity type a content mod can declare.
pub const ENDER_AIR_TAG: &str = "botania.ender_air";
/// `EnderAirEntity.MAX_AGE`.
pub const ENDER_AIR_MAX_AGE: i32 = 60;
/// `EnderAirBottleEntity.PARTICLE_COLOR`: the cloud's particles and the bottle's splash.
pub const ENDER_AIR_COLOR: u32 = 0x00_0008;
/// The block tag a bottle turns into end stone.
pub const CONVERTABLE: &str = "botania:ender_air_convertable";

/// A box: lowest corner, then highest.
pub type Aabb = [f64; 6];

/// `EnderAirEntity.tick` on the server: discarded once `tickCount` (counted before the tick)
/// passes `MAX_AGE`, so a cloud lives 61 ticks.
pub fn ender_air_gone(tick_count: i32) -> bool {
    tick_count > ENDER_AIR_MAX_AGE
}

/// An entity's box (`EntityDimensions.makeBoundingBox`): `width` wide and `height` tall, standing
/// on `feet`.
pub fn entity_box(feet: [f64; 3], width: f64, height: f64) -> Aabb {
    let w = width / 2.0;
    [
        feet[0] - w,
        feet[1],
        feet[2] - w,
        feet[0] + w,
        feet[1] + height,
        feet[2] + w,
    ]
}

/// The cloud's box: `EntityType.sized(1, 1)`.
pub fn ender_air_box(feet: [f64; 3]) -> Aabb {
    entity_box(feet, 1.0, 1.0)
}

/// `AABB.inflate(d)`.
pub fn inflate(b: Aabb, d: f64) -> Aabb {
    [b[0] - d, b[1] - d, b[2] - d, b[3] + d, b[4] + d, b[5] + d]
}

/// `AABB.intersects`: boxes that only touch don't.
pub fn intersects(a: Aabb, b: Aabb) -> bool {
    a[0] < b[3] && a[3] > b[0] && a[1] < b[4] && a[4] > b[1] && a[2] < b[5] && a[5] > b[2]
}

/// The unit box of a block (`new AABB(pos)`).
pub fn block_box(pos: [i32; 3]) -> Aabb {
    let [x, y, z] = pos.map(f64::from);
    [x, y, z, x + 1.0, y + 1.0, z + 1.0]
}

/// `pickupFromEntity(level, player.getBoundingBox().inflate(1.0))`: how far around the player a
/// glass bottle reaches a cloud.
pub const HAND_REACH: f64 = 1.0;
/// `isClearFromDragonBreath(level, player.getBoundingBox().inflate(3.5))`.
pub const HAND_BREATH_RANGE: f64 = 3.5;
/// `Item.getPlayerPOVHitResult` in 1.20.1: the look reaches 5 blocks.
pub const POV_REACH: f64 = 5.0;
/// `EnderAirBottlingBehavior.pickupInEnd`: `new AABB(facingPos).inflate(2.0D)`.
pub const DISPENSER_BREATH_RANGE: f64 = 2.0;

/// `EnderAirItem.onPlayerInteract`: a glass bottle is filled in the End while no dragon's breath
/// is near and the player isn't aiming at a fluid (`notAimingAtFluid`), or anywhere from a cloud
/// within reach. The End's own air is checked first: a cloud is used up only when the End
/// doesn't fill the bottle.
pub fn bottles_by_hand(in_end: bool, breath_clear: bool, aiming_at_fluid: bool) -> bool {
    in_end && breath_clear && !aiming_at_fluid
}

/// `EnderAirBottlingBehavior.pickupInEnd`: a dispenser fills a glass bottle in the End when the
/// block in front of it and the one above that are empty and no dragon's breath is near.
pub fn bottles_in_end(in_end: bool, front_air: bool, above_air: bool, breath_clear: bool) -> bool {
    in_end && front_air && above_air && breath_clear
}

/// `shootFromRotation(player, xRot, yRot, 0, 1.5F, 1F)`.
pub const THROW_SPEED: f32 = 1.5;
pub const THROW_UNCERTAINTY: f32 = 1.0;

/// `0.4F / (random.nextFloat() * 0.4F + 0.8F)`: the pitch of `botania:ender_air_throw`.
pub fn throw_pitch(random: f32) -> f32 {
    0.4 / (random * 0.4 + 0.8)
}

/// `AbstractProjectileDispenseBehavior` as Botania registers it for the bottle: thrown from 0.7
/// blocks in front of the dispenser (`getDispensePosition`), along the facing raised by 0.1, at
/// 1.1 with an uncertainty of 6.
pub const DISPENSE_OUT: f64 = 0.7;
pub const DISPENSE_RAISE: f64 = 0.1;
pub const DISPENSE_POWER: f32 = 1.1;
pub const DISPENSE_UNCERTAINTY: f32 = 6.0;

/// `getCoordsToPut`: `range` and `rangeY`.
pub const CONVERT_RANGE: i32 = 4;
pub const CONVERT_RANGE_Y: i32 = 4;
/// At most this many blocks turn.
pub const MAX_CONVERTED: usize = 64;
/// `Math.random() < 0.1`: a turned block shows its breaking particles (level event 2001).
pub const DESTROY_PARTICLE_CHANCE: f64 = 0.1;

/// `BlockPos.betweenClosed(pos.offset(-4, -4, -4), pos.offset(4, 4, 4))`: the blocks a bottle
/// looks at, x changing fastest, then y, then z.
pub fn conversion_area(pos: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    let (r, ry) = (CONVERT_RANGE, CONVERT_RANGE_Y);
    (-r..=r).flat_map(move |dz| {
        (-ry..=ry)
            .flat_map(move |dy| (-r..=r).map(move |dx| [pos[0] + dx, pos[1] + dy, pos[2] + dz]))
    })
}

/// `Collections.shuffle(list, rnd)` on a random access list: from the end, each element swaps
/// with one at `next_int(i)` below it.
pub fn shuffle<T>(list: &mut [T], mut next_int: impl FnMut(i32) -> i32) {
    for i in (2..=list.len()).rev() {
        let j = next_int(i as i32).clamp(0, i as i32 - 1) as usize;
        list.swap(i - 1, j);
    }
}

/// `getCoordsToPut` once the convertible blocks are found: shuffled, the first 64.
pub fn pick_converted(
    mut candidates: Vec<[i32; 3]>,
    next_int: impl FnMut(i32) -> i32,
) -> Vec<[i32; 3]> {
    shuffle(&mut candidates, next_int);
    candidates.truncate(MAX_CONVERTED);
    candidates
}

/// `EnderAirBottleEntity.onHitEntity`: a ghast in the Overworld cries instead of the air turning
/// blocks (a hurt of 0, its tears and `botania:ghast_ender_air_crying`).
pub fn ghast_cries(entity_kind: &str, dimension: &str) -> bool {
    entity_kind == "minecraft:ghast" && dimension == "minecraft:overworld"
}

/// The loot table a crying ghast rolls.
pub const GHAST_LOOT_TABLE: &str = "botania:ghast_ender_air_crying";

// ---- pixies -------------------------------------------------------------------------------------

/// `pixie.setPos(player.getX(), player.getY() + 2, player.getZ())`.
pub const PIXIE_SPAWN_HEIGHT: f64 = 2.0;
/// `PixieEntity.baseTick`: discarded once `tickCount` passes 200.
pub const PIXIE_MAX_AGE: i32 = 200;
/// `BotaniaEntities.PIXIE`: `sized(1, 1)`, and the default eye height of such a box.
pub const PIXIE_SIZE: f64 = 1.0;
pub const PIXIE_EYE_HEIGHT: f64 = 0.85;

/// `PixieEntity.baseTick` on the server: a pixie with no target, or one older than 200 ticks, is
/// discarded (`tickCount` counted before the tick).
pub fn pixie_gone(tick_count: i32, has_target: bool) -> bool {
    !has_target || tick_count > PIXIE_MAX_AGE
}

/// The `PIXIE_SPAWN_CHANCE` modifier an armour piece adds in its own slot: helmet 0.11,
/// chestplate 0.17, leggings 0.15, boots 0.09 (`Elementium*Item.getDefaultAttributeModifiers`).
pub fn armor_chance(slot: usize, item: &str) -> f64 {
    const CHANCES: [f64; 4] = [0.11, 0.17, 0.15, 0.09];
    if ELEMENTIUM_ARMOR.get(slot) == Some(&item) {
        CHANCES[slot]
    } else {
        0.0
    }
}

/// The elementium sword's modifier, in the main hand only.
pub const SWORD_CHANCE: f64 = 0.05;
/// The Great Fairy Ring's modifier (`GreatFairyRingItem`), while worn.
pub const RING_CHANCE: f64 = 0.25;

/// `player.getAttributeValue(PIXIE_SPAWN_CHANCE)`: the armour worn (head to feet), the item in the
/// main hand and a worn Great Fairy Ring, added to a base of 0 and kept within the attribute's
/// 0 to 1.
pub fn spawn_chance(worn: [Option<&str>; 4], main_hand: Option<&str>, ring: bool) -> f64 {
    let mut chance = 0.0;
    for (slot, item) in worn.iter().enumerate() {
        if let Some(item) = item {
            chance += armor_chance(slot, item);
        }
    }
    if main_hand == Some(ELEMENTIUM_SWORD) {
        chance += SWORD_CHANCE;
    }
    if ring {
        chance += RING_CHANCE;
    }
    chance.clamp(0.0, 1.0)
}

/// `ElementiumHelmItem.hasArmorSet`: all four elementium pieces worn, each in its slot.
pub fn has_elementium_set(worn: [Option<&str>; 4]) -> bool {
    worn.iter()
        .zip(ELEMENTIUM_ARMOR)
        .all(|(item, piece)| *item == Some(piece))
}

/// `PixieHandler.onDamageTaken`: 4 damage, 6 with an elementium sword in either hand.
pub fn pixie_damage(sword_held: bool) -> f32 {
    if sword_held { 6.0 } else { 4.0 }
}

/// `PixieHandler.effectSuppliers`, one picked by `random.nextInt(4)` for a wearer of the full
/// set: the effect and its duration, amplifier 0.
pub const PIXIE_EFFECTS: [(&str, i32); 4] = [
    ("minecraft:blindness", 40),
    ("minecraft:wither", 50),
    ("minecraft:slowness", 40),
    ("minecraft:weakness", 40),
];

/// What `PixieEntity.customServerAiStep` decides in a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixieStep {
    /// The motion it is given.
    pub velocity: [f64; 3],
    /// `yBodyRot` and `yRot` after it.
    pub yaw: f32,
    /// Within a block of its aim: it strikes and is discarded.
    pub reached: bool,
}

/// `PixieEntity.customServerAiStep` toward a target standing at `target` (its feet), `width` wide
/// and `height` tall: it aims at `x + width / 2`, `y + height / 2`, `z + width / 2` (Botania's
/// own aim, half a width off the middle on x and z), moving `mod / distance` a tick (0.45, a dark
/// pixie's 0.1), and strikes once it is within a block.
pub fn pixie_step(
    pixie: [f64; 3],
    target: [f64; 3],
    width: f32,
    height: f32,
    dark: bool,
) -> PixieStep {
    let d0 = target[0] + f64::from(width / 2.0) - pixie[0];
    let d1 = target[1] + f64::from(height / 2.0) - pixie[1];
    let d2 = target[2] + f64::from(width / 2.0) - pixie[2];
    let d3 = d0 * d0 + d1 * d1 + d2 * d2;
    let m = f64::from(if dark { 0.1_f32 } else { 0.45_f32 });
    let velocity = [d0 / d3 * m, d1 / d3 * m, d2 / d3 * m];
    let yaw = -(velocity[0].atan2(velocity[2]) as f32) * 180.0 / std::f32::consts::PI;
    PixieStep {
        velocity,
        yaw,
        reached: d3.sqrt() < 1.0,
    }
}

/// `PixieEntity.isInvulnerableTo`: a summoned pixie (type 0) takes no harm from its summoner, a
/// dark one (type 1, the Gaia Guardian's) none but from players.
pub fn pixie_invulnerable(dark: bool, from_summoner: bool, from_player: bool) -> bool {
    if dark { !from_player } else { from_summoner }
}

/// `PixieEntity.baseTick` on a client: four sparkles a tick.
pub const SPARKLES_PER_TICK: usize = 4;
/// `PixieEntity.remove` on a client: twelve sparkles where a summoned pixie goes.
pub const SPARKLES_ON_REMOVAL: usize = 12;

/// One of a pixie's sparkles from four `Math.random()` draws in Botania's order: its size, then
/// its offsets on x, y and z. A sparkle of its trail (`baseTick`) is 0.1 to 0.35 big and lives 36
/// ticks, pink or, for a dark pixie, nearly black; one of its parting burst (`remove`) is 1 to
/// 1.25 big, pink, and lives 15. Both start within 0.125 of the point half a block over its feet.
pub fn pixie_sparkle(feet: [f64; 3], dark: bool, parting: bool, r: [f64; 4]) -> Sparkle {
    let (base, m) = if parting { (1.0, 5) } else { (0.1, 12) };
    let rgb = if dark && !parting {
        [0.1, 0.025, 0.09]
    } else {
        [1.0, 0.25, 0.9]
    };
    let size = base + r[0] as f32 * 0.25;
    let pos = [
        feet[0] + (r[1] - 0.5) * 0.25,
        feet[1] + 0.5 + (r[2] - 0.5) * 0.25,
        feet[2] + (r[3] - 0.5) * 0.25,
    ];
    Sparkle::new(pos, size, rgb, m)
}

// ---- the pixie model ----------------------------------------------------------------------------

/// `LayerDefinition.create(PixieModel.createMesh(), 32, 32)`.
pub const PIXIE_TEXTURE_SIZE: [u32; 2] = [32, 32];

const fn cube(tex: [i32; 2], origin: [f32; 3], size: [f32; 3]) -> Cube {
    Cube {
        tex,
        mirror: false,
        origin,
        size,
        grow: 0.0,
    }
}

/// `PixieModel.createMesh`: a 5 texel cube of a body and two pairs of flat wings.
pub const PIXIE_PARTS: &[Part] = &[
    Part {
        name: "body",
        parent: "",
        pose: [0.0, 16.0, 0.0, 0.0, 0.0, 0.0],
        cubes: &[cube([0, 0], [-2.5, 0.0, -2.5], [5.0, 5.0, 5.0])],
    },
    Part {
        name: "leftWingT",
        parent: "",
        pose: [2.5, 18.0, 0.5, 0.2618, 0.5236, 0.2618],
        cubes: &[cube([0, 4], [0.0, -5.0, 0.0], [0.0, 5.0, 6.0])],
    },
    Part {
        name: "leftWingB",
        parent: "",
        pose: [2.5, 18.0, 0.5, -0.2618, 0.2618, -0.2618],
        cubes: &[cube([0, 11], [0.0, 0.0, 0.0], [0.0, 3.0, 4.0])],
    },
    Part {
        name: "rightWingT",
        parent: "",
        pose: [-2.5, 18.0, 0.5, 0.2618, -0.5236, -0.2618],
        cubes: &[cube([0, 4], [0.0, -5.0, 0.0], [0.0, 5.0, 6.0])],
    },
    Part {
        name: "rightWingB",
        parent: "",
        pose: [-2.5, 18.0, 0.5, -0.2618, -0.2618, 0.2618],
        cubes: &[cube([0, 11], [0.0, 0.0, 0.0], [0.0, 3.0, 4.0])],
    },
];

/// `PixieModel.setupAnim`: the turn about y of `rightWingT`, `leftWingT`, `rightWingB` and
/// `leftWingB` at `age` (ticks lived plus the partial tick), with `Mth.cos`.
pub fn wing_turns(age: f32) -> [(&'static str, f32); 4] {
    let c = mth::cos(age * 1.7);
    let pi = std::f32::consts::PI;
    [
        ("rightWingT", -(c * pi * 0.5)),
        ("leftWingT", c * pi * 0.5),
        ("rightWingB", -(c * pi * 0.25)),
        ("leftWingB", c * pi * 0.25),
    ]
}

/// `PixieRenderer.getTextureLocation`: Botania's `textures/model/pixie.png`, which the content mod
/// also carries under `textures/entity/model/` so the game's atlas packs it, by its atlas name;
/// the Halloween one while `dootDoot`.
pub fn pixie_texture(halloween: bool) -> &'static str {
    if halloween {
        "botania:entity/model/pixie_halloween"
    } else {
        "botania:entity/model/pixie"
    }
}

/// `ClientProxy.dootDoot`, from the day (days since 1970-01-01): all of October.
pub fn doot_doot(unix_days: i64) -> bool {
    armor_model::month_day(unix_days).0 == 10
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELM: &str = "botania:elementium_helmet";
    const CHEST: &str = "botania:elementium_chestplate";
    const LEGS: &str = "botania:elementium_leggings";
    const BOOTS: &str = "botania:elementium_boots";

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn the_pixie_chance_adds_the_worn_pieces_the_sword_and_the_ring() {
        let set = [Some(HELM), Some(CHEST), Some(LEGS), Some(BOOTS)];
        assert!(close(spawn_chance(set, None, false), 0.52));
        assert!(close(
            spawn_chance([Some(HELM), None, None, None], None, false),
            0.11
        ));
        assert!(close(
            spawn_chance([None, None, None, Some(BOOTS)], None, false),
            0.09
        ));
        // A piece counts in its own slot only.
        assert_eq!(
            spawn_chance([Some(BOOTS), None, None, None], None, false),
            0.0
        );
        assert!(close(
            spawn_chance([None; 4], Some(ELEMENTIUM_SWORD), false),
            0.05
        ));
        assert!(close(spawn_chance([None; 4], None, true), 0.25));
        assert!(close(spawn_chance(set, Some(ELEMENTIUM_SWORD), true), 0.82));
        assert_eq!(
            spawn_chance([None; 4], Some("botania:manasteel_sword"), false),
            0.0
        );
    }

    #[test]
    fn the_full_set_picks_an_effect_and_a_sword_adds_two_damage() {
        let set = [Some(HELM), Some(CHEST), Some(LEGS), Some(BOOTS)];
        assert!(has_elementium_set(set));
        assert!(!has_elementium_set([
            Some(HELM),
            Some(CHEST),
            Some(LEGS),
            None
        ]));
        assert!(!has_elementium_set([
            Some(HELM),
            Some(CHEST),
            Some(LEGS),
            Some("botania:manasteel_boots")
        ]));
        assert_eq!(pixie_damage(false), 4.0);
        assert_eq!(pixie_damage(true), 6.0);
        assert_eq!(PIXIE_EFFECTS[1], ("minecraft:wither", 50));
    }

    #[test]
    fn a_pixie_flies_at_its_aim_slower_from_afar() {
        // A zombie (0.6 x 1.95) standing at 3.5 64 4.5, the pixie at 0.5 66 0.5. Expected values
        // worked out from Botania's code with Java's float and double arithmetic: the width and
        // height halved as floats, `mod` the float 0.45 widened.
        let s = pixie_step([0.5, 66.0, 0.5], [3.5, 64.0, 4.5], 0.6, 1.95, false);
        assert!(!s.reached);
        let expected = [
            0.04879952217767748,
            -0.015157426935744706,
            0.06358725611749383,
        ];
        for (v, e) in s.velocity.iter().zip(expected) {
            assert!(close(*v, e), "{v} != {e}");
        }
        assert_eq!(s.yaw, -37.50414276123047);
        let dark = pixie_step([0.5, 66.0, 0.5], [3.5, 64.0, 4.5], 0.6, 1.95, true);
        assert!(close(dark.velocity[0], 0.01084433871057621));
    }

    #[test]
    fn a_pixie_strikes_within_a_block_of_its_aim() {
        // The aim is 3.8 64.975 4.8: 0.4, 0.375 and 0.3 away, under a block.
        assert!(pixie_step([3.4, 64.6, 4.5], [3.5, 64.0, 4.5], 0.6, 1.95, false).reached);
        // 0.8, 0.475 and 0.6 away: 1.107 blocks.
        assert!(!pixie_step([3.0, 64.5, 4.2], [3.5, 64.0, 4.5], 0.6, 1.95, false).reached);
    }

    #[test]
    fn pixies_live_200_ticks_and_ender_air_61() {
        assert!(!pixie_gone(200, true));
        assert!(pixie_gone(201, true));
        assert!(pixie_gone(1, false));
        assert!(!ender_air_gone(60));
        assert!(ender_air_gone(61));
    }

    #[test]
    fn a_pixie_spares_its_summoner_and_a_dark_one_all_but_players() {
        assert!(pixie_invulnerable(false, true, true));
        assert!(!pixie_invulnerable(false, false, true));
        assert!(!pixie_invulnerable(false, false, false));
        assert!(pixie_invulnerable(true, false, false));
        assert!(!pixie_invulnerable(true, true, true));
    }

    #[test]
    fn a_glass_bottle_reaches_a_cloud_within_a_block_of_the_player() {
        // A player (0.6 x 1.8) at 0.5 64 0.5: the box inflated by 1 is -0.8 63 -0.8 to 1.8 66.8 1.8.
        let reach = inflate(entity_box([0.5, 64.0, 0.5], 0.6, 1.8), HAND_REACH);
        assert!(close(reach[4], 66.8));
        // The daisy's cloud fills the cell above the block it turned: 0 66 0 to 1 67 1.
        assert!(intersects(reach, ender_air_box([0.5, 66.0, 0.5])));
        assert!(!intersects(reach, ender_air_box([0.5, 66.8, 0.5])));
        assert!(!intersects(reach, ender_air_box([3.3, 64.0, 0.5])));
        // A dispenser reaches only a cloud in the block in front of it; a box beside doesn't.
        assert!(intersects(
            block_box([0, 66, 0]),
            ender_air_box([0.5, 66.0, 0.5])
        ));
        assert!(!intersects(
            block_box([1, 66, 0]),
            ender_air_box([0.5, 66.0, 0.5])
        ));
    }

    #[test]
    fn the_end_fills_a_bottle_unless_dragon_breath_or_a_fluid() {
        assert!(bottles_by_hand(true, true, false));
        assert!(!bottles_by_hand(true, false, false));
        assert!(!bottles_by_hand(true, true, true));
        assert!(!bottles_by_hand(false, true, false));
        assert!(bottles_in_end(true, true, true, true));
        assert!(!bottles_in_end(true, true, false, true));
        assert!(!bottles_in_end(true, false, true, true));
        assert!(!bottles_in_end(true, true, true, false));
        assert!(!bottles_in_end(false, true, true, true));
    }

    #[test]
    fn a_bottle_looks_at_nine_by_nine_by_nine_blocks_x_first() {
        let area: Vec<[i32; 3]> = conversion_area([10, 64, -3]).collect();
        assert_eq!(area.len(), 729);
        assert_eq!(area[0], [6, 60, -7]);
        assert_eq!(area[1], [7, 60, -7]);
        assert_eq!(area[9], [6, 61, -7]);
        assert_eq!(area[81], [6, 60, -6]);
        assert_eq!(area[728], [14, 68, 1]);
    }

    #[test]
    fn the_blocks_turned_are_shuffled_as_collections_shuffle_and_at_most_64() {
        // nextInt always 0: 4 swaps with 0 (d b c a), 3 with 0 (c b d a), 2 with 0 (b c d a).
        let mut list = ['a', 'b', 'c', 'd'];
        shuffle(&mut list, |_| 0);
        assert_eq!(list, ['b', 'c', 'd', 'a']);
        // nextInt(i) = i - 1 swaps each element with itself.
        let mut list = ['a', 'b', 'c', 'd'];
        shuffle(&mut list, |i| i - 1);
        assert_eq!(list, ['a', 'b', 'c', 'd']);
        let all: Vec<[i32; 3]> = conversion_area([0, 0, 0]).collect();
        let picked = pick_converted(all.clone(), |i| i - 1);
        assert_eq!(picked.len(), 64);
        assert_eq!(picked[..], all[..64]);
        let few = pick_converted(all[..10].to_vec(), |i| i - 1);
        assert_eq!(few.len(), 10);
    }

    #[test]
    fn only_an_overworld_ghast_cries() {
        assert!(ghast_cries("minecraft:ghast", "minecraft:overworld"));
        assert!(!ghast_cries("minecraft:ghast", "minecraft:the_nether"));
        assert!(!ghast_cries("minecraft:happy_ghast", "minecraft:overworld"));
    }

    #[test]
    fn the_wings_flap_as_pixie_model_turns_them() {
        // Mth.cos(age * 1.7F) from the float table, times (float) PI and a half or a quarter.
        let at = |age: f32| wing_turns(age).map(|(_, a)| a);
        assert_eq!(
            at(0.0),
            [
                -1.5707963705062866,
                1.5707963705062866,
                -0.7853981852531433,
                0.7853981852531433
            ]
        );
        assert_eq!(
            at(1.0),
            [
                0.20229241251945496,
                -0.20229241251945496,
                0.10114620625972748,
                -0.10114620625972748
            ]
        );
        assert_eq!(at(2.5)[0], 0.7007271647453308);
        assert_eq!(at(10.0)[3], -0.21614404022693634);
    }

    #[test]
    fn a_pixie_trails_small_sparkles_and_parts_in_big_ones() {
        let s = pixie_sparkle([0.0, 64.0, 0.0], false, false, [0.0, 0.0, 0.5, 1.0]);
        assert_eq!(s.pos, [-0.125, 64.5, 0.125]);
        assert_eq!((s.size, s.m, s.rgb), (0.1, 12, [1.0, 0.25, 0.9]));
        let s = pixie_sparkle([0.0, 64.0, 0.0], true, false, [1.0, 0.5, 0.5, 0.5]);
        assert_eq!((s.size, s.m, s.rgb), (0.35, 12, [0.1, 0.025, 0.09]));
        let s = pixie_sparkle([0.0, 64.0, 0.0], true, true, [1.0, 0.5, 0.5, 0.5]);
        assert_eq!((s.size, s.m, s.rgb), (1.25, 5, [1.0, 0.25, 0.9]));
    }

    #[test]
    fn the_throw_sound_is_pitched_by_the_players_random() {
        assert_eq!(throw_pitch(0.0), 0.5);
        assert_eq!(throw_pitch(0.5), 0.4000000059604645);
        assert_eq!(throw_pitch(1.0), 0.3333333134651184);
    }

    #[test]
    fn pixies_wear_their_halloween_look_in_october() {
        assert!(!doot_doot(20_726)); // 2026-09-30
        assert!(doot_doot(20_727)); // 2026-10-01
        assert!(doot_doot(20_757)); // 2026-10-31
        assert!(!doot_doot(20_758)); // 2026-11-01
        assert!(doot_doot(20_011)); // 2024-10-15
        assert_eq!(pixie_texture(true), "botania:entity/model/pixie_halloween");
    }
}
