//! What Botania's block entity renderers work out before they draw: the mana's height in a pool
//! and the colours of its surface (`ManaPoolBlockEntityRenderer`, the `mana_pool` core shader),
//! the petals floating in an apothecary (`PetalApothecaryBlockEntityRenderer`), a spreader's
//! turning core and the Gaia spreader's and fabulous pool's colours
//! (`ManaSpreaderBlockEntityRenderer`). The game side draws with these numbers.
//!
//! `time` is Botania's `ClientTickHandler.ticksInGame + partialTicks`; the port passes the
//! level's game time plus the partial tick.

use crate::consts::{POOL_MAX_MANA, PoolVariant};
use crate::mth;
use crate::wand::hsv_to_rgb;

/// `new java.util.Random(seed).nextInt(bound)`: the first draw of a fresh generator, for a
/// positive `bound`.
pub fn java_random_int(seed: i64, bound: i32) -> i32 {
    const MULTIPLIER: i64 = 0x5_DEEC_E66D;
    const MASK: i64 = (1 << 48) - 1;
    let mut state = (seed ^ MULTIPLIER) & MASK;
    let mut next = || {
        state = state.wrapping_mul(MULTIPLIER).wrapping_add(0xB) & MASK;
        (state >> 17) as i32
    };
    let mut r = next();
    let m = bound - 1;
    if bound & m == 0 {
        return ((i64::from(bound) * i64::from(r)) >> 31) as i32;
    }
    let mut u = r;
    loop {
        r = u % bound;
        if u.wrapping_sub(r).wrapping_add(m) >= 0 {
            return r;
        }
        u = next();
    }
}

/// `Vec3i.hashCode` (a `BlockPos`'s).
pub fn block_pos_hash(pos: [i32; 3]) -> i32 {
    pos[1]
        .wrapping_add(pos[2].wrapping_mul(31))
        .wrapping_mul(31)
        .wrapping_add(pos[0])
}

/// The inside of a pool as its renderer crops the mana: from `start` to `end` texels across
/// (`insideUVStart`, `insideUVEnd`), the surface between `bottom` and `top` blocks up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoolInside {
    pub start: f32,
    pub end: f32,
    pub bottom: f32,
    pub top: f32,
}

/// `ManaPoolBlockEntityRenderer.render`'s crop and heights for a pool variant.
pub fn pool_inside(variant: PoolVariant) -> PoolInside {
    let diluted = variant == PoolVariant::Diluted;
    let start = if diluted { 1 } else { 2 };
    let top = match variant {
        PoolVariant::Diluted => 5,
        PoolVariant::Creative => 9,
        PoolVariant::Default | PoolVariant::Fabulous => 7,
    };
    PoolInside {
        start: start as f32,
        end: (16 - start) as f32,
        bottom: start as f32 / 16.0 + 0.001,
        top: top as f32 / 16.0,
    }
}

/// How high the mana stands in a pool holding `mana` of `max_mana` (`getCurrentMana`,
/// `getMaxMana`), in blocks from its bottom: `Mth.clampedMap(manaLevel, 0, 1, poolBottom,
/// poolTop)`. None for an empty pool, which shows no surface.
pub fn pool_surface_height(variant: PoolVariant, mana: i32, max_mana: i32) -> Option<f32> {
    let max = if max_mana <= 0 {
        POOL_MAX_MANA
    } else {
        max_mana
    };
    let level = mana as f32 / max as f32;
    if level <= 0.0 {
        return None;
    }
    let inside = pool_inside(variant);
    Some(inside.bottom + level.min(1.0) * (inside.top - inside.bottom))
}

/// The sprite a pool draws on its bottom for the block under it (`PoolOverlayProvider.getIcon`):
/// the alchemy and conjuration catalysts and the mana void.
pub fn pool_overlay(block_below: &str) -> Option<&'static str> {
    Some(match block_below {
        "botania:alchemy_catalyst" => "botania:block/alchemy_catalyst_overlay",
        "botania:conjuration_catalyst" => "botania:block/conjuration_catalyst_overlay",
        "botania:mana_void" => "botania:block/mana_void_overlay",
        _ => return None,
    })
}

/// The `mana_pool` core shader's colour at atlas UV `u`, `v` (`texCoord0`) and shader time `t`
/// (`GameTime * 24000`: the game time modulo a day, plus the partial tick): green and blue, its
/// red being 0. It replaces the texture's colour and keeps its alpha.
pub fn mana_water_shade(u: f32, v: f32, t: f32) -> [f32; 2] {
    let (u, v, t) = (f64::from(u), f64::from(v), f64::from(t));
    let wave =
        (u / 5.0 + (v - (t / 300.0).cos() * 0.25) / ((1.0 - (t / 200.0).sin() / 10.0) * 5.0)).sin();
    let c = ((t / 80.0 + wave * (t / 600.0).sin() * 1000.0).sin() * 0.5 + 0.5) as f32;
    [1.0 - c * 0.2 - 0.2, 1.0 - c * 0.2]
}

/// The shader time `mana_water_shade` takes: `RenderSystem.setShaderGameTime`'s
/// `(gameTime % 24000 + partialTicks) / 24000`, times 24000.
pub fn shader_time(game_time: i64, partial: f32) -> f32 {
    game_time.rem_euclid(24_000) as f32 + partial
}

/// Where the port places the mana sprite in a nominal atlas for `mana_water_shade`: its
/// position in Botania's atlas isn't known here, so a sprite of a 2048 texel atlas at its middle,
/// which gives the shader's bands their size across the surface.
pub fn mana_water_atlas_uv(texel: [f32; 2]) -> [f32; 2] {
    const SPRITE: f32 = 16.0 / 2048.0;
    texel.map(|t| 0.5 + t / 16.0 * SPRITE)
}

/// `mana_water.png`'s mean colour, which a tint multiplies: (36, 222, 255).
const MANA_WATER_TEXEL: [f32; 3] = [36.0 / 255.0, 222.0 / 255.0, 255.0 / 255.0];

/// The tint (0xRRGGBB) that brings `mana_water.png` to the shader's colour at `texel` (16 to a
/// block), for a renderer without Botania's shader: red 0, green and blue divided by the
/// texture's own.
pub fn mana_water_tint(texel: [f32; 2], t: f32) -> u32 {
    let [u, v] = mana_water_atlas_uv(texel);
    let [g, b] = mana_water_shade(u, v, t);
    let byte = |c: f32, texture: f32| ((c / texture).clamp(0.0, 1.0) * 255.0) as u32;
    (byte(g, MANA_WATER_TEXEL[1]) << 8) | byte(b, MANA_WATER_TEXEL[2])
}

/// The fabulous pool's colour (0xRRGGBB), going round the rainbow: `Mth.hsvToRgb(Mth.frac(time),
/// 0.6, 1)` with `time = (ticks + new Random(x ^ y ^ z).nextInt(100000)) * 0.005`, a pool with
/// no dye (`getColor` empty, so `multiplyColor` with white leaves it).
pub fn fabulous_pool_color(time: f64, pos: [i32; 3]) -> u32 {
    let offset = java_random_int(i64::from(pos[0] ^ pos[1] ^ pos[2]), 100_000);
    let t = (time + f64::from(offset)) * f64::from(0.005_f32);
    hsv_to_rgb(mth::frac(t) as f32, 0.6, 1.0)
}

/// The Gaia spreader's colour (0xRRGGBB): `Mth.hsvToRgb(((time * 2 + new
/// Random(pos.hashCode()).nextInt(10000)) % 360) / 360, 0.4, 0.9)`.
pub fn gaia_spreader_color(time: f64, pos: [i32; 3]) -> u32 {
    let offset = java_random_int(i64::from(block_pos_hash(pos)), 10_000);
    let hue = ((time * 2.0 + f64::from(offset)) % 360.0) as f32 / 360.0;
    hsv_to_rgb(hue, 0.4, 0.9)
}

/// A spreader's core: turned `degrees` about the spreader's own vertical axis through the
/// block's centre (`time % 360`) and raised `bob` blocks along it (`sin(time / 20) * 0.05`).
pub fn spreader_core(time: f64) -> (f32, f32) {
    ((time % 360.0) as f32, (time / 20.0).sin() as f32 * 0.05)
}

/// One petal floating in an apothecary's water, in the renderer's frame (an eighth of a block a
/// unit, from `(0.45, 0.87, 0.5)` of the block): moved to `at`, then turned `degrees` about
/// `axis` through `(1/16, 1/16, 1/16)`, then drawn as a dropped item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Petal {
    pub at: [f32; 3],
    pub degrees: f32,
    pub axis: [f32; 3],
}

/// `PetalApothecaryBlockEntityRenderer.render`'s petals: `count` of them (the slots holding
/// something, up to the first empty one) going round the water, at `time`.
pub fn apothecary_petals(time: f64, count: usize) -> Vec<Petal> {
    const MODIFIER: f32 = 6.0;
    const ROTATION_MODIFIER: f32 = 0.25;
    const RADIUS_BASE: f32 = 1.2;
    const RADIUS_MOD: f32 = 0.1;
    if count == 0 {
        return Vec::new();
    }
    let ticks = time * 0.5;
    let offset_per_petal = 360.0_f32 / count as f32;
    let radius_x = (f64::from(RADIUS_BASE)
        + f64::from(RADIUS_MOD) * (ticks / f64::from(MODIFIER)).sin()) as f32;
    let radius_z = (f64::from(RADIUS_BASE)
        + f64::from(RADIUS_MOD) * (ticks / f64::from(MODIFIER)).cos()) as f32;
    let axis = [
        (ticks * f64::from(ROTATION_MODIFIER)).sin() as f32 / 2.0,
        f64::from(0.6_f32).max((ticks * f64::from(0.1_f32)).sin() / 2.0 + 0.5) as f32,
        (ticks * f64::from(ROTATION_MODIFIER)).cos() as f32 / 2.0,
    ];
    (0..count)
        .map(|i| {
            let offset = offset_per_petal * i as f32;
            // `(int) (ticks / rotationModifier % 360F + offset)`.
            let degrees =
                ((ticks / f64::from(ROTATION_MODIFIER)) % 360.0 + f64::from(offset)) as i32 as f32;
            // `VecHelper.toRadians`.
            let rad = (f64::from(degrees / 180.0) * std::f64::consts::PI) as f32;
            let x = (f64::from(radius_x) * f64::from(rad).cos()) as f32;
            let z = (f64::from(radius_z) * f64::from(rad).sin()) as f32;
            let y = ((ticks + 50.0 * i as f64) / 5.0).cos() as f32 / 10.0;
            Petal {
                at: [x, y, z],
                degrees,
                axis,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values: Botania's renderer arithmetic run in Java (java.util.Random, Math, float
    // casts as written), with 1.20.1's `Mth.hsvToRgb`.

    #[test]
    fn java_random_first_draws() {
        assert_eq!(java_random_int(42, 10), 0);
        assert_eq!(java_random_int(0, 100), 60);
        assert_eq!(java_random_int(-5, 100_000), 19411);
        assert_eq!(block_pos_hash([12, 64, -7]), -4731);
        assert_eq!(java_random_int(-4731, 10_000), 7735);
    }

    #[test]
    fn mana_stands_between_the_bottom_and_the_rim() {
        let close = |a: Option<f32>, b: f32| (a.unwrap() - b).abs() < 1e-6;
        assert!(close(
            pool_surface_height(PoolVariant::Default, 500_000, 1_000_000),
            0.281_750_02
        ));
        assert!(close(
            pool_surface_height(PoolVariant::Diluted, 10_000, 10_000),
            0.3125
        ));
        assert!(close(
            pool_surface_height(PoolVariant::Creative, 1_000_000, 1_000_000),
            0.5625
        ));
        assert!(close(
            pool_surface_height(PoolVariant::Default, 1, 1_000_000),
            0.126_000_31
        ));
        assert_eq!(
            pool_surface_height(PoolVariant::Default, 0, 1_000_000),
            None
        );
        assert_eq!(pool_inside(PoolVariant::Diluted).start, 1.0);
        assert_eq!(pool_inside(PoolVariant::Fabulous).end, 14.0);
    }

    #[test]
    fn spreader_core_and_rainbow_colours() {
        assert_eq!(gaia_spreader_color(1234.5, [12, 64, -7]), 0x89E58F);
        assert_eq!(fabulous_pool_color(1234.5, [12, 64, -7]), 0xDE65FF);
        let (degrees, bob) = spreader_core(1234.5);
        assert_eq!(degrees, 154.5);
        assert!((bob - -0.044_714_745).abs() < 1e-6);
    }

    #[test]
    fn petals_go_round_the_water() {
        let petals = apothecary_petals(1234.5, 4);
        let expected = [
            ([0.800_222, -0.059_952_687, -0.878_294_94], 309.0),
            ([0.988_191_7, 0.006_763_61, 0.711_229_3], 399.0),
            ([-0.800_222_3, 0.048_602_38, 0.878_294_7], 489.0),
            ([-0.988_191_84, -0.088_325_36, -0.711_229_2], 579.0),
        ];
        for (petal, (at, degrees)) in petals.iter().zip(expected) {
            for (a, b) in petal.at.iter().zip(at) {
                assert!((a - b).abs() < 1e-5, "{petal:?}");
            }
            assert_eq!(petal.degrees, degrees);
            assert_eq!(petal.axis, [-0.182_884_99, 0.6, -0.465_352_65]);
        }
        assert!(apothecary_petals(10.0, 0).is_empty());
    }

    #[test]
    fn the_mana_shader_colours_in_bands() {
        let t = shader_time(30_000, 0.25);
        let close =
            |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4;
        assert!(close(
            mana_water_shade(0.5, 0.5, t),
            [0.658_553_06, 0.858_553_05]
        ));
        assert!(close(
            mana_water_shade(0.501, 0.502, t),
            [0.687_543_6, 0.887_543_56]
        ));
        // The tint keeps red out and stays within the shader's range.
        let tint = mana_water_tint([8.0, 8.0], t);
        assert_eq!(tint >> 16, 0);
        assert!((tint & 0xFF) >= 203);
    }
}
