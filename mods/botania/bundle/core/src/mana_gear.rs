//! Botania's mana-using tools, bows and armour: what a point of their damage costs in mana
//! (`damageItem` → `ToolCommons.damageItemIfPossible(stack, amount, entity, manaPerDamage)`), what
//! their `inventoryTick` pays to mend a point each tick, the armour sets, the helmets' mana
//! discount (`ManaDiscountArmor`) and the Terrasteel set's bonus.

/// What a tool or bow pays in mana (`ManasteelPickaxeItem`, `ManasteelSwordItem`...).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolMana {
    /// Mana per point of damage paid instead (`getManaPerDamage` or the class's constant).
    pub per_damage: i32,
    /// Mana a tick for mending one point (`requestManaExactForTool(..., cost, true)`), with the
    /// armour's discount; None when the item doesn't mend (the Soulscribe).
    pub repair: Option<i32>,
}

/// The tools and bows, by item id. Each class's own numbers, which aren't always
/// `2 * getManaPerDamage()`: `ManasteelPickaxeItem.inventoryTick` uses its private
/// `MANA_PER_DAMAGE` (60), so the Terra Shatterer and the Vitreous Pickaxe mend for 120;
/// `LivingwoodBowItem` uses its static 40, so the Crystal Bow mends for 80.
pub fn tool(item: &str, infinity: bool) -> Option<ToolMana> {
    let (per_damage, repair) = match item {
        // ManasteelPickaxeItem, ElementiumPickaxeItem (extends it).
        "botania:manasteel_pick" | "botania:elementium_pickaxe" => (60, Some(120)),
        // ManasteelAxeItem, ManasteelShovelItem, ManasteelHoeItem, ManasteelSwordItem and the
        // elementium ones: getManaPerDamage() * 2.
        "botania:manasteel_axe"
        | "botania:manasteel_shovel"
        | "botania:manasteel_hoe"
        | "botania:manasteel_sword"
        | "botania:elementium_axe"
        | "botania:elementium_shovel"
        | "botania:elementium_hoe"
        | "botania:elementium_sword"
        // ThundercallerItem extends ManasteelSwordItem without its own cost.
        | "botania:thunder_sword" => (60, Some(120)),
        // ManasteelShearsItem.MANA_PER_DAMAGE, ElementiumShearsItem extends it.
        "botania:manasteel_shears" | "botania:elementium_shears" => (30, Some(60)),
        // TerraShattererItem: 100 a point, mended by ManasteelPickaxeItem's 60 * 2.
        "botania:terra_pick" => (100, Some(120)),
        // TerraTruncatorItem (ManasteelAxeItem) and TerraBladeItem (ManasteelSwordItem): 100 * 2.
        "botania:terra_axe" | "botania:terra_sword" => (100, Some(200)),
        // StarcallerItem: 120, mended by ManasteelSwordItem's getManaPerDamage() * 2.
        "botania:star_sword" => (120, Some(240)),
        // VitreousPickaxeItem: 160 a point, mended by ManasteelPickaxeItem's 60 * 2.
        "botania:glass_pickaxe" => (160, Some(120)),
        // LivingwoodBowItem.MANA_PER_DAMAGE.
        "botania:livingwood_bow" => (40, Some(80)),
        // CrystalBowItem: ARROW_COST, halved with Infinity; LivingwoodBowItem mends it.
        "botania:crystal_bow" => (if infinity { 100 } else { 200 }, Some(80)),
        _ => return None,
    };
    Some(ToolMana { per_damage, repair })
}

/// The armour sets (`ManasteelArmorItem` and its subclasses).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmorSet {
    Manasteel,
    Manaweave,
    Elementium,
    Terrasteel,
}

impl ArmorSet {
    /// The set's pieces, head first.
    pub fn pieces(self) -> [&'static str; 4] {
        match self {
            ArmorSet::Manasteel => [
                "botania:manasteel_helmet",
                "botania:manasteel_chestplate",
                "botania:manasteel_leggings",
                "botania:manasteel_boots",
            ],
            ArmorSet::Manaweave => [
                "botania:manaweave_helmet",
                "botania:manaweave_chestplate",
                "botania:manaweave_leggings",
                "botania:manaweave_boots",
            ],
            ArmorSet::Elementium => [
                "botania:elementium_helmet",
                "botania:elementium_chestplate",
                "botania:elementium_leggings",
                "botania:elementium_boots",
            ],
            ArmorSet::Terrasteel => [
                "botania:terrasteel_helmet",
                "botania:terrasteel_chestplate",
                "botania:terrasteel_leggings",
                "botania:terrasteel_boots",
            ],
        }
    }

    /// The helmet's `getDiscount` with the whole set worn (`ManasteelHelmItem` 0.1,
    /// `ManaweaveHelmItem` 0.4, `ElementiumHelmItem` 0.1, `TerrasteelHelmItem` 0.2).
    pub fn discount(self) -> f32 {
        match self {
            ArmorSet::Manasteel => 0.1,
            ArmorSet::Manaweave => 0.4,
            ArmorSet::Elementium => 0.1,
            ArmorSet::Terrasteel => 0.2,
        }
    }
}

const SETS: [ArmorSet; 4] = [
    ArmorSet::Manasteel,
    ArmorSet::Manaweave,
    ArmorSet::Elementium,
    ArmorSet::Terrasteel,
];

/// What a piece of armour pays in mana.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArmorMana {
    pub set: ArmorSet,
    /// `getManaPerDamage`: 70, the Manaweave Helmet's 30.
    pub per_damage: i32,
    /// `inventoryTick` mends a point a tick for `requestManaExact(stack, player, 2 * perDamage)`,
    /// with no discount.
    pub repair: i32,
    pub helmet: bool,
}

pub fn armor(item: &str) -> Option<ArmorMana> {
    for set in SETS {
        if let Some(slot) = set.pieces().iter().position(|p| *p == item) {
            let per_damage = if set == ArmorSet::Manaweave && slot == 0 {
                30
            } else {
                70
            };
            return Some(ArmorMana {
                set,
                per_damage,
                repair: per_damage * 2,
                helmet: slot == 0,
            });
        }
    }
    None
}

/// `hasArmorSet`: the set whose four pieces are worn (`armor` head first).
pub fn worn_set(armor: [Option<&str>; 4]) -> Option<ArmorSet> {
    SETS.into_iter().find(|set| {
        set.pieces()
            .iter()
            .zip(armor)
            .all(|(piece, worn)| worn == Some(*piece))
    })
}

/// The worn armour's discounts, `Inventory.armor`'s order (feet first), for
/// `ManaItemHandler.getFullDiscountForTools`: a helmet gives its set's with the set worn.
pub fn armor_discounts(armor: [Option<&str>; 4]) -> Vec<f32> {
    let set = worn_set(armor);
    let mut out = Vec::new();
    for worn in armor.iter().rev().flatten() {
        if let Some(a) = self::armor(worn)
            && a.helmet
        {
            out.push(if set == Some(a.set) {
                a.set.discount()
            } else {
                0.0
            });
        }
    }
    out
}

/// `TerrasteelHelmItem.inventoryTick`, with the helmet worn and the set complete: heal 1 every
/// 80 ticks while hurt with food between 1 and 17 ...
pub fn terrasteel_heals(food: i32, hurt: bool, tick_count: i32) -> bool {
    food > 0 && food < 18 && hurt && tick_count % 80 == 0
}

/// ... and give the player's mana items 10 every 10 ticks (`dispatchManaExact(stack, player, 10,
/// true)`).
pub fn terrasteel_gives_mana(tick_count: i32) -> bool {
    tick_count % 10 == 0
}

/// The Terrasteel set's mana every 10 ticks.
pub const TERRASTEEL_MANA: i32 = 10;

/// `TerrasteelArmorItem.getDefaultAttributeModifiers`: knockback resistance of the piece's
/// defence / 20 (`ArmorMaterial.getDefenseForType`: helmet 3, chestplate 8, leggings 6, boots 3).
pub fn terrasteel_knockback(item: &str) -> Option<(f64, &'static str)> {
    Some(match item {
        "botania:terrasteel_helmet" => (3.0 / 20.0, "head"),
        "botania:terrasteel_chestplate" => (8.0 / 20.0, "chest"),
        "botania:terrasteel_leggings" => (6.0 / 20.0, "legs"),
        "botania:terrasteel_boots" => (3.0 / 20.0, "feet"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_costs_per_class() {
        let t = |id| tool(id, false).map(|t| (t.per_damage, t.repair));
        assert_eq!(t("botania:manasteel_pick"), Some((60, Some(120))));
        assert_eq!(t("botania:elementium_sword"), Some((60, Some(120))));
        assert_eq!(t("botania:manasteel_shears"), Some((30, Some(60))));
        assert_eq!(t("botania:terra_pick"), Some((100, Some(120))));
        assert_eq!(t("botania:terra_sword"), Some((100, Some(200))));
        assert_eq!(t("botania:terra_axe"), Some((100, Some(200))));
        assert_eq!(t("botania:star_sword"), Some((120, Some(240))));
        assert_eq!(t("botania:thunder_sword"), Some((60, Some(120))));
        assert_eq!(t("botania:glass_pickaxe"), Some((160, Some(120))));
        assert_eq!(t("botania:livingwood_bow"), Some((40, Some(80))));
        assert_eq!(t("botania:crystal_bow"), Some((200, Some(80))));
        assert_eq!(
            tool("botania:crystal_bow", true).map(|t| t.per_damage),
            Some(100)
        );
        // SoulscribeItem.damageItem returns the damage untouched and its inventoryTick is empty.
        assert_eq!(t("botania:ender_dagger"), None);
        assert_eq!(t("minecraft:diamond_pickaxe"), None);
    }

    #[test]
    fn armor_costs_and_sets() {
        let a = armor("botania:manaweave_helmet").unwrap();
        assert_eq!((a.per_damage, a.repair, a.helmet), (30, 60, true));
        let a = armor("botania:manaweave_boots").unwrap();
        assert_eq!((a.per_damage, a.repair), (70, 140));
        let a = armor("botania:terrasteel_chestplate").unwrap();
        assert_eq!((a.set, a.per_damage, a.repair), (ArmorSet::Terrasteel, 70, 140));
        assert_eq!(armor("minecraft:iron_helmet"), None);
        let full = ArmorSet::Manaweave.pieces().map(Some);
        assert_eq!(worn_set(full), Some(ArmorSet::Manaweave));
        let mut mixed = full;
        mixed[3] = Some("botania:manasteel_boots");
        assert_eq!(worn_set(mixed), None);
        assert_eq!(armor_discounts(full), vec![0.4]);
        assert_eq!(armor_discounts(mixed), vec![0.0]);
        assert_eq!(
            armor_discounts(ArmorSet::Terrasteel.pieces().map(Some)),
            vec![0.2]
        );
        assert_eq!(
            armor_discounts([None, Some("botania:manasteel_chestplate"), None, None]),
            Vec::<f32>::new()
        );
    }

    #[test]
    fn terrasteel_bonus_ticks() {
        assert!(terrasteel_heals(17, true, 160));
        assert!(!terrasteel_heals(18, true, 160), "food 18 heals by itself");
        assert!(!terrasteel_heals(0, true, 160));
        assert!(!terrasteel_heals(10, false, 160));
        assert!(!terrasteel_heals(10, true, 170));
        let gives: Vec<i32> = (0..35).filter(|&t| terrasteel_gives_mana(t)).collect();
        assert_eq!(gives, vec![0, 10, 20, 30]);
        assert_eq!(terrasteel_knockback("botania:terrasteel_chestplate"), Some((0.4, "chest")));
        let total: f64 = ArmorSet::Terrasteel
            .pieces()
            .iter()
            .filter_map(|p| terrasteel_knockback(p))
            .map(|(k, _)| k)
            .sum();
        assert!((total - 1.0).abs() < 1e-12, "the full set resists all knockback");
    }
}
