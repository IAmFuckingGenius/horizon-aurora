//! Botania's client particles and the effects that add them: `FXWisp` and `FXSparkle` (their
//! sizes, lifetimes and motion), `ManaBurstEntity.particles` and its death, the mana pool's, the
//! generating flowers', the Pure Daisy's and the apothecary's effects, the wand's beam, the
//! falling star's sparkles and the Thundercaller's bolt (`BoltParticleOptions`).
//!
//! Botania adds these on the client from the entities and block entities it syncs. The port's
//! bursts, beams and stars live on the server only, so the server sends what happened as
//! [`Record`]s (one message a tick to each player near) and the client turns each into the wisps
//! and sparkles Botania's client would add, with the same numbers.

use crate::consts::POOL_PARTICLE_COLOR;
use crate::mana_item;
use crate::mth;

/// Random numbers as Botania draws them: `Math.random()`, a particle's `random`, the bolt's
/// `new Random()`.
pub trait Rng {
    /// A uniform float in `0..1` (`Random.nextFloat`).
    fn float(&mut self) -> f32;
    /// A uniform double in `0..1` (`Math.random()`, `Random.nextDouble`).
    fn double(&mut self) -> f64;
    /// A standard normal number (`Random.nextGaussian`).
    fn gaussian(&mut self) -> f64;
}

/// `FXWisp`'s opacity.
pub const WISP_ALPHA: f32 = 0.375;
/// `FXWisp.tick`'s drag on every axis, without gravity only.
pub const WISP_DRAG: f64 = 0.98;
/// `FXSparkle`'s opacity.
pub const SPARKLE_ALPHA: f32 = 0.75;
/// `FXSparkle.tick`'s drag on every axis (`slowdown`).
pub const SPARKLE_DRAG: f64 = 0.908000001907348633;
/// `FXSparkle.tick`'s extra horizontal drag on the ground.
pub const SPARKLE_GROUND_DRAG: f64 = 0.69999998807907104;
/// Sprites in `botania:sparkle` (`sparkle_0` to `sparkle_3`).
pub const SPARKLE_SPRITES: usize = 4;
/// `FXWisp` and `FXSparkle`: `setSize(0.01F, 0.01F)`.
pub const BOX_SIZE: f32 = 0.01;

/// A colour `0xRRGGBB` as Botania's particle data takes it (`(color >> 16 & 0xFF) / 255F`).
pub fn rgb(color: u32) -> [f32; 3] {
    [
        ((color >> 16) & 0xFF) as f32 / 255.0,
        ((color >> 8) & 0xFF) as f32 / 255.0,
        (color & 0xFF) as f32 / 255.0,
    ]
}

/// One wisp to add (`WispParticleData` and where it starts).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wisp {
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub size: f32,
    pub rgb: [f32; 3],
    /// `maxAgeMul`.
    pub max_age_mul: f32,
    /// Taken off the vertical speed each tick, not scaled as vanilla's gravity is.
    pub gravity: f32,
    pub no_clip: bool,
}

impl Wisp {
    /// `WispParticleData.wisp(size, r, g, b, maxAgeMul)` at `pos` moving by `vel`.
    pub fn new(pos: [f64; 3], vel: [f64; 3], size: f32, rgb: [f32; 3], max_age_mul: f32) -> Wisp {
        Wisp {
            pos,
            vel,
            size,
            rgb,
            max_age_mul,
            gravity: 0.0,
            no_clip: false,
        }
    }
}

/// One sparkle to add (`SparkleParticleData` and where it starts).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sparkle {
    pub pos: [f64; 3],
    pub size: f32,
    pub rgb: [f32; 3],
    /// Its lifetime is `3 * m` ticks.
    pub m: i32,
    pub no_clip: bool,
    /// Gone after its second tick, never moved.
    pub fake: bool,
}

impl Sparkle {
    /// `SparkleParticleData.sparkle(size, r, g, b, m)` at `pos`.
    pub fn new(pos: [f64; 3], size: f32, rgb: [f32; 3], m: i32) -> Sparkle {
        Sparkle {
            pos,
            size,
            rgb,
            m,
            no_clip: false,
            fake: false,
        }
    }
}

// ---- the particles ---------------------------------------------------------------------------

/// `FXWisp`'s `quadSize` from its `random.nextFloat()`: `(r * 0.5 + 0.5) * 2 * size`.
pub fn wisp_quad_size(r: f32, size: f32) -> f32 {
    (r * 0.5 + 0.5) * 2.0 * size
}

/// `FXWisp`'s lifetime from `Math.random()`: `(int) (28 / (r * 0.3 + 0.7) * maxAgeMul)`, 28 to 40
/// ticks times the multiplier.
pub fn wisp_lifetime(r: f64, max_age_mul: f32) -> i32 {
    (28.0 / (r * 0.3 + 0.7) * f64::from(max_age_mul)) as i32
}

/// `FXWisp.getQuadSize`: it grows to half its `quadSize` at half its life (`lifetime / 2`), then
/// shrinks to nothing.
pub fn wisp_size(quad: f32, age: i32, half_life: i32) -> f32 {
    if half_life <= 0 {
        return 0.0;
    }
    let mut scale = age as f32 / half_life as f32;
    if scale > 1.0 {
        scale = 2.0 - scale;
    }
    quad * scale * 0.5
}

/// `FXSparkle`'s `quadSize` from its `random.nextFloat()`: `(r * 0.5 + 0.5) * 0.2 * size`.
pub fn sparkle_quad_size(r: f32, size: f32) -> f32 {
    (r * 0.5 + 0.5) * 0.2 * size
}

/// `FXSparkle`'s lifetime: `3 * m`.
pub fn sparkle_lifetime(m: i32) -> i32 {
    m.saturating_mul(3)
}

/// `FXSparkle.getQuadSize`: `quadSize * (lifetime - age + 1) / lifetime`.
pub fn sparkle_size(quad: f32, age: i32, lifetime: i32) -> f32 {
    if lifetime <= 0 {
        return 0.0;
    }
    quad * (lifetime - age + 1) as f32 / lifetime as f32
}

/// `setSpriteFromAge`: `SpriteSet.get(age, lifetime)`, the sprite `age * (n - 1) / lifetime`.
pub fn sprite_from_age(age: i32, lifetime: i32, sprites: usize) -> usize {
    if sprites == 0 || lifetime <= 0 {
        return 0;
    }
    let i = i64::from(age.max(0)) * (sprites as i64 - 1) / i64::from(lifetime);
    (i.max(0) as usize).min(sprites - 1)
}

/// `FXSparkle.wiggleAround` (Botania's copy of `Entity.moveTowardsClosestSpace`), which a sparkle
/// that collides runs every tick: its new velocity, 0.025 to 0.075 towards the nearest side of
/// its block whose neighbour isn't a full block (north, south, west, east, then up; up when all
/// are), and up to 0.1 either way along the other two axes. `at` is the sparkle's middle;
/// `full(block)` says whether a block's collision shape is a full block.
pub fn sparkle_wiggle(at: [f64; 3], full: impl Fn([i32; 3]) -> bool, r: &mut impl Rng) -> [f64; 3] {
    let block = mth::block_of(at);
    let inside = [
        at[0] - f64::from(block[0]),
        at[1] - f64::from(block[1]),
        at[2] - f64::from(block[2]),
    ];
    // North, south, west, east, up: (axis, step).
    let mut best = (1usize, 1i32);
    let mut nearest = f64::MAX;
    for (axis, step) in [(2usize, -1i32), (2, 1), (0, -1), (0, 1), (1, 1)] {
        let mut next = block;
        next[axis] += step;
        if !full(next) {
            let d = if step > 0 {
                1.0 - inside[axis]
            } else {
                inside[axis]
            };
            if d < nearest {
                nearest = d;
                best = (axis, step);
            }
        }
    }
    let f = r.float() * 0.05 + 0.025;
    let along = f64::from(best.1 as f32 * f);
    let secondary = f64::from((r.float() - r.float()) * 0.1);
    let secondary2 = f64::from((r.float() - r.float()) * 0.1);
    match best.0 {
        0 => [along, secondary, secondary2],
        1 => [secondary, along, secondary2],
        _ => [secondary, secondary2, along],
    }
}

// ---- mana bursts -----------------------------------------------------------------------------

/// The per-burst part of a trail's size: `Math.sin(new Random(uuid.getMostSignificantBits())
/// .nextInt(9001)) * 0.4`, from that `nextInt` (0 to 9000).
pub fn burst_wobble(n: i32) -> f32 {
    (f64::from(n).sin() as f32) * 0.4
}

/// `ManaBurstEntity.particles` in Botania's default mode (not `subtlePowerSystem`): wisps from
/// where the burst is back towards where it was a tick before, one every 0.095 blocks (dark
/// colours lightened a little at random), each `0.2 * size` and drifting against the burst's
/// motion, then a `0.1 * size` one of the burst's own colour where the walk stopped. `size` is
/// the mana left over the mana it started with.
pub fn burst_trail(
    pos: [f64; 3],
    prev: [f64; 3],
    motion: [f64; 3],
    color: u32,
    mana_size: f32,
    wobble: f32,
    r: &mut impl Rng,
) -> Vec<Wisp> {
    let [or, og, ob] = rgb(color);
    let luminance = 0.2126 * f64::from(or) + 0.7152 * f64::from(og) + 0.0722 * f64::from(ob);
    let diff = [prev[0] - pos[0], prev[1] - pos[1], prev[2] - pos[2]];
    let norm = normalize(diff);
    const DISTANCE: f64 = 0.095;
    let vel = [
        f64::from(-motion[0] as f32 * 0.01),
        f64::from(-motion[1] as f32 * 0.01),
        f64::from(-motion[2] as f32 * 0.01),
    ];
    let mut at = pos;
    let mut out = Vec::new();
    let size = loop {
        let mut c = [or, og, ob];
        if luminance < 0.1 {
            c = [
                or + r.double() as f32 * 0.125,
                og + r.double() as f32 * 0.125,
                ob + r.double() as f32 * 0.125,
            ];
        }
        let size = mana_size + (r.double() as f32 - 0.5) * 0.065 + wobble;
        out.push(Wisp::new(at, vel, 0.2 * size, c, 1.0));
        at = [
            at[0] + norm[0] * DISTANCE,
            at[1] + norm[1] * DISTANCE,
            at[2] + norm[2] * DISTANCE,
        ];
        let left = [prev[0] - at[0], prev[1] - at[1], prev[2] - at[2]];
        let length = (left[0] * left[0] + left[1] * left[1] + left[2] * left[2]).sqrt();
        // A runaway walk (a burst that jumped) stops where Botania's would have long ended.
        if length <= DISTANCE || out.len() >= 256 {
            break size;
        }
    };
    let v = [
        f64::from((r.double() - 0.5) as f32 * 0.06),
        f64::from((r.double() - 0.5) as f32 * 0.06),
        f64::from((r.double() - 0.5) as f32 * 0.06),
    ];
    out.push(Wisp::new(at, v, 0.1 * size, [or, og, ob], 1.0));
    out
}

/// `ManaBurstEntity.handleEntityEvent(DEATH)`: four wisps of the burst's colour, `0.15 * size`
/// (the mana left over the mana it started with), scattering a little, and a sparkle of size 4.
pub fn burst_death(pos: [f64; 3], color: u32, size: f32, r: &mut impl Rng) -> (Vec<Wisp>, Sparkle) {
    let c = rgb(color);
    let mut wisps = Vec::with_capacity(4);
    for _ in 0..4 {
        let v = [
            f64::from((r.double() - 0.5) as f32 * 0.04),
            f64::from((r.double() - 0.5) as f32 * 0.04),
            f64::from((r.double() - 0.5) as f32 * 0.04),
        ];
        wisps.push(Wisp::new(pos, v, 0.15 * size, c, 1.0));
    }
    (wisps, Sparkle::new(pos, 4.0, c, 2))
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    // `Vec3.normalize`: zero below 1e-4.
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length < 1.0e-4 {
        [0.0; 3]
    } else {
        [v[0] / length, v[1] / length, v[2] / length]
    }
}

// ---- blocks ----------------------------------------------------------------------------------

/// `ManaPoolBlockEntity.clientTick`'s wisp: with a chance of a tenth of how full the pool is, one
/// of the pool's colour rises from over it, up to a third in size and living twice as long.
/// Draws the chance first, then the size, the place and the speed, as Botania does.
pub fn pool_wisp(block: [i32; 3], mana: i32, max_mana: i32, r: &mut impl Rng) -> Option<Wisp> {
    if max_mana <= 0 {
        return None;
    }
    let chance = 1.0 - f64::from(mana) / f64::from(max_mana) * 0.1;
    if r.double() <= chance {
        return None;
    }
    let size = r.double() as f32 / 3.0;
    let pos = [
        f64::from(block[0]) + 0.3 + r.double() * 0.5,
        f64::from(block[1]) + 0.6 + r.double() * 0.25,
        f64::from(block[2]) + r.double(),
    ];
    let vel = [0.0, f64::from(r.double() as f32 / 25.0), 0.0];
    Some(Wisp::new(pos, vel, size, rgb(POOL_PARTICLE_COLOR), 2.0))
}

/// `GeneratingFlowerBlockEntity.tickFlower`'s sparkle: with a chance of how full the flower is
/// over 3.5, one of the flower's colour over it (`BotaniaAPI.sparkleFX`, `m` 5).
pub fn generating_sparkle(
    block: [i32; 3],
    mana: i32,
    max_mana: i32,
    color: u32,
    r: &mut impl Rng,
) -> Option<Sparkle> {
    if max_mana <= 0 {
        return None;
    }
    let chance = 1.0 - f64::from(mana) / f64::from(max_mana) / 3.5;
    if r.double() <= chance {
        return None;
    }
    let pos = [
        f64::from(block[0]) + 0.3 + r.double() * 0.5,
        f64::from(block[1]) + 0.5 + r.double() * 0.5,
        f64::from(block[2]) + 0.3 + r.double() * 0.5,
    ];
    let size = r.double() as f32;
    Some(Sparkle::new(pos, size, rgb(color), 5))
}

/// The `CRAFT_EFFECT_EVENT` of the mana pool (`height` 0.75) and of the apothecary (1): 25
/// sparkles of random colours and sizes over the middle of the block.
pub fn craft_sparkles(block: [i32; 3], height: f64, r: &mut impl Rng) -> Vec<Sparkle> {
    (0..25)
        .map(|_| {
            let c = [r.double() as f32, r.double() as f32, r.double() as f32];
            let size = r.double() as f32;
            let pos = [
                f64::from(block[0]) + 0.5 + r.double() * 0.4 - 0.2,
                f64::from(block[1]) + height,
                f64::from(block[2]) + 0.5 + r.double() * 0.4 - 0.2,
            ];
            Sparkle::new(pos, size, c, 10)
        })
        .collect()
}

/// `addManaFlowParticle` for one of a pool's charging (`displayChargingParticles`) wisps: from
/// the pool to the item lying in it (`charging`) or from the item to the pool, along a parabola
/// that falls onto its end. `param` is the item's place (`encodeRelativeItemPosition`) and
/// `relative` how full the pool is. No bellows are ported, so one wisp a call.
pub fn flow_wisp(
    pool: [i32; 3],
    param: i32,
    relative: f32,
    charging: bool,
    r: &mut impl Rng,
) -> Wisp {
    let base = mana_item::decode_item_position(param, relative);
    let item = mana_item::randomize_item_pos(base, [r.double(), r.double(), r.double()]);
    let in_pool = mana_item::pool_flow_pos(charging, relative, [r.double(), r.double()]);
    let (start, end) = if charging {
        (in_pool, item)
    } else {
        (item, in_pool)
    };
    let (v0, lifetime) = mana_item::flow(start, end, 0.05 * r.double());
    let pos = [
        f64::from(pool[0]) + start[0],
        f64::from(pool[1]) + start[1],
        f64::from(pool[2]) + start[2],
    ];
    Wisp {
        gravity: mana_item::CHARGING_GRAVITY as f32,
        no_clip: true,
        ..Wisp::new(
            pos,
            v0,
            0.1,
            rgb(POOL_PARTICLE_COLOR),
            (0.025 * lifetime) as f32,
        )
    }
}

/// `PureDaisyBlockEntity.tickFlower` on the client: a white sparkle in each block it is turning
/// (`mask` bit `i` for [`crate::daisy::POSITIONS`]`[i]`), every tick.
pub fn daisy_sparkles(daisy: [i32; 3], mask: u8, r: &mut impl Rng) -> Vec<Sparkle> {
    let mut out = Vec::new();
    for (i, o) in crate::daisy::POSITIONS.iter().enumerate() {
        if mask & (1 << i) == 0 {
            continue;
        }
        let size = r.double() as f32;
        let pos = [
            f64::from(daisy[0] + o[0]) + r.double(),
            f64::from(daisy[1] + o[1]) + r.double(),
            f64::from(daisy[2] + o[2]) + r.double(),
        ];
        out.push(Sparkle::new(pos, size, [1.0; 3], 5));
    }
    out
}

/// `PureDaisyBlockEntity`'s `RECIPE_COMPLETE_EVENT`: 25 white wisps up to half a block in size in
/// and over the block it turned.
pub fn daisy_wisps(block: [i32; 3], r: &mut impl Rng) -> Vec<Wisp> {
    (0..25)
        .map(|_| {
            let pos = [
                f64::from(block[0]) + r.double(),
                f64::from(block[1]) + r.double() + 0.5,
                f64::from(block[2]) + r.double(),
            ];
            let size = r.double() as f32 / 2.0;
            Wisp::new(pos, [0.0; 3], size, [1.0; 3], 1.0)
        })
        .collect()
}

/// `MysticalPetalItem.getPetalLikeColor`: the colour of a mystical petal (`botania:<dye>_petal`),
/// which its sparkles in an apothecary take (`CustomApothecaryColor`).
pub fn petal_color(item: &str) -> Option<u32> {
    let dye = item.strip_prefix("botania:")?.strip_suffix("_petal")?;
    Some(match dye {
        "white" => 0xF0FFFF,
        "orange" => 0xF88A10,
        "magenta" => 0xDC04D7,
        "light_blue" => 0x72C4FF,
        "yellow" => 0xFFF148,
        "lime" => 0x62FF2C,
        "pink" => 0xFB8BC7,
        "gray" => 0x71747C,
        "light_gray" => 0x9DA8A7,
        "cyan" => 0x34B0D0,
        "purple" => 0x950BA8,
        "blue" => 0x2448EB,
        "brown" => 0x976405,
        "green" => 0x3AAB0D,
        "red" => 0xDD111F,
        "black" => 0x261E28,
        _ => return None,
    })
}

/// `PetalApothecaryBlockEntity.clientTick` for one item in the apothecary: three ticks in a
/// hundred a sparkle of the petal's colour (grey for anything else) over the middle of the
/// water, and with it, one time in four, a splash (the second value).
pub fn apothecary_sparkle(
    block: [i32; 3],
    item: &str,
    r: &mut impl Rng,
) -> Option<(Sparkle, bool)> {
    if r.double() < 0.97 {
        return None;
    }
    let color = petal_color(item).unwrap_or(0x888888);
    let splash = r.double() >= 0.75;
    let size = r.double() as f32;
    let pos = [
        f64::from(block[0]) + 0.5 + r.double() * 0.4 - 0.2,
        f64::from(block[1]) + 1.2,
        f64::from(block[2]) + 0.5 + r.double() * 0.4 - 0.2,
    ];
    Some((Sparkle::new(pos, size, rgb(color), 10), splash))
}

// ---- items -----------------------------------------------------------------------------------

/// `WandOfTheForestItem.doParticleBeam`: a sparkle every 0.05 blocks from `from` to `to`, going
/// once round the rainbow from `hue` (`Math.random()` in Botania).
pub fn beam(from: [f64; 3], to: [f64; 3], hue: f32) -> Vec<Sparkle> {
    let diff = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let n = normalize(diff);
    let movement = [n[0] * 0.05, n[1] * 0.05, n[2] * 0.05];
    let step =
        (movement[0] * movement[0] + movement[1] * movement[1] + movement[2] * movement[2]).sqrt();
    let length = (diff[0] * diff[0] + diff[1] * diff[1] + diff[2] * diff[2]).sqrt();
    if step <= 0.0 {
        return Vec::new();
    }
    // A bound for a beam across a loaded world (Botania has none).
    let iters = ((length / step) as i32).clamp(0, 4096);
    let per = 1.0 / iters as f32;
    let mut at = from;
    let mut out = Vec::with_capacity(iters as usize);
    for i in 0..iters {
        let h = i as f32 * per + hue;
        let c = crate::wand::hsv_to_rgb(h - h.floor(), 1.0, 1.0);
        out.push(Sparkle {
            no_clip: true,
            ..Sparkle::new(at, 0.5, rgb(c), 4)
        });
        at = [
            at[0] + movement[0],
            at[1] + movement[1],
            at[2] + movement[2],
        ];
    }
    out
}

/// `FallingStarEntity.tick`'s sparkles: ten of size 2 within 0.75 blocks of the star.
pub fn star_sparkles(pos: [f64; 3], r: &mut impl Rng) -> Vec<Sparkle> {
    (0..10)
        .map(|_| {
            let o = [
                (r.double() - 0.5) as f32 * 1.5,
                (r.double() - 0.5) as f32 * 1.5,
                (r.double() - 0.5) as f32 * 1.5,
            ];
            let at = [
                pos[0] + f64::from(o[0]),
                pos[1] + f64::from(o[1]),
                pos[2] + f64::from(o[2]),
            ];
            Sparkle::new(at, 2.0, [1.0, 0.4, 1.0], 6)
        })
        .collect()
}

/// The Thundercaller's bolt (`ClientProxy.lightningFX`): `BoltParticleOptions(start, end)` with
/// Botania's default render info, size 0.08, 30 ticks, fading in and out over half of them.
pub const BOLT_SIZE: f32 = 0.08;
pub const BOLT_LIFESPAN: i32 = 30;
/// `BoltRenderInfo.DEFAULT`'s colour, RGBA.
pub const BOLT_COLOR: [f32; 4] = [0.0, 0.7764, 1.0, 0.8];

/// One segment of a bolt: from, to, and its width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoltSegment {
    pub from: [f64; 3],
    pub to: [f64; 3],
    pub size: f32,
}

/// `BoltParticleOptions.generate` for one bolt with `BoltRenderInfo.DEFAULT` (parallel noise 0.1,
/// spread 0.1, no branches, gaussian randoms, a sine spread, no memory) and size 0.08: its
/// segments from `start` to `end`, `sqrt(distance * 100)` of them, each one ending off the
/// straight line by up to a tenth of the bolt's length times `sin(pi * progress)`, and getting
/// thinner towards the end. Draws its randoms in Botania's order.
pub fn bolt(start: [f64; 3], end: [f64; 3], r: &mut impl Rng) -> Vec<BoltSegment> {
    const PARALLEL_NOISE: f32 = 0.1;
    const SPREAD_FACTOR: f32 = 0.1;
    let diff = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    let distance = (diff[0] * diff[0] + diff[1] * diff[1] + diff[2] * diff[2]).sqrt();
    let segments = (distance * 100.0).sqrt() as i32;
    if segments < 1 || !distance.is_finite() {
        return Vec::new();
    }
    let total = distance as f32;
    let mut out = Vec::new();
    let mut from = start;
    let mut progress = 0.0f32;
    // A bound Botania doesn't have: the progress always reaches 1 within about 1.1 * segments.
    for _ in 0..(segments * 2 + 2) {
        progress +=
            (1.0 / segments as f32) * (1.0 - PARALLEL_NOISE + r.float() * PARALLEL_NOISE * 2.0);
        let spread = (std::f64::consts::PI * f64::from(progress)).sin() as f32;
        let to = if progress >= 1.0 && spread <= 0.0 {
            end
        } else {
            let max_diff = SPREAD_FACTOR * spread * total;
            // `findRandomOrthogonalVector`.
            let other = [r.double() - 0.5, r.double() - 0.5, r.double() - 0.5];
            let away = normalize(cross(diff, other));
            let rand = f64::from(r.gaussian() as f32);
            let scale = f64::from(max_diff) * rand;
            let p = f64::from(progress);
            [
                start[0] + diff[0] * p + away[0] * scale,
                start[1] + diff[1] * p + away[1] * scale,
                start[2] + diff[2] * p + away[2] * scale,
            ]
        };
        let size = BOLT_SIZE * (0.5 + (1.0 - progress) * 0.5);
        out.push(BoltSegment { from, to, size });
        if progress >= 1.0 {
            break;
        }
        // The branch roll (`branchInitiationFactor` 0: never a branch).
        r.float();
        from = to;
    }
    out
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// When segment `i` of a bolt of `n` shows: `FadeFunction.fade(0.5)` over its 30 ticks draws the
/// segments from the first up to `n * age / 15` while it grows, and from `n * (age - 15) / 15`
/// up to the last while it fades, until it ends at 30. The first tick it shows and the first it
/// doesn't, counted from its start (whole ticks).
pub fn bolt_window(i: usize, n: usize) -> (i32, i32) {
    let bounds = |t: i32| -> (usize, usize) {
        let life = t as f32 / BOLT_LIFESPAN as f32;
        let fade = 0.5f32;
        let start = if life > 1.0 - fade {
            (n as f32 * (life - (1.0 - fade)) / fade) as usize
        } else {
            0
        };
        let end = if life < fade {
            (n as f32 * (life / fade)) as usize
        } else {
            n
        };
        (start, end)
    };
    let shown = |t: i32| {
        let (s, e) = bounds(t);
        s <= i && i < e
    };
    let on = (0..BOLT_LIFESPAN)
        .find(|&t| shown(t))
        .unwrap_or(BOLT_LIFESPAN);
    let off = (on..BOLT_LIFESPAN)
        .find(|&t| !shown(t))
        .unwrap_or(BOLT_LIFESPAN);
    (on, off)
}

// ---- what the server sends -------------------------------------------------------------------

/// The mod message channel the records go on.
pub const CHANNEL: &str = "botania:fx";
/// The first byte of every message: the records' format.
pub const FORMAT: u8 = 1;

/// One effect a client draws, as the server tells it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Record {
    /// A burst's trail this tick: where it is, where it was, its motion, colour, the mana left
    /// over the mana it started with, and its [`burst_wobble`].
    Burst {
        pos: [f64; 3],
        prev: [f64; 3],
        motion: [f32; 3],
        color: u32,
        size: f32,
        wobble: f32,
    },
    /// A burst died here ([`burst_death`]).
    BurstDeath {
        pos: [f64; 3],
        color: u32,
        size: f32,
    },
    Wisp(Wisp),
    Sparkle(Sparkle),
    /// [`craft_sparkles`] over a block at `height`.
    Craft {
        block: [i32; 3],
        height: f32,
    },
    /// A pool's charging (or draining) animation for one item in it: 15 ticks, a
    /// [`flow_wisp`] every second one (`CHARGE_EFFECT_EVENT`, `DRAIN_EFFECT_EVENT`).
    Flow {
        pool: [i32; 3],
        param: u8,
        relative: f32,
        charging: bool,
    },
    /// The wand's rainbow [`beam`].
    Beam {
        from: [f64; 3],
        to: [f64; 3],
    },
    /// A falling star's [`star_sparkles`] this tick.
    Star {
        pos: [f64; 3],
    },
    /// A Thundercaller [`bolt`].
    Bolt {
        from: [f64; 3],
        to: [f64; 3],
    },
    /// A Pure Daisy's [`daisy_sparkles`] this tick.
    Daisy {
        daisy: [i32; 3],
        mask: u8,
    },
    /// A Pure Daisy turned this block ([`daisy_wisps`]).
    DaisyDone {
        block: [i32; 3],
    },
}

impl Record {
    /// Where it happens, and how much farther than that point it reaches: a player within 32
    /// blocks of what it draws is sent it (`ParticleEngine` draws nothing farther).
    pub fn anchor(&self) -> ([f64; 3], f64) {
        let block = |b: [i32; 3]| {
            [
                f64::from(b[0]) + 0.5,
                f64::from(b[1]) + 0.5,
                f64::from(b[2]) + 0.5,
            ]
        };
        let span = |a: [f64; 3], b: [f64; 3]| {
            let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
        };
        match *self {
            Record::Burst { pos, prev, .. } => (pos, span(pos, prev)),
            Record::BurstDeath { pos, .. } | Record::Star { pos } => (pos, 1.0),
            Record::Wisp(w) => (w.pos, 0.0),
            Record::Sparkle(s) => (s.pos, 0.0),
            Record::Craft { block: b, .. }
            | Record::Flow { pool: b, .. }
            | Record::DaisyDone { block: b } => (block(b), 1.0),
            Record::Daisy { daisy, .. } => (block(daisy), 2.0),
            Record::Beam { from, to } | Record::Bolt { from, to } => (from, span(from, to)),
        }
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        let mut w = Writer(out);
        match *self {
            Record::Burst {
                pos,
                prev,
                motion,
                color,
                size,
                wobble,
            } => {
                w.u8(1);
                w.pos(pos);
                w.pos(prev);
                motion.iter().for_each(|v| w.f32(*v));
                w.u32(color);
                w.f32(size);
                w.f32(wobble);
            }
            Record::BurstDeath { pos, color, size } => {
                w.u8(2);
                w.pos(pos);
                w.u32(color);
                w.f32(size);
            }
            Record::Wisp(p) => {
                w.u8(3);
                w.pos(p.pos);
                p.vel.iter().for_each(|v| w.f32(*v as f32));
                w.f32(p.size);
                p.rgb.iter().for_each(|v| w.f32(*v));
                w.f32(p.max_age_mul);
                w.f32(p.gravity);
                w.u8(u8::from(p.no_clip));
            }
            Record::Sparkle(p) => {
                w.u8(4);
                w.pos(p.pos);
                w.f32(p.size);
                p.rgb.iter().for_each(|v| w.f32(*v));
                w.i32(p.m);
                w.u8(u8::from(p.no_clip) | (u8::from(p.fake) << 1));
            }
            Record::Craft { block, height } => {
                w.u8(5);
                w.block(block);
                w.f32(height);
            }
            Record::Flow {
                pool,
                param,
                relative,
                charging,
            } => {
                w.u8(6);
                w.block(pool);
                w.u8(param);
                w.f32(relative);
                w.u8(u8::from(charging));
            }
            Record::Beam { from, to } => {
                w.u8(7);
                w.pos(from);
                w.pos(to);
            }
            Record::Star { pos } => {
                w.u8(8);
                w.pos(pos);
            }
            Record::Bolt { from, to } => {
                w.u8(9);
                w.pos(from);
                w.pos(to);
            }
            Record::Daisy { daisy, mask } => {
                w.u8(10);
                w.block(daisy);
                w.u8(mask);
            }
            Record::DaisyDone { block } => {
                w.u8(11);
                w.block(block);
            }
        }
    }
}

/// The records of one message ([`FORMAT`] first). Stops at the first it can't read: a message of
/// another format, or cut short, gives what came before.
pub fn decode(data: &[u8]) -> Vec<Record> {
    let mut r = Reader { data, at: 0 };
    let mut out = Vec::new();
    if r.u8() != Some(FORMAT) {
        return out;
    }
    while r.at < data.len() {
        match read_record(&mut r) {
            Some(record) => out.push(record),
            None => break,
        }
    }
    out
}

fn read_record(r: &mut Reader<'_>) -> Option<Record> {
    Some(match r.u8()? {
        1 => Record::Burst {
            pos: r.pos()?,
            prev: r.pos()?,
            motion: [r.f32()?, r.f32()?, r.f32()?],
            color: r.u32()?,
            size: r.f32()?,
            wobble: r.f32()?,
        },
        2 => Record::BurstDeath {
            pos: r.pos()?,
            color: r.u32()?,
            size: r.f32()?,
        },
        3 => Record::Wisp(Wisp {
            pos: r.pos()?,
            vel: [
                f64::from(r.f32()?),
                f64::from(r.f32()?),
                f64::from(r.f32()?),
            ],
            size: r.f32()?,
            rgb: [r.f32()?, r.f32()?, r.f32()?],
            max_age_mul: r.f32()?,
            gravity: r.f32()?,
            no_clip: r.u8()? != 0,
        }),
        4 => {
            let pos = r.pos()?;
            let size = r.f32()?;
            let rgb = [r.f32()?, r.f32()?, r.f32()?];
            let m = r.i32()?;
            let flags = r.u8()?;
            Record::Sparkle(Sparkle {
                pos,
                size,
                rgb,
                m,
                no_clip: flags & 1 != 0,
                fake: flags & 2 != 0,
            })
        }
        5 => Record::Craft {
            block: r.block()?,
            height: r.f32()?,
        },
        6 => Record::Flow {
            pool: r.block()?,
            param: r.u8()?,
            relative: r.f32()?,
            charging: r.u8()? != 0,
        },
        7 => Record::Beam {
            from: r.pos()?,
            to: r.pos()?,
        },
        8 => Record::Star { pos: r.pos()? },
        9 => Record::Bolt {
            from: r.pos()?,
            to: r.pos()?,
        },
        10 => Record::Daisy {
            daisy: r.block()?,
            mask: r.u8()?,
        },
        11 => Record::DaisyDone { block: r.block()? },
        _ => return None,
    })
}

struct Writer<'a>(&'a mut Vec<u8>);

impl Writer<'_> {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn pos(&mut self, p: [f64; 3]) {
        for v in p {
            self.0.extend_from_slice(&v.to_le_bytes());
        }
    }
    fn block(&mut self, b: [i32; 3]) {
        b.iter().for_each(|v| self.i32(*v));
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let bytes = self.data.get(self.at..self.at + N)?;
        self.at += N;
        bytes.try_into().ok()
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> Option<f32> {
        let v = f32::from_le_bytes(self.take()?);
        v.is_finite().then_some(v)
    }
    fn pos(&mut self) -> Option<[f64; 3]> {
        let mut p = [0.0; 3];
        for v in &mut p {
            *v = f64::from_le_bytes(self.take()?);
            if !v.is_finite() {
                return None;
            }
        }
        Some(p)
    }
    fn block(&mut self) -> Option<[i32; 3]> {
        Some([self.i32()?, self.i32()?, self.i32()?])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values below are worked out by hand from Botania's code (`FXWisp`, `FXSparkle`,
    // `ManaBurstEntity.particles`, `BoltParticleOptions`, `WandOfTheForestItem.doParticleBeam`).

    /// Hands out the same numbers every time: a float, a double and a gaussian.
    struct Fixed(f32, f64, f64);
    impl Rng for Fixed {
        fn float(&mut self) -> f32 {
            self.0
        }
        fn double(&mut self) -> f64 {
            self.1
        }
        fn gaussian(&mut self) -> f64 {
            self.2
        }
    }

    /// Hands out these floats in turn.
    struct Floats(Vec<f32>);
    impl Rng for Floats {
        fn float(&mut self) -> f32 {
            self.0.remove(0)
        }
        fn double(&mut self) -> f64 {
            f64::from(self.0.remove(0))
        }
        fn gaussian(&mut self) -> f64 {
            0.0
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn close3(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| close(a[i], b[i]))
    }

    #[test]
    fn wisps_live_28_to_40_ticks_and_swell_then_shrink() {
        // 28 / (0 * 0.3 + 0.7) = 40; 28 / 1.0 = 28; twice as long with maxAgeMul 2.
        assert_eq!(wisp_lifetime(0.0, 1.0), 40);
        assert_eq!(wisp_lifetime(1.0, 1.0), 28);
        assert_eq!(wisp_lifetime(0.0, 2.0), 80);
        // 28 / 0.85 * 0.25 = 8.23.
        assert_eq!(wisp_lifetime(0.5, 0.25), 8);
        // quadSize: (0.5 * 0.5 + 0.5) * 2 * 0.3.
        assert!(close(f64::from(wisp_quad_size(0.5, 0.3)), 0.45));
        // Half-life 20: nothing at first, a quarter at 10, half at 20, a quarter at 30.
        assert_eq!(wisp_size(1.0, 0, 20), 0.0);
        assert_eq!(wisp_size(1.0, 10, 20), 0.25);
        assert_eq!(wisp_size(1.0, 20, 20), 0.5);
        assert_eq!(wisp_size(1.0, 30, 20), 0.25);
        assert_eq!(wisp_size(1.0, 40, 20), 0.0);
    }

    #[test]
    fn sparkles_shrink_and_walk_through_their_four_sprites() {
        assert_eq!(sparkle_lifetime(2), 6);
        // (0.5 * 0.5 + 0.5) * 0.2 * 4.
        assert!(close(f64::from(sparkle_quad_size(0.5, 4.0)), 0.6));
        // quadSize * (6 - age + 1) / 6.
        assert!(close(f64::from(sparkle_size(0.6, 0, 6)), 0.7));
        assert!(close(f64::from(sparkle_size(0.6, 6, 6)), 0.1));
        // age * 3 / lifetime.
        let frames: Vec<usize> = (0..=6).map(|a| sprite_from_age(a, 6, 4)).collect();
        assert_eq!(frames, [0, 0, 1, 1, 2, 2, 3]);
    }

    #[test]
    fn a_sparkle_wiggles_towards_the_nearest_open_side() {
        let open = |_: [i32; 3]| false;
        // In the middle of its block's floor plan, low down: north, south, west and east are all
        // 0.5 away and up 0.8; north comes first. f = 0.5 * 0.05 + 0.025, secondary
        // (0.75 - 0.25) * 0.1, secondary2 0.
        let mut r = Floats(vec![0.5, 0.75, 0.25, 0.5, 0.5]);
        let v = sparkle_wiggle([0.5, 0.2, 0.5], open, &mut r);
        assert!(close3(v, [0.05, 0.0, -0.05]), "{v:?}");
        // North and west walled off: south.
        let walls = |b: [i32; 3]| b == [0, 0, -1] || b == [-1, 0, 0];
        let mut r = Floats(vec![0.5, 0.5, 0.5, 0.5, 0.5]);
        let v = sparkle_wiggle([0.5, 0.2, 0.5], walls, &mut r);
        assert!(close3(v, [0.0, 0.0, 0.05]), "{v:?}");
        // Near the top: up.
        let mut r = Floats(vec![0.5, 0.5, 0.5, 0.5, 0.5]);
        let v = sparkle_wiggle([0.5, 0.9, 0.5], open, &mut r);
        assert!(close3(v, [0.0, 0.05, 0.0]), "{v:?}");
        // Everything around full: up all the same.
        let mut r = Floats(vec![0.0, 0.5, 0.5, 0.5, 0.5]);
        let v = sparkle_wiggle([0.1, 0.5, 0.1], |_| true, &mut r);
        assert!(close3(v, [0.0, 0.025, 0.0]), "{v:?}");
    }

    #[test]
    fn a_burst_trail_walks_back_along_its_last_move() {
        // Moving 0.2 along x: wisps at 10.2 and 10.105 (10.01 is within 0.095 of 10.0, so the
        // walk stops there), then the small one at 10.01. Size 1 + (0.5 - 0.5) * 0.065 + 0.
        let mut r = Fixed(0.5, 0.5, 0.0);
        let w = burst_trail(
            [10.2, 64.5, 0.5],
            [10.0, 64.5, 0.5],
            [0.2, 0.0, 0.0],
            0x20FF20,
            1.0,
            0.0,
            &mut r,
        );
        assert_eq!(w.len(), 3);
        assert!(close3(w[0].pos, [10.2, 64.5, 0.5]));
        assert!(close3(w[1].pos, [10.105, 64.5, 0.5]));
        assert!(close3(w[2].pos, [10.01, 64.5, 0.5]));
        assert!(close(f64::from(w[0].size), 0.2));
        assert!(close(f64::from(w[2].size), 0.1));
        // (float) -0.2 * 0.01F, against the motion; the last one's (0.5 - 0.5) * 0.06.
        assert!(close3(w[0].vel, [-0.002, 0.0, 0.0]));
        assert_eq!(w[2].vel, [0.0; 3]);
        assert_eq!(w[0].rgb, rgb(0x20FF20));
        assert_eq!(w[0].max_age_mul, 1.0);
        // A burst that didn't move: one wisp and the small one.
        let w = burst_trail([0.0; 3], [0.0; 3], [0.0; 3], 0x20FF20, 0.5, 0.2, &mut r);
        assert_eq!(w.len(), 2);
        // 0.2 * (0.5 + 0 + 0.2).
        assert!(close(f64::from(w[0].size), 0.14));
    }

    #[test]
    fn a_dark_burst_is_lightened_and_wobbles() {
        // Luminance 0 < 0.1: each channel + 0.5 * 0.125. The wobble sin(n) * 0.4 adds to the size.
        let mut r = Fixed(0.5, 0.5, 0.0);
        let wobble = burst_wobble(0);
        assert_eq!(wobble, 0.0);
        let w = burst_trail([0.0; 3], [0.0; 3], [0.0; 3], 0x000000, 1.0, wobble, &mut r);
        assert_eq!(w[0].rgb, [0.0625; 3]);
        assert_eq!(w[1].rgb, [0.0; 3]);
        // sin(9000) * 0.4.
        assert!(close(f64::from(burst_wobble(9000)), 9000f64.sin() * 0.4));
    }

    #[test]
    fn a_burst_dies_in_four_wisps_and_a_sparkle() {
        let mut r = Fixed(0.5, 0.75, 0.0);
        let (wisps, sparkle) = burst_death([1.0, 2.0, 3.0], 0xFF45C4, 0.5, &mut r);
        assert_eq!(wisps.len(), 4);
        // 0.15 * 0.5; (0.75 - 0.5) * 0.04.
        assert!(close(f64::from(wisps[0].size), 0.075));
        assert!(close3(wisps[0].vel, [0.01, 0.01, 0.01]));
        assert_eq!(
            sparkle,
            Sparkle::new([1.0, 2.0, 3.0], 4.0, rgb(0xFF45C4), 2)
        );
    }

    #[test]
    fn a_full_pool_breathes_wisps_a_tenth_of_its_ticks() {
        // Full: chance 1 - 0.1 = 0.9.
        assert!(pool_wisp([0, 64, 0], 1_000_000, 1_000_000, &mut Fixed(0.0, 0.85, 0.0)).is_none());
        let w = pool_wisp([0, 64, 0], 1_000_000, 1_000_000, &mut Fixed(0.0, 0.95, 0.0)).unwrap();
        // Size 0.95 / 3, at (0.3 + 0.475, 64.6 + 0.2375, 0.95), rising 0.95 / 25, living twice as
        // long, in 0x00C6FF.
        assert!(close(f64::from(w.size), 0.95 / 3.0));
        assert!(close3(w.pos, [0.775, 64.8375, 0.95]));
        assert!(close(w.vel[1], 0.038));
        assert_eq!(w.max_age_mul, 2.0);
        assert_eq!(w.rgb, rgb(0x00C6FF));
        // Empty: never.
        assert!(pool_wisp([0, 64, 0], 0, 1_000_000, &mut Fixed(0.0, 0.999, 0.0)).is_none());
    }

    #[test]
    fn a_full_endoflame_sparkles_more_than_a_quarter_of_its_ticks() {
        // 300 of 300: chance 1 - 1 / 3.5 = 0.714.
        let s = generating_sparkle([0, 64, 0], 300, 300, 0x785000, &mut Fixed(0.0, 0.8, 0.0));
        let s = s.unwrap();
        assert_eq!(s.m, 5);
        assert_eq!(s.rgb, rgb(0x785000));
        assert!(close3(s.pos, [0.7, 64.9, 0.7]));
        let none = generating_sparkle([0, 64, 0], 300, 300, 0x785000, &mut Fixed(0.0, 0.7, 0.0));
        assert!(none.is_none());
    }

    #[test]
    fn crafting_throws_25_sparkles_over_the_block() {
        let s = craft_sparkles([2, 64, 2], 0.75, &mut Fixed(0.0, 0.5, 0.0));
        assert_eq!(s.len(), 25);
        assert!(
            s.iter()
                .all(|s| s.m == 10 && close3(s.pos, [2.5, 64.75, 2.5]))
        );
    }

    #[test]
    fn petals_sparkle_in_their_colour_three_ticks_in_a_hundred() {
        assert_eq!(petal_color("botania:light_blue_petal"), Some(0x72C4FF));
        assert_eq!(petal_color("botania:white_petal"), Some(0xF0FFFF));
        assert_eq!(petal_color("minecraft:wheat_seeds"), None);
        assert!(
            apothecary_sparkle([0, 64, 0], "botania:red_petal", &mut Fixed(0.0, 0.96, 0.0))
                .is_none()
        );
        let (s, splash) =
            apothecary_sparkle([0, 64, 0], "botania:red_petal", &mut Fixed(0.0, 0.98, 0.0))
                .unwrap();
        assert!(splash);
        assert_eq!((s.rgb, s.m), (rgb(0xDD111F), 10));
        // 0.5 + 0.98 * 0.4 - 0.2, 1.2 up.
        assert!(close3(s.pos, [0.692, 65.2, 0.692]));
        let (s, _) =
            apothecary_sparkle([0, 64, 0], "minecraft:stick", &mut Fixed(0.0, 0.98, 0.0)).unwrap();
        assert_eq!(s.rgb, rgb(0x888888));
    }

    #[test]
    fn a_charging_wisp_starts_in_the_pool_and_falls_onto_the_item() {
        // An item at the bottom middle (x 4, y 0, z 4 → param 4 | 0 << 3 | 4 << 5), a half full
        // pool.
        let param = 4 | (4 << 5);
        let w = flow_wisp([0, 64, 0], param, 0.5, true, &mut Fixed(0.0, 0.5, 0.0));
        // The pool's point: 0.1 + 0.8 * 0.5, 0.1 + 0.4 * 0.5.
        assert!(close3(w.pos, [0.5, 64.3, 0.5]));
        assert!(w.no_clip);
        assert_eq!(w.gravity, 0.003);
        assert_eq!(w.size, 0.1);
        let (_, lifetime) = mana_item::flow(
            [0.5, 0.3, 0.5],
            mana_item::randomize_item_pos(
                mana_item::decode_item_position(param, 0.5),
                [0.5, 0.5, 0.5],
            ),
            0.025,
        );
        assert!(close(f64::from(w.max_age_mul), 0.025 * lifetime));
    }

    #[test]
    fn a_daisy_sparkles_over_the_blocks_it_turns() {
        let s = daisy_sparkles([0, 64, 0], 0b1000_0001, &mut Fixed(0.0, 0.5, 0.0));
        // POSITIONS[0] (-1, 0, -1) and POSITIONS[7] (0, 0, -1), plus 0.5 each way.
        assert_eq!(s.len(), 2);
        assert!(close3(s[0].pos, [-0.5, 64.5, -0.5]));
        assert!(close3(s[1].pos, [0.5, 64.5, -0.5]));
        assert_eq!(s[0].rgb, [1.0; 3]);
        let w = daisy_wisps([3, 64, 3], &mut Fixed(0.0, 0.5, 0.0));
        assert_eq!(w.len(), 25);
        assert!(close3(w[0].pos, [3.5, 65.0, 3.5]));
        assert_eq!(w[0].size, 0.25);
    }

    #[test]
    fn the_wand_beam_is_a_rainbow_of_sparkles_every_twentieth_of_a_block() {
        let b = beam([0.0, 64.0, 0.0], [1.0, 64.0, 0.0], 0.0);
        // 1 / 0.05 = 20 sparkles, red first, a third of the way round green.
        assert_eq!(b.len(), 20);
        assert!(close3(b[1].pos, [0.05, 64.0, 0.0]));
        assert_eq!(b[0].rgb, [1.0, 0.0, 0.0]);
        assert!(b.iter().all(|s| s.no_clip && s.size == 0.5 && s.m == 4));
        assert!(beam([0.0; 3], [0.0; 3], 0.0).is_empty());
    }

    #[test]
    fn a_falling_star_sparkles_around_itself() {
        let s = star_sparkles([0.0, 100.0, 0.0], &mut Fixed(0.0, 1.0, 0.0));
        assert_eq!(s.len(), 10);
        // (1 - 0.5) * 1.5 = 0.75 out on each axis.
        assert!(close3(s[0].pos, [0.75, 100.75, 0.75]));
        assert_eq!((s[0].size, s[0].rgb, s[0].m), (2.0, [1.0, 0.4, 1.0], 6));
    }

    #[test]
    fn a_straight_bolt_has_sqrt_100_d_segments_thinning_to_its_end() {
        // 4 blocks: (int) sqrt(400) = 20 segments, each 1/20 * (0.9 + 0.5 * 0.2) = 0.05 along; a
        // gaussian of 0 keeps them on the line. Twenty 0.05F add up past 1, so the 20th ends on
        // `end`.
        let s = bolt(
            [0.0, 64.0, 0.0],
            [4.0, 64.0, 0.0],
            &mut Fixed(0.5, 0.5, 0.0),
        );
        assert_eq!(s.len(), 20);
        assert!(close3(s[0].from, [0.0, 64.0, 0.0]));
        assert!(close3(s[0].to, [0.2, 64.0, 0.0]));
        assert_eq!(s[1].from, s[0].to);
        assert_eq!(s[19].to, [4.0, 64.0, 0.0]);
        // 0.08 * (0.5 + (1 - 0.05) * 0.5).
        assert!(close(f64::from(s[0].size), 0.078));
        assert!(s.iter().all(|q| q.from[1] == 64.0 && q.to[2] == 0.0));
        // Too short for a segment.
        assert!(bolt([0.0; 3], [0.0, 0.005, 0.0], &mut Fixed(0.5, 0.5, 0.0)).is_empty());
    }

    #[test]
    fn a_bolt_grows_for_15_ticks_and_fades_from_its_start() {
        // Of 20: the first shows from tick 1 (20 * (1/30) / 0.5 = 1.33) until the fade passes it
        // at tick 16; the last from tick 15 until the bolt ends at 30.
        assert_eq!(bolt_window(0, 20), (1, 16));
        assert_eq!(bolt_window(19, 20), (15, 30));
        // Every segment shows for 15 ticks, or 16 where Botania's float rounding keeps it a tick
        // longer (segment 7: 20 * (21 / 30F - 0.5) / 0.5 is 7.9999995).
        assert_eq!(bolt_window(7, 20), (6, 22));
        assert!((0..20).all(|i| {
            let (on, off) = bolt_window(i, 20);
            (15..=16).contains(&(off - on))
        }));
    }

    #[test]
    fn records_round_trip() {
        let all = [
            Record::Burst {
                pos: [1.5, -2.0, 3.25],
                prev: [1.0, -2.0, 3.0],
                motion: [0.5, 0.0, 0.25],
                color: 0x20FF20,
                size: 0.75,
                wobble: -0.3,
            },
            Record::BurstDeath {
                pos: [1.0, 2.0, 3.0],
                color: 0xFF45C4,
                size: 0.5,
            },
            Record::Wisp(Wisp {
                gravity: 0.003,
                no_clip: true,
                ..Wisp::new([1.0; 3], [0.5, -0.25, 0.0], 0.1, [0.0, 0.5, 1.0], 0.75)
            }),
            Record::Sparkle(Sparkle {
                fake: true,
                ..Sparkle::new([-4.0, 70.0, 9.0], 2.0, [1.0, 0.4, 1.0], 6)
            }),
            Record::Craft {
                block: [-30_000_000, -64, 29_999_999],
                height: 0.75,
            },
            Record::Flow {
                pool: [1, 2, 3],
                param: 0xA4,
                relative: 0.5,
                charging: true,
            },
            Record::Beam {
                from: [0.5; 3],
                to: [10.5, 64.5, -3.5],
            },
            Record::Star {
                pos: [100.0, 200.0, 300.0],
            },
            Record::Bolt {
                from: [0.0; 3],
                to: [4.0, 1.0, 0.0],
            },
            Record::Daisy {
                daisy: [5, 64, 5],
                mask: 0b1010_0101,
            },
            Record::DaisyDone { block: [6, 64, 5] },
        ];
        let mut data = vec![FORMAT];
        for r in &all {
            r.encode(&mut data);
        }
        assert_eq!(decode(&data), all);
        // Cut short: what came before the cut.
        assert_eq!(decode(&data[..data.len() - 1]), &all[..all.len() - 1]);
        // Another format: nothing.
        data[0] = FORMAT + 1;
        assert!(decode(&data).is_empty());
    }
}
