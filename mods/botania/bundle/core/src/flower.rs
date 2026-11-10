//! Generating flowers: the binding and transfer every one has (`SpecialFlowerBlockEntity`,
//! `BindableSpecialFlowerBlockEntity`, `GeneratingFlowerBlockEntity`), the item delay
//! (`DelayHelper`) and the Endoflame (`EndoflameBlockEntity`).

use crate::consts::{
    ENDOFLAME_FUEL_CAP, ENDOFLAME_MANA_PER_BURN, ENDOFLAME_MAX_MANA, GENERATING_INHERENT_DELAY,
    GENERATING_LINK_RANGE, MYCELIUM_DELAY, PODZOL_DELAY,
};

/// `MathHelper.distSqr`: the squared distance between two block positions' corners, in longs.
pub fn dist_sqr(a: [i32; 3], b: [i32; 3]) -> i64 {
    let d = |i: usize| i64::from(a[i]) - i64::from(b[i]);
    d(0) * d(0) + d(1) * d(1) + d(2) * d(2)
}

/// `ManaNetwork.getClosest`: the nearest of `candidates` within `limit` blocks of `pos`
/// (`distSqr <= limit²`); the first of equally near ones.
pub fn closest(
    candidates: impl IntoIterator<Item = [i32; 3]>,
    pos: [i32; 3],
    limit: i32,
) -> Option<[i32; 3]> {
    let max = i64::from(limit) * i64::from(limit);
    let mut best: Option<([i32; 3], i64)> = None;
    for c in candidates {
        let d = dist_sqr(pos, c);
        if d <= max && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((c, d));
        }
    }
    best.map(|(c, _)| c)
}

/// `SpecialFlowerBlockEntity.getModulatedDelay` for a flower on the ground: more ticks before it
/// sees an item on podzol and mycelium.
pub fn soil_delay(block_below: &str) -> i32 {
    match block_below {
        "minecraft:mycelium" => MYCELIUM_DELAY,
        "minecraft:podzol" => PODZOL_DELAY,
        _ => 0,
    }
}

/// `DelayHelper.canInteractWith` for a generating flower: the item has existed longer than
/// `59 + soil delay` ticks.
pub fn generating_sees(item_age: i32, soil_delay: i32) -> bool {
    item_age > GENERATING_INHERENT_DELAY + soil_delay
}

/// What a flower binds to and gives mana to: in Botania a generating flower's targets are mana
/// collectors (spreaders), found through the mana network.
pub trait Collectors {
    /// A block entity of the target kind at `pos` in a loaded chunk (`findBindCandidateAt`).
    fn is_target(&self, pos: [i32; 3]) -> bool;
    /// `findClosestTarget`: the nearest target within `radius` blocks of `pos`.
    fn closest(&self, pos: [i32; 3], radius: i32) -> Option<[i32; 3]>;
    /// The bound target's current and maximum mana and whether it is full; none without a
    /// target there (`findBoundTile`: no range check).
    fn status(&mut self, pos: [i32; 3]) -> Option<(i32, i32, bool)>;
    /// `receiveMana` on the target at `pos`.
    fn give(&mut self, pos: [i32; 3], mana: i32);
}

/// The state every generating flower has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Generating {
    /// `ticksExisted`, saved.
    pub ticks_existed: i32,
    /// `mana`, saved.
    pub mana: i32,
    /// `binding`, saved.
    pub binding: Option<[i32; 3]>,
}

impl Generating {
    /// `wouldBeValidBinding`: a target there within the flower's radius.
    pub fn would_be_valid(
        me: [i32; 3],
        pos: Option<[i32; 3]>,
        radius: i32,
        c: &impl Collectors,
    ) -> bool {
        pos.is_some_and(|p| {
            dist_sqr(me, p) <= i64::from(radius) * i64::from(radius) && c.is_target(p)
        })
    }

    /// `addMana`: capped at the flower's maximum (the rest is lost).
    pub fn add_mana(&mut self, mana: i32, max: i32) {
        self.mana = (self.mana + mana).min(max);
    }

    /// The common part of `tickFlower` on the server, before the flower's own: count the tick,
    /// bind to the nearest target on the first tick when the binding isn't valid, and empty the
    /// mana into the bound target (`emptyManaIntoCollector`). Whether the binding changed.
    pub fn tick(&mut self, me: [i32; 3], radius: i32, c: &mut impl Collectors) -> bool {
        self.ticks_existed += 1;
        let mut changed = false;
        if self.ticks_existed == 1 && !Self::would_be_valid(me, self.binding, radius, c) {
            let target = c.closest(me, radius);
            changed = target != self.binding;
            self.binding = target;
        }
        if let Some(pos) = self.binding
            && let Some((current, max, full)) = c.status(pos)
            && !full
            && self.mana > 0
        {
            let give = self.mana.min(max - current);
            self.mana -= give;
            c.give(pos, give);
        }
        changed
    }
}

/// An item lying near an Endoflame, as it judges it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fuel {
    /// Ticks the item entity has existed (Botania's `timeCounter`).
    pub age: i32,
    /// Its furnace burn time (`getSmeltingBurnTime`); 0 for a mana spreader.
    pub burn: i32,
    /// It leaves something behind when used (`hasCraftingRemainingItem`: a lava bucket).
    pub remainder: bool,
    pub count: u32,
}

/// An Endoflame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Endoflame {
    pub flower: Generating,
    /// `burnTime`, saved.
    pub burn_time: i32,
}

/// What an Endoflame's tick did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EndoflameTick {
    /// The fuel it lit: the index into the items, one of which is used up.
    pub lit: Option<usize>,
    /// It just stopped burning with nothing to light (`BLOCK_DEACTIVATE`).
    pub went_out: bool,
    /// Its binding changed.
    pub rebound: bool,
}

impl Endoflame {
    /// `EndoflameBlockEntity.tickFlower` on the server. `items` are the item entities in the
    /// 7×7×7 box around it, in the level's order; `soil` is its soil delay.
    pub fn tick(
        &mut self,
        me: [i32; 3],
        c: &mut impl Collectors,
        items: &[Fuel],
        soil: i32,
    ) -> EndoflameTick {
        let mut out = EndoflameTick {
            rebound: self.flower.tick(me, GENERATING_LINK_RANGE, c),
            ..EndoflameTick::default()
        };
        let was_burning = self.burn_time > 0;
        if was_burning {
            self.burn_time -= 1;
        }
        if self.burn_time > 0 && self.flower.ticks_existed % 2 == 0 {
            self.flower
                .add_mana(ENDOFLAME_MANA_PER_BURN, ENDOFLAME_MAX_MANA);
        }
        if self.burn_time == 0 {
            if self.flower.mana < ENDOFLAME_MAX_MANA {
                for (i, item) in items.iter().enumerate() {
                    if item.count == 0 || !generating_sees(item.age, soil) || item.remainder {
                        continue;
                    }
                    if item.burn > 0 {
                        self.burn_time = item.burn.min(ENDOFLAME_FUEL_CAP) / 2;
                        out.lit = Some(i);
                        return out;
                    }
                }
            }
            out.went_out = was_burning;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::POOL_MAX_MANA;
    use crate::pool::Pool;
    use std::collections::HashMap;

    /// Pools standing in for collectors, as a test world.
    #[derive(Default)]
    struct Targets(HashMap<[i32; 3], Pool>);

    impl Collectors for Targets {
        fn is_target(&self, pos: [i32; 3]) -> bool {
            self.0.contains_key(&pos)
        }
        fn closest(&self, pos: [i32; 3], radius: i32) -> Option<[i32; 3]> {
            let mut all: Vec<[i32; 3]> = self.0.keys().copied().collect();
            all.sort();
            closest(all, pos, radius)
        }
        fn status(&mut self, pos: [i32; 3]) -> Option<(i32, i32, bool)> {
            self.0
                .get(&pos)
                .map(|p| (p.current(), p.cap, p.is_full(false)))
        }
        fn give(&mut self, pos: [i32; 3], mana: i32) {
            self.0.get_mut(&pos).unwrap().receive(mana);
        }
    }

    fn coal(age: i32) -> Fuel {
        Fuel {
            age,
            burn: 1600,
            remainder: false,
            count: 1,
        }
    }

    #[test]
    fn distance_and_closest_are_botanias() {
        assert_eq!(dist_sqr([0, 0, 0], [6, 0, 0]), 36);
        assert_eq!(dist_sqr([0, 64, 0], [3, 60, 1]), 9 + 16 + 1);
        // Corners, not centres: 6 blocks away along x is in range 6, (6, 1, 0) is not.
        assert_eq!(closest([[6, 0, 0]], [0, 0, 0], 6), Some([6, 0, 0]));
        assert_eq!(closest([[6, 1, 0]], [0, 0, 0], 6), None);
        assert_eq!(
            closest([[3, 0, 0], [0, 2, 0], [-2, 0, 0]], [0, 0, 0], 6),
            Some([0, 2, 0]),
            "the first of the nearest"
        );
        assert_eq!(soil_delay("minecraft:podzol"), 5);
        assert_eq!(soil_delay("minecraft:mycelium"), 10);
        assert_eq!(soil_delay("minecraft:grass_block"), 0);
        // DelayHelper: a generating flower sees an item older than 59 ticks (+ soil).
        assert!(!generating_sees(59, 0));
        assert!(generating_sees(60, 0));
        assert!(!generating_sees(64, 5));
        assert!(generating_sees(65, 5));
    }

    #[test]
    fn a_flower_binds_on_its_first_tick_only() {
        let mut t = Targets::default();
        let me = [0, 64, 0];
        let mut f = Generating::default();
        f.tick(me, 6, &mut t);
        assert_eq!(f.binding, None, "nothing in range");
        t.0.insert([4, 64, 0], Pool::new(crate::consts::PoolVariant::Default));
        f.tick(me, 6, &mut t);
        assert_eq!(f.binding, None, "only the first tick binds");

        let mut g = Generating::default();
        assert!(g.tick(me, 6, &mut t));
        assert_eq!(g.binding, Some([4, 64, 0]));
        // A binding kept from a pick-block placement survives when valid, is replaced if not.
        let mut h = Generating {
            binding: Some([40, 64, 0]),
            ..Generating::default()
        };
        h.tick(me, 6, &mut t);
        assert_eq!(h.binding, Some([4, 64, 0]));
    }

    /// The six blocks `distance` away from `center` along the axes
    /// (`BindingTest.placeAxialFlowers`).
    fn axial(center: [i32; 3], distance: i32) -> Vec<[i32; 3]> {
        let [x, y, z] = center;
        vec![
            [x, y - distance, z],
            [x, y + distance, z],
            [x, y, z - distance],
            [x, y, z + distance],
            [x - distance, y, z],
            [x + distance, y, z],
        ]
    }

    #[test]
    fn botanias_binding_gametests() {
        // BindingTest.generatingFlowerAutoBindTest: a spreader in the middle of the arena;
        // generating flowers six blocks away along each axis bind to it on their first tick,
        // seven away don't.
        let middle = [16, 16, 16];
        let mut t = Targets::default();
        t.0.insert(middle, Pool::new(crate::consts::PoolVariant::Default));
        for pos in axial(middle, GENERATING_LINK_RANGE) {
            let mut f = Generating::default();
            f.tick(pos, GENERATING_LINK_RANGE, &mut t);
            assert_eq!(f.binding, Some(middle), "{pos:?}");
        }
        for pos in axial(middle, GENERATING_LINK_RANGE + 1) {
            let mut f = Generating::default();
            f.tick(pos, GENERATING_LINK_RANGE, &mut t);
            assert_eq!(f.binding, None, "{pos:?}");
        }
        // generatingFlowerManualBindTest: flowers placed first don't bind; the wand binds the
        // close ones (`bindTo` checks `wouldBeValidBinding`) and refuses the far ones.
        for (distance, binds) in [(6, true), (7, false)] {
            for pos in axial(middle, distance) {
                assert_eq!(
                    Generating::would_be_valid(pos, Some(middle), GENERATING_LINK_RANGE, &t),
                    binds,
                    "{pos:?}"
                );
            }
        }
        // Only a target there counts.
        assert!(!Generating::would_be_valid(
            [16, 16, 20],
            Some([16, 16, 18]),
            GENERATING_LINK_RANGE,
            &t
        ));
    }

    #[test]
    fn mana_moves_to_the_target_each_tick_up_to_its_room() {
        let mut t = Targets::default();
        let pos = [1, 64, 0];
        let mut pool = Pool::new(crate::consts::PoolVariant::Default);
        pool.mana = POOL_MAX_MANA - 100;
        t.0.insert(pos, pool);
        let mut f = Generating {
            ticks_existed: 5,
            mana: 300,
            binding: Some(pos),
        };
        f.tick([0, 64, 0], 6, &mut t);
        assert_eq!(f.mana, 200);
        assert_eq!(t.0[&pos].mana, POOL_MAX_MANA);
        f.tick([0, 64, 0], 6, &mut t);
        assert_eq!(f.mana, 200, "a full target takes nothing");
        // The transfer doesn't check the range again (Botania's emptyManaIntoCollector).
        let far = [30, 64, 0];
        t.0.insert(far, Pool::new(crate::consts::PoolVariant::Default));
        f.binding = Some(far);
        f.tick([0, 64, 0], 6, &mut t);
        assert_eq!(f.mana, 0);
        assert_eq!(t.0[&far].mana, 200);
    }

    /// Ticks an Endoflame bound to a pool, lighting one coal on `light_tick`; the pool's mana at
    /// the end.
    fn burn_one_coal(light_tick: i32) -> (i32, i32) {
        let mut t = Targets::default();
        let pool = [2, 64, 0];
        t.0.insert(pool, Pool::new(crate::consts::PoolVariant::Default));
        let mut e = Endoflame::default();
        let mut coal_left = 1;
        let mut lit_at = None;
        for tick in 1..=3000 {
            let items: Vec<Fuel> = if tick >= light_tick && coal_left > 0 {
                vec![coal(100)]
            } else {
                Vec::new()
            };
            let r = e.tick([0, 64, 0], &mut t, &items, 0);
            if r.lit.is_some() {
                coal_left -= 1;
                lit_at = Some(tick);
            }
        }
        assert_eq!(lit_at, Some(light_tick));
        (t.0[&pool].mana, e.flower.mana)
    }

    #[test]
    fn coal_gives_1200_or_1197_mana_by_tick_parity() {
        // Coal burns 1600 ticks in a furnace: the Endoflame burns it 800, adding 3 mana on the
        // even ticksExisted of the 799 ticks after the one it was lit on that still burn.
        assert_eq!(
            burn_one_coal(10),
            (1197, 0),
            "lit on an even tick: 399 even ticks follow"
        );
        assert_eq!(
            burn_one_coal(11),
            (1200, 0),
            "lit on an odd tick: 400 even ticks follow"
        );
    }

    #[test]
    fn endoflame_burns_on_when_full_and_lights_one_item_a_tick() {
        let mut t = Targets::default();
        let mut e = Endoflame {
            flower: Generating {
                ticks_existed: 10,
                mana: 300,
                binding: None,
            },
            burn_time: 0,
        };
        // Full: it lights nothing.
        let r = e.tick([0, 64, 0], &mut t, &[coal(100)], 0);
        assert_eq!(r.lit, None);
        e.flower.mana = 0;
        // Too young, a lava bucket, an empty stack and a non-fuel are skipped; the first coal
        // is lit, the burn time capped at 32000 / 2.
        let items = [
            coal(30),
            Fuel {
                age: 100,
                burn: 20000,
                remainder: true,
                count: 1,
            },
            Fuel {
                age: 100,
                burn: 1600,
                remainder: false,
                count: 0,
            },
            Fuel {
                age: 100,
                burn: 0,
                remainder: false,
                count: 1,
            },
            Fuel {
                age: 100,
                burn: 80000,
                remainder: false,
                count: 64,
            },
            coal(100),
        ];
        let r = e.tick([0, 64, 0], &mut t, &items, 0);
        assert_eq!(r.lit, Some(4));
        assert_eq!(e.burn_time, 16000);
        // It keeps burning while full; what doesn't fit is lost.
        e.flower.mana = 300;
        let before = e.burn_time;
        let r = e.tick([0, 64, 0], &mut t, &items, 0);
        assert_eq!(r.lit, None);
        assert_eq!(e.burn_time, before - 1);
        assert_eq!(e.flower.mana, 300);
        // On podzol the item must be 5 ticks older.
        let mut p = Endoflame::default();
        p.flower.ticks_existed = 2;
        assert_eq!(p.tick([0, 64, 0], &mut t, &[coal(62)], 5).lit, None);
        assert_eq!(p.tick([0, 64, 0], &mut t, &[coal(65)], 5).lit, Some(0));
        // Going out with nothing to light is told once.
        let mut q = Endoflame {
            burn_time: 1,
            ..Endoflame::default()
        };
        assert!(q.tick([0, 64, 0], &mut t, &[], 0).went_out);
        assert!(!q.tick([0, 64, 0], &mut t, &[], 0).went_out);
    }
}
