//! The mana burst (`ManaBurstEntity`, a `ThrowableProjectile` without drag or gravity) and the
//! vanilla 1.20.1 ray cast it flies by (`BlockGetter.clip` with `ClipContext.Block.COLLIDER`,
//! `VoxelShape.clip`, `AABB.clip`), with a spreader's aim (`ManaSpreaderBlockEntity.bindTo`) and
//! receiver scan (`checkForReceiver`).

use crate::consts::{SPREADER_TRACE_TIME, SpreaderStats};
use crate::mth;

/// A box in block-relative or absolute coordinates: `[min_x, min_y, min_z, max_x, max_y, max_z]`.
pub type Aabb = [f64; 6];

/// What a burst needs of the level.
pub trait BurstWorld {
    /// The boxes a burst collides with at `pos`, relative to the block (a pool's burst slab, a
    /// block's collision shape); empty when it flies through.
    fn boxes(&self, pos: [i32; 3]) -> Vec<Aabb>;
    /// The burst ignores a hit there (`BushBlock`, `LeavesBlock`, a mana ghost that skips all).
    fn passes(&self, pos: [i32; 3]) -> bool;
    /// A mana receiver that takes bursts is there (`findManaReceiver`, `canReceiveManaFromBursts`).
    fn receiver(&self, pos: [i32; 3]) -> bool;
}

/// `ManaBurstEntity.calculateBurstVelocity(xRot, yRot)`: 0.2 blocks a tick along the look, with
/// `Mth`'s table sine in float, halved in double.
pub fn velocity(x_rot: f32, y_rot: f32) -> [f64; 3] {
    let f = 0.4_f32;
    let rad = std::f32::consts::PI;
    let (yr, xr) = (y_rot / 180.0 * rad, x_rot / 180.0 * rad);
    let mx = f64::from(mth::sin(yr) * mth::cos(xr) * f) / 2.0;
    let mz = -f64::from(mth::cos(yr) * mth::cos(xr) * f) / 2.0;
    let my = f64::from(mth::sin(xr) * f) / 2.0;
    [mx, my, mz]
}

/// A spreader's burst velocity: its `rotationX`/`rotationY` become the entity's
/// `yRot = -(rotationX + 90)` and `xRot = rotationY`, scaled by the variant's motion.
pub fn spreader_velocity(rotation_x: f32, rotation_y: f32, motion: f32) -> [f64; 3] {
    let v = velocity(rotation_y, -(rotation_x + 90.0));
    let m = f64::from(motion);
    [v[0] * m, v[1] * m, v[2] * m]
}

/// Where a spreader's burst starts: just under the block's centre, so a level shot hits a pool
/// beside it (`ManaBurstEntity(Level, BlockPos, ...)`).
pub fn spawn_position(spreader: [i32; 3]) -> [f64; 3] {
    [
        f64::from(spreader[0]) + 0.5,
        f64::from(spreader[1]) + (0.5 - 1.0 / 1024.0),
        f64::from(spreader[2]) + 0.5,
    ]
}

/// A hit of the ray cast: the block and where along the ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub pos: [i32; 3],
    pub location: [f64; 3],
    /// The ray started inside the shape (`BlockHitResult.isInside`).
    pub inside: bool,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `AABB.clipPoint` for one face: the ray's parameter on the plane, if it lies on the face within
/// `1e-7` and before the best so far.
#[allow(clippy::too_many_arguments)]
fn clip_point(
    best: &mut f64,
    d: [f64; 3],
    s: [f64; 3],
    axis: usize,
    plane: f64,
    min: [f64; 2],
    max: [f64; 2],
) -> bool {
    let (a, b) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    let t = (plane - s[axis]) / d[axis];
    let p = s[a] + t * d[a];
    let q = s[b] + t * d[b];
    if 0.0 < t
        && t < *best
        && min[0] - 1.0e-7 < p
        && p < max[0] + 1.0e-7
        && min[1] - 1.0e-7 < q
        && q < max[1] + 1.0e-7
    {
        *best = t;
        true
    } else {
        false
    }
}

/// `AABB.clip(Iterable<AABB>, from, to, pos)`: the nearest entry into one of the boxes (moved to
/// `pos`), strictly between the ends.
fn aabb_clip(boxes: &[Aabb], from: [f64; 3], to: [f64; 3], pos: [i32; 3]) -> Option<[f64; 3]> {
    let mut best = 1.0_f64;
    let mut hit = false;
    let d = sub(to, from);
    let off = [f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])];
    for b in boxes {
        let lo = [b[0] + off[0], b[1] + off[1], b[2] + off[2]];
        let hi = [b[3] + off[0], b[4] + off[1], b[5] + off[2]];
        // `AABB.getDirection`: each axis's near face when the ray moves along it.
        for axis in 0..3 {
            let (a, c) = match axis {
                0 => (1, 2),
                1 => (0, 2),
                _ => (0, 1),
            };
            let plane = if d[axis] > 1.0e-7 {
                Some(lo[axis])
            } else if d[axis] < -1.0e-7 {
                Some(hi[axis])
            } else {
                None
            };
            if let Some(plane) = plane {
                hit |= clip_point(
                    &mut best,
                    d,
                    from,
                    axis,
                    plane,
                    [lo[a], lo[c]],
                    [hi[a], hi[c]],
                );
            }
        }
    }
    hit.then(|| {
        [
            from[0] + best * d[0],
            from[1] + best * d[1],
            from[2] + best * d[2],
        ]
    })
}

/// `VoxelShape.clip`: a hit at once when the start (nudged a thousandth along the ray) is inside
/// the shape, else the boxes' nearest entry.
fn shape_clip(boxes: &[Aabb], from: [f64; 3], to: [f64; 3], pos: [i32; 3]) -> Option<Hit> {
    if boxes.is_empty() {
        return None;
    }
    let d = sub(to, from);
    if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < 1.0e-7 {
        return None;
    }
    let n = [
        from[0] + d[0] * 0.001,
        from[1] + d[1] * 0.001,
        from[2] + d[2] * 0.001,
    ];
    let rel = [
        n[0] - f64::from(pos[0]),
        n[1] - f64::from(pos[1]),
        n[2] - f64::from(pos[2]),
    ];
    // `isFullWide(findIndex(...))`: the cell holding the point is filled; a coordinate on a
    // boundary belongs to the cell above it.
    let inside = boxes
        .iter()
        .any(|b| (0..3).all(|i| b[i] <= rel[i] && rel[i] < b[i + 3]));
    if inside {
        return Some(Hit {
            pos,
            location: n,
            inside: true,
        });
    }
    aabb_clip(boxes, from, to, pos).map(|location| Hit {
        pos,
        location,
        inside: false,
    })
}

/// `BlockGetter.clip` (via `traverseBlocks`) without fluids: the first block along the ray, in
/// the order the ray enters them, whose burst shape the ray meets.
pub fn clip(world: &impl BurstWorld, from: [f64; 3], to: [f64; 3]) -> Option<Hit> {
    if from == to {
        return None;
    }
    let tester = |p: [i32; 3]| shape_clip(&world.boxes(p), from, to, p);
    let end = [
        mth::lerp(-1.0e-7, to[0], from[0]),
        mth::lerp(-1.0e-7, to[1], from[1]),
        mth::lerp(-1.0e-7, to[2], from[2]),
    ];
    let start = [
        mth::lerp(-1.0e-7, from[0], to[0]),
        mth::lerp(-1.0e-7, from[1], to[1]),
        mth::lerp(-1.0e-7, from[2], to[2]),
    ];
    let mut b = [
        mth::floor(start[0]),
        mth::floor(start[1]),
        mth::floor(start[2]),
    ];
    if let Some(h) = tester(b) {
        return Some(h);
    }
    let dd = sub(end, start);
    let step = [mth::sign(dd[0]), mth::sign(dd[1]), mth::sign(dd[2])];
    let delta: [f64; 3] = std::array::from_fn(|i| {
        if step[i] == 0 {
            f64::MAX
        } else {
            f64::from(step[i]) / dd[i]
        }
    });
    let mut t: [f64; 3] = std::array::from_fn(|i| {
        delta[i]
            * if step[i] > 0 {
                1.0 - mth::frac(start[i])
            } else {
                mth::frac(start[i])
            }
    });
    while t[0] <= 1.0 || t[1] <= 1.0 || t[2] <= 1.0 {
        let axis = if t[0] < t[1] {
            if t[0] < t[2] { 0 } else { 2 }
        } else if t[1] < t[2] {
            1
        } else {
            2
        };
        b[axis] += step[axis];
        t[axis] += delta[axis];
        if let Some(h) = tester(b) {
            return Some(h);
        }
    }
    None
}

/// What a burst's tick did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BurstTick {
    /// It hit a block it doesn't pass (and died): the block, and whether a receiver took mana.
    pub hit: Option<[i32; 3]>,
    /// The receiver it hit, given `mana` (a real burst only).
    pub delivered: Option<([i32; 3], i32)>,
    /// It ran out of mana this tick.
    pub spent: bool,
}

/// A mana burst in flight.
#[derive(Clone, Debug, PartialEq)]
pub struct Burst {
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub mana: i32,
    pub starting_mana: i32,
    pub min_mana_loss: i32,
    pub mana_loss_per_tick: f32,
    pub accumulated_loss: f32,
    pub ticks_existed: i32,
    /// The spreader that shot it.
    pub source: [i32; 3],
    pub left_source: bool,
    pub last_collision: Option<[i32; 3]>,
    pub alive: bool,
    /// A receiver scan's burst: it delivers nothing.
    pub fake: bool,
    /// The receiver it last hit (`collidedTile`), for a scan.
    pub collided: Option<[i32; 3]>,
}

impl Burst {
    /// The burst a spreader of `stats` aimed `rotation_x`/`rotation_y` shoots (`getBurst`).
    pub fn from_spreader(
        spreader: [i32; 3],
        rotation_x: f32,
        rotation_y: f32,
        stats: &SpreaderStats,
        fake: bool,
    ) -> Burst {
        Burst {
            pos: spawn_position(spreader),
            vel: spreader_velocity(rotation_x, rotation_y, stats.motion),
            mana: stats.burst_mana,
            starting_mana: stats.burst_mana,
            min_mana_loss: stats.ticks_before_loss,
            mana_loss_per_tick: stats.loss_per_tick,
            accumulated_loss: 0.0,
            ticks_existed: 0,
            source: spreader,
            left_source: false,
            last_collision: None,
            alive: true,
            fake,
            collided: None,
        }
    }

    /// `ManaBurstEntity.tick` (with `ThrowableProjectile.tick`): leave the source, cast the ray to
    /// where it moves, hit, move, lose mana.
    pub fn tick(&mut self, world: &impl BurstWorld) -> BurstTick {
        self.tick_with(world, |_, _| {})
    }

    /// `tick` with the lens's `updateBurst` (`LensEffectItem`), which `ManaBurstEntity.tick` runs
    /// after the move and before the mana loss: `update` gets the burst and where it was.
    pub fn tick_with(
        &mut self,
        world: &impl BurstWorld,
        mut update: impl FnMut(&mut Burst, [f64; 3]),
    ) -> BurstTick {
        let mut out = BurstTick::default();
        let old = self.pos;
        self.ticks_existed += 1;
        if !self.left_source && mth::block_of(self.pos) != self.source {
            self.left_source = true;
        }
        let to = [
            self.pos[0] + self.vel[0],
            self.pos[1] + self.vel[1],
            self.pos[2] + self.vel[2],
        ];
        if let Some(hit) = clip(world, self.pos, to) {
            self.on_hit_block(hit.pos, world, &mut out);
        }
        // No drag (ThrowableProjectileMixin) and no gravity: a full step.
        self.pos = to;
        update(self, old);
        if self.ticks_existed >= self.min_mana_loss {
            self.accumulated_loss += self.mana_loss_per_tick;
            let loss = self.accumulated_loss as i32;
            self.mana -= loss;
            self.accumulated_loss -= loss as f32;
            if self.mana <= 0 && self.alive {
                self.alive = false;
                out.spent = true;
            }
        }
        out
    }

    /// `onHitBlock` without lenses, triggers or ghosts' partial behaviours.
    fn on_hit_block(&mut self, pos: [i32; 3], world: &impl BurstWorld, out: &mut BurstTick) {
        if self.last_collision == Some(pos) {
            return;
        }
        self.last_collision = Some(pos);
        if world.passes(pos) {
            return;
        }
        if !self.left_source && pos == self.source {
            return;
        }
        let receiver = world.receiver(pos);
        self.collided = receiver.then_some(pos);
        if receiver && !self.fake && self.mana > 0 {
            out.delivered = Some((pos, self.mana));
        }
        // `onHitCommon(hit, true)`: every hit it doesn't pass kills it.
        if self.alive {
            self.alive = false;
            out.hit = Some(pos);
        }
    }
}

/// `checkForReceiver`: a fake burst flies up to 400 ticks; the receiver it hit last, and the
/// blocks it passed through (`propsList`, consecutive ones once), for telling when the path
/// changes.
pub fn scan_receiver(
    world: &impl BurstWorld,
    spreader: [i32; 3],
    rotation_x: f32,
    rotation_y: f32,
    stats: &SpreaderStats,
) -> (Option<[i32; 3]>, Vec<[i32; 3]>) {
    let mut burst = Burst::from_spreader(spreader, rotation_x, rotation_y, stats, true);
    let mut path: Vec<[i32; 3]> = Vec::new();
    let mut i = 0;
    while burst.alive && i < SPREADER_TRACE_TIME {
        burst.tick(world);
        let b = mth::block_of(burst.pos);
        if path.last() != Some(&b) {
            path.push(b);
        }
        i += 1;
    }
    (burst.collided, path)
}

/// `MathHelper.angleBetween` with `Vec3.normalize` (a vector shorter than 1e-4 is zero).
fn angle_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let norm = |v: [f64; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if l < 1.0e-4 {
            [0.0; 3]
        } else {
            [v[0] / l, v[1] / l, v[2] / l]
        }
    };
    let (a, b) = (norm(a), norm(b));
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    mth::clamp(dot, -1.0, 1.0).acos()
}

/// `ManaSpreaderBlockEntity.bindTo`: the rotation that aims the spreader at the centre of the
/// target's shape bounds (`target`, absolute). Returns `(rotationX, rotationY)`.
pub fn aim_at(spreader: [i32; 3], target: Aabb) -> (f32, f32) {
    let this = [
        f64::from(spreader[0]) + 0.5,
        f64::from(spreader[1]) + 0.5,
        f64::from(spreader[2]) + 0.5,
    ];
    let block = [
        target[0] + (target[3] - target[0]) / 2.0,
        target[1] + (target[4] - target[1]) / 2.0,
        target[2] + (target[5] - target[2]) / 2.0,
    ];
    let diff = sub(block, this);
    let mut angle =
        angle_between([0.0, 1.0, 0.0], [diff[0], diff[2], 0.0]) / std::f64::consts::PI * 180.0;
    if block[0] < this[0] {
        angle = -angle;
    }
    let rotation_x = angle as f32 + 90.0;
    let mut angle = angle_between(diff, [diff[0], 0.0, diff[2]]) * 180.0 / std::f64::consts::PI;
    if block[1] < this[1] {
        angle = -angle;
    }
    (rotation_x, angle as f32)
}

/// The port's burst trail, which the server draws as vanilla dust because a mod particle carries
/// no colour or size (engine gap G2): Botania's subtle mode, one wisp a tick where the burst is,
/// sized by the mana it has left (`ManaBurstEntity.particles`: `size = mana / startingMana`). The
/// dust's scale, within vanilla's `0.01..=4`.
pub fn trail_scale(mana: i32, starting_mana: i32) -> f32 {
    let left = (mana as f32 / starting_mana.max(1) as f32).clamp(0.0, 1.0);
    0.35 + 0.65 * left
}

/// `ManaSpreaderBlock.setPlacedBy`: a placed spreader faces the player (the opposite of the side
/// the player faces most), as `rotationX`/`rotationY`.
pub fn placed_rotation(player_yaw: f32, player_pitch: f32) -> (f32, f32) {
    use mth::Direction::*;
    match mth::Direction::facing(player_yaw, player_pitch).opposite() {
        Down => (0.0, -90.0),
        Up => (0.0, 90.0),
        North => (270.0, 0.0),
        South => (90.0, 0.0),
        West => (0.0, 0.0),
        East => (180.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::SpreaderVariant;
    use std::collections::{HashMap, HashSet};

    /// A test level: full cubes, slabs and receivers placed by hand.
    #[derive(Default)]
    struct Level {
        shapes: HashMap<[i32; 3], Vec<Aabb>>,
        passing: HashSet<[i32; 3]>,
        receivers: HashSet<[i32; 3]>,
    }

    const FULL: Aabb = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    const POOL_SLAB: Aabb = [0.0, 0.0, 0.0, 1.0, 0.5, 1.0];
    const SPREADER: Aabb = [0.125, 0.125, 0.125, 0.875, 0.875, 0.875];

    impl Level {
        fn put(&mut self, pos: [i32; 3], shape: Aabb) {
            self.shapes.insert(pos, vec![shape]);
        }
        fn pool(&mut self, pos: [i32; 3]) {
            self.put(pos, POOL_SLAB);
            self.receivers.insert(pos);
        }
    }

    impl BurstWorld for Level {
        fn boxes(&self, pos: [i32; 3]) -> Vec<Aabb> {
            self.shapes.get(&pos).cloned().unwrap_or_default()
        }
        fn passes(&self, pos: [i32; 3]) -> bool {
            self.passing.contains(&pos)
        }
        fn receiver(&self, pos: [i32; 3]) -> bool {
            self.receivers.contains(&pos)
        }
    }

    #[test]
    fn velocity_points_where_the_spreader_faces() {
        // rotationX 0 faces -X, 90 +Z, 180 +X, 270 -Z (ManaSpreaderBlock.setPlacedBy).
        let close = |v: [f64; 3], e: [f64; 3]| (0..3).all(|i| (v[i] - e[i]).abs() < 1e-6);
        assert!(close(spreader_velocity(0.0, 0.0, 1.0), [-0.2, 0.0, 0.0]));
        assert!(close(spreader_velocity(90.0, 0.0, 1.0), [0.0, 0.0, 0.2]));
        assert!(close(spreader_velocity(180.0, 0.0, 1.0), [0.2, 0.0, 0.0]));
        assert!(close(spreader_velocity(270.0, 0.0, 1.0), [0.0, 0.0, -0.2]));
        assert!(close(spreader_velocity(0.0, 90.0, 1.0), [0.0, 0.2, 0.0]));
        assert!(close(spreader_velocity(0.0, -90.0, 1.0), [0.0, -0.2, 0.0]));
        assert!(close(spreader_velocity(180.0, 0.0, 2.0), [0.4, 0.0, 0.0]));
        // 0.2 is (float)0.4 halved in double: not exactly 0.2.
        assert_eq!(velocity(0.0, -90.0)[0], -f64::from(0.4_f32) / 2.0);
        assert_eq!(spawn_position([2, 1, 1]), [2.5, 1.5 - 1.0 / 1024.0, 1.5]);
    }

    #[test]
    fn a_burst_dies_on_tick_99_after_19_8_blocks() {
        // 160 mana, loss from tick 60 at 4 a tick: 40 ticks of loss, the last on tick 99.
        let level = Level::default();
        let stats = SpreaderVariant::Mana.stats();
        let mut b = Burst::from_spreader([0, 100, 0], 180.0, 0.0, &stats, false);
        let mut died = None;
        for tick in 1..=200 {
            let r = b.tick(&level);
            if tick == 59 {
                assert_eq!(b.mana, 160, "no loss before tick 60");
            }
            if r.spent {
                died = Some(tick);
                break;
            }
        }
        assert_eq!(died, Some(99));
        assert!((b.pos[0] - (0.5 + 99.0 * 0.2)).abs() < 1e-4, "{:?}", b.pos);
        // The other variants' ranges at full mana are 12, 20 and 48 blocks (ticks before loss ×
        // speed), and a Gaia burst loses 20 a tick: 640 / 20 = 32 more ticks.
        for (v, range, death) in [
            (SpreaderVariant::Elven, 20.0, 80 + 240 / 4 - 1),
            (SpreaderVariant::Gaia, 48.0, 120 + 640 / 20 - 1),
        ] {
            let s = v.stats();
            let speed = 0.2 * f64::from(s.motion);
            assert!((f64::from(s.ticks_before_loss) * speed - range).abs() < 1e-9);
            let mut b = Burst::from_spreader([0, 100, 0], 180.0, 0.0, &s, false);
            let mut t = 0;
            while b.alive {
                t += 1;
                b.tick(&level);
            }
            assert_eq!(t, death, "{v:?}");
        }
    }

    #[test]
    fn fractional_loss_accumulates() {
        let level = Level::default();
        let stats = SpreaderStats {
            loss_per_tick: 0.5,
            ticks_before_loss: 1,
            burst_mana: 3,
            ..SpreaderVariant::Mana.stats()
        };
        let mut b = Burst::from_spreader([0, 100, 0], 0.0, 0.0, &stats, false);
        let mut manas = Vec::new();
        while b.alive {
            b.tick(&level);
            manas.push(b.mana);
        }
        assert_eq!(manas, vec![3, 2, 2, 1, 1, 0]);
    }

    #[test]
    fn spreaders_hit_the_pool_beside_them_level() {
        // Botania's gametest spreader_pool_default_aim: each spreader is bound to a block two
        // east of it, so it shoots level; the pool between gets the burst (and the block behind
        // nothing) within 10 ticks.
        for v in SpreaderVariant::ALL {
            let mut level = Level::default();
            let spreader = [2, 1, 1];
            level.put(spreader, SPREADER);
            level.pool([3, 1, 1]);
            level.put([4, 1, 1], FULL);
            level.receivers.insert([4, 1, 1]);
            let (rx, ry) = aim_at(spreader, [4.0, 1.0, 1.0, 5.0, 2.0, 2.0]);
            assert_eq!((rx, ry), (180.0, 0.0));
            let stats = v.stats();
            let mut b = Burst::from_spreader(spreader, rx, ry, &stats, false);
            let mut delivered = None;
            for tick in 1..=10 {
                let r = b.tick(&level);
                if let Some(d) = r.delivered {
                    delivered = Some((tick, d));
                    break;
                }
            }
            let (tick, (pos, mana)) = delivered.expect("the pool got mana");
            assert_eq!(pos, [3, 1, 1], "{v:?}");
            assert_eq!(mana, stats.burst_mana);
            assert!(tick <= 3, "{v:?} on tick {tick}");
            assert!(!b.alive);
        }
    }

    #[test]
    fn a_level_burst_over_a_lower_pool_misses_it() {
        // A burst at the centre's height flies over a pool one block lower.
        let mut level = Level::default();
        level.pool([3, 0, 1]);
        let stats = SpreaderVariant::Mana.stats();
        let (receiver, path) = scan_receiver(&level, [2, 1, 1], 180.0, 0.0, &stats);
        assert_eq!(receiver, None);
        // 99 ticks of flight east: the blocks it passed, each once.
        assert_eq!(path.first(), Some(&[2, 1, 1]));
        assert_eq!(path.last(), Some(&[22, 1, 1]));
        assert_eq!(path.len(), 21);
    }

    #[test]
    fn scans_find_the_receiver_and_stop_at_walls_and_pass_leaves() {
        let stats = SpreaderVariant::Mana.stats();
        let mut level = Level::default();
        level.put([0, 0, 0], SPREADER);
        level.pool([0, 0, 8]);
        // Aimed south at the pool eight blocks away: found.
        let (rx, ry) = aim_at([0, 0, 0], [0.0, 0.0, 8.0, 1.0, 0.5, 9.0]);
        assert_eq!(
            scan_receiver(&level, [0, 0, 0], rx, ry, &stats).0,
            Some([0, 0, 8])
        );
        // A leaves block on the way is passed; a stone one stops it.
        level.put([0, 0, 4], FULL);
        level.passing.insert([0, 0, 4]);
        assert_eq!(
            scan_receiver(&level, [0, 0, 0], rx, ry, &stats).0,
            Some([0, 0, 8])
        );
        level.passing.clear();
        assert_eq!(scan_receiver(&level, [0, 0, 0], rx, ry, &stats).0, None);
        // A fake burst delivers nothing.
        let mut level = Level::default();
        level.pool([1, 0, 0]);
        let mut fake = Burst::from_spreader([0, 0, 0], 180.0, 0.0, &stats, true);
        for _ in 0..10 {
            assert_eq!(fake.tick(&level).delivered, None);
        }
        assert_eq!(fake.collided, Some([1, 0, 0]));
    }

    #[test]
    fn the_trail_shrinks_as_the_burst_loses_mana() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-6;
        assert!(near(trail_scale(160, 160), 1.0));
        assert!(near(trail_scale(80, 160), 0.675));
        assert!(near(trail_scale(0, 160), 0.35));
        assert!(near(trail_scale(-4, 160), 0.35));
        assert!(near(trail_scale(5, 0), 1.0));
    }

    #[test]
    fn aim_and_placement_follow_botania() {
        // bindTo: rotationX 90 + the signed angle from +Y in the (dx, dz) plane; rotationY the
        // elevation.
        let s = [0, 0, 0];
        let at = |x: f64, y: f64, z: f64| aim_at(s, [x, y, z, x + 1.0, y + 1.0, z + 1.0]);
        assert_eq!(at(-5.0, 0.0, 0.0), (0.0, 0.0));
        assert_eq!(at(0.0, 0.0, 5.0), (90.0, 0.0));
        assert_eq!(at(0.0, 0.0, -5.0), (270.0, 0.0));
        let (rx, ry) = at(3.0, 3.0, 0.0);
        assert_eq!(rx, 180.0);
        assert!((ry - 45.0).abs() < 1e-4);
        let (_, ry) = at(3.0, -3.0, 0.0);
        assert!((ry + 45.0).abs() < 1e-4);
        // Straight up: the horizontal part is too short to normalize, so its angle is 90.
        assert_eq!(at(0.0, 4.0, 0.0), (180.0, 90.0));
        // A placed spreader faces the player: a player looking south gets one facing north.
        assert_eq!(placed_rotation(0.0, 0.0), (270.0, 0.0));
        assert_eq!(placed_rotation(90.0, 0.0), (180.0, 0.0));
        assert_eq!(placed_rotation(0.0, 80.0), (0.0, 90.0));
    }
}
