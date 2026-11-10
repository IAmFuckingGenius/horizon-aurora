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
}
