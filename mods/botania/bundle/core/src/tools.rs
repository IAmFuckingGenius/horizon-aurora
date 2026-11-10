//! Botania's remaining tools, without the game: the Terra Truncator's tree felling
//! (`TerraTruncatorItem.BlockSwapper`, a search through a `java.util.PriorityQueue`), the
//! elementium axe's beheading (the `elementium_axe_beheading` loot table `LootHandler` adds to
//! every entity's), the elementium shears' area shearing (`ElementiumShearsItem.onUseTick`), the
//! rings' changes to the Terra Shatterer's area (`TerraShattererItem.breakOtherBlock`) and the
//! rank letters its tooltip draws by the mana bar (`ManaBarTooltipComponent`).

use crate::burst::Aabb;
use crate::weapons;
use std::collections::HashSet;

pub const TERRA_AXE: &str = "botania:terra_axe";
pub const ELEMENTIUM_AXE: &str = "botania:elementium_axe";
pub const ELEMENTIUM_SHEARS: &str = "botania:elementium_shears";
pub const ELEMENTIUM_HOE: &str = "botania:elementium_hoe";

// ---- the rings --------------------------------------------------------------------------------

/// What a player carries that changes an area tool: a Ring of Thor (`RingOfThorItem.getThorRing`)
/// and an active Stone of Temperance (`StoneOfTemperanceItem.hasTemperanceActive`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AreaRings {
    pub thor: bool,
    pub temperance: bool,
}

/// `TerraShattererItem.breakOtherBlock` with the rings: a Ring of Thor spreads the area along
/// every axis and adds a rank, a Stone of Temperance holds it to rank B (2) at most.
pub fn pick_area_with(
    level: i32,
    face: [i32; 3],
    rings: AreaRings,
) -> Option<([i32; 3], [i32; 3])> {
    let face = if rings.thor { [0; 3] } else { face };
    let mut level = level + i32::from(rings.thor);
    if rings.temperance && level > 2 {
        level = 2;
    }
    weapons::pick_area(level, face)
}

// ---- the Terra Truncator ----------------------------------------------------------------------

/// `TerraTruncatorItem.BLOCK_SWAP_RATE`: blocks a felling takes a tick.
pub const BLOCK_SWAP_RATE: i32 = 10;
/// `TerraTruncatorItem.BLOCK_RANGE`: steps a felling goes from the block mined.
pub const BLOCK_RANGE: i32 = 32;
/// `TerraTruncatorItem.LEAF_BLOCK_RANGE`: steps it goes on through leaves.
pub const LEAF_BLOCK_RANGE: i32 = 3;
/// How far `onBlockStartBreak` looks for the block the player mines (`raytraceFromEntity`).
pub const AXE_REACH: f64 = 10.0;

/// `TerraTruncatorItem.shouldBreak`: it fells unless the player sneaks or has an active Stone of
/// Temperance.
pub fn truncator_fells(sneaking: bool, temperance: bool) -> bool {
    !sneaking && !temperance
}

/// `BlockSwapper.SwapCandidate`: a block to break and the steps left from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub pos: [i32; 3],
    pub range: i32,
}

impl Candidate {
    /// `compareTo`: `other.range - range`, so the larger range comes first.
    fn compare(&self, other: &Candidate) -> i32 {
        other.range - self.range
    }
}

/// `java.util.PriorityQueue<SwapCandidate>`: a binary min-heap with Java's own sift up and sift
/// down, so candidates of the same range come out in the order Botania's do.
#[derive(Clone, Debug, Default)]
pub struct CandidateQueue {
    heap: Vec<Candidate>,
}

impl CandidateQueue {
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// `offer`: `siftUpComparable`.
    pub fn offer(&mut self, c: Candidate) {
        let mut k = self.heap.len();
        self.heap.push(c);
        while k > 0 {
            let parent = (k - 1) >> 1;
            if c.compare(&self.heap[parent]) >= 0 {
                break;
            }
            self.heap[k] = self.heap[parent];
            k = parent;
        }
        self.heap[k] = c;
    }

    /// `poll`: the head, the last element sifted down from the top (`siftDownComparable`).
    pub fn poll(&mut self) -> Option<Candidate> {
        let last = self.heap.pop()?;
        let n = self.heap.len();
        if n == 0 {
            return Some(last);
        }
        let result = self.heap[0];
        let mut k = 0;
        let half = n >> 1;
        while k < half {
            let mut child = (k << 1) + 1;
            let right = child + 1;
            if right < n && self.heap[child].compare(&self.heap[right]) > 0 {
                child = right;
            }
            if last.compare(&self.heap[child]) <= 0 {
                break;
            }
            self.heap[k] = self.heap[child];
            k = child;
        }
        self.heap[k] = last;
        Some(result)
    }
}

/// What a block is to a felling: `#minecraft:logs` and mangrove roots carry it on a step,
/// `#minecraft:leaves` at most `LEAF_BLOCK_RANGE` more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Growth {
    Wood,
    Leaves,
}

/// What the Terra Truncator needs of the level.
pub trait TreeWorld {
    /// The block at `pos`, if it carries the felling on.
    fn growth(&self, pos: [i32; 3]) -> Option<Growth>;
    /// `ToolCommons.removeBlockWithDrops` with the truncator's filter (`#minecraft:mineable/axe`
    /// or `#minecraft:leaves`): broken as the player, with the truncator, if it is loaded, not air
    /// and breakable.
    fn remove(&mut self, pos: [i32; 3]);
}

/// `TerraTruncatorItem.BlockSwapper`: one felling, from the block the player mined.
#[derive(Clone, Debug, Default)]
pub struct Swapper {
    queue: CandidateQueue,
    completed: HashSet<[i32; 3]>,
}

impl Swapper {
    /// A felling from `origin` with `range` steps (`BLOCK_RANGE` for the truncator's own).
    pub fn new(origin: [i32; 3], range: i32) -> Swapper {
        let mut queue = CandidateQueue::default();
        queue.offer(Candidate { pos: origin, range });
        Swapper {
            queue,
            completed: HashSet::new(),
        }
    }

    /// `BlockSwapper.tick`: up to `BLOCK_SWAP_RATE` candidates, the larger range first, each one
    /// not seen yet and in range broken and its 26 neighbours that are wood or leaves queued
    /// (`adjacent`: x outermost, then y, then z), a leaf with at most `LEAF_BLOCK_RANGE` steps
    /// left. False once there was nothing left to look at: the felling is over.
    pub fn tick(&mut self, world: &mut impl TreeWorld) -> bool {
        if self.queue.is_empty() {
            return false;
        }
        let mut remaining = BLOCK_SWAP_RATE;
        while remaining > 0 {
            let Some(cand) = self.queue.poll() else {
                break;
            };
            if self.completed.contains(&cand.pos) || cand.range <= 0 {
                continue;
            }
            world.remove(cand.pos);
            remaining -= 1;
            self.completed.insert(cand.pos);
            for adj in adjacent(cand.pos) {
                let range = match world.growth(adj) {
                    None => continue,
                    Some(Growth::Leaves) => LEAF_BLOCK_RANGE.min(cand.range - 1),
                    Some(Growth::Wood) => cand.range - 1,
                };
                self.queue.offer(Candidate { pos: adj, range });
            }
        }
        true
    }
}

/// `BlockSwapper.adjacent`: the 26 blocks around `pos`, x outermost, then y, then z.
pub fn adjacent(pos: [i32; 3]) -> Vec<[i32; 3]> {
    let mut out = Vec::with_capacity(26);
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if dx == 0 && dy == 0 && dz == 0 {
                    continue;
                }
                out.push([pos[0] + dx, pos[1] + dy, pos[2] + dz]);
            }
        }
    }
    out
}

// ---- the elementium axe ----------------------------------------------------------------------

/// A pool of `elementium_axe_beheading`: the head and its `random_chance_with_looting`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Head {
    pub item: &'static str,
    pub chance: f32,
    pub looting_multiplier: f32,
}

/// The pool an entity's death rolls when a player holding an elementium axe killed it: a
/// wither skeleton its skull, the rest of `#minecraft:skeletons` a skeleton skull (`in_skeletons`
/// says the entity is in the tag), zombies, zombie villagers, husks and drowned a zombie head,
/// piglins and piglin brutes a piglin head, creepers a creeper head, players their own head.
pub fn beheading(entity: &str, in_skeletons: bool) -> Option<Head> {
    let (item, chance, looting_multiplier) = match entity {
        "minecraft:wither_skeleton" => ("minecraft:wither_skeleton_skull", 0.1154, 0.0385),
        _ if in_skeletons => ("minecraft:skeleton_skull", 0.1154, 0.0385),
        "minecraft:zombie"
        | "minecraft:zombie_villager"
        | "minecraft:husk"
        | "minecraft:drowned" => ("minecraft:zombie_head", 0.0769, 0.0769),
        "minecraft:piglin" | "minecraft:piglin_brute" => ("minecraft:piglin_head", 0.0769, 0.0769),
        "minecraft:creeper" => ("minecraft:creeper_head", 0.0769, 0.0769),
        "minecraft:player" => ("minecraft:player_head", 0.0909, 0.0909),
        _ => return None,
    };
    Some(Head {
        item,
        chance,
        looting_multiplier,
    })
}

/// `LootItemRandomChanceWithLootingCondition`: `nextFloat() < chance + looting * multiplier`, in
/// float, with the killer's Looting level.
pub fn beheads(head: Head, looting: i32, r: f32) -> bool {
    r < head.chance + looting as f32 * head.looting_multiplier
}

// ---- the elementium shears -------------------------------------------------------------------

/// `ElementiumShearsItem.getUseDuration`: held until let go.
pub const SHEARS_USE_DURATION: i32 = 72000;
/// How far around its user the shears reach, along each axis.
pub const SHEARS_RANGE: f64 = 12.0;

/// `onUseTick`'s `count != getUseDuration(stack) && count % 5 == 0`: whether the use shears on
/// the tick with `remaining` ticks of the use left.
pub fn shears_shear_now(remaining: i32) -> bool {
    remaining != SHEARS_USE_DURATION && remaining % 5 == 0
}

/// The box the shears look for `Shearable` entities in: 12 blocks round the user's feet.
pub fn shears_area(at: [f64; 3]) -> Aabb {
    let r = SHEARS_RANGE;
    [
        at[0] - r,
        at[1] - r,
        at[2] - r,
        at[0] + r,
        at[1] + r,
        at[2] + r,
    ]
}

/// `Shearable.readyForShearing` of the vanilla `Shearable`s the shears reach by their type (a
/// copper golem's own rule reads its item): a sheep alive, grown and unshorn, a mooshroom alive
/// and grown, a snow golem with its pumpkin, a bogged with its mushrooms. None for any other.
pub fn ready_for_shearing(kind: &str, alive: bool, baby: bool, sheared: bool) -> Option<bool> {
    Some(match kind {
        "minecraft:sheep" => alive && !sheared && !baby,
        "minecraft:mooshroom" => alive && !baby,
        "minecraft:snow_golem" | "minecraft:bogged" => alive && !sheared,
        _ => return None,
    })
}

// ---- the elementium hoe -----------------------------------------------------------------------

/// `ElementiumHoeItem.useOn`: the farmland a till made takes `FarmBlock.MOISTURE` 7.
pub const TILLED_MOISTURE: &str = "7";

// ---- the Terra Shatterer's tooltip -------------------------------------------------------------

/// `ChatFormatting`'s colour for a `§` code (`0` to `f`).
pub fn legacy_color(code: char) -> Option<u32> {
    Some(match code.to_ascii_lowercase() {
        '0' => 0x000000,
        '1' => 0x0000AA,
        '2' => 0x00AA00,
        '3' => 0x00AAAA,
        '4' => 0xAA0000,
        '5' => 0xAA00AA,
        '6' => 0xFFAA00,
        '7' => 0xAAAAAA,
        '8' => 0x555555,
        '9' => 0x5555FF,
        'a' => 0x55FF55,
        'b' => 0x55FFFF,
        'c' => 0xFF5555,
        'd' => 0xFF55FF,
        'e' => 0xFFFF55,
        'f' => 0xFFFFFF,
        _ => return None,
    })
}

/// A line with `§` codes as `Font` draws it: its runs of text and their colours, from `color`
/// (`§r` goes back to it; the style codes keep the colour).
pub fn legacy_runs(s: &str, color: u32) -> Vec<(String, u32)> {
    let mut runs: Vec<(String, u32)> = Vec::new();
    let mut current = color;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            match chars.next() {
                Some(code) => {
                    if let Some(rgb) = legacy_color(code) {
                        current = rgb;
                    } else if code.eq_ignore_ascii_case(&'r') {
                        current = color;
                    }
                }
                None => break,
            }
            continue;
        }
        match runs.last_mut() {
            Some((text, rgb)) if *rgb == current => text.push(c),
            _ => runs.push((c.to_string(), current)),
        }
    }
    runs
}

/// `TerraShattererItem.LEVELS.length - 1`: rank SS, which has no next rank to show.
pub fn rank_is_last(level: usize) -> bool {
    level + 1 >= weapons::PICK_LEVELS.len()
}

/// `ManaBarTooltipComponent.renderImage` for the Terra Shatterer: how many columns of the bar
/// the rainbow fills, one short of the end below rank SS.
pub fn rank_bar_width(total: i32, percent: f32, last: bool) -> i32 {
    let room = total - if last { 0 } else { 1 };
    room.min((total as f32 * percent) as i32)
}

/// The hue of the rainbow's column `column` of `total`, at `ticks` client ticks
/// (`(ticksInGame + partialTicks) * 0.01F + column / total`, wrapped).
pub fn rank_bar_hue(column: i32, total: i32, ticks: f32) -> f32 {
    let per = if total == 0 { 0.0 } else { 1.0 / total as f32 };
    (ticks * 0.01 + per * column as f32) % 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cand(x: i32, range: i32) -> Candidate {
        Candidate {
            pos: [x, 0, 0],
            range,
        }
    }

    #[test]
    fn candidates_come_out_as_javas_priority_queue_gives_them() {
        // Offered with ranges 5, 3, 5, 7, 3, 5 (x = 1 to 6). Java's heap, step by step:
        // [1] → [1,2] → [1,2,3] → 4 sifts up past 2 and 1: [4,1,3,2] → [4,1,3,2,5] →
        // [4,1,3,2,5,6]. Polls: 4; then 6 (the last) sifts down from the top and stays there,
        // as 1 isn't larger: 6 comes before 1 and 3, which a stable sort would not give.
        let mut q = CandidateQueue::default();
        for (x, r) in [(1, 5), (2, 3), (3, 5), (4, 7), (5, 3), (6, 5)] {
            q.offer(cand(x, r));
        }
        let order: Vec<i32> = std::iter::from_fn(|| q.poll()).map(|c| c.pos[0]).collect();
        assert_eq!(order, vec![4, 6, 1, 3, 2, 5]);
        assert!(q.is_empty());
    }

    #[derive(Default)]
    struct Forest {
        blocks: HashMap<[i32; 3], Growth>,
        removed: Vec<[i32; 3]>,
    }

    impl TreeWorld for Forest {
        fn growth(&self, pos: [i32; 3]) -> Option<Growth> {
            self.blocks.get(&pos).copied()
        }
        fn remove(&mut self, pos: [i32; 3]) {
            if self.blocks.remove(&pos).is_some() {
                self.removed.push(pos);
            }
        }
    }

    /// Runs a felling from `origin` (already broken by the player) to its end: the blocks each
    /// tick broke, and the ticks it ran (the last one returns false).
    fn fell(forest: &mut Forest, origin: [i32; 3]) -> (Vec<Vec<[i32; 3]>>, usize) {
        let mut s = Swapper::new(origin, BLOCK_RANGE);
        let mut ticks = Vec::new();
        for n in 1..100 {
            let before = forest.removed.len();
            if !s.tick(forest) {
                return (ticks, n);
            }
            ticks.push(forest.removed[before..].to_vec());
        }
        panic!("the felling never ended");
    }

    #[test]
    fn a_trunk_falls_ten_candidates_a_tick() {
        // Logs from y 65 to 76 over the one mined at 64. Tick 1 looks at the mined block (air,
        // but it counts) and nine logs; tick 2 the last three and finds nothing more; tick 3
        // has an empty queue and ends the felling.
        let mut forest = Forest::default();
        for y in 65..=76 {
            forest.blocks.insert([0, y, 0], Growth::Wood);
        }
        let (ticks, ended) = fell(&mut forest, [0, 64, 0]);
        let ys: Vec<Vec<i32>> = ticks
            .iter()
            .map(|t| t.iter().map(|p| p[1]).collect())
            .collect();
        assert_eq!(ys, vec![(65..=73).collect::<Vec<_>>(), vec![74, 75, 76]]);
        assert_eq!(ended, 3);
    }

    #[test]
    fn leaves_take_three_steps_at_most() {
        // A trunk of 3 (y 65 to 67) and a row of leaves going east from its top at y 67.
        // The top log has range 29: the first leaf gets min(3, 28) = 3, then 2, then 1, and
        // the fourth leaf 0, which is dropped when it comes out: leaves 1 to 3 go.
        let mut forest = Forest::default();
        for y in 65..=67 {
            forest.blocks.insert([0, y, 0], Growth::Wood);
        }
        for x in 1..=6 {
            forest.blocks.insert([x, 67, 0], Growth::Leaves);
        }
        fell(&mut forest, [0, 64, 0]);
        let left: Vec<i32> = (1..=6)
            .filter(|x| forest.blocks.contains_key(&[*x, 67, 0]))
            .collect();
        assert_eq!(left, vec![4, 5, 6]);
        // A log under the leaves goes back to the full range: a leaf bridge to a second trunk
        // at x 3 reaches it with 1 step left, so its first log is taken and no more.
        let mut bridge = Forest::default();
        for y in 65..=67 {
            bridge.blocks.insert([0, y, 0], Growth::Wood);
        }
        for x in 1..=2 {
            bridge.blocks.insert([x, 67, 0], Growth::Leaves);
        }
        for y in 60..=67 {
            bridge.blocks.insert([3, y, 0], Growth::Wood);
        }
        fell(&mut bridge, [0, 64, 0]);
        let trunk: Vec<i32> = (60..=67)
            .filter(|y| bridge.blocks.contains_key(&[3, *y, 0]))
            .collect();
        // The leaf at x 2 has 2 steps left; the logs beside it (x 3, y 66 and 67) get 1 and are
        // broken; their own neighbours would get 0 and stay.
        assert_eq!(trunk, (60..=65).collect::<Vec<_>>());
    }

    #[test]
    fn the_truncator_rests_while_sneaking_or_tempered() {
        assert!(truncator_fells(false, false));
        assert!(!truncator_fells(true, false));
        assert!(!truncator_fells(false, true));
        assert_eq!(adjacent([0, 0, 0]).len(), 26);
        assert_eq!(adjacent([0, 0, 0])[0], [-1, -1, -1]);
        assert_eq!(adjacent([0, 0, 0])[1], [-1, -1, 0]);
        assert_eq!(adjacent([0, 0, 0])[25], [1, 1, 1]);
    }

    #[test]
    fn the_elementium_axe_beheads_by_the_tables_chances() {
        let zombie = beheading("minecraft:zombie", false).unwrap();
        assert_eq!(zombie.item, "minecraft:zombie_head");
        // 7.69% bare; Looting III adds three times as much: 0.0769 + 3 * 0.0769 = 0.3076.
        assert!(beheads(zombie, 0, 0.0768));
        assert!(!beheads(zombie, 0, 0.0769));
        assert!(beheads(zombie, 3, 0.3075));
        assert!(!beheads(zombie, 3, 0.3077));
        // Wither skeletons drop their skull at 11.54% + 3.85% a Looting level; the others of
        // the skeletons' tag a skeleton skull at the same rate.
        let wither = beheading("minecraft:wither_skeleton", true).unwrap();
        assert_eq!(wither.item, "minecraft:wither_skeleton_skull");
        assert!(beheads(wither, 1, 0.1538));
        assert!(!beheads(wither, 1, 0.154));
        assert_eq!(
            beheading("minecraft:stray", true).map(|h| h.item),
            Some("minecraft:skeleton_skull")
        );
        assert_eq!(
            beheading("minecraft:drowned", false).map(|h| h.item),
            Some("minecraft:zombie_head")
        );
        assert_eq!(
            beheading("minecraft:piglin_brute", false).map(|h| h.item),
            Some("minecraft:piglin_head")
        );
        assert_eq!(beheading("minecraft:zombified_piglin", false), None);
        assert_eq!(beheading("minecraft:ender_dragon", false), None);
        assert_eq!(beheading("minecraft:player", false).unwrap().chance, 0.0909);
    }

    #[test]
    fn the_shears_shear_every_fifth_tick_of_their_use() {
        // The first tick of the use has 72000 ticks left: nothing. Then the 5th, 10th...
        assert!(!shears_shear_now(72000));
        assert!(!shears_shear_now(71999));
        assert!(shears_shear_now(71995));
        assert!(!shears_shear_now(71994));
        assert!(shears_shear_now(71990));
        assert_eq!(
            shears_area([0.5, 64.0, -3.5]),
            [-11.5, 52.0, -15.5, 12.5, 76.0, 8.5]
        );
        assert_eq!(
            ready_for_shearing("minecraft:sheep", true, false, false),
            Some(true)
        );
        assert_eq!(
            ready_for_shearing("minecraft:sheep", true, true, false),
            Some(false)
        );
        assert_eq!(
            ready_for_shearing("minecraft:sheep", true, false, true),
            Some(false)
        );
        // A mooshroom is always ready once grown; a snow golem while it has its pumpkin.
        assert_eq!(
            ready_for_shearing("minecraft:mooshroom", true, false, true),
            Some(true)
        );
        assert_eq!(
            ready_for_shearing("minecraft:snow_golem", true, false, true),
            Some(false)
        );
        assert_eq!(
            ready_for_shearing("minecraft:cow", true, false, false),
            None
        );
    }

    #[test]
    fn the_rings_change_the_shatterers_area() {
        // Rank A (3) facing north with a Ring of Thor: rank S's reach on every axis, 7 by 7 by
        // 7 from one below to five above: 342 blocks besides the one mined.
        let thor = AreaRings {
            thor: true,
            temperance: false,
        };
        let (b, e) = pick_area_with(3, [0, 0, -1], thor).unwrap();
        assert_eq!((b, e), ([-3, -1, -3], [3, 5, 3]));
        assert_eq!(weapons::between_closed([0, 64, 0], b, e).len(), 342);
        // Temperance holds rank SS on the top face to rank B: the 3 by 3.
        let tempered = AreaRings {
            thor: false,
            temperance: true,
        };
        assert_eq!(
            pick_area_with(5, [0, 1, 0], tempered),
            Some(([-1, 0, -1], [1, 0, 1]))
        );
        // Both on rank C (1): the ring makes it rank B on every axis, under the cap.
        let both = AreaRings {
            thor: true,
            temperance: true,
        };
        assert_eq!(
            pick_area_with(1, [1, 0, 0], both),
            Some(([-1, -1, -1], [1, 1, 1]))
        );
        // No rings: the area as before.
        assert_eq!(
            pick_area_with(2, [0, 1, 0], AreaRings::default()),
            weapons::pick_area(2, [0, 1, 0])
        );
    }

    #[test]
    fn rank_letters_and_the_rainbow_bar() {
        // The lang file's ranks: "§cD" is a red D, "§6SS" a gold SS.
        assert_eq!(
            legacy_runs("§cD", 0xFFFFFF),
            vec![("D".to_owned(), 0xFF5555)]
        );
        assert_eq!(
            legacy_runs("§6SS", 0xFFFFFF),
            vec![("SS".to_owned(), 0xFFAA00)]
        );
        assert_eq!(
            legacy_runs("§bB§7 Rank", 0xFFFFFF),
            vec![("B".to_owned(), 0x55FFFF), (" Rank".to_owned(), 0xAAAAAA)]
        );
        assert_eq!(
            legacy_runs("a§lb§rc", 0x123456),
            vec![("abc".to_owned(), 0x123456)]
        );
        assert!(rank_is_last(5) && !rank_is_last(4));
        // 100 wide: half way fills 50 columns; full stops one short below SS.
        assert_eq!(rank_bar_width(100, 0.5, false), 50);
        assert_eq!(rank_bar_width(100, 1.0, false), 99);
        assert_eq!(rank_bar_width(100, 1.0, true), 100);
        assert_eq!(rank_bar_width(100, 0.0, false), 0);
        // Column 25 of 100 after 50 ticks: 0.5 + 0.25.
        assert!((rank_bar_hue(25, 100, 50.0) - 0.75).abs() < 1e-6);
        assert!((rank_bar_hue(50, 100, 70.0) - 0.2).abs() < 1e-5);
        assert_eq!(rank_bar_hue(3, 0, 0.0), 0.0);
    }
}
