//! Botania's worn trinkets ("baubles"), its relics and the Stone of Temperance, without the game:
//! the rings (`BandOfAuraItem`, `RingOfMagnetizationItem`, `RingOfChordataItem`,
//! `RingOfCorrectionItem`, `RingOfDexterousMotionItem`, `RingOfTheMantleItem`,
//! `GreatFairyRingItem`, `RingOfFarReachItem`), the relic rings (`RingOfThorItem`, `RingOfOdinItem`,
//! `RingOfLokiItem`) and their binding (`RelicImpl`), the belts, pendants and cloaks, and
//! `StoneOfTemperanceItem`. Botania wears them in Curios or Trinkets slots; Horizon has none, so
//! the port wears a trinket while the player carries it (the main inventory or the off hand), as
//! it already does for the bands of mana.

use crate::mth;

// ---- the items ----------------------------------------------------------------------------------

pub const AURA_RING: &str = "botania:aura_ring";
pub const AURA_RING_GREATER: &str = "botania:aura_ring_greater";
pub const MAGNET_RING: &str = "botania:magnet_ring";
pub const MAGNET_RING_GREATER: &str = "botania:magnet_ring_greater";
pub const WATER_RING: &str = "botania:water_ring";
pub const SWAP_RING: &str = "botania:swap_ring";
pub const DODGE_RING: &str = "botania:dodge_ring";
pub const MINING_RING: &str = "botania:mining_ring";
pub const PIXIE_RING: &str = "botania:pixie_ring";
pub const REACH_RING: &str = "botania:reach_ring";
pub const THOR_RING: &str = "botania:thor_ring";
pub const ODIN_RING: &str = "botania:odin_ring";
pub const LOKI_RING: &str = "botania:loki_ring";
pub const TEMPERANCE_STONE: &str = "botania:temperance_stone";
pub const TRAVEL_BELT: &str = "botania:travel_belt";
pub const SUPER_TRAVEL_BELT: &str = "botania:super_travel_belt";
pub const SPEED_UP_BELT: &str = "botania:speed_up_belt";
pub const KNOCKBACK_BELT: &str = "botania:knockback_belt";
pub const LAVA_PENDANT: &str = "botania:lava_pendant";
pub const SUPER_LAVA_PENDANT: &str = "botania:super_lava_pendant";
pub const CLOUD_PENDANT: &str = "botania:cloud_pendant";
pub const SUPER_CLOUD_PENDANT: &str = "botania:super_cloud_pendant";
pub const HOLY_CLOAK: &str = "botania:holy_cloak";
pub const UNHOLY_CLOAK: &str = "botania:unholy_cloak";
pub const BALANCE_CLOAK: &str = "botania:balance_cloak";
pub const INVISIBILITY_CLOAK: &str = "botania:invisibility_cloak";
pub const GODDESS_CHARM: &str = "botania:goddess_charm";

/// Botania's `BaubleItem`s: every item a Curios or Trinkets slot takes (`BotaniaItems`, the
/// "Baubles" block and the cosmetics), the relic rings among them.
pub fn is_bauble(item: &str) -> bool {
    matches!(
        item,
        "botania:mana_ring"
            | "botania:mana_ring_greater"
            | AURA_RING
            | AURA_RING_GREATER
            | MAGNET_RING
            | MAGNET_RING_GREATER
            | WATER_RING
            | SWAP_RING
            | DODGE_RING
            | MINING_RING
            | PIXIE_RING
            | REACH_RING
            | TRAVEL_BELT
            | SUPER_TRAVEL_BELT
            | SPEED_UP_BELT
            | KNOCKBACK_BELT
            | "botania:ice_pendant"
            | LAVA_PENDANT
            | SUPER_LAVA_PENDANT
            | CLOUD_PENDANT
            | SUPER_CLOUD_PENDANT
            | HOLY_CLOAK
            | UNHOLY_CLOAK
            | BALANCE_CLOAK
            | INVISIBILITY_CLOAK
            | "botania:third_eye"
            | "botania:monocle"
            | "botania:tiny_planet"
            | GODDESS_CHARM
            | "botania:diva_charm"
            | "botania:itemfinder"
            | "botania:flight_tiara"
            | "botania:blood_pendant"
            | THOR_RING
            | ODIN_RING
            | LOKI_RING
    ) || item.starts_with("botania:cosmetic_")
}

/// The items with Botania's `Relic` capability: the three rings, the Dice of Fate, the Eye of the
/// Flügel, the Fruit of Grisaia and the Key of the King's Law.
pub fn is_relic(item: &str) -> bool {
    matches!(
        item,
        THOR_RING
            | ODIN_RING
            | LOKI_RING
            | "botania:dice"
            | "botania:flugel_eye"
            | "botania:infinite_fruit"
            | "botania:king_key"
    )
}

/// The items registered `unstackable()` that this module looks after: the baubles (the bands of
/// mana are the mana module's), the relics and the Stone of Temperance.
pub fn unstackable(item: &str) -> bool {
    (is_bauble(item) && !matches!(item, "botania:mana_ring" | "botania:mana_ring_greater"))
        || is_relic(item)
        || item == TEMPERANCE_STONE
}

/// `Item.Properties.rarity` of these items (`BotaniaItems`): the relics are epic, the Ring of
/// Chordata rare, the Charm of the Diva and the Flügel Tiara uncommon.
pub fn rarity(item: &str) -> Option<&'static str> {
    match item {
        _ if is_relic(item) => Some("epic"),
        WATER_RING => Some("rare"),
        "botania:diva_charm" | "botania:flight_tiara" => Some("uncommon"),
        _ => None,
    }
}

/// `Item.Properties.fireResistant` of these items: the relics.
pub fn fire_resistant(item: &str) -> bool {
    is_relic(item)
}

// ---- relics (`RelicImpl`) -------------------------------------------------------------------------

/// The stack's NBT key holding the owner's UUID as text.
pub const TAG_SOULBIND: &str = "soulbindUUID";
/// What a relic deals to a player it isn't bound to, every 10 of their ticks (`relicDamage`).
pub const RELIC_WRONG_PLAYER_DAMAGE: f32 = 2.0;

/// What `RelicImpl.tickBinding` does for a player carrying the relic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelicTick {
    /// Unbound: it binds to the player.
    Bind,
    /// Another player's, on a tick count that is a multiple of 10: it hurts them.
    Hurt,
    Nothing,
}

/// `RelicImpl.tickBinding`: `bound` is the owner `getSoulbindUUID` reads (None when unbound or
/// not a well-formed UUID), `tick_count` the player's `tickCount`.
pub fn relic_tick(bound: Option<u128>, player: u128, tick_count: i32) -> RelicTick {
    match bound {
        None => RelicTick::Bind,
        Some(owner) if owner != player && tick_count % 10 == 0 => RelicTick::Hurt,
        Some(_) => RelicTick::Nothing,
    }
}

/// `UUID.toString`: `8-4-4-4-12` lowercase hex digits of the 128 bits, most significant first.
pub fn uuid_to_string(uuid: u128) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (uuid >> 96) as u32,
        (uuid >> 80) as u16,
        (uuid >> 64) as u16,
        (uuid >> 48) as u16,
        uuid & 0xFFFF_FFFF_FFFF
    )
}

/// `UUID.fromString`: five dash-separated hex fields, each masked to its width (8, 4, 4, 4 and
/// 12 digits); None for anything else, which `getSoulbindUUID` treats as unbound.
pub fn uuid_from_string(text: &str) -> Option<u128> {
    if text.len() > 36 {
        return None;
    }
    let parts: Vec<&str> = text.split('-').collect();
    if parts.len() != 5 {
        return None;
    }
    let mut fields = [0u64; 5];
    for (field, part) in fields.iter_mut().zip(&parts) {
        if part.is_empty() || part.len() > 16 || !part.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        *field = u64::from_str_radix(part, 16).ok()?;
    }
    let msb =
        ((fields[0] & 0xFFFF_FFFF) << 32) | ((fields[1] & 0xFFFF) << 16) | (fields[2] & 0xFFFF);
    let lsb = ((fields[3] & 0xFFFF) << 48) | (fields[4] & 0xFFFF_FFFF_FFFF);
    Some((u128::from(msb) << 64) | u128::from(lsb))
}

/// The tooltip line `RelicImpl.addDefaultTooltip` adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelicLine {
    /// `botaniamisc.relicUnbound`.
    Unbound,
    /// `botaniamisc.relicSoulbound`, with the viewer's name.
    Yours,
    /// `botaniamisc.notYourSagittarius`.
    NotYours,
}

pub fn relic_line(bound: Option<u128>, viewer: Option<u128>) -> RelicLine {
    match (bound, viewer) {
        (None, _) => RelicLine::Unbound,
        (Some(owner), Some(me)) if owner == me => RelicLine::Yours,
        _ => RelicLine::NotYours,
    }
}

// ---- Band of Aura -------------------------------------------------------------------------------

/// What a Band of Aura gives the mana items each time (`dispatchManaExact(stack, player, 5, true)`).
pub const AURA_MANA: i32 = 5;

/// `BandOfAuraItem`'s `interval`: 5 × the constructor's 10 (Band of Aura) or 2 (Greater).
pub fn aura_interval(item: &str) -> Option<i32> {
    match item {
        AURA_RING => Some(50),
        AURA_RING_GREATER => Some(10),
        _ => None,
    }
}

/// `onWornTick`: the band gives on the ticks whose `tickCount` the interval divides.
pub fn aura_due(tick_count: i32, interval: i32) -> bool {
    interval > 0 && tick_count % interval == 0
}

// ---- Ring of Magnetization ----------------------------------------------------------------------

pub const MAGNET_TAG_COOLDOWN: &str = "cooldown";
/// `onTossItem`: a thrown item stops the ring for 100 ticks.
pub const MAGNET_TOSS_COOLDOWN: i32 = 100;
/// `pulled > 200` stops the loop: at most 201 items a tick.
pub const MAGNET_MAX_PULLED: usize = 201;
/// `MathHelper.setEntityMotionFromVector(item, target, 0.45F)`.
pub const MAGNET_PULL_SPEED: f64 = 0.45_f32 as f64;
/// `canPullItem`: an item whose pickup delay is this or more stays.
pub const MAGNET_MAX_PICKUP_DELAY: i64 = 40;
/// The item and block tags it leaves alone.
pub const MAGNET_BLACKLIST: &str = "botania:magnet_ring_blacklist";

/// The ring's range: 6 blocks, 16 for the greater one.
pub fn magnet_range(item: &str) -> Option<f64> {
    match item {
        MAGNET_RING => Some(6.0),
        MAGNET_RING_GREATER => Some(16.0),
        _ => None,
    }
}

/// `RingOfMagnetizationItem.onWornTick` without the items: the cooldown it keeps and whether it
/// pulls this tick. `solegnolia` is a Solegnolia in reach (`hasSolegnoliaAround`), `sneaking`
/// the wearer's `isShiftKeyDown` (the config's `invertMagnetRing` is false by default).
pub fn magnet_tick(cooldown: i32, sneaking: bool, solegnolia: bool) -> (i32, bool) {
    if solegnolia {
        return (if cooldown < 0 { 2 } else { cooldown }, false);
    }
    if cooldown <= 0 {
        (cooldown, !sneaking)
    } else {
        (cooldown - 1, false)
    }
}

/// Where the ring pulls to (the wearer's feet raised by 0.75) and the box it looks in.
pub fn magnet_box(feet: [f64; 3], range: f64) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let target = [feet[0], feet[1] + 0.75, feet[2]];
    let lo = [target[0] - range, target[1] - range, target[2] - range];
    let hi = [target[0] + range, target[1] + range, target[2] + range];
    (target, lo, hi)
}

/// `MathHelper.setEntityMotionFromVector(entity, target, modifier)`: from the entity's centre
/// (`VecHelper.fromEntityCenter`) toward `target`, at most one block long, times `modifier`.
pub fn pull_velocity(target: [f64; 3], centre: [f64; 3], modifier: f64) -> [f64; 3] {
    let mut v = [
        target[0] - centre[0],
        target[1] - centre[1],
        target[2] - centre[2],
    ];
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length > 1.0 {
        // `Vec3.normalize`: zero below 1.0E-4, which a length over 1 never is.
        v = [v[0] / length, v[1] / length, v[2] / length];
    }
    [v[0] * modifier, v[1] * modifier, v[2] * modifier]
}

/// `canPullItem` from what the game knows of the item.
pub struct PullCheck {
    pub alive: bool,
    pub pickup_delay: i64,
    pub mana_item: bool,
    pub relic: bool,
    pub blacklisted_item: bool,
    pub blacklisted_block_here: bool,
    pub blacklisted_block_below: bool,
}

pub fn can_pull(c: &PullCheck) -> bool {
    c.alive
        && c.pickup_delay < MAGNET_MAX_PICKUP_DELAY
        && !c.mana_item
        && !c.relic
        && !c.blacklisted_item
        && !c.blacklisted_block_here
        && !c.blacklisted_block_below
}

// ---- static effects (`EntityHelper`) ------------------------------------------------------------

/// An effect a living entity has, as `getEffect` gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect {
    pub amplifier: i32,
    pub infinite: bool,
}

/// `addStaticEffect`: an infinite effect is given unless one as strong or stronger and infinite is
/// already there.
pub fn needs_static_effect(current: Option<Effect>, amplifier: i32) -> bool {
    match current {
        None => true,
        Some(e) => e.amplifier < amplifier || (e.amplifier == amplifier && !e.infinite),
    }
}

/// `removeStaticEffect` and `convertStaticEffectToFinite` act only on the infinite effect of the
/// level they gave.
pub fn is_static_effect(current: Option<Effect>, amplifier: i32) -> bool {
    current.is_some_and(|e| e.amplifier == amplifier && e.infinite)
}

// ---- Ring of Chordata ----------------------------------------------------------------------------

/// Mana a tick while in water (`MANA_COST`).
pub const WATER_RING_MANA: i32 = 3;
pub const CONDUIT_POWER: &str = "minecraft:conduit_power";
pub const DOLPHINS_GRACE: &str = "minecraft:dolphins_grace";
/// `OUT_OF_MANA_DURATION`: the effects' last ticks once the mana runs out.
pub const WATER_RING_OUT_OF_MANA: i32 = 60;

// ---- Ring of Correction --------------------------------------------------------------------------

/// `SortableTool.getSortingPriority` of Botania's tools (`ManasteelPickaxeItem`, `…AxeItem`,
/// `…ShovelItem`, `…HoeItem`, `…ShearsItem` and the elementium, terrasteel and vitreous ones that
/// extend them): `ToolCommons.getToolPriority` is tier × 100 + modifier × 10 + Efficiency
/// (manasteel 10, elementium 11, terrasteel 20; the Terra Shatterer's modifier is its rank);
/// shears are 1000 + Efficiency (elementium + 100); the Vitreous Pickaxe is `Integer.MAX_VALUE`
/// on glass and 0 elsewhere. None for anything that isn't a `SortableTool`.
pub fn sorting_priority(item: &str, efficiency: i32, pick_rank: i32, glass: bool) -> Option<i32> {
    let tier = match item {
        "botania:manasteel_pick"
        | "botania:manasteel_axe"
        | "botania:manasteel_shovel"
        | "botania:manasteel_hoe" => 10,
        "botania:elementium_pickaxe"
        | "botania:elementium_axe"
        | "botania:elementium_shovel"
        | "botania:elementium_hoe" => 11,
        "botania:terra_pick" | "botania:terra_axe" => 20,
        "botania:manasteel_shears" => return Some(1000 + efficiency),
        "botania:elementium_shears" => return Some(1100 + efficiency),
        "botania:glass_pickaxe" => return Some(if glass { i32::MAX } else { 0 }),
        _ => return None,
    };
    let modifier = if item == "botania:terra_pick" {
        pick_rank
    } else {
        0
    };
    Some(tier * 100 + modifier * 10 + efficiency)
}

/// A carried tool the ring may pick: its slot, its priority and whether it is fast on the block
/// (`getDestroySpeed(state) > 1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolChoice {
    pub slot: usize,
    pub priority: i32,
    pub fast: bool,
}

/// `RingOfCorrectionItem.onWornTick`'s search: the held tool counts with its priority when it is
/// fast on the block, else -1; the first carried tool of a higher priority wins each time. The
/// slot to swap into the main hand, if any.
pub fn best_tool(held: (i32, bool), carried: &[ToolChoice]) -> Option<usize> {
    let mut best = if held.1 { held.0 } else { -1 };
    let mut slot = None;
    for c in carried {
        if c.fast && c.priority > best {
            best = c.priority;
            slot = Some(c.slot);
        }
    }
    slot
}

/// `ToolCommons.raytraceFromEntity(player, 4.5F, false)`.
pub const SWAP_RING_REACH: f64 = 4.5;

// ---- Ring of Dexterous Motion --------------------------------------------------------------------

pub const DODGE_TAG_COOLDOWN: &str = "dodgeCooldown";
/// `MAX_CD`: ticks between dodges.
pub const DODGE_COOLDOWN: i32 = 20;
/// `DodgePacket`: the food a dodge costs.
pub const DODGE_EXHAUSTION: f32 = 0.3;
/// `ClientLogic.onKeyDown`'s `threshold`: two presses closer than this many ticks dodge.
pub const DODGE_THRESHOLD: i32 = 5;

/// The way a dodge goes, by the key pressed twice (`Direction` as Botania names them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dodge {
    /// Left.
    West,
    /// Right.
    East,
    /// Forward.
    North,
    /// Back.
    South,
}

/// `ClientLogic.onKeyDown`'s state: when each movement key went down last
/// (`ClientTickHandler.ticksInGame`) and whether it was down the last time it looked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DodgeKeys {
    old: [bool; 4],
    down: [i32; 4],
}

impl DodgeKeys {
    /// One look at the keys (left, right, forward, back) at `now`: the first key, in that order,
    /// that has just gone down records the time, and dodges when it last went down less than
    /// `DODGE_THRESHOLD` ticks before. Botania looks on each key event with the ring worn and
    /// cooled down; the port looks each client tick under the same conditions.
    pub fn press(&mut self, now: i32, keys: [bool; 4]) -> Option<Dodge> {
        const DIRS: [Dodge; 4] = [Dodge::West, Dodge::East, Dodge::North, Dodge::South];
        let mut out = None;
        if let Some(i) = (0..4).find(|&i| keys[i] && !self.old[i]) {
            let old = self.down[i];
            self.down[i] = now;
            if now - old < DODGE_THRESHOLD {
                out = Some(DIRS[i]);
            }
        }
        self.old = keys;
        out
    }
}

/// `RingOfDexterousMotionItem.dodge`: the velocity a dodge sets, 1.25 blocks a tick to the side
/// (or ahead, or back) of where the player looks, level.
pub fn dodge_velocity(yaw: f32, dir: Dodge) -> [f64; 3] {
    let r = yaw * 0.017453292_f32;
    let (x, z) = match dir {
        Dodge::North | Dodge::South => (mth::cos(-r), mth::sin(r)),
        Dodge::West | Dodge::East => (
            mth::sin(-r - std::f32::consts::PI),
            mth::cos(-r - std::f32::consts::PI),
        ),
    };
    let s = match dir {
        Dodge::West | Dodge::North => 1.0,
        Dodge::East | Dodge::South => -1.0,
    };
    // `lookVec.cross(new Vec3(0, s, 0)).scale(1.25)`.
    let (x, z) = (f64::from(x), f64::from(z));
    [-z * s * 1.25, 0.0, x * s * 1.25]
}

/// `ClientLogic.renderHUD`: the white part of the bar under the crosshair, 40 wide at most.
pub fn dodge_bar_width(cooldown: i32, partial: f32) -> i32 {
    (((cooldown as f32 - partial) * 2.0) as i32).min(40)
}

// ---- Ring of the Mantle ---------------------------------------------------------------------------

/// What the ring needs and pays (`MANA_COST`).
pub const MINING_RING_MANA: i32 = 5;
/// Haste II (`HASTE_AMPLIFIER`).
pub const MINING_RING_HASTE: i32 = 1;
pub const HASTE: &str = "minecraft:haste";

/// `player.attackAnim == 0.25F`: the swing's second tick of four, which Haste II makes it (six
/// less two ticks), so the ring pays once a swing; a swing of another length never pays.
pub fn mining_ring_pays(attack_anim: f32) -> bool {
    attack_anim == 0.25
}

// ---- Great Fairy Ring and the pixies (`PixieHandler`) --------------------------------------------

/// The ring's `PIXIE_SPAWN_CHANCE` modifier.
pub const PIXIE_RING_CHANCE: f64 = 0.25;

/// `PIXIE_SPAWN_CHANCE` of a player: the ring's 0.25 and each worn elementium piece's (helmet
/// 0.11, chestplate 0.17, leggings 0.15, boots 0.09), capped at the attribute's 1.
pub fn pixie_chance(ring: bool, elementium: [bool; 4]) -> f64 {
    const PIECES: [f64; 4] = [0.11, 0.17, 0.15, 0.09];
    let mut chance = if ring { PIXIE_RING_CHANCE } else { 0.0 };
    for (worn, c) in elementium.iter().zip(PIECES) {
        if *worn {
            chance += c;
        }
    }
    chance.clamp(0.0, 1.0)
}

/// `PixieHandler.onDamageTaken`: a pixie's damage, 4 and 2 more with an Elementium Sword in hand.
pub fn pixie_damage(sword: bool) -> f32 {
    if sword { 6.0 } else { 4.0 }
}

// ---- Ring of Far Reach, Ring of Odin, Tectonic Girdle ---------------------------------------------

/// The Ring of Far Reach's reach modifier (`getReachDistanceAttribute`, + 3.5).
pub const REACH_BONUS: f64 = 3.5;
pub const REACH_ATTRIBUTE: &str = "minecraft:block_interaction_range";
/// The Ring of Odin's ten hearts.
pub const ODIN_HEALTH: f64 = 20.0;
/// The Tectonic Girdle's knockback resistance.
pub const GIRDLE_KNOCKBACK_RESISTANCE: f64 = 1.0;

/// `#minecraft:is_fire` in 26.3.
pub fn is_fire(damage_type: &str) -> bool {
    matches!(
        damage_type,
        "minecraft:in_fire"
            | "minecraft:campfire"
            | "minecraft:on_fire"
            | "minecraft:lava"
            | "minecraft:hot_floor"
            | "minecraft:sulfur_cube_hot"
            | "minecraft:unattributed_fireball"
            | "minecraft:fireball"
    )
}

/// `#botania:ring_of_odin_immune`: drowning, falling, suffocating, starving, flying into a wall,
/// and fire.
pub fn odin_immune(damage_type: &str) -> bool {
    matches!(
        damage_type,
        "minecraft:drown"
            | "minecraft:fall"
            | "minecraft:in_wall"
            | "minecraft:starve"
            | "minecraft:fly_into_wall"
    ) || is_fire(damage_type)
}

// ---- Stone of Temperance -------------------------------------------------------------------------

/// The Stone of Temperance's NBT key (`TAG_ACTIVE`).
pub const TEMPERANCE_TAG_ACTIVE: &str = "active";

// ---- Ring of Loki ---------------------------------------------------------------------------------

/// `MAX_NUM_CURSORS`.
pub const LOKI_MAX_CURSORS: usize = 1023;
pub const LOKI_TAG_CURSOR_LIST: &str = "cursorList";
pub const LOKI_TAG_CURSOR_PREFIX: &str = "cursor";
pub const LOKI_TAG_CURSOR_COUNT: &str = "cursorCount";
pub const LOKI_TAG_X_OFFSET: &str = "xOffset";
pub const LOKI_TAG_Y_OFFSET: &str = "yOffset";
pub const LOKI_TAG_Z_OFFSET: &str = "zOffset";
pub const LOKI_TAG_X_ORIGIN: &str = "xOrigin";
pub const LOKI_TAG_Y_ORIGIN: &str = "yOrigin";
pub const LOKI_TAG_Z_ORIGIN: &str = "zOrigin";
/// `Bound.UNBOUND_POS`: no selection under way (`y` is `Integer.MIN_VALUE`).
pub const LOKI_UNBOUND: [i32; 3] = [0, i32::MIN, 0];

/// What a sneaking empty-handed click on a block does to the ring's selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LokiSelect {
    /// No selection was under way: the block is the new origin, the cursors are cleared.
    Started,
    /// The origin itself: the selection ends.
    Finished,
    /// Another block: its offset from the origin was added.
    Added,
    /// Another block whose offset was already there: taken out.
    Removed,
    /// `botaniamisc.lokiRingLimitReached`: the ring holds 1023 already.
    Full,
}

/// `RingOfLokiItem.onPlayerInteract` with an empty main hand: the new origin and cursors.
pub fn loki_select(
    origin: [i32; 3],
    cursors: &mut Vec<[i32; 3]>,
    hit: [i32; 3],
) -> ([i32; 3], LokiSelect) {
    if origin[1] == i32::MIN {
        cursors.clear();
        return (hit, LokiSelect::Started);
    }
    if origin == hit {
        return (LOKI_UNBOUND, LokiSelect::Finished);
    }
    let rel = [hit[0] - origin[0], hit[1] - origin[1], hit[2] - origin[2]];
    if let Some(i) = cursors.iter().position(|c| *c == rel) {
        cursors.remove(i);
        return (origin, LokiSelect::Removed);
    }
    if cursors.len() < LOKI_MAX_CURSORS {
        cursors.push(rel);
        (origin, LokiSelect::Added)
    } else {
        (origin, LokiSelect::Full)
    }
}

/// The mana each copy costs: as many as the cursors past 10, else
/// `min(n, (int) Math.pow(Math.E, n * 0.25))`.
pub fn loki_cost(cursors: usize) -> i32 {
    let n = cursors.min(i32::MAX as usize) as i32;
    if n > 10 {
        n
    } else {
        n.min((std::f64::consts::E.powf(f64::from(n) * 0.25)) as i32)
    }
}

/// `getUseOnContext`: the point clicked moved to `pos`, keeping where in its block it is.
pub fn loki_hit_point(pos: [i32; 3], look_hit: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| f64::from(pos[i]) + mth::frac(look_hit[i]))
}

/// `pos.offset(cursor)` for each cursor.
pub fn loki_targets(at: [i32; 3], cursors: &[[i32; 3]]) -> Vec<[i32; 3]> {
    cursors
        .iter()
        .map(|c| [at[0] + c[0], at[1] + c[1], at[2] + c[2]])
        .collect()
}

/// `getWireframesToDraw`: during a selection the cursors around the origin, otherwise around the
/// block looked at (none without one).
pub fn loki_wireframes(
    origin: [i32; 3],
    cursors: &[[i32; 3]],
    looked_at: Option<[i32; 3]>,
) -> Vec<[i32; 3]> {
    let Some(looked_at) = looked_at else {
        return Vec::new();
    };
    let base = if origin[1] != i32::MIN {
        origin
    } else {
        looked_at
    };
    loki_targets(base, cursors)
}

// ---- belts: Sojourner's, Globetrotter's and Planestrider's Sashes ----------------------------------

/// `SojournersSashItem`'s step boost (`STEP_BOOST`) and its cost: 1 mana every 10 ticks of walking.
pub const SASH_STEP_BOOST: f64 = 0.65;
pub const SASH_COST: i32 = 1;
pub const SASH_COST_INTERVAL: i32 = 10;

/// A sash's numbers: its push a tick, its jump and the fall it shrugs off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sash {
    pub speed: f32,
    pub jump: f32,
    pub fall_buffer: f32,
}

pub fn sash(item: &str) -> Option<Sash> {
    let (speed, jump, fall_buffer) = match item {
        TRAVEL_BELT => (0.035, 0.2, 2.0),
        SUPER_TRAVEL_BELT => (0.085, 0.3, 4.0),
        // The Planestrider's Sash keeps its speed in the stack (`TAG_SPEED`).
        SPEED_UP_BELT => (0.0, 0.2, 2.0),
        _ => return None,
    };
    Some(Sash {
        speed,
        jump,
        fall_buffer,
    })
}

/// `player.moveRelative(speed, new Vec3(0, 0, 1))`: what the sash adds to the velocity, `speed`
/// toward where the player faces (`Entity.getInputVector` with `Mth.sin`/`Mth.cos` of the yaw).
pub fn sash_push(yaw: f32, speed: f32) -> [f64; 3] {
    let r = yaw * 0.017453292_f32;
    let (sin, cos) = (mth::sin(r), mth::cos(r));
    [f64::from(-speed * sin), 0.0, f64::from(speed * cos)]
}

/// `jumpFromGround`'s `getJumpPower` in the air (a block factor of 1): the jump strength and 0.1
/// for each level of Jump Boost (`amplifier` 0 for level I).
pub fn jump_power(strength: f64, jump_boost: Option<i32>) -> f32 {
    strength as f32 + jump_boost.map_or(0.0, |a| 0.1 * (a as f32 + 1.0))
}

/// `PlanestridersSashItem.onMovedTick`: the speed grows by 0.00035 a tick of walking, up to 0.25.
pub fn planestrider_speed(speed: f32) -> f32 {
    (speed + 0.00035).min(0.25)
}

/// `SojournersSashItem.onPlayerFall`: the fall shortened by the sash's buffer (times the amulet's
/// jumps when the fall followed an amulet jump).
pub fn sash_fall(distance: f32, buffer: f32, amulet_jumps: Option<i32>) -> f32 {
    let buffer = match amulet_jumps {
        Some(jumps) => buffer * jumps as f32,
        None => buffer,
    };
    (distance - buffer).max(0.0)
}

// ---- Cirrus and Nimbus Amulets --------------------------------------------------------------------

/// `getMaxAllowedJumps`: 2, the Nimbus Amulet 3.
pub fn amulet_jumps(item: &str) -> Option<i32> {
    match item {
        CLOUD_PENDANT => Some(2),
        SUPER_CLOUD_PENDANT => Some(3),
        _ => None,
    }
}

/// `CirrusAmuletItem.onWornTick` on the client: how many jumps the player made since leaving the
/// ground and whether the jump key was down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AmuletJumps {
    pub times: i32,
    pub jump_down: bool,
}

impl AmuletJumps {
    /// One worn tick: whether the player jumps again in the air.
    pub fn tick(&mut self, on_ground: bool, jumping: bool, max: i32) -> bool {
        if on_ground {
            self.times = 0;
            return false;
        }
        if self.times == 0 {
            // However the ground was left, that is the first jump.
            self.times = 1;
            self.jump_down = true;
        }
        let mut jump = false;
        if jumping {
            if !self.jump_down && self.times < max {
                jump = true;
                self.times += 1;
            }
            self.jump_down = true;
        } else {
            self.jump_down = false;
        }
        jump
    }
}

/// `JumpPacket`: the food an air jump costs.
pub const AMULET_EXHAUSTION: f32 = 0.3;

// ---- cloaks (`CloakOfVirtueItem` and the two that extend it) --------------------------------------

pub const CLOAK_TAG_COOLDOWN: &str = "cooldown";
/// `getCooldownTime`: 200 ticks after a cloak acts.
pub const CLOAK_COOLDOWN: i32 = 200;
/// The Cloak of Sin's reach (`range`).
pub const SIN_RANGE: f64 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cloak {
    Virtue,
    Sin,
    Balance,
}

impl Cloak {
    pub fn of_item(item: &str) -> Option<Cloak> {
        match item {
            HOLY_CLOAK => Some(Cloak::Virtue),
            UNHOLY_CLOAK => Some(Cloak::Sin),
            BALANCE_CLOAK => Some(Cloak::Balance),
            _ => None,
        }
    }

    /// `effectOnDamage`'s test: Virtue and Balance act on anything a witch doesn't resist, Sin on
    /// what armour stops. None of them on what bypasses invulnerability (`onPlayerDamage`).
    pub fn acts_on(self, damage_type: &str) -> bool {
        if matches!(
            damage_type,
            "minecraft:out_of_world" | "minecraft:generic_kill"
        ) {
            return false;
        }
        match self {
            Cloak::Virtue | Cloak::Balance => !witch_resistant_to(damage_type),
            Cloak::Sin => !bypasses_armor(damage_type),
        }
    }

    /// What is left of the hit: nothing for Virtue, all of it for Sin (which hurts the enemies
    /// around with it), half for Balance (which hurts the attacker with that half), and never more
    /// than leaves the wearer one point of health.
    pub fn amount(self, amount: f32, health: f32) -> f32 {
        match self {
            Cloak::Virtue => 0.0,
            Cloak::Sin => amount,
            Cloak::Balance => {
                let half = amount / 2.0;
                if half > health { health - 1.0 } else { half }
            }
        }
    }
}

/// `#minecraft:witch_resistant_to` in 26.3.
pub fn witch_resistant_to(damage_type: &str) -> bool {
    matches!(
        damage_type,
        "minecraft:magic"
            | "minecraft:indirect_magic"
            | "minecraft:sonic_boom"
            | "minecraft:thorns"
    )
}

/// `#minecraft:bypasses_armor` in 26.3, with Botania's two additions.
pub fn bypasses_armor(damage_type: &str) -> bool {
    matches!(
        damage_type,
        "minecraft:on_fire"
            | "minecraft:in_wall"
            | "minecraft:cramming"
            | "minecraft:drown"
            | "minecraft:fly_into_wall"
            | "minecraft:generic"
            | "minecraft:wither"
            | "minecraft:dragon_breath"
            | "minecraft:starve"
            | "minecraft:fall"
            | "minecraft:ender_pearl"
            | "minecraft:freeze"
            | "minecraft:stalagmite"
            | "minecraft:magic"
            | "minecraft:indirect_magic"
            | "minecraft:out_of_world"
            | "minecraft:generic_kill"
            | "minecraft:sonic_boom"
            | "minecraft:outside_border"
            | "botania:player_attack_armor_piercing"
            | "botania:relic_damage"
    )
}

// ---- the Cloak of Invisibility, the Benevolent Goddess' Charm -------------------------------------

/// The Cloak of Invisibility's mana a tick.
pub const INVISIBILITY_MANA: i32 = 2;
pub const INVISIBILITY: &str = "minecraft:invisibility";
/// The Benevolent Goddess' Charm's mana for each explosion it stops (`COST`).
pub const GODDESS_CHARM_MANA: i32 = 1000;
/// `shouldProtectExplosion`: the players within 8 blocks of the blast (`inflate(8)`).
pub const GODDESS_CHARM_RANGE: f64 = 8.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-3)
    }

    #[test]
    fn items_and_relics() {
        assert!(is_bauble(LOKI_RING) && is_relic(LOKI_RING));
        assert!(is_bauble("botania:cosmetic_cat_ears") && !is_relic("botania:cosmetic_cat_ears"));
        assert!(!is_bauble(TEMPERANCE_STONE) && unstackable(TEMPERANCE_STONE));
        assert!(is_relic("botania:dice") && !is_bauble("botania:dice"));
        // The bands of mana are the mana module's.
        assert!(is_bauble("botania:mana_ring") && !unstackable("botania:mana_ring"));
        assert!(!unstackable("botania:mana_tablet") && !unstackable("botania:petal_white"));
        assert_eq!(rarity(THOR_RING), Some("epic"));
        assert_eq!(rarity(WATER_RING), Some("rare"));
        assert_eq!(rarity(AURA_RING), None);
        assert!(fire_resistant(ODIN_RING) && !fire_resistant(MAGNET_RING));
    }

    #[test]
    fn relic_binding() {
        let (me, you) = (7u128, 9u128);
        assert_eq!(relic_tick(None, me, 3), RelicTick::Bind);
        assert_eq!(relic_tick(Some(me), me, 10), RelicTick::Nothing);
        assert_eq!(relic_tick(Some(you), me, 20), RelicTick::Hurt);
        assert_eq!(relic_tick(Some(you), me, 21), RelicTick::Nothing);
        assert_eq!(relic_line(None, Some(me)), RelicLine::Unbound);
        assert_eq!(relic_line(Some(me), Some(me)), RelicLine::Yours);
        assert_eq!(relic_line(Some(you), Some(me)), RelicLine::NotYours);
        assert_eq!(relic_line(Some(you), None), RelicLine::NotYours);
    }

    #[test]
    fn uuids_as_java_writes_them() {
        let u = 0x0123_4567_89ab_cdef_0123_4567_89ab_cdef_u128;
        assert_eq!(uuid_to_string(u), "01234567-89ab-cdef-0123-456789abcdef");
        assert_eq!(
            uuid_from_string("01234567-89ab-cdef-0123-456789abcdef"),
            Some(u)
        );
        assert_eq!(
            uuid_from_string("01234567-89AB-CDEF-0123-456789ABCDEF"),
            Some(u)
        );
        // `UUID.fromString` takes short fields too: "1-2-3-4-5".
        assert_eq!(
            uuid_from_string("1-2-3-4-5"),
            Some(0x0000_0001_0002_0003_0004_0000_0000_0005)
        );
        assert_eq!(uuid_from_string("not a uuid"), None);
        assert_eq!(uuid_from_string("1-2-3-4"), None);
        assert_eq!(uuid_from_string("g-2-3-4-5"), None);
        assert_eq!(uuid_from_string(""), None);
    }

    #[test]
    fn aura_bands() {
        assert_eq!(aura_interval(AURA_RING), Some(50));
        assert_eq!(aura_interval(AURA_RING_GREATER), Some(10));
        assert!(aura_due(100, 50) && !aura_due(101, 50) && aura_due(0, 10));
        // In 200 ticks the band gives 4 times and the greater one 20, 5 mana each.
        let gives = |interval| (1..=200).filter(|&t| aura_due(t, interval)).count();
        assert_eq!((gives(50), gives(10)), (4, 20));
    }

    #[test]
    fn magnet_cooldown_and_pull() {
        assert_eq!(magnet_range(MAGNET_RING), Some(6.0));
        assert_eq!(magnet_range(MAGNET_RING_GREATER), Some(16.0));
        assert_eq!(magnet_tick(0, false, false), (0, true));
        assert_eq!(magnet_tick(0, true, false), (0, false));
        assert_eq!(magnet_tick(100, false, false), (99, false));
        assert_eq!(magnet_tick(1, false, false), (0, false));
        assert_eq!(magnet_tick(-3, false, true), (2, false));
        assert_eq!(magnet_tick(5, false, true), (5, false));
        // A toss stops it for 100 ticks: it pulls again on the 101st.
        let mut c = MAGNET_TOSS_COOLDOWN;
        let mut first = None;
        for t in 1..=200 {
            let (next, pull) = magnet_tick(c, false, false);
            c = next;
            if pull {
                first = Some(t);
                break;
            }
        }
        assert_eq!(first, Some(101));
        let (target, lo, hi) = magnet_box([10.0, 64.0, -3.0], 6.0);
        assert_eq!(target, [10.0, 64.75, -3.0]);
        assert_eq!((lo, hi), ([4.0, 58.75, -9.0], [16.0, 70.75, 3.0]));
        // Three blocks away along x: a unit vector times 0.45F.
        let v = pull_velocity([0.0, 0.0, 0.0], [3.0, 0.0, 0.0], MAGNET_PULL_SPEED);
        assert_eq!(v, [-0.44999998807907104, 0.0, 0.0]);
        // Within a block it isn't normalised.
        let v = pull_velocity([0.5, 0.0, 0.0], [0.0, 0.0, 0.0], MAGNET_PULL_SPEED);
        assert_eq!(v, [0.5 * 0.44999998807907104, 0.0, 0.0]);
        let ok = PullCheck {
            alive: true,
            pickup_delay: 39,
            mana_item: false,
            relic: false,
            blacklisted_item: false,
            blacklisted_block_here: false,
            blacklisted_block_below: false,
        };
        assert!(can_pull(&ok));
        assert!(!can_pull(&PullCheck {
            pickup_delay: 40,
            ..ok
        }));
        assert!(!can_pull(&PullCheck {
            mana_item: true,
            ..ok
        }));
        assert!(!can_pull(&PullCheck { relic: true, ..ok }));
        assert!(!can_pull(&PullCheck {
            blacklisted_block_below: true,
            ..ok
        }));
    }

    #[test]
    fn static_effects() {
        let inf = |amplifier| {
            Some(Effect {
                amplifier,
                infinite: true,
            })
        };
        let timed = |amplifier| {
            Some(Effect {
                amplifier,
                infinite: false,
            })
        };
        assert!(needs_static_effect(None, 1));
        assert!(needs_static_effect(inf(0), 1));
        assert!(needs_static_effect(timed(1), 1));
        assert!(!needs_static_effect(inf(1), 1));
        // A stronger potion of the same kind is left alone.
        assert!(!needs_static_effect(timed(2), 1));
        assert!(is_static_effect(inf(1), 1));
        assert!(!is_static_effect(timed(1), 1));
        assert!(!is_static_effect(inf(2), 1));
        assert!(!is_static_effect(None, 1));
    }

    #[test]
    fn correction_priorities() {
        // The interface's own examples: a Manasteel Pickaxe with Efficiency IV is 1004, a Terra
        // Shatterer of rank B with Efficiency V 2025.
        assert_eq!(
            sorting_priority("botania:manasteel_pick", 4, 0, false),
            Some(1004)
        );
        assert_eq!(
            sorting_priority("botania:terra_pick", 5, 2, false),
            Some(2025)
        );
        assert_eq!(
            sorting_priority("botania:elementium_shovel", 0, 0, false),
            Some(1100)
        );
        assert_eq!(
            sorting_priority("botania:terra_axe", 1, 0, false),
            Some(2001)
        );
        assert_eq!(
            sorting_priority("botania:manasteel_shears", 2, 0, false),
            Some(1002)
        );
        assert_eq!(
            sorting_priority("botania:elementium_shears", 2, 0, false),
            Some(1102)
        );
        assert_eq!(
            sorting_priority("botania:glass_pickaxe", 5, 0, true),
            Some(i32::MAX)
        );
        assert_eq!(
            sorting_priority("botania:glass_pickaxe", 5, 0, false),
            Some(0)
        );
        assert_eq!(
            sorting_priority("minecraft:diamond_pickaxe", 5, 0, false),
            None
        );
        assert_eq!(
            sorting_priority("botania:manasteel_sword", 0, 0, false),
            None
        );
        let t = |slot, priority, fast| ToolChoice {
            slot,
            priority,
            fast,
        };
        // A held pickaxe slow on the block loses to a fast shovel of a lower priority.
        assert_eq!(best_tool((2025, false), &[t(5, 1000, true)]), Some(5));
        // A fast held tool keeps its place against lower ones and slow higher ones.
        assert_eq!(
            best_tool((1100, true), &[t(3, 1000, true), t(4, 2000, false)]),
            None
        );
        // The first of two equal ones wins; a later higher one beats it.
        assert_eq!(
            best_tool((0, false), &[t(1, 1000, true), t(2, 1000, true)]),
            Some(1)
        );
        assert_eq!(
            best_tool((0, false), &[t(1, 1000, true), t(7, 1100, true)]),
            Some(7)
        );
        assert_eq!(best_tool((0, false), &[]), None);
    }

    #[test]
    fn dodge_double_tap() {
        let mut k = DodgeKeys::default();
        let left = [true, false, false, false];
        let none = [false; 4];
        // Botania starts its press times at 0: a first press at tick 100 doesn't dodge.
        assert_eq!(k.press(100, left), None);
        assert_eq!(k.press(101, left), None);
        assert_eq!(k.press(102, none), None);
        // Down again 4 ticks after the first press: a dodge to the left.
        assert_eq!(k.press(104, left), Some(Dodge::West));
        assert_eq!(k.press(105, none), None);
        // Five ticks after the last press is too slow.
        assert_eq!(k.press(109, left), None);
        let mut k = DodgeKeys::default();
        let back = [false, false, false, true];
        assert_eq!(k.press(50, back), None);
        assert_eq!(k.press(51, none), None);
        assert_eq!(k.press(52, back), Some(Dodge::South));
        // Held, a key doesn't count again.
        assert_eq!(k.press(53, back), None);
        // Left and forward down together: left is looked at first.
        let mut k = DodgeKeys::default();
        assert_eq!(k.press(10, [true, false, true, false]), None);
        assert_eq!(k.press(11, none), None);
        assert_eq!(k.press(12, [false, false, true, false]), None);
        assert_eq!(k.press(13, none), None);
        assert_eq!(k.press(14, [false, false, true, false]), Some(Dodge::North));
    }

    #[test]
    fn dodge_directions() {
        // Facing south (yaw 0, +z): left is east (+x), right west, forward south, back north.
        assert!(near(dodge_velocity(0.0, Dodge::West), [1.25, 0.0, 0.0]));
        assert!(near(dodge_velocity(0.0, Dodge::East), [-1.25, 0.0, 0.0]));
        assert!(near(dodge_velocity(0.0, Dodge::North), [0.0, 0.0, 1.25]));
        assert!(near(dodge_velocity(0.0, Dodge::South), [0.0, 0.0, -1.25]));
        // Facing west (yaw 90, -x): left is south, forward west.
        assert!(near(dodge_velocity(90.0, Dodge::West), [0.0, 0.0, 1.25]));
        assert!(near(dodge_velocity(90.0, Dodge::North), [-1.25, 0.0, 0.0]));
        assert_eq!(dodge_bar_width(20, 0.0), 40);
        assert_eq!(dodge_bar_width(10, 0.5), 19);
        assert_eq!(dodge_bar_width(0, 0.5), -1);
    }

    #[test]
    fn mantle_pays_once_a_haste_swing() {
        // Haste II: a swing of 4 ticks, attackAnim 0, 0.25, 0.5, 0.75.
        let paid = (0..4).filter(|t| mining_ring_pays(*t as f32 / 4.0)).count();
        assert_eq!(paid, 1);
        // A plain six-tick swing never reads 0.25.
        let paid = (0..6).filter(|t| mining_ring_pays(*t as f32 / 6.0)).count();
        assert_eq!(paid, 0);
    }

    #[test]
    fn pixies() {
        assert_eq!(pixie_chance(true, [false; 4]), 0.25);
        assert!((pixie_chance(true, [true; 4]) - 0.77).abs() < 1e-9);
        assert!((pixie_chance(false, [true, false, false, true]) - 0.20).abs() < 1e-9);
        assert_eq!(pixie_damage(false), 4.0);
        assert_eq!(pixie_damage(true), 6.0);
    }

    #[test]
    fn odin_and_fire() {
        for t in [
            "minecraft:drown",
            "minecraft:fall",
            "minecraft:lava",
            "minecraft:on_fire",
        ] {
            assert!(odin_immune(t), "{t}");
        }
        for t in [
            "minecraft:mob_attack",
            "minecraft:magic",
            "minecraft:freeze",
        ] {
            assert!(!odin_immune(t), "{t}");
        }
        assert!(is_fire("minecraft:campfire") && !is_fire("minecraft:drown"));
    }

    #[test]
    fn loki_selection() {
        let mut cursors = vec![[5, 5, 5]];
        let origin = LOKI_UNBOUND;
        let (origin, what) = loki_select(origin, &mut cursors, [10, 64, 10]);
        assert_eq!((origin, what), ([10, 64, 10], LokiSelect::Started));
        assert!(cursors.is_empty());
        let (origin, what) = loki_select(origin, &mut cursors, [11, 64, 10]);
        assert_eq!(what, LokiSelect::Added);
        let (origin, what) = loki_select(origin, &mut cursors, [10, 65, 12]);
        assert_eq!(what, LokiSelect::Added);
        assert_eq!(cursors, vec![[1, 0, 0], [0, 1, 2]]);
        let (origin, what) = loki_select(origin, &mut cursors, [11, 64, 10]);
        assert_eq!(what, LokiSelect::Removed);
        assert_eq!(cursors, vec![[0, 1, 2]]);
        let (origin, what) = loki_select(origin, &mut cursors, [10, 64, 10]);
        assert_eq!((origin, what), (LOKI_UNBOUND, LokiSelect::Finished));
        assert_eq!(cursors, vec![[0, 1, 2]]);
        let mut full = vec![[0, 0, 1]; LOKI_MAX_CURSORS];
        let (_, what) = loki_select([0, 0, 0], &mut full, [3, 3, 3]);
        assert_eq!(what, LokiSelect::Full);
        assert_eq!(full.len(), LOKI_MAX_CURSORS);
    }

    #[test]
    fn loki_costs_and_points() {
        // min(n, (int) e^(n/4)) up to 10 cursors, then n.
        let costs: Vec<i32> = (0..=12).map(loki_cost).collect();
        assert_eq!(costs, vec![0, 1, 1, 2, 2, 3, 4, 5, 7, 9, 10, 11, 12]);
        assert_eq!(loki_cost(255), 255);
        let p = loki_hit_point([3, 70, -5], [10.25, 64.5, -1.75]);
        assert_eq!(p, [3.25, 70.5, -4.75]);
        assert_eq!(
            loki_targets([1, 2, 3], &[[0, 1, 0], [-1, 0, 2]]),
            vec![[1, 3, 3], [0, 2, 5]]
        );
        assert_eq!(
            loki_wireframes(LOKI_UNBOUND, &[[1, 0, 0]], Some([4, 4, 4])),
            vec![[5, 4, 4]]
        );
        assert_eq!(
            loki_wireframes([0, 0, 0], &[[1, 0, 0]], Some([4, 4, 4])),
            vec![[1, 0, 0]]
        );
        assert!(loki_wireframes([0, 0, 0], &[[1, 0, 0]], None).is_empty());
    }

    #[test]
    fn sashes() {
        assert_eq!(sash(TRAVEL_BELT).map(|s| s.fall_buffer), Some(2.0));
        assert_eq!(
            sash(SUPER_TRAVEL_BELT).map(|s| (s.speed, s.jump)),
            Some((0.085, 0.3))
        );
        assert_eq!(sash(KNOCKBACK_BELT), None);
        assert_eq!(sash_fall(5.0, 2.0, None), 3.0);
        assert_eq!(sash_fall(5.0, 2.0, Some(2)), 1.0);
        assert_eq!(sash_fall(1.0, 4.0, None), 0.0);
        // The Planestrider's Sash reaches 0.25 after 715 ticks of walking (0.00035 each).
        let mut s = 0.0f32;
        let mut ticks = 0;
        while s < 0.25 {
            s = planestrider_speed(s);
            ticks += 1;
        }
        assert!((714..=716).contains(&ticks), "{ticks}");
    }

    #[test]
    fn sash_pushes_and_air_jumps() {
        // Facing south (yaw 0) the push is +z, facing west (yaw 90) -x.
        assert!(near(sash_push(0.0, 0.035), [0.0, 0.0, 0.035]));
        assert!(near(sash_push(90.0, 0.085), [-0.085, 0.0, 0.0]));
        assert!(near(sash_push(180.0, 0.1), [0.0, 0.0, -0.1]));
        // 0.42 for a plain jump, 0.62 with Jump Boost II.
        assert!((jump_power(0.42, None) - 0.42).abs() < 1e-6);
        assert!((jump_power(0.42, Some(1)) - 0.62).abs() < 1e-6);
    }

    #[test]
    fn amulet_air_jumps() {
        let mut j = AmuletJumps::default();
        // A jump off the ground counts as the first; the key must come up and down again.
        assert!(!j.tick(false, true, 2));
        assert!(!j.tick(false, true, 2));
        assert!(!j.tick(false, false, 2));
        assert!(j.tick(false, true, 2));
        assert!(!j.tick(false, false, 2));
        assert!(
            !j.tick(false, true, 2),
            "the Cirrus Amulet allows two jumps"
        );
        assert!(!j.tick(true, false, 2));
        assert_eq!(j.times, 0);
        let mut j = AmuletJumps::default();
        let mut air = 0;
        for jumping in [true, false, true, false, true, false, true] {
            if j.tick(false, jumping, 3) {
                air += 1;
            }
        }
        assert_eq!(air, 2, "the Nimbus Amulet allows two jumps in the air");
    }

    #[test]
    fn cloaks() {
        let (virtue, sin, balance) = (Cloak::Virtue, Cloak::Sin, Cloak::Balance);
        assert_eq!(Cloak::of_item(UNHOLY_CLOAK), Some(sin));
        assert!(virtue.acts_on("minecraft:mob_attack") && !virtue.acts_on("minecraft:magic"));
        assert!(!virtue.acts_on("minecraft:out_of_world"));
        assert!(sin.acts_on("minecraft:mob_attack") && !sin.acts_on("minecraft:fall"));
        assert!(balance.acts_on("minecraft:fall") && !balance.acts_on("minecraft:thorns"));
        assert_eq!(virtue.amount(9.0, 20.0), 0.0);
        assert_eq!(sin.amount(9.0, 20.0), 9.0);
        assert_eq!(balance.amount(9.0, 20.0), 4.5);
        // Balance never leaves less than one point: 30 halves to 15, over 6 health → 5.
        assert_eq!(balance.amount(30.0, 6.0), 5.0);
    }
}
