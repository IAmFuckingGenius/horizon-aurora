//! The mana pool (`ManaPoolBlockEntity`, `ManaPoolBlock`) and what a spreader draws from one
//! (`ManaSpreaderBlockEntity.commonTick`).

use crate::consts::{POOL_MAX_MANA, POOL_SYNC_INTERVAL, PoolVariant};

/// A pool's mana, as its block entity keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pool {
    /// `mana`, saved.
    pub mana: i32,
    /// `manaCap`, saved: the variant's capacity once `initManaCapAndNetwork` has run.
    pub cap: i32,
    /// A creative pool always reads full (`getCurrentMana`).
    pub creative: bool,
}

impl Pool {
    pub fn new(variant: PoolVariant) -> Pool {
        Pool {
            mana: 0,
            cap: variant.capacity(),
            creative: variant == PoolVariant::Creative,
        }
    }

    /// `getCurrentMana`.
    pub fn current(&self) -> i32 {
        if self.creative {
            POOL_MAX_MANA
        } else {
            self.mana
        }
    }

    /// `isFull`: never with a mana void under it.
    pub fn is_full(&self, mana_void_below: bool) -> bool {
        !mana_void_below && self.current() >= self.cap
    }

    /// `receiveMana`: the mana clamped to `0..=cap` (what doesn't fit is lost); whether it
    /// changed (`markDispatchable`).
    pub fn receive(&mut self, mana: i32) -> bool {
        let old = self.mana;
        self.mana = (self.current().saturating_add(mana)).clamp(0, self.cap.max(0));
        old != self.mana
    }

    /// `ManaPoolBlock.getAnalogOutputSignal`.
    pub fn comparator(&self) -> i32 {
        comparator_level(self.current(), self.cap)
    }

    /// Room left for a generating flower's transfer (`emptyManaIntoCollector` uses
    /// `getMaxMana() - getCurrentMana()`).
    pub fn room(&self) -> i32 {
        self.cap - self.current()
    }
}

/// `ManaPoolBlockEntity.calculateComparatorLevel`: `mana / max * 15` rounded down, at least 1
/// with any mana.
pub fn comparator_level(mana: i32, max: i32) -> i32 {
    if max <= 0 {
        return 0;
    }
    let level = (f64::from(mana) / f64::from(max) * 15.0) as i32;
    if mana > 0 { level.max(1) } else { level }
}

/// A pool sends itself to clients on ticks where this holds and it changed since
/// (`serverTick`: `sendPacket && ticks % 10 == 0`); `ticks` counts from 0 when it loads.
pub fn pool_sync_tick(ticks: u32) -> bool {
    ticks % POOL_SYNC_INTERVAL as u32 == 0
}

/// What a spreader beside a pool takes from it in one tick (`commonTick`): as much as the pool
/// has and the spreader lacks, when the pool has any and the spreader isn't full.
pub fn spreader_draw(pool_mana: i32, spreader_mana: i32, spreader_cap: i32) -> i32 {
    if pool_mana > 0 && spreader_mana < spreader_cap {
        pool_mana.min(spreader_cap - spreader_mana)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::SpreaderVariant;

    #[test]
    fn comparator_follows_botania() {
        // calculateComparatorLevel by hand: floor(mana / max * 15), at least 1 with any mana.
        assert_eq!(comparator_level(0, 1_000_000), 0);
        assert_eq!(comparator_level(1, 1_000_000), 1);
        assert_eq!(comparator_level(66_666, 1_000_000), 1);
        assert_eq!(comparator_level(133_334, 1_000_000), 2);
        assert_eq!(comparator_level(500_000, 1_000_000), 7);
        assert_eq!(comparator_level(999_999, 1_000_000), 14);
        assert_eq!(comparator_level(1_000_000, 1_000_000), 15);
        assert_eq!(comparator_level(10_000, 10_000), 15);
        assert_eq!(comparator_level(3_000, 10_000), 4);
    }

    #[test]
    fn pool_receives_within_its_capacity() {
        let mut pool = Pool::new(PoolVariant::Diluted);
        assert_eq!(pool.cap, 10_000);
        assert!(pool.receive(9_500));
        assert!(pool.receive(1_000));
        assert_eq!(pool.mana, 10_000, "what doesn't fit is lost");
        assert!(pool.is_full(false));
        assert!(!pool.is_full(true), "a mana void under it takes any amount");
        assert!(!pool.receive(5), "a full pool doesn't change");
        assert!(pool.receive(-20_000));
        assert_eq!(pool.mana, 0);
        let creative = Pool::new(PoolVariant::Creative);
        assert_eq!(creative.current(), 1_000_000);
        assert_eq!(creative.comparator(), 15);
        assert!(creative.is_full(false));
    }

    #[test]
    fn pool_syncs_every_tenth_tick() {
        let ticks: Vec<u32> = (0..35).filter(|&t| pool_sync_tick(t)).collect();
        assert_eq!(ticks, vec![0, 10, 20, 30]);
    }

    #[test]
    fn spreader_draws_what_it_lacks() {
        let cap = SpreaderVariant::Mana.stats().capacity;
        assert_eq!(cap, 1000);
        assert_eq!(spreader_draw(5000, 0, cap), 1000);
        assert_eq!(spreader_draw(300, 900, cap), 100);
        assert_eq!(spreader_draw(50, 900, cap), 50);
        assert_eq!(spreader_draw(0, 0, cap), 0);
        assert_eq!(spreader_draw(5000, 1000, cap), 0);
        let gaia = SpreaderVariant::Gaia.stats().capacity;
        assert_eq!(spreader_draw(1_000_000, 640, gaia), 6400 - 640);
    }
}
