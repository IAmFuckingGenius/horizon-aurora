//! The Petal Apothecary (`PetalApothecaryBlockEntity.collideEntityItem`, `serverTick`): what an
//! item thrown into it does.

use crate::consts::{APOTHECARY_RECIPE_KEEP_TICKS, APOTHECARY_SLOTS};
use crate::recipe::{InTag, Ingredient, Output, PetalRecipe};

/// The block's `fluid` property.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fluid {
    #[default]
    Empty,
    Water,
    Lava,
}

impl Fluid {
    pub fn name(self) -> &'static str {
        match self {
            Fluid::Empty => "empty",
            Fluid::Water => "water",
            Fluid::Lava => "lava",
        }
    }
    pub fn of_name(name: &str) -> Option<Fluid> {
        Some(match name {
            "empty" => Fluid::Empty,
            "water" => Fluid::Water,
            "lava" => Fluid::Lava,
            _ => return None,
        })
    }
}

/// An item entity touching the apothecary, as it judges it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Thrown<'a> {
    pub item: &'a str,
    pub count: u32,
    /// The apothecary or this flag's other owners made it (`apothecarySpawned`).
    pub spawned: bool,
    /// The fluid it holds and gives up (a water or lava bucket), if any.
    pub holds: Option<Fluid>,
    /// It is a fluid container at all (`isFluidContainer`): never taken as an ingredient.
    pub container: bool,
}

/// What happens to a thrown item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Collide {
    /// Nothing.
    Nothing,
    /// The apothecary takes its fluid: the item gives it up (an empty bucket stays) and is
    /// marked spawned; the block's fluid is now `fluid`.
    Fill(Fluid),
    /// Lava sets it on fire for 100 seconds.
    Ignite,
    /// A recipe completes: one of the item (the reagent) is used, `output` appears at
    /// `(x + 0.5, y + 1.5, z + 0.5)` marked spawned, and the water is used up.
    Craft { recipe: usize, output: Output },
    /// One of the item goes into the first empty slot.
    Take,
}

/// A Petal Apothecary's block entity, with its block's fluid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Apothecary {
    pub fluid: Fluid,
    /// The slots' items from the first; at most 16, one each.
    pub slots: Vec<String>,
    /// `lastRecipe`: the slots of the last craft.
    pub last_recipe: Option<Vec<String>>,
    /// `lastReagent`: the last craft's reagent; refused as an ingredient while remembered.
    pub last_reagent: Option<Ingredient>,
    /// `recipeKeepTicks`.
    pub keep_ticks: i32,
}

impl Apothecary {
    /// `collideEntityItem`, on the server, for one item entity. `recipes` in the recipe
    /// manager's order (`getRecipeFor` takes the first that matches).
    pub fn collide(&mut self, item: &Thrown, recipes: &[PetalRecipe], in_tag: InTag) -> Collide {
        if item.count == 0 {
            return Collide::Nothing;
        }
        match self.fluid {
            Fluid::Empty => {
                return match item.holds {
                    Some(f @ (Fluid::Water | Fluid::Lava)) => {
                        self.fluid = f;
                        Collide::Fill(f)
                    }
                    _ => Collide::Nothing,
                };
            }
            Fluid::Lava => return Collide::Ignite,
            Fluid::Water => {}
        }
        let slots: Vec<Option<&str>> = self.slots.iter().map(|s| Some(s.as_str())).collect();
        if let Some((i, r)) = recipes
            .iter()
            .enumerate()
            .find(|(_, r)| r.matches(&slots, in_tag))
            && r.reagent.test(item.item, in_tag)
        {
            self.last_recipe = Some(self.slots.clone());
            self.last_reagent = Some(r.reagent.clone());
            self.keep_ticks = APOTHECARY_RECIPE_KEEP_TICKS;
            self.slots.clear();
            self.fluid = Fluid::Empty;
            return Collide::Craft {
                recipe: i,
                output: r.output.clone(),
            };
        }
        if !item.container && !item.spawned {
            if self.slots.len() >= APOTHECARY_SLOTS {
                return Collide::Nothing;
            }
            if self
                .last_reagent
                .as_ref()
                .is_some_and(|r| r.test(item.item, in_tag))
            {
                return Collide::Nothing;
            }
            self.slots.push(item.item.to_owned());
            self.clear_last_recipe();
            return Collide::Take;
        }
        Collide::Nothing
    }

    /// `clearLastRecipe`.
    pub fn clear_last_recipe(&mut self) {
        self.last_recipe = None;
        self.last_reagent = None;
    }

    /// `tickRecipeKeep`, at the end of each server tick.
    pub fn tick_keep(&mut self) {
        if self.keep_ticks > 0 {
            self.keep_ticks -= 1;
        } else {
            self.clear_last_recipe();
        }
    }

    /// `PetalApothecaryBlock.getAnalogOutputSignal`: 15 with water, else 0.
    pub fn comparator(&self) -> i32 {
        if self.fluid == Fluid::Water { 15 } else { 0 }
    }

    /// `canAddLastRecipe`: empty, with water.
    pub fn can_add_last_recipe(&self) -> bool {
        self.slots.is_empty() && self.fluid == Fluid::Water
    }

    /// `InventoryHelper.tryToSetLastRecipe`: the remembered recipe's items the player can put
    /// back, in order, one each: `carried` says how many of an item the player has; a creative
    /// player needs none. Empty when nothing is remembered.
    pub fn last_recipe_refill(&self, carried: &dyn Fn(&str) -> u32, creative: bool) -> Vec<String> {
        let mut used: Vec<(&str, u32)> = Vec::new();
        let mut out = Vec::new();
        for item in self.last_recipe.iter().flatten() {
            let n = used
                .iter()
                .find(|(i, _)| *i == item.as_str())
                .map_or(0, |u| u.1);
            if creative || carried(item) > n {
                match used.iter_mut().find(|(i, _)| *i == item.as_str()) {
                    Some(u) => u.1 += 1,
                    None => used.push((item.as_str(), 1)),
                }
                out.push(item.clone());
            }
        }
        out
    }

    /// `PetalApothecaryBlock.use` by a player whose main hand holds `main_hand` (None when
    /// empty). An empty hand puts the last recipe back or takes the last item out; an empty
    /// bucket takes the fluid, a full one fills an empty apothecary.
    pub fn on_use(
        &self,
        main_hand: Option<&str>,
        carried: &dyn Fn(&str) -> u32,
        creative: bool,
    ) -> AltarUse {
        let empty_hand = main_hand.is_none();
        if self.can_add_last_recipe() && empty_hand {
            let items = self.last_recipe_refill(carried, creative);
            return if items.is_empty() {
                AltarUse::Pass
            } else {
                AltarUse::LastRecipe(items)
            };
        }
        if !self.slots.is_empty() && empty_hand {
            return AltarUse::Withdraw;
        }
        match (main_hand, self.fluid) {
            (Some("minecraft:bucket"), f @ (Fluid::Water | Fluid::Lava)) => AltarUse::TakeFluid(f),
            (Some("minecraft:water_bucket"), Fluid::Empty) => AltarUse::PutFluid(Fluid::Water),
            (Some("minecraft:lava_bucket"), Fluid::Empty) => AltarUse::PutFluid(Fluid::Lava),
            _ => AltarUse::Pass,
        }
    }
}

/// What a right click on the apothecary does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AltarUse {
    /// `trySetLastRecipe`: these items go back into the slots from the first, each paid with
    /// one of the player's (a creative player's are copies); a splash at the player.
    LastRecipe(Vec<String>),
    /// `withdrawFromInventory`: the last item in the slots goes back to the player.
    Withdraw,
    /// `tryWithdrawFluid`: the empty bucket in hand is filled with this fluid; the apothecary
    /// empties.
    TakeFluid(Fluid),
    /// `tryDepositFluid`: the bucket in hand empties into the apothecary.
    PutFluid(Fluid),
    Pass,
}

/// The mob a fish bucket releases into an empty apothecary it fills with water
/// (`MobBucketItem.checkExtraContent`), at the block above.
pub fn bucket_mob(item: &str) -> Option<&'static str> {
    Some(match item {
        "minecraft:cod_bucket" => "minecraft:cod",
        "minecraft:salmon_bucket" => "minecraft:salmon",
        "minecraft:pufferfish_bucket" => "minecraft:pufferfish",
        "minecraft:tropical_fish_bucket" => "minecraft:tropical_fish",
        "minecraft:axolotl_bucket" => "minecraft:axolotl",
        "minecraft:tadpole_bucket" => "minecraft:tadpole",
        _ => return None,
    })
}

/// An item entity's health (`ItemEntity.health`).
pub const ITEM_ENTITY_HEALTH: i32 = 5;

/// An item in a lava apothecary is set on fire for 100 seconds (`setSecondsOnFire(100)`) each
/// tick it touches it, after its own tick: from its second tick of contact the fire, renewed to
/// a multiple of 20, hurts it 1 (`remainingFireTicks % 20 == 0`), and at 0 health it is gone.
/// Whether it is gone after touching for `ticks` ticks in a row; fire-resistant items never burn.
pub fn burnt_up(ticks: i32, fire_resistant: bool) -> bool {
    !fire_resistant && ticks > ITEM_ENTITY_HEALTH
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    fn tags(tag: &str, item: &str) -> bool {
        match tag {
            "botania:petals/white" => item == "botania:white_petal",
            "botania:seed_apothecary_reagent" => matches!(
                item,
                "minecraft:wheat_seeds"
                    | "minecraft:beetroot_seeds"
                    | "minecraft:melon_seeds"
                    | "minecraft:pumpkin_seeds"
            ),
            _ => false,
        }
    }

    fn thrown(item: &str) -> Thrown<'_> {
        Thrown {
            item,
            count: 1,
            spawned: false,
            holds: None,
            container: false,
        }
    }

    fn pure_daisy() -> Vec<PetalRecipe> {
        // botania/recipe/petal_apothecary/pure_daisy.json.
        let json = r#"{"type":"botania:petal_apothecary","ingredients":[{"tag":"botania:petals/white"},{"tag":"botania:petals/white"},{"tag":"botania:petals/white"},{"tag":"botania:petals/white"}],"output":{"item":"botania:pure_daisy"},"reagent":{"tag":"botania:seed_apothecary_reagent"}}"#;
        vec![
            PetalRecipe::parse("botania:petal_apothecary/pure_daisy", &parse(json).unwrap())
                .unwrap(),
        ]
    }

    #[test]
    fn botanias_item_enter_prevention_gametest() {
        // ApothecaryRecipeTest.testItemEnterPrevention, item by item.
        let recipes = pure_daisy();
        let mut a = Apothecary {
            fluid: Fluid::Water,
            ..Apothecary::default()
        };
        for _ in 0..4 {
            assert_eq!(
                a.collide(&thrown("botania:white_petal"), &recipes, &tags),
                Collide::Take
            );
        }
        a.tick_keep();
        match a.collide(&thrown("minecraft:melon_seeds"), &recipes, &tags) {
            Collide::Craft { recipe: 0, output } => assert_eq!(output.item, "botania:pure_daisy"),
            other => panic!("{other:?}"),
        }
        assert_eq!(a.fluid, Fluid::Empty, "the craft uses the water up");
        assert!(a.slots.is_empty());
        a.tick_keep();
        // Filled again: the remembered reagent keeps both kinds of seeds out.
        a.fluid = Fluid::Water;
        for _ in 0..10 {
            assert_eq!(
                a.collide(&thrown("minecraft:melon_seeds"), &recipes, &tags),
                Collide::Nothing
            );
            assert_eq!(
                a.collide(&thrown("minecraft:pumpkin_seeds"), &recipes, &tags),
                Collide::Nothing
            );
            a.tick_keep();
        }
        // A diamond goes in and makes the apothecary forget the recipe: the seeds follow.
        assert_eq!(
            a.collide(&thrown("minecraft:diamond"), &recipes, &tags),
            Collide::Take
        );
        assert_eq!(
            a.collide(&thrown("minecraft:melon_seeds"), &recipes, &tags),
            Collide::Take
        );
        assert_eq!(
            a.collide(&thrown("minecraft:pumpkin_seeds"), &recipes, &tags),
            Collide::Take
        );
        assert_eq!(
            a.slots,
            [
                "minecraft:diamond",
                "minecraft:melon_seeds",
                "minecraft:pumpkin_seeds"
            ]
        );
    }

    #[test]
    fn fluids_buckets_lava_and_a_full_apothecary() {
        let recipes = pure_daisy();
        let mut a = Apothecary::default();
        // Empty: only a fluid goes in; a petal does nothing.
        assert_eq!(
            a.collide(&thrown("botania:white_petal"), &recipes, &tags),
            Collide::Nothing
        );
        let lava = Thrown {
            holds: Some(Fluid::Lava),
            container: true,
            ..thrown("minecraft:lava_bucket")
        };
        assert_eq!(
            a.collide(&lava, &recipes, &tags),
            Collide::Fill(Fluid::Lava)
        );
        assert_eq!(a.comparator(), 0);
        assert_eq!(
            a.collide(&thrown("botania:white_petal"), &recipes, &tags),
            Collide::Ignite
        );
        let mut w = Apothecary::default();
        let water = Thrown {
            holds: Some(Fluid::Water),
            container: true,
            ..thrown("minecraft:water_bucket")
        };
        assert_eq!(
            w.collide(&water, &recipes, &tags),
            Collide::Fill(Fluid::Water)
        );
        assert_eq!(w.comparator(), 15);
        // With water, a bucket and a spawned item aren't ingredients.
        let empty_bucket = Thrown {
            container: true,
            ..thrown("minecraft:bucket")
        };
        assert_eq!(w.collide(&empty_bucket, &recipes, &tags), Collide::Nothing);
        let spawned = Thrown {
            spawned: true,
            ..thrown("botania:pure_daisy")
        };
        assert_eq!(w.collide(&spawned, &recipes, &tags), Collide::Nothing);
        // Sixteen slots, then nothing more.
        for _ in 0..16 {
            assert_eq!(
                w.collide(&thrown("minecraft:dirt"), &recipes, &tags),
                Collide::Take
            );
        }
        assert_eq!(
            w.collide(&thrown("minecraft:dirt"), &recipes, &tags),
            Collide::Nothing
        );
        // Seeds with no matching recipe are an ingredient like any other item.
        let mut s = Apothecary {
            fluid: Fluid::Water,
            ..Apothecary::default()
        };
        assert_eq!(
            s.collide(&thrown("minecraft:wheat_seeds"), &recipes, &tags),
            Collide::Take
        );
    }

    #[test]
    fn the_recipe_is_forgotten_after_400_ticks() {
        let mut a = Apothecary {
            last_reagent: Some(Ingredient::default()),
            last_recipe: Some(vec![]),
            keep_ticks: APOTHECARY_RECIPE_KEEP_TICKS,
            ..Apothecary::default()
        };
        for _ in 0..400 {
            a.tick_keep();
            assert!(a.last_reagent.is_some());
        }
        a.tick_keep();
        assert!(a.last_reagent.is_none());
    }

    #[test]
    fn an_empty_hand_refills_the_last_recipe_or_takes_the_last_item() {
        let recipes = pure_daisy();
        let mut a = Apothecary {
            fluid: Fluid::Water,
            ..Apothecary::default()
        };
        for _ in 0..4 {
            a.collide(&thrown("botania:white_petal"), &recipes, &tags);
        }
        // Items in it: an empty hand takes the last one back; a held item does nothing.
        let none = |_: &str| 0_u32;
        assert_eq!(a.on_use(None, &none, false), AltarUse::Withdraw);
        assert_eq!(
            a.on_use(Some("botania:white_petal"), &none, false),
            AltarUse::Pass
        );
        a.collide(&thrown("minecraft:wheat_seeds"), &recipes, &tags);
        assert!(a.slots.is_empty());
        // Crafted and refilled with water: the four petals go back, each paid for; three
        // carried put three back; none carried, nothing (and the click passes).
        a.fluid = Fluid::Water;
        assert!(a.can_add_last_recipe());
        let petals = |n: u32| move |i: &str| if i == "botania:white_petal" { n } else { 0 };
        assert_eq!(
            a.on_use(None, &petals(64), false),
            AltarUse::LastRecipe(vec!["botania:white_petal".to_owned(); 4])
        );
        assert_eq!(
            a.on_use(None, &petals(3), false),
            AltarUse::LastRecipe(vec!["botania:white_petal".to_owned(); 3])
        );
        assert_eq!(a.on_use(None, &petals(0), false), AltarUse::Pass);
        // A creative player needs none.
        assert_eq!(
            a.on_use(None, &petals(0), true),
            AltarUse::LastRecipe(vec!["botania:white_petal".to_owned(); 4])
        );
        // Forgotten after 400 ticks: nothing to put back.
        for _ in 0..=APOTHECARY_RECIPE_KEEP_TICKS {
            a.tick_keep();
        }
        assert_eq!(a.on_use(None, &petals(64), false), AltarUse::Pass);
    }

    #[test]
    fn buckets_take_and_give_the_fluid() {
        let none = |_: &str| 0_u32;
        let water = Apothecary {
            fluid: Fluid::Water,
            ..Apothecary::default()
        };
        assert_eq!(
            water.on_use(Some("minecraft:bucket"), &none, false),
            AltarUse::TakeFluid(Fluid::Water)
        );
        assert_eq!(
            water.on_use(Some("minecraft:water_bucket"), &none, false),
            AltarUse::Pass,
            "only an empty apothecary is filled"
        );
        let lava = Apothecary {
            fluid: Fluid::Lava,
            ..Apothecary::default()
        };
        assert_eq!(
            lava.on_use(Some("minecraft:bucket"), &none, false),
            AltarUse::TakeFluid(Fluid::Lava)
        );
        let empty = Apothecary::default();
        assert_eq!(
            empty.on_use(Some("minecraft:bucket"), &none, false),
            AltarUse::Pass
        );
        assert_eq!(
            empty.on_use(Some("minecraft:lava_bucket"), &none, false),
            AltarUse::PutFluid(Fluid::Lava)
        );
        assert_eq!(
            empty.on_use(Some("minecraft:water_bucket"), &none, false),
            AltarUse::PutFluid(Fluid::Water)
        );
        assert_eq!(empty.on_use(None, &none, false), AltarUse::Pass);
        assert_eq!(
            bucket_mob("minecraft:axolotl_bucket"),
            Some("minecraft:axolotl")
        );
        assert_eq!(bucket_mob("minecraft:water_bucket"), None);
    }

    #[test]
    fn items_burn_up_on_their_sixth_tick_in_lava() {
        // Tick 1 lights it; ticks 2 to 6 hurt it 1 each; 5 health.
        assert!(!burnt_up(5, false));
        assert!(burnt_up(6, false));
        assert!(!burnt_up(1000, true), "netherite doesn't burn");
    }
}
