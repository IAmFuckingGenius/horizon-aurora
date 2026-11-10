//! Botania's numbers (1.20.x, commit 0c06dcf7ae), each with the class it comes from.

/// A mana spreader variant: `ManaSpreaderBlock.Variant`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpreaderVariant {
    Mana,
    Redstone,
    Elven,
    Gaia,
}

/// What a spreader's burst carries: `ManaSpreaderBlock.Variant` and the `BurstProperties` made from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpreaderStats {
    pub burst_mana: i32,
    pub capacity: i32,
    pub color: u32,
    pub hud_color: u32,
    pub ticks_before_loss: i32,
    pub loss_per_tick: f32,
    pub motion: f32,
}

impl SpreaderVariant {
    pub const ALL: [SpreaderVariant; 4] = [
        SpreaderVariant::Mana,
        SpreaderVariant::Redstone,
        SpreaderVariant::Elven,
        SpreaderVariant::Gaia,
    ];

    pub fn stats(self) -> SpreaderStats {
        let (burst_mana, capacity, color, hud_color, ticks_before_loss, loss_per_tick, motion) =
            match self {
                SpreaderVariant::Mana => (160, 1000, 0x20FF20, 0x00FF00, 60, 4.0, 1.0),
                SpreaderVariant::Redstone => (160, 1000, 0xFF2020, 0xFF0000, 60, 4.0, 1.0),
                SpreaderVariant::Elven => (240, 1000, 0xFF45C4, 0xFF00AE, 80, 4.0, 1.25),
                SpreaderVariant::Gaia => (640, 6400, 0x20FF20, 0x00FF00, 120, 20.0, 2.0),
            };
        SpreaderStats {
            burst_mana,
            capacity,
            color,
            hud_color,
            ticks_before_loss,
            loss_per_tick,
            motion,
        }
    }

    /// The variant of a spreader block id (`botania:mana_spreader`...).
    pub fn of_block(block: &str) -> Option<SpreaderVariant> {
        Some(match block {
            "botania:mana_spreader" => SpreaderVariant::Mana,
            "botania:redstone_spreader" => SpreaderVariant::Redstone,
            "botania:elven_spreader" => SpreaderVariant::Elven,
            "botania:gaia_spreader" => SpreaderVariant::Gaia,
            _ => return None,
        })
    }
}

/// Ticks a spreader waits for its burst's ping before it may shoot again
/// (`ManaSpreaderBlockEntity.TICKS_ALLOWED_WITHOUT_PINGBACK`).
pub const SPREADER_PINGBACK_TICKS: i32 = 20;
/// How far from the last ping a spreader looks for its burst (`PINGBACK_EXPIRED_SEARCH_DISTANCE`).
pub const SPREADER_PINGBACK_SEARCH: f64 = 0.5;
/// Steps a spreader's receiver scan simulates (`FiberBotaniaConfig.spreaderTraceTime`).
pub const SPREADER_TRACE_TIME: usize = 400;
/// The spreader fire sound's volume, and with wool padding (`tryShootBurst`).
pub const SPREADER_FIRE_VOLUME: f32 = 0.05;
pub const SPREADER_FIRE_VOLUME_PADDED: f32 = 0.05 * 0.2;

/// Mana pool variants: `ManaPoolBlock.Variant`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PoolVariant {
    Default,
    Creative,
    Diluted,
    Fabulous,
}

impl PoolVariant {
    pub fn of_block(block: &str) -> Option<PoolVariant> {
        Some(match block {
            "botania:mana_pool" => PoolVariant::Default,
            "botania:creative_pool" => PoolVariant::Creative,
            "botania:diluted_pool" => PoolVariant::Diluted,
            "botania:fabulous_pool" => PoolVariant::Fabulous,
            _ => return None,
        })
    }

    /// `manaCap` as `initManaCapAndNetwork` sets it.
    pub fn capacity(self) -> i32 {
        match self {
            PoolVariant::Diluted => POOL_MAX_MANA_DILUTED,
            _ => POOL_MAX_MANA,
        }
    }

    /// The top of the slab a burst collides with, in pixels (`*_SHAPE_INTERACT`).
    pub fn burst_slab_height(self) -> f64 {
        match self {
            PoolVariant::Diluted => 6.0,
            PoolVariant::Creative => 10.0,
            PoolVariant::Default | PoolVariant::Fabulous => 8.0,
        }
    }
}

/// `ManaPoolBlockEntity.MAX_MANA` and `MAX_MANA_DILLUTED`.
pub const POOL_MAX_MANA: i32 = 1_000_000;
pub const POOL_MAX_MANA_DILUTED: i32 = 10_000;
/// A pool resends itself at most every this many ticks (`serverTick`: `ticks % 10 == 0`).
pub const POOL_SYNC_INTERVAL: i32 = 10;
/// Ticks between two craft sounds of a pool (`craftingEffect`: `soundTicks = 6`).
pub const POOL_CRAFT_SOUND_TICKS: i32 = 6;
/// Mana a pool moves into or out of a mana item a tick, per bellows plus one.
pub const POOL_ITEM_TRANSFER_RATE: i32 = 1000;
/// Colour of a pool's own particles (`PARTICLE_COLOR`).
pub const POOL_PARTICLE_COLOR: u32 = 0x00C6FF;

/// `GeneratingFlowerBlockEntity.LINK_RANGE` and `FunctionalFlowerBlockEntity.LINK_RANGE`.
pub const GENERATING_LINK_RANGE: i32 = 6;
pub const FUNCTIONAL_LINK_RANGE: i32 = 10;
/// `DelayHelper`: a flower sees an item once its time counter passes this.
pub const FUNCTIONAL_INHERENT_DELAY: i32 = 60;
pub const GENERATING_INHERENT_DELAY: i32 = FUNCTIONAL_INHERENT_DELAY - 1;
/// `SpecialFlowerBlockEntity.PODZOL_DELAY` and `MYCELIUM_DELAY`.
pub const PODZOL_DELAY: i32 = 5;
pub const MYCELIUM_DELAY: i32 = 10;

/// `EndoflameBlockEntity`: fuel cap, range, mana buffer and colour.
pub const ENDOFLAME_FUEL_CAP: i32 = 32_000;
pub const ENDOFLAME_RANGE: i32 = 3;
pub const ENDOFLAME_MAX_MANA: i32 = 300;
pub const ENDOFLAME_MANA_PER_BURN: i32 = 3;
pub const ENDOFLAME_COLOR: u32 = 0x785000;

/// `PureDaisyRecipe.DEFAULT_TIME`: visits a block needs before it turns.
pub const PURE_DAISY_DEFAULT_TIME: i32 = 150;

/// `PetalApothecaryBlockEntity`: slots, the remembered recipe's life, rain's chance (1 in 20).
pub const APOTHECARY_SLOTS: usize = 16;
pub const APOTHECARY_RECIPE_KEEP_TICKS: i32 = 400;
pub const APOTHECARY_RAIN_CHANCE: i32 = 20;

/// The colour of a mystical flower's petals, by dye name (`ColorHelper.getPetalLikeColor` ... ).
pub fn petal_color(dye: &str) -> Option<u32> {
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
