//! The Pure Daisy (`PureDaisyBlockEntity`): it visits the eight blocks around it one a tick and
//! turns each into its recipe's output after `time` visits.

/// `POSITIONS`, in Botania's order.
pub const POSITIONS: [[i32; 3]; 8] = [
    [-1, 0, -1],
    [-1, 0, 0],
    [-1, 0, 1],
    [0, 0, 1],
    [1, 0, 1],
    [1, 0, 0],
    [1, 0, -1],
    [0, 0, -1],
];

/// A Pure Daisy's state, as its block entity saves it (`position`, `ticksRemaining0..7`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PureDaisy {
    /// `positionAt`: the last position visited.
    pub position: usize,
    /// Visits left for each position's block; -1 when none is under way.
    pub ticks_remaining: [i32; 8],
}

impl Default for PureDaisy {
    fn default() -> Self {
        PureDaisy {
            position: 0,
            ticks_remaining: [-1; 8],
        }
    }
}

/// What a Pure Daisy's tick did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DaisyTick {
    /// It converts the block at this position now (its index in [`POSITIONS`]) and where it is.
    pub convert: Option<(usize, [i32; 3])>,
    /// `ticksRemaining` changed: clients are sent the block entity again.
    pub changed: bool,
}

impl PureDaisy {
    /// The server half of `tickFlower`. `recipe_time(pos)` is the time of the first recipe that
    /// matches the block at `pos` (`findRecipe`), none for air or no recipe. The caller sets the
    /// block when `convert` is told.
    pub fn tick(
        &mut self,
        me: [i32; 3],
        mut recipe_time: impl FnMut([i32; 3]) -> Option<i32>,
    ) -> DaisyTick {
        let before = self.ticks_remaining;
        self.position = (self.position + 1) % POSITIONS.len();
        let i = self.position;
        let o = POSITIONS[i];
        let pos = [me[0] + o[0], me[1] + o[1], me[2] + o[2]];
        let mut out = DaisyTick::default();
        match recipe_time(pos) {
            Some(time) => {
                if self.ticks_remaining[i] == -1 {
                    self.ticks_remaining[i] = time;
                }
                self.ticks_remaining[i] -= 1;
                if self.ticks_remaining[i] <= 0 {
                    self.ticks_remaining[i] = -1;
                    out.convert = Some((i, pos));
                }
            }
            None => self.ticks_remaining[i] = -1,
        }
        out.changed = before != self.ticks_remaining;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::PURE_DAISY_DEFAULT_TIME;
    use std::collections::HashMap;

    #[test]
    fn stone_turns_after_1193_ticks_at_the_first_position_visited() {
        // Eight stone blocks around the daisy; each conversion is recorded with its tick.
        let me = [0, 64, 0];
        let mut stone: HashMap<[i32; 3], bool> =
            POSITIONS.iter().map(|o| ([o[0], 64, o[2]], true)).collect();
        let mut daisy = PureDaisy::default();
        let mut converted = Vec::new();
        for tick in 1..=1300 {
            let r = daisy.tick(me, |p| {
                stone
                    .get(&p)
                    .copied()
                    .filter(|&s| s)
                    .map(|_| PURE_DAISY_DEFAULT_TIME)
            });
            if let Some((i, p)) = r.convert {
                stone.insert(p, false);
                converted.push((tick, i));
            }
        }
        // Visits every 8 ticks: 150 visits end 149 × 8 ticks after the first. Position 1 is
        // visited first (tick 1), position 0 last (tick 8).
        assert_eq!(converted.len(), 8);
        assert_eq!(converted[0], (1193, 1));
        assert_eq!(converted[6], (1199, 7));
        assert_eq!(converted[7], (1200, 0));
        assert_eq!(daisy.ticks_remaining, [-1; 8]);
    }

    #[test]
    fn progress_resets_when_the_block_changes() {
        let me = [0, 0, 0];
        let mut daisy = PureDaisy::default();
        let mut present = true;
        let mut first = None;
        for tick in 1..=2000 {
            if tick == 401 {
                present = false; // the block was mined for a while
            }
            if tick == 441 {
                present = true;
            }
            let r = daisy.tick(me, |p| (p == [-1, 0, 0] && present).then_some(150));
            if r.convert.is_some() && first.is_none() {
                first = Some(tick);
            }
        }
        // Position 1 is visited on ticks 1, 9, 17...; after the gap the count starts again at
        // the first visit from tick 441 on (tick 441), so it turns 149 visits later.
        assert_eq!(first, Some(441 + 149 * 8));
        // A recipe of time 0 or 1 converts on the first visit.
        let mut quick = PureDaisy::default();
        assert_eq!(quick.tick(me, |_| Some(1)).convert, Some((1, [-1, 0, 0])));
        assert_eq!(quick.tick(me, |_| Some(0)).convert, Some((2, [-1, 0, 1])));
    }
}
