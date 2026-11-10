//! Mana items: the items with Botania's `ManaItem` capability that the port has (the Mana Tablet
//! and its creative variant, the Band of Mana and the Greater Band of Mana, the Mana Mirror and the
//! Terra Shatterer), their mana as their stack keeps it, the bar and tooltip Botania shows for
//! them, and what a mana pool does with one lying in it (`ManaPoolBlockEntity.serverTick`, with the
//! charging animation's positions).
//!
//! Botania keeps an item's mana in its stack's NBT (`ItemNBTHelper`); 26.3 stacks keep the same
//! keys in `minecraft:custom_data`. Arithmetic is Java's `int` arithmetic (wrapping), as the
//! `ManaItemImpl`s do it.

use crate::consts::{POOL_ITEM_TRANSFER_RATE, POOL_MAX_MANA};
use crate::pool::Pool;
use crate::wand::hsv_to_rgb;

/// `ManaTabletItem.MAX_MANA`.
pub const TABLET_MAX_MANA: i32 = 500_000;
/// A creative tablet holds 1000 more (`getMaxMana`), so it never reads as full in a pool.
pub const TABLET_CREATIVE_EXTRA: i32 = 1000;
/// `BandOfManaItem.MAX_MANA`.
pub const BAND_MAX_MANA: i32 = 500_000;
/// `GreaterBandOfManaItem.MAX_MANA`: four times the band's.
pub const GREATER_BAND_MAX_MANA: i32 = BAND_MAX_MANA * 4;
/// `TerraShattererItem.MAX_MANA`.
pub const TERRA_PICK_MAX_MANA: i32 = i32::MAX;
/// `TerraShattererItem.LEVELS`: the mana each rank starts at.
pub const TERRA_PICK_LEVELS: [i32; 6] = [0, 10_000, 1_000_000, 10_000_000, 100_000_000, 1_000_000_000];
/// The pool's charging animation event is sent on these ticks (`ticks % 10 == 0`).
pub const POOL_EFFECT_INTERVAL: u32 = 10;
/// `ManaPoolBlockEntity.CHARGING_GRAVITY`, the wisps' gravity in the charging animation.
pub const CHARGING_GRAVITY: f64 = 0.003_f32 as f64;
/// The tag of items a Terra Shatterer takes no mana from (`BotaniaTags.Items.TERRA_PICK_BLACKLIST`).
pub const TERRA_PICK_BLACKLIST: &str = "botania:terra_pick_blacklist";

/// The mana items the port has, by their class in Botania.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `ManaTabletItem` (`botania:mana_tablet`), creative or not.
    Tablet,
    /// `BandOfManaItem` (`botania:mana_ring`).
    Band,
    /// `GreaterBandOfManaItem` (`botania:mana_ring_greater`).
    GreaterBand,
    /// `ManaMirrorItem` (`botania:mana_mirror`): the mana of the pool it is bound to.
    Mirror,
    /// `TerraShattererItem` (`botania:terra_pick`): stores mana to rank up, never gives any.
    TerraPick,
}

impl Kind {
    pub fn of_item(item: &str) -> Option<Kind> {
        Some(match item {
            "botania:mana_tablet" => Kind::Tablet,
            "botania:mana_ring" => Kind::Band,
            "botania:mana_ring_greater" => Kind::GreaterBand,
            "botania:mana_mirror" => Kind::Mirror,
            "botania:terra_pick" => Kind::TerraPick,
            _ => return None,
        })
    }
}

/// The NBT keys a mana item keeps (Botania's names): `mana` (removed when it reaches 0),
/// `creative` and `oneUse` (tablet), `maxMana` and `manaBacklog` (mirror).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Nbt {
    pub mana: i32,
    pub creative: bool,
    pub one_use: bool,
    pub max_mana: Option<i32>,
    pub backlog: Option<i32>,
}

/// A stack of a mana item: its kind, count and NBT (the `ManaItemImpl` wrapped around it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManaStack {
    pub kind: Kind,
    pub count: i32,
    pub nbt: Nbt,
    /// The item is in `#botania:terra_pick_blacklist`, which a Terra Shatterer reads of the stack
    /// it would take mana from (`canReceiveManaFromItem`).
    pub terra_blacklisted: bool,
}

/// The bar under an item's icon (`isBarVisible`, `getBarWidth`, `getBarColor`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    /// The item doesn't override them: vanilla's (the Terra Shatterer's durability).
    Vanilla,
    /// `isBarVisible` false (a creative tablet).
    Hidden,
    /// Width 0 to 13 and colour `0xRRGGBB`.
    Shown(i32, u32),
}

impl ManaStack {
    pub fn new(kind: Kind, count: i32, nbt: Nbt) -> ManaStack {
        ManaStack {
            kind,
            count,
            nbt,
            terra_blacklisted: false,
        }
    }

    /// `getMana`.
    pub fn mana(&self) -> i32 {
        match self.kind {
            Kind::Tablet if self.nbt.creative => self.max_mana(),
            Kind::Mirror => self.nbt.mana,
            _ => self.nbt.mana.wrapping_mul(self.count),
        }
    }

    /// `getMaxMana`.
    pub fn max_mana(&self) -> i32 {
        match self.kind {
            Kind::Tablet if self.nbt.creative => {
                (TABLET_MAX_MANA + TABLET_CREATIVE_EXTRA).wrapping_mul(self.count)
            }
            Kind::Tablet => TABLET_MAX_MANA.wrapping_mul(self.count),
            Kind::Band => BAND_MAX_MANA.wrapping_mul(self.count),
            Kind::GreaterBand => GREATER_BAND_MAX_MANA.wrapping_mul(self.count),
            Kind::Mirror => self.nbt.max_mana.unwrap_or(POOL_MAX_MANA),
            Kind::TerraPick => TERRA_PICK_MAX_MANA.wrapping_mul(self.count),
        }
    }

    /// `setMana`: the tag goes when the mana isn't positive.
    fn set_mana(&mut self, mana: i32) {
        self.nbt.mana = if mana > 0 { mana } else { 0 };
    }

    /// `addMana`.
    pub fn add_mana(&mut self, mana: i32) {
        match self.kind {
            Kind::Tablet if self.nbt.creative => {}
            Kind::Mirror => {
                self.set_mana(self.mana().wrapping_add(mana));
                self.nbt.backlog = Some(self.nbt.backlog.unwrap_or(0).wrapping_add(mana));
            }
            _ => {
                let count = self.count.max(1);
                let total = self.mana().wrapping_add(mana).min(self.max_mana());
                self.set_mana(total.wrapping_div(count));
            }
        }
    }

    /// `canReceiveManaFromPool`.
    pub fn can_receive_from_pool(&self) -> bool {
        match self.kind {
            Kind::Tablet => !self.nbt.one_use,
            Kind::Band | Kind::GreaterBand | Kind::TerraPick => true,
            Kind::Mirror => false,
        }
    }

    /// `canExportManaToPool`.
    pub fn can_export_to_pool(&self) -> bool {
        matches!(self.kind, Kind::Tablet | Kind::Band | Kind::GreaterBand)
    }

    /// `canReceiveManaFromItem(other)`; `other_blacklisted` says the other stack is in
    /// `#botania:terra_pick_blacklist`.
    pub fn can_receive_from_item(&self, other_blacklisted: bool) -> bool {
        match self.kind {
            Kind::Tablet => !self.nbt.creative,
            Kind::Band | Kind::GreaterBand => true,
            Kind::Mirror => false,
            Kind::TerraPick => !other_blacklisted,
        }
    }

    /// `canExportManaToItem`: every one of them but the Terra Shatterer.
    pub fn can_export_to_item(&self) -> bool {
        self.kind != Kind::TerraPick
    }

    /// `isNoExport`.
    pub fn is_no_export(&self) -> bool {
        self.kind == Kind::TerraPick
    }

    /// `ManaBarTooltip.getFractionForDisplay`: `getMana() / (float) getMaxMana()`.
    pub fn fraction(&self) -> f32 {
        self.mana() as f32 / self.max_mana() as f32
    }

    /// The bar under the icon: `Math.round(13 * fraction)` wide, `Mth.hsvToRgb(fraction / 3, 1, 1)`.
    pub fn bar(&self) -> Bar {
        match self.kind {
            Kind::TerraPick => Bar::Vanilla,
            Kind::Tablet if self.nbt.creative => Bar::Hidden,
            _ => {
                let f = self.fraction();
                Bar::Shown(java_round(13.0 * f), hsv_to_rgb(f / 3.0, 1.0, 1.0))
            }
        }
    }

    /// The fraction the tooltip's mana bar shows (`getTooltipImage`): the Terra Shatterer's is
    /// its progress toward the next rank, 0 at rank 0.
    pub fn tooltip_fraction(&self) -> f32 {
        if self.kind != Kind::TerraPick {
            return self.fraction();
        }
        let level = terra_pick_level(self.nbt.mana);
        let max = TERRA_PICK_LEVELS[(level + 1).min(TERRA_PICK_LEVELS.len() - 1)];
        if level == 0 {
            0.0
        } else {
            self.nbt.mana as f32 / max as f32
        }
    }
}

/// Java's `Math.round(float)`: `floor(x + 0.5)`.
pub fn java_round(x: f32) -> i32 {
    (x + 0.5).floor() as i32
}

/// `TerraShattererItem.getLevel`: the highest rank whose mana the stack's `mana` reaches.
pub fn terra_pick_level(mana: i32) -> usize {
    (1..TERRA_PICK_LEVELS.len())
        .rev()
        .find(|&i| mana >= TERRA_PICK_LEVELS[i])
        .unwrap_or(0)
}

/// A pool's side of the item transfer (`outputting`, `canAccept`, `canSpare`, a mana void under it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolMode {
    pub outputting: bool,
    pub can_accept: bool,
    pub can_spare: bool,
    pub mana_void_below: bool,
    /// Bellows linked to the pool (none are ported: 0).
    pub bellows: i32,
}

impl Default for PoolMode {
    fn default() -> Self {
        PoolMode {
            outputting: false,
            can_accept: true,
            can_spare: true,
            mana_void_below: false,
            bellows: 0,
        }
    }
}

/// `ManaPoolBlockEntity.serverTick` for one mana item in the pool's cell: an outputting pool
/// charges it, any other drains it, `1000 * (bellows + 1)` a tick. Whether it did something
/// (`didSomething`: the charging animation, the item synced, `isDoingTransfer` when outputting).
pub fn pool_transfer(pool: &mut Pool, mode: PoolMode, item: &mut ManaStack) -> bool {
    let wants = if mode.outputting {
        item.can_receive_from_pool()
    } else {
        item.can_export_to_pool()
    };
    if !wants {
        return false;
    }
    let rate = POOL_ITEM_TRANSFER_RATE.wrapping_mul(mode.bellows + 1);
    let mut did = false;
    if mode.outputting {
        if mode.can_spare {
            if pool.current() > 0 && item.mana() < item.max_mana() {
                did = true;
            }
            let room = item.max_mana().wrapping_sub(item.mana());
            let mana = rate.min(pool.current().min(room));
            item.add_mana(mana);
            pool.receive(mana.wrapping_neg());
        }
    } else if mode.can_accept {
        if item.mana() > 0 && !pool.is_full(mode.mana_void_below) {
            did = true;
        }
        let room = pool.cap.wrapping_sub(pool.current());
        let mut mana = rate.min(room.min(item.mana()));
        if mana == 0 && mode.mana_void_below {
            mana = rate.min(item.mana());
        }
        item.add_mana(mana.wrapping_neg());
        pool.receive(mana);
    }
    did
}

/// `Mth.ceillog2`: the bits of the smallest power of two at least `v` (v > 0).
fn ceillog2(v: i32) -> i32 {
    let p = (v as u32).next_power_of_two();
    p.trailing_zeros() as i32
}

/// `encodeRelativeItemPosition`: an item's position in the pool's cell squeezed into 8 bits
/// (3 for x, 2 for y, 3 for z), the block event's parameter.
pub fn encode_item_position(pool: [i32; 3], item: [f64; 3]) -> i32 {
    let rel_x = (item[0] - f64::from(pool[0])).clamp(0.0, 1.0);
    let rel_y = (0.125 + 0.875 * (item[1] - f64::from(pool[1]))).clamp(0.125, 0.9);
    let rel_z = (item[2] - f64::from(pool[2])).clamp(0.0, 1.0);
    let cx = java_round_d(7.0 * rel_x) as i32;
    let cy = 4 - ceillog2(14 - (14.0 * rel_y) as i32);
    let cz = java_round_d(7.0 * rel_z) as i32;
    cx | (cy << 3) | (cz << 5)
}

/// Java's `Math.round(double)`: `floor(x + 0.5)`.
fn java_round_d(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

/// `decodeRelativeItemPosition`: the position the client draws the animation to or from, no
/// lower than half the pool's fill.
pub fn decode_item_position(param: i32, relative_mana: f32) -> [f64; 3] {
    let cx = param & 0x7;
    let cy = (param >> 3) & 0x3;
    let cz = (param >> 5) & 0x7;
    let rel_y = 1.0 - (14.0 / 16.0) / f64::from(1 << cy);
    [
        f64::from(cx) / 7.0,
        rel_y.max(0.5 * f64::from(relative_mana)),
        f64::from(cz) / 7.0,
    ]
}

/// `addManaFlowParticle`: the starting velocity and lifetime of a wisp thrown from `start` so it
/// falls onto `end` (positions relative to the pool), peaking `extra` above the higher of the two
/// (`0.05 * Math.random()` in Botania).
pub fn flow(start: [f64; 3], end: [f64; 3], extra: f64) -> ([f64; 3], f64) {
    let max_height = start[1].max(end[1]) - end[1] + extra;
    let dx = end[0] - start[0];
    let dz = end[2] - start[2];
    let distance = (dx * dx + dz * dz).sqrt();
    let (ux, uz) = if distance > 0.0 {
        (dx / distance, dz / distance)
    } else {
        (0.0, 0.0)
    };
    let start_height = start[1] - end[1];
    let vy0_squared = 2.0 * CHARGING_GRAVITY * (max_height - start_height);
    let vy0 = vy0_squared.sqrt();
    let lifetime =
        (vy0 + (vy0_squared + 2.0 * CHARGING_GRAVITY * start_height).sqrt()) / CHARGING_GRAVITY;
    let vx0 = distance / lifetime;
    ([ux * vx0, vy0, uz * vx0], lifetime)
}

/// Points along that flight, `n` of them from start to end: where the port draws the wisp's path
/// with dust (a server can't send Botania's wisps).
pub fn flow_points(start: [f64; 3], end: [f64; 3], extra: f64, n: usize) -> Vec<[f64; 3]> {
    let (v, lifetime) = flow(start, end, extra);
    (0..n)
        .map(|i| {
            let t = if n > 1 {
                lifetime * i as f64 / (n - 1) as f64
            } else {
                0.0
            };
            [
                start[0] + v[0] * t,
                start[1] + v[1] * t - 0.5 * CHARGING_GRAVITY * t * t,
                start[2] + v[2] * t,
            ]
        })
        .collect()
}

/// `randomizeItemPos`, with three random numbers in `0..1`.
pub fn randomize_item_pos(base: [f64; 3], r: [f64; 3]) -> [f64; 3] {
    [
        base[0] + 0.1 * r[0] - 0.05,
        base[1] + 0.1 * r[1] + 0.25,
        base[2] + 0.1 * r[2] - 0.05,
    ]
}

/// Where in the pool a charging wisp starts (`0.1 + 0.8 * random`, `0.1 + 0.4 * relativeMana`)
/// and a draining one lands (`0.05 + 0.9 * random`, `0.35 * relativeMana`).
pub fn pool_flow_pos(charging: bool, relative_mana: f32, r: [f64; 2]) -> [f64; 3] {
    let m = f64::from(relative_mana);
    if charging {
        [0.1 + 0.8 * r[0], 0.1 + 0.4 * m, 0.1 + 0.8 * r[1]]
    } else {
        [0.05 + 0.9 * r[0], 0.35 * m, 0.05 + 0.9 * r[1]]
    }
}

/// `ManaPoolBlockEntity.PARTICLE_COLOR`.
pub const POOL_PARTICLE_COLOR: u32 = crate::consts::POOL_PARTICLE_COLOR;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::PoolVariant;

    // Expected values below were computed by running Botania's own expressions in Java
    // (`ManaTabletItem`, `BandOfManaItem`, `ManaMirrorItem`, `TerraShattererItem` `ManaItemImpl`s,
    // `ManaPoolBlockEntity.serverTick` and its position codec, `ManaBarTooltip`, `Mth`).

    fn tablet(mana: i32) -> ManaStack {
        ManaStack::new(
            Kind::Tablet,
            1,
            Nbt {
                mana,
                ..Nbt::default()
            },
        )
    }

    #[test]
    fn tablet_mana_follows_botania() {
        let mut t = tablet(0);
        assert_eq!(t.max_mana(), 500_000);
        t.add_mana(1000);
        assert_eq!(t.mana(), 1000);
        t.add_mana(600_000);
        assert_eq!(t.mana(), 500_000, "clamped to the maximum");
        t.add_mana(-600_000);
        assert_eq!(t.mana(), 0);
        assert_eq!(t.nbt.mana, 0, "the tag goes at 0");
        // Two tablets in one stack hold twice as much, the tag keeping the share of one.
        let mut two = ManaStack::new(Kind::Tablet, 2, Nbt::default());
        two.add_mana(1001);
        assert_eq!(two.nbt.mana, 500);
        assert_eq!(two.mana(), 1000);
        assert_eq!(two.max_mana(), 1_000_000);
        // The creative tablet reads full, takes nothing, and is never full to a pool.
        let mut c = ManaStack::new(
            Kind::Tablet,
            1,
            Nbt {
                mana: 500_000,
                creative: true,
                ..Nbt::default()
            },
        );
        assert_eq!(c.mana(), 501_000);
        assert_eq!(c.max_mana(), 501_000);
        c.add_mana(-400_000);
        assert_eq!(c.mana(), 501_000);
        assert!(!c.can_receive_from_item(false));
        assert_eq!(c.bar(), Bar::Hidden);
        let one_use = ManaStack::new(
            Kind::Tablet,
            1,
            Nbt {
                one_use: true,
                ..Nbt::default()
            },
        );
        assert!(!one_use.can_receive_from_pool());
        assert!(one_use.can_export_to_pool());
    }

    #[test]
    fn bands_and_mirror_follow_botania() {
        let mut band = ManaStack::new(Kind::Band, 1, Nbt::default());
        band.add_mana(700_000);
        assert_eq!(band.mana(), 500_000);
        let mut greater = ManaStack::new(Kind::GreaterBand, 1, Nbt::default());
        assert_eq!(greater.max_mana(), 2_000_000);
        greater.add_mana(1_500_000);
        assert_eq!(greater.mana(), 1_500_000);
        // The mirror is the pool's mana: it doesn't clamp, and keeps a backlog for the pool.
        let mut mirror = ManaStack::new(
            Kind::Mirror,
            1,
            Nbt {
                mana: 5000,
                max_mana: Some(10_000),
                ..Nbt::default()
            },
        );
        assert_eq!(mirror.max_mana(), 10_000);
        mirror.add_mana(-3000);
        assert_eq!(mirror.mana(), 2000);
        assert_eq!(mirror.nbt.backlog, Some(-3000));
        mirror.add_mana(-5000);
        assert_eq!(mirror.mana(), 0);
        assert_eq!(mirror.nbt.backlog, Some(-8000));
        assert!(!mirror.can_receive_from_pool() && !mirror.can_export_to_pool());
        assert!(mirror.can_export_to_item() && !mirror.can_receive_from_item(false));
        let unbound = ManaStack::new(Kind::Mirror, 1, Nbt::default());
        assert_eq!(unbound.max_mana(), 1_000_000);
    }

    #[test]
    fn terra_pick_ranks_and_overflow() {
        assert_eq!(terra_pick_level(0), 0);
        assert_eq!(terra_pick_level(9_999), 0);
        assert_eq!(terra_pick_level(10_000), 1);
        assert_eq!(terra_pick_level(999_999), 1);
        assert_eq!(terra_pick_level(1_000_000), 2);
        assert_eq!(terra_pick_level(1_000_000_000), 5);
        assert_eq!(terra_pick_level(i32::MAX), 5);
        let mut pick = ManaStack::new(
            Kind::TerraPick,
            1,
            Nbt {
                mana: i32::MAX - 5,
                ..Nbt::default()
            },
        );
        assert!(!pick.can_export_to_item() && pick.is_no_export());
        assert!(pick.can_receive_from_pool() && !pick.can_export_to_pool());
        assert!(!pick.can_receive_from_item(true));
        assert_eq!(pick.bar(), Bar::Vanilla);
        // Java's int overflow: past the maximum the sum wraps negative and the tag goes.
        pick.add_mana(10);
        assert_eq!(pick.mana(), 0);
        let rank = ManaStack::new(
            Kind::TerraPick,
            1,
            Nbt {
                mana: 500_000,
                ..Nbt::default()
            },
        );
        assert_eq!(rank.tooltip_fraction(), 0.5);
    }

    #[test]
    fn bar_width_and_colour() {
        assert_eq!(tablet(0).bar(), Bar::Shown(0, 0xFF0000));
        assert_eq!(tablet(500_000).bar(), Bar::Shown(13, 0x00FF00));
        assert_eq!(tablet(250_000).bar(), Bar::Shown(7, 0xFFFF00));
        assert_eq!(tablet(19_230).bar(), Bar::Shown(0, 0xFF1300));
        assert_eq!(tablet(19_231).bar(), Bar::Shown(1, 0xFF1300));
        assert_eq!(tablet(100_000).bar(), Bar::Shown(3, 0xFF6600));
        let mut ring = ManaStack::new(Kind::GreaterBand, 1, Nbt::default());
        ring.add_mana(1_500_000);
        assert_eq!(ring.bar(), Bar::Shown(10, 0x7FFF00));
        assert_eq!(tablet(400_000).bar(), Bar::Shown(10, 0x65FF00));
    }

    fn pool(variant: PoolVariant, mana: i32) -> Pool {
        let mut p = Pool::new(variant);
        p.mana = mana;
        p
    }

    #[test]
    fn outputting_pool_charges_items() {
        let mode = PoolMode {
            outputting: true,
            ..PoolMode::default()
        };
        let mut p = pool(PoolVariant::Default, 2500);
        let mut t = tablet(0);
        let mut ticks = Vec::new();
        for _ in 0..4 {
            let did = pool_transfer(&mut p, mode, &mut t);
            ticks.push((did, p.mana, t.mana()));
        }
        assert_eq!(
            ticks,
            vec![
                (true, 1500, 1000),
                (true, 500, 2000),
                (true, 0, 2500),
                (false, 0, 2500),
            ]
        );
        // A nearly full tablet takes what it lacks.
        let mut p = pool(PoolVariant::Default, 1_000_000);
        let mut t = tablet(499_700);
        assert!(pool_transfer(&mut p, mode, &mut t));
        assert_eq!((p.mana, t.mana()), (999_700, 500_000));
        assert!(!pool_transfer(&mut p, mode, &mut t), "full: nothing to do");
        // A creative pool never runs out.
        let mut p = pool(PoolVariant::Creative, 0);
        let mut b = ManaStack::new(Kind::Band, 1, Nbt::default());
        assert!(pool_transfer(&mut p, mode, &mut b));
        assert_eq!((p.current(), b.mana()), (1_000_000, 1000));
        // An outputting pool doesn't take from items, and the mirror is never charged.
        let mut m = ManaStack::new(Kind::Mirror, 1, Nbt::default());
        assert!(!pool_transfer(&mut p, mode, &mut m));
        // canSpare false: nothing moves.
        let mut p = pool(PoolVariant::Default, 5000);
        let mut t = tablet(0);
        let spare = PoolMode {
            can_spare: false,
            ..mode
        };
        assert!(!pool_transfer(&mut p, spare, &mut t));
        assert_eq!((p.mana, t.mana()), (5000, 0));
    }

    #[test]
    fn pool_drains_items() {
        let mode = PoolMode::default();
        let mut p = pool(PoolVariant::Diluted, 8500);
        let mut t = tablet(3000);
        let mut ticks = Vec::new();
        for _ in 0..3 {
            let did = pool_transfer(&mut p, mode, &mut t);
            ticks.push((did, p.mana, t.mana()));
        }
        assert_eq!(
            ticks,
            vec![(true, 9500, 2000), (true, 10_000, 1500), (false, 10_000, 1500)]
        );
        // A mana void under it swallows the item's mana though the pool is full.
        let void = PoolMode {
            mana_void_below: true,
            ..mode
        };
        assert!(pool_transfer(&mut p, void, &mut t));
        assert_eq!((p.mana, t.mana()), (10_000, 500));
        // The Terra Shatterer only takes mana from a pool.
        let mut pick = ManaStack::new(
            Kind::TerraPick,
            1,
            Nbt {
                mana: 50_000,
                ..Nbt::default()
            },
        );
        let mut p = pool(PoolVariant::Default, 0);
        assert!(!pool_transfer(&mut p, mode, &mut pick));
        assert_eq!(pick.mana(), 50_000);
        // A creative tablet feeds a pool without end.
        let mut c = ManaStack::new(
            Kind::Tablet,
            1,
            Nbt {
                creative: true,
                ..Nbt::default()
            },
        );
        assert!(pool_transfer(&mut p, mode, &mut c));
        assert_eq!((p.mana, c.mana()), (1000, 501_000));
    }

    #[test]
    fn item_position_codec() {
        let pos = [10, 64, -3];
        assert_eq!(encode_item_position(pos, [10.5, 64.0, -2.5]), 132);
        assert_eq!(encode_item_position(pos, [10.0, 64.2, -3.0]), 0);
        assert_eq!(encode_item_position(pos, [11.2, 64.9, -1.0]), 255);
        assert_eq!(encode_item_position(pos, [10.31, 64.45, -2.12]), 202);
        assert_eq!(encode_item_position(pos, [10.9, 64.6, -2.95]), 14);
        assert_eq!(decode_item_position(132, 0.0), [4.0 / 7.0, 0.125, 4.0 / 7.0]);
        assert_eq!(decode_item_position(247, 0.0), [1.0, 0.78125, 1.0]);
        assert_eq!(decode_item_position(210, 0.0), [2.0 / 7.0, 0.78125, 6.0 / 7.0]);
        assert_eq!(decode_item_position(8, 0.6), [0.0, 0.5625, 0.0]);
        assert_eq!(decode_item_position(8, 1.0), [0.0, 0.5625, 0.0]);
        assert_eq!(decode_item_position(0, 0.5), [0.0, 0.25, 0.0]);
    }

    #[test]
    fn flow_lands_on_its_end() {
        let start = [0.5, 0.3, 0.5];
        let end = [0.2, 0.6, 0.8];
        let (v, life) = flow(start, end, 0.02);
        assert!((v[1] - 0.043817804790853).abs() < 1e-12);
        assert!((life - 18.25741850415566).abs() < 1e-9);
        let horizontal = (v[0] * v[0] + v[2] * v[2]).sqrt();
        assert!((horizontal - 0.02323790017824041).abs() < 1e-12);
        let (v, life) = flow([0.3, 0.9, 0.4], [0.6, 0.2, 0.7], 0.01);
        assert!((v[1] - 0.007745966726080141).abs() < 1e-12);
        assert!((life - 24.33821395446784).abs() < 1e-9);
        let points = flow_points(start, end, 0.02, 5);
        assert_eq!(points.len(), 5);
        assert_eq!(points[0], start);
        for (a, b) in points[4].iter().zip(end) {
            assert!((a - b).abs() < 1e-9);
        }
    }
}
