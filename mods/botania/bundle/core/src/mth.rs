//! The pieces of Minecraft 1.20.1's `Mth` and `Direction` that Botania's maths goes through,
//! reproduced so a burst flies the same path: `Mth.sin`/`cos` read a 65536-entry float table,
//! not `Math.sin`.

use std::f64::consts::PI;

/// `Mth.sin(float)`: the table entry `(int)(v * 10430.378F) & 0xFFFF`.
pub fn sin(v: f32) -> f32 {
    table((v * 10430.378_f32) as i32)
}

/// `Mth.cos(float)`: the table entry a quarter turn on.
pub fn cos(v: f32) -> f32 {
    table((v * 10430.378_f32 + 16384.0_f32) as i32)
}

/// `SIN[i] = (float) Math.sin((double) i * Math.PI * 2.0 / 65536.0)`, computed when asked.
fn table(i: i32) -> f32 {
    let i = f64::from(i & 0xFFFF);
    (i * PI * 2.0 / 65536.0).sin() as f32
}

/// `Mth.floor(double)`.
pub fn floor(v: f64) -> i32 {
    let i = v as i32;
    if v < f64::from(i) { i - 1 } else { i }
}

/// `Mth.lfloor(double)`.
pub fn lfloor(v: f64) -> i64 {
    let i = v as i64;
    if v < i as f64 { i - 1 } else { i }
}

/// `Mth.frac(double)`.
pub fn frac(v: f64) -> f64 {
    v - lfloor(v) as f64
}

/// `Mth.sign(double)`.
pub fn sign(v: f64) -> i32 {
    if v == 0.0 {
        0
    } else if v > 0.0 {
        1
    } else {
        -1
    }
}

/// `Mth.lerp(double delta, double start, double end)`.
pub fn lerp(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
}

/// `Mth.clamp(double, double, double)`.
pub fn clamp(v: f64, min: f64, max: f64) -> f64 {
    if v < min { min } else { v.min(max) }
}

/// `BlockPos.containing(x, y, z)`: the block a point is in.
pub fn block_of(p: [f64; 3]) -> [i32; 3] {
    [floor(p[0]), floor(p[1]), floor(p[2])]
}

/// Vanilla's `Direction`, in its declaration order (`Direction.values()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    pub const ALL: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    pub fn normal(self) -> [i32; 3] {
        match self {
            Direction::Down => [0, -1, 0],
            Direction::Up => [0, 1, 0],
            Direction::North => [0, 0, -1],
            Direction::South => [0, 0, 1],
            Direction::West => [-1, 0, 0],
            Direction::East => [1, 0, 0],
        }
    }

    pub fn opposite(self) -> Direction {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }

    /// The neighbour of `pos` on this side.
    pub fn offset(self, pos: [i32; 3]) -> [i32; 3] {
        let n = self.normal();
        [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
    }

    /// `Direction.getNearest(double, double, double)`: the side whose normal is closest to the
    /// vector, compared in float; `North` when none is ahead of it.
    pub fn nearest(x: f64, y: f64, z: f64) -> Direction {
        let (x, y, z) = (x as f32, y as f32, z as f32);
        let mut best = Direction::North;
        let mut best_dot = f32::from_bits(1); // Float.MIN_VALUE
        for d in Direction::ALL {
            let n = d.normal();
            let dot = x * n[0] as f32 + y * n[1] as f32 + z * n[2] as f32;
            if dot > best_dot {
                best_dot = dot;
                best = d;
            }
        }
        best
    }

    /// `Direction.orderedByNearest(entity)[0]`: the side a player looking along `yaw` and `pitch`
    /// (degrees, Minecraft's convention: yaw 0 faces south, pitch 90 faces down) faces most.
    pub fn facing(yaw: f32, pitch: f32) -> Direction {
        let rad = std::f32::consts::PI / 180.0;
        let f = pitch * rad;
        let f1 = -yaw * rad;
        let f2 = sin(f);
        let f3 = cos(f);
        let f4 = sin(f1);
        let f5 = cos(f1);
        let east = f4 > 0.0;
        let up = f2 < 0.0;
        let south = f5 > 0.0;
        let f6 = if east { f4 } else { -f4 };
        let f7 = if up { -f2 } else { f2 };
        let f8 = if south { f5 } else { -f5 };
        let f9 = f6 * f3;
        let f10 = f8 * f3;
        let horizontal_x = if east {
            Direction::East
        } else {
            Direction::West
        };
        let vertical = if up { Direction::Up } else { Direction::Down };
        let horizontal_z = if south {
            Direction::South
        } else {
            Direction::North
        };
        if f6 > f8 {
            if f7 > f9 { vertical } else { horizontal_x }
        } else if f7 > f10 {
            vertical
        } else {
            horizontal_z
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_sine_hits_the_quarter_turns() {
        // sin(-pi/2) = -1, cos(0) = 1, sin(pi) is the float of sin(pi), not 0.
        assert_eq!(sin(-std::f32::consts::FRAC_PI_2), -1.0);
        assert_eq!(cos(0.0), 1.0);
        assert!(sin(std::f32::consts::PI).abs() < 1e-6);
        // The table is coarse: sin(1e-5) reads entry 0.
        assert_eq!(sin(1e-5), 0.0);
    }

    #[test]
    fn floor_frac_sign_match_java() {
        assert_eq!(floor(-0.5), -1);
        assert_eq!(floor(2.0), 2);
        assert_eq!(floor(2.999), 2);
        assert!((frac(-0.25) - 0.75).abs() < 1e-12);
        assert_eq!(sign(-0.0), 0);
        assert_eq!(sign(-3.0), -1);
    }

    #[test]
    fn nearest_side_of_a_vector() {
        assert_eq!(Direction::nearest(0.2, 0.0, 0.0), Direction::East);
        assert_eq!(Direction::nearest(0.0, -1.0, 0.1), Direction::Down);
        assert_eq!(Direction::nearest(0.0, 0.0, 0.0), Direction::North);
    }

    #[test]
    fn player_facing() {
        // Yaw 0 faces south (+Z), 90 west, 180 north, 270 east; pitch 90 looks down.
        assert_eq!(Direction::facing(0.0, 0.0), Direction::South);
        assert_eq!(Direction::facing(90.0, 10.0), Direction::West);
        assert_eq!(Direction::facing(180.0, -10.0), Direction::North);
        assert_eq!(Direction::facing(-90.0, 0.0), Direction::East);
        assert_eq!(Direction::facing(30.0, 80.0), Direction::Down);
        assert_eq!(Direction::facing(30.0, -75.0), Direction::Up);
    }
}
