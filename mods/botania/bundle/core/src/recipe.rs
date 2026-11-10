//! Botania's recipe files in their 1.20 form, read as Botania's serializers read them, and the
//! wrapper recipes the game's crafting table doesn't know, unwrapped into 26.3 crafting recipes.
//!
//! - `botania:petal_apothecary` (`PetalsRecipe.Serializer`): `ingredients`, `output`, `reagent`;
//! - `botania:pure_daisy` (`PureDaisyRecipe.Serializer`): `input` (a state ingredient), `output`
//!   (a block state), `time` (150), `success_function`;
//! - `botania:state_copying_pure_daisy` (`StateCopyingPureDaisyRecipe.Serializer`): `output` is a
//!   block id whose state copies the input's properties;
//! - `botania:mana_infusion` (`ManaInfusionRecipe.Serializer`): `input`, `output`, `mana`,
//!   `group`, `catalyst` (a state ingredient with a `type`);
//! - `botania:gog_alternation`, `botania:nbt_output_wrapper` and `botania:twig_wand` wrap a vanilla
//!   crafting recipe: [`unwrap`] gives that recipe in 26.3's form.

use crate::json::Value;

/// Answers whether `id` (an item or a block) is in tag `tag` (without `#`).
pub type InTag<'a> = &'a dyn Fn(&str, &str) -> bool;

/// One choice of a vanilla `Ingredient`: an item, or every item of a tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    Item(String),
    Tag(String),
}

/// A vanilla `Ingredient` (`Ingredient.fromJson`): what any of its choices accepts. Empty matches
/// nothing (`Ingredient.EMPTY`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ingredient(pub Vec<Choice>);

impl Ingredient {
    /// 1.20's `{"item": id}`, `{"tag": t}` or a list of those; also 26.3's `"id"`, `"#tag"` and
    /// list of ids. None for a custom ingredient type (`{"type": ...}`) or a malformed one.
    pub fn parse(v: &Value) -> Option<Ingredient> {
        match v {
            Value::String(s) => Some(Ingredient(vec![choice_of_str(s)?])),
            Value::Object(_) => Some(Ingredient(vec![choice_of_object(v)?])),
            Value::Array(list) => {
                let mut out = Vec::new();
                for x in list {
                    out.push(match x {
                        Value::String(s) => choice_of_str(s)?,
                        Value::Object(_) => choice_of_object(x)?,
                        _ => return None,
                    });
                }
                if out.is_empty() {
                    return None;
                }
                Some(Ingredient(out))
            }
            _ => None,
        }
    }

    /// `Ingredient.test` for an item id.
    pub fn test(&self, item: &str, in_tag: InTag) -> bool {
        self.0.iter().any(|c| match c {
            Choice::Item(id) => id == item,
            Choice::Tag(tag) => in_tag(tag, item),
        })
    }

    /// The same ingredient as 26.3 writes it: `"id"`, `"#tag"`, or a list of ids. None when it
    /// can't be written so (a list holding a tag; 26.3's holder sets don't mix them).
    pub fn to_json_26(&self) -> Option<Value> {
        match self.0.as_slice() {
            [] => None,
            [Choice::Item(id)] => Some(Value::str(id)),
            [Choice::Tag(tag)] => Some(Value::String(format!("#{tag}"))),
            list => {
                let mut ids = Vec::new();
                for c in list {
                    match c {
                        Choice::Item(id) => ids.push(Value::str(id)),
                        Choice::Tag(_) => return None,
                    }
                }
                Some(Value::Array(ids))
            }
        }
    }
}

fn choice_of_str(s: &str) -> Option<Choice> {
    if let Some(tag) = s.strip_prefix('#') {
        (!tag.is_empty()).then(|| Choice::Tag(tag.to_owned()))
    } else {
        (!s.is_empty()).then(|| Choice::Item(s.to_owned()))
    }
}

fn choice_of_object(v: &Value) -> Option<Choice> {
    let o = v.as_object()?;
    if o.len() != 1 {
        return None;
    }
    if let Some(id) = v.get("item").and_then(Value::as_str) {
        return Some(Choice::Item(id.to_owned()));
    }
    v.get("tag")
        .and_then(Value::as_str)
        .map(|t| Choice::Tag(t.to_owned()))
}

/// A recipe's result: `ShapedRecipe.itemStackFromJson` (`item`, `count`, `nbt`), or 26.3's `id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub item: String,
    pub count: i32,
    /// The stack's 1.20 NBT as SNBT text, when it has any.
    pub nbt: Option<String>,
}

impl Output {
    pub fn parse(v: &Value) -> Option<Output> {
        if let Value::String(id) = v {
            return Some(Output {
                item: id.clone(),
                count: 1,
                nbt: None,
            });
        }
        let item = v
            .get("item")
            .or_else(|| v.get("id"))
            .and_then(Value::as_str)?
            .to_owned();
        let count = match v.get("count") {
            None => 1,
            Some(c) => i32::try_from(c.as_i64()?).ok()?,
        };
        if count < 1 {
            return None;
        }
        let nbt = match v.get("nbt") {
            None => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(other) => Some(other.to_json()),
        };
        Some(Output { item, count, nbt })
    }

    /// 26.3's `ItemStackTemplate`: `{"id", "count"}` (count left out when 1), the 1.20 NBT kept
    /// as the stack's `minecraft:custom_data`, as the datafixer moves an item's leftover tag.
    pub fn to_json_26(&self) -> Value {
        let mut o = vec![("id".to_owned(), Value::str(&self.item))];
        if self.count != 1 {
            o.push(("count".to_owned(), Value::Number(f64::from(self.count))));
        }
        if let Some(nbt) = &self.nbt {
            o.push((
                "components".to_owned(),
                Value::obj(vec![("minecraft:custom_data", Value::str(nbt))]),
            ));
        }
        Value::Object(o)
    }
}

/// A block state as recipes name it: the block and the properties written (others are the
/// block's defaults), sorted by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockState {
    pub name: String,
    pub properties: Vec<(String, String)>,
}

impl BlockState {
    pub fn new(name: &str, properties: &[(&str, &str)]) -> BlockState {
        let mut properties: Vec<(String, String)> = properties
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        properties.sort();
        BlockState {
            name: name.to_owned(),
            properties,
        }
    }

    /// `StateIngredientHelper.readBlockState`: `{"name": id, "properties": {k: v}}`.
    pub fn parse(v: &Value) -> Option<BlockState> {
        let name = v.get("name").and_then(Value::as_str)?;
        if name.is_empty() {
            return None;
        }
        let mut properties = Vec::new();
        if let Some(props) = v.get("properties") {
            for (k, x) in props.as_object()? {
                let value = match x {
                    Value::String(s) => s.clone(),
                    Value::Bool(b) => b.to_string(),
                    Value::Number(_) => x.to_json(),
                    _ => return None,
                };
                properties.push((k.clone(), value));
            }
        }
        properties.sort();
        Some(BlockState {
            name: name.to_owned(),
            properties,
        })
    }

    pub fn property(&self, key: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Whether `self`, a state a recipe writes, is `other`: same block, and every property
    /// written has `other`'s value. (A recipe's unwritten properties are the block's defaults,
    /// which only the game knows; it compares full states itself when it needs to.)
    pub fn written_matches(&self, other: &BlockState) -> bool {
        self.name == other.name
            && self
                .properties
                .iter()
                .all(|(k, v)| other.property(k) == Some(v.as_str()))
    }
}

/// Botania's `StateIngredient` (`StateIngredientHelper.deserialize`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateIngredient {
    /// `"block"`: any state of the block.
    Block(String),
    /// `"state"`: exactly that state.
    State(BlockState),
    /// `"tag"`: any block of the block tag.
    Tag(String),
    /// `"blocks"`: any state of any of them.
    Blocks(Vec<String>),
    /// `"tag_excluding"`: the tag's blocks that none of `exclude` matches.
    TagExcluding {
        tag: String,
        exclude: Vec<StateIngredient>,
    },
    /// `"compound"`: any of them.
    Compound(Vec<StateIngredient>),
}

impl StateIngredient {
    pub fn parse(v: &Value) -> Option<StateIngredient> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        Some(match v.get("type").and_then(Value::as_str)? {
            "block" => StateIngredient::Block(s("block")?),
            "state" => StateIngredient::State(BlockState::parse(v)?),
            "tag" => StateIngredient::Tag(s("tag")?),
            "blocks" => StateIngredient::Blocks(
                v.get("blocks")?
                    .as_array()?
                    .iter()
                    .map(|b| b.as_str().map(str::to_owned))
                    .collect::<Option<_>>()?,
            ),
            "tag_excluding" => StateIngredient::TagExcluding {
                tag: s("tag")?,
                exclude: v
                    .get("exclude")?
                    .as_array()?
                    .iter()
                    .map(StateIngredient::parse)
                    .collect::<Option<_>>()?,
            },
            "compound" => StateIngredient::Compound(
                v.get("ingredients")?
                    .as_array()?
                    .iter()
                    .map(StateIngredient::parse)
                    .collect::<Option<_>>()?,
            ),
            _ => return None,
        })
    }

    /// `StateIngredient.test`. `in_tag(tag, block)` reads the block tags.
    pub fn test(&self, state: &BlockState, in_tag: InTag) -> bool {
        match self {
            StateIngredient::Block(b) => &state.name == b,
            StateIngredient::State(s) => s.written_matches(state),
            StateIngredient::Tag(t) => in_tag(t, &state.name),
            StateIngredient::Blocks(list) => list.iter().any(|b| b == &state.name),
            StateIngredient::TagExcluding { tag, exclude } => {
                in_tag(tag, &state.name) && !exclude.iter().any(|e| e.test(state, in_tag))
            }
            StateIngredient::Compound(list) => list.iter().any(|e| e.test(state, in_tag)),
        }
    }
}

/// A `botania:petal_apothecary` recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct PetalRecipe {
    pub id: String,
    pub inputs: Vec<Ingredient>,
    pub output: Output,
    pub reagent: Ingredient,
}

impl PetalRecipe {
    pub fn parse(id: &str, v: &Value) -> Option<PetalRecipe> {
        if v.get("type").and_then(Value::as_str) != Some("botania:petal_apothecary") {
            return None;
        }
        let inputs: Vec<Ingredient> = v
            .get("ingredients")?
            .as_array()?
            .iter()
            .map(Ingredient::parse)
            .collect::<Option<_>>()?;
        // `Preconditions.checkArgument(inputs.length <= 16)`.
        if inputs.len() > crate::consts::APOTHECARY_SLOTS {
            return None;
        }
        Some(PetalRecipe {
            id: id.to_owned(),
            inputs,
            output: Output::parse(v.get("output")?)?,
            reagent: Ingredient::parse(v.get("reagent")?)?,
        })
    }

    /// `PetalsRecipe.matches`: the slots are read up to the first empty one; each item takes the
    /// first ingredient still missing that accepts it, an item no missing ingredient accepts
    /// fails, and every ingredient must be taken.
    pub fn matches(&self, slots: &[Option<&str>], in_tag: InTag) -> bool {
        let mut missing: Vec<&Ingredient> = self.inputs.iter().collect();
        for slot in slots {
            let Some(item) = slot else { break };
            match missing.iter().position(|ing| ing.test(item, in_tag)) {
                Some(i) => {
                    missing.remove(i);
                }
                None => return false,
            }
        }
        missing.is_empty()
    }
}

/// What a Pure Daisy leaves in place of the block it converts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DaisyOutput {
    /// `botania:pure_daisy`: this state.
    State(BlockState),
    /// `botania:state_copying_pure_daisy`: this block, with the converted block's properties.
    CopyProperties(String),
}

/// A `botania:pure_daisy` or `botania:state_copying_pure_daisy` recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct PureDaisyRecipe {
    pub id: String,
    pub input: StateIngredient,
    pub output: DaisyOutput,
    pub time: i32,
    pub success_function: Option<String>,
}

impl PureDaisyRecipe {
    pub fn parse(id: &str, v: &Value) -> Option<PureDaisyRecipe> {
        let kind = v.get("type").and_then(Value::as_str)?;
        let input = StateIngredient::parse(v.get("input")?)?;
        let time = match v.get("time") {
            None => crate::consts::PURE_DAISY_DEFAULT_TIME,
            Some(t) => i32::try_from(t.as_i64()?).ok()?,
        };
        // `Preconditions.checkArgument(time >= 0)`.
        if time < 0 {
            return None;
        }
        let (output, success_function) = match kind {
            "botania:pure_daisy" => (
                DaisyOutput::State(BlockState::parse(v.get("output")?)?),
                match v.get("success_function") {
                    None | Some(Value::Null) => None,
                    Some(f) => Some(f.as_str()?.to_owned()),
                },
            ),
            "botania:state_copying_pure_daisy" => (
                DaisyOutput::CopyProperties(v.get("output")?.as_str()?.to_owned()),
                None,
            ),
            _ => return None,
        };
        Some(PureDaisyRecipe {
            id: id.to_owned(),
            input,
            output,
            time,
            success_function,
        })
    }

    /// `PureDaisyRecipe.matches` (the input matches and the output isn't already there) and
    /// `StateCopyingPureDaisyRecipe.matches` (the input matches and copying its properties onto
    /// the output block changes it).
    pub fn matches(&self, state: &BlockState, in_tag: InTag) -> bool {
        if !self.input.test(state, in_tag) {
            return false;
        }
        match &self.output {
            DaisyOutput::State(out) => !out.written_matches(state),
            DaisyOutput::CopyProperties(block) => &state.name != block,
        }
    }

    /// The state the converted block becomes (`set`): the output state, or the output block
    /// with `state`'s properties (`Block.withPropertiesOf`, which keeps those the block has: the
    /// game drops the ones it doesn't).
    pub fn result(&self, state: &BlockState) -> BlockState {
        match &self.output {
            DaisyOutput::State(out) => out.clone(),
            DaisyOutput::CopyProperties(block) => BlockState {
                name: block.clone(),
                properties: state.properties.clone(),
            },
        }
    }
}

/// A `botania:mana_infusion` recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct InfusionRecipe {
    pub id: String,
    pub input: Ingredient,
    pub output: Output,
    pub mana: i32,
    pub group: String,
    pub catalyst: Option<StateIngredient>,
}

impl InfusionRecipe {
    pub fn parse(id: &str, v: &Value) -> Option<InfusionRecipe> {
        if v.get("type").and_then(Value::as_str) != Some("botania:mana_infusion") {
            return None;
        }
        let mana = i32::try_from(v.get("mana")?.as_i64()?).ok()?;
        // "Mana cost must be positive" and "at most a pool" (1_000_001).
        if mana <= 0 || mana > 1_000_001 {
            return None;
        }
        let catalyst = match v.get("catalyst") {
            None => None,
            // "Legacy mana infusion catalyst syntax used": a catalyst needs a `type`.
            Some(c) if c.get("type").is_none() => return None,
            Some(c) => Some(StateIngredient::parse(c)?),
        };
        Some(InfusionRecipe {
            id: id.to_owned(),
            input: Ingredient::parse(v.get("input")?)?,
            output: Output::parse(v.get("output")?)?,
            mana,
            group: v
                .get("group")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            catalyst,
        })
    }
}

/// `ManaPoolBlockEntity.getMatchingRecipe`: among the recipes the item matches, the first whose
/// catalyst is the block under the pool, else the first with no catalyst. A recipe whose
/// catalyst is another block is never taken.
pub fn infusion_pick<'r>(
    recipes: &'r [InfusionRecipe],
    item: &str,
    below: &BlockState,
    item_tag: InTag,
    block_tag: InTag,
) -> Option<&'r InfusionRecipe> {
    let mut plain = None;
    for r in recipes.iter().filter(|r| r.input.test(item, item_tag)) {
        match &r.catalyst {
            None => {
                plain.get_or_insert(r);
            }
            Some(c) if c.test(below, block_tag) => return Some(r),
            Some(_) => {}
        }
    }
    plain
}

/// A vanilla 1.20 `crafting_shaped` or `crafting_shapeless` recipe in 26.3's form: ingredients
/// as ids and `#tags`, the result as `{"id", "count"}`. A recipe already in 26.3's form comes
/// back the same. None for other types, and for an ingredient 26.3 can't write.
pub fn upgrade_crafting(v: &Value) -> Option<Value> {
    let kind = v.get("type").and_then(Value::as_str)?;
    let mut out: Vec<(String, Value)> = Vec::new();
    out.push(("type".to_owned(), Value::str(kind)));
    for field in ["category", "group"] {
        if let Some(x) = v.get(field) {
            out.push((field.to_owned(), x.clone()));
        }
    }
    match kind {
        "minecraft:crafting_shaped" => {
            let mut key = Vec::new();
            for (k, ing) in v.get("key")?.as_object()? {
                key.push((k.clone(), Ingredient::parse(ing)?.to_json_26()?));
            }
            out.push(("key".to_owned(), Value::Object(key)));
            let pattern = v.get("pattern")?;
            pattern
                .as_array()?
                .iter()
                .try_for_each(|row| row.as_str().map(drop))?;
            out.push(("pattern".to_owned(), pattern.clone()));
        }
        "minecraft:crafting_shapeless" => {
            let list: Vec<Value> = v
                .get("ingredients")?
                .as_array()?
                .iter()
                .map(|i| Ingredient::parse(i)?.to_json_26())
                .collect::<Option<_>>()?;
            out.push(("ingredients".to_owned(), Value::Array(list)));
        }
        _ => return None,
    }
    out.push((
        "result".to_owned(),
        Output::parse(v.get("result")?)?.to_json_26(),
    ));
    if let Some(n) = v.get("show_notification") {
        out.push(("show_notification".to_owned(), n.clone()));
    }
    Some(Value::Object(out))
}

/// The vanilla recipe inside one of Botania's wrapper recipes, in 26.3's form:
/// - `botania:gog_alternation`: `base` (Botania picks `gog` only with Garden of Glass);
/// - `botania:nbt_output_wrapper`: `recipe`, its result carrying `nbt` as custom data;
/// - `botania:twig_wand`: the shaped recipe itself (the wand's colours stay the default: the
///   crafting table can't read the petals as `WandOfTheForestRecipe.assemble` does).
///
/// A vanilla crafting recipe in 1.20's form is upgraded too. None for anything else.
pub fn unwrap(v: &Value) -> Option<Value> {
    match v.get("type").and_then(Value::as_str)? {
        "botania:gog_alternation" => {
            let base = v.get("base")?;
            // `GogAlternationRecipe`: "Subrecipes must have matching types".
            let gog = v.get("gog")?;
            if gog.get("type") != base.get("type") {
                return None;
            }
            upgrade_crafting(base)
        }
        "botania:nbt_output_wrapper" => {
            let nbt = match v.get("nbt")? {
                Value::String(s) => s.clone(),
                other => other.to_json(),
            };
            let mut inner = v.get("recipe")?.clone();
            if let Value::Object(o) = &mut inner
                && let Some((_, result)) = o.iter_mut().find(|(k, _)| k == "result")
                && let Value::Object(r) = result
            {
                r.retain(|(k, _)| k != "nbt");
                r.push(("nbt".to_owned(), Value::String(nbt)));
            }
            upgrade_crafting(&inner)
        }
        "botania:twig_wand" => {
            let mut inner = v.clone();
            if let Value::Object(o) = &mut inner {
                for (k, x) in o.iter_mut() {
                    if k == "type" {
                        *x = Value::str("minecraft:crafting_shaped");
                    }
                }
            }
            upgrade_crafting(&inner)
        }
        "minecraft:crafting_shaped" | "minecraft:crafting_shapeless" => upgrade_crafting(v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    // Item tags as Botania 1.20's data has them for these tests.
    fn item_tags(tag: &str, item: &str) -> bool {
        match tag {
            "botania:petals/white" => item == "botania:white_petal",
            "botania:petals/brown" => item == "botania:brown_petal",
            "botania:petals/red" => item == "botania:red_petal",
            "botania:petals/light_gray" => item == "botania:light_gray_petal",
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

    fn block_tags(tag: &str, block: &str) -> bool {
        tag == "minecraft:logs" && (block.ends_with("_log") || block.ends_with("_wood"))
    }

    // botania/recipe/petal_apothecary/endoflame.json from Botania's jar.
    const ENDOFLAME: &str = r#"{"type":"botania:petal_apothecary","ingredients":[{"tag":"botania:petals/brown"},{"tag":"botania:petals/brown"},{"tag":"botania:petals/red"},{"tag":"botania:petals/light_gray"}],"output":{"item":"botania:endoflame"},"reagent":{"tag":"botania:seed_apothecary_reagent"}}"#;

    #[test]
    fn petal_recipe_reads_and_matches_as_a_multiset() {
        let r = PetalRecipe::parse(
            "botania:petal_apothecary/endoflame",
            &parse(ENDOFLAME).unwrap(),
        )
        .unwrap();
        assert_eq!(r.inputs.len(), 4);
        assert_eq!(r.output.item, "botania:endoflame");
        assert!(r.reagent.test("minecraft:melon_seeds", &item_tags));
        let (b, rd, g) = (
            Some("botania:brown_petal"),
            Some("botania:red_petal"),
            Some("botania:light_gray_petal"),
        );
        // Any order of the multiset.
        assert!(r.matches(&[b, rd, b, g], &item_tags));
        assert!(
            r.matches(&[g, b, b, rd, None, b], &item_tags),
            "reading stops at the first empty slot"
        );
        // One missing, one extra, one wrong.
        assert!(!r.matches(&[b, rd, g], &item_tags));
        assert!(!r.matches(&[b, rd, b, g, b], &item_tags));
        assert!(!r.matches(&[b, rd, rd, g], &item_tags));
        assert!(!r.matches(&[], &item_tags));
    }

    #[test]
    fn petal_matching_is_greedy_like_botania() {
        // Ingredients [white or red] then [white]: Botania gives each item the first missing
        // ingredient that takes it, so (white, red) fails although (red, white) succeeds.
        let r = PetalRecipe {
            id: "t".into(),
            inputs: vec![
                Ingredient(vec![
                    Choice::Item("botania:white_petal".into()),
                    Choice::Item("botania:red_petal".into()),
                ]),
                Ingredient(vec![Choice::Item("botania:white_petal".into())]),
            ],
            output: Output::parse(&Value::str("x:y")).unwrap(),
            reagent: Ingredient::default(),
        };
        assert!(!r.matches(
            &[Some("botania:white_petal"), Some("botania:red_petal")],
            &item_tags
        ));
        assert!(r.matches(
            &[Some("botania:red_petal"), Some("botania:white_petal")],
            &item_tags
        ));
    }

    #[test]
    fn pure_daisy_recipes_read_with_their_defaults() {
        // pure_daisy/livingrock.json, livingwood.json and end_stone_to_cobbled_deepslate.json.
        let rock = PureDaisyRecipe::parse(
            "botania:pure_daisy/livingrock",
            &parse(r#"{"type":"botania:pure_daisy","input":{"type":"block","block":"minecraft:stone"},"output":{"name":"botania:livingrock"}}"#).unwrap(),
        )
        .unwrap();
        assert_eq!(rock.time, 150);
        assert_eq!(rock.success_function, None);
        let stone = BlockState::new("minecraft:stone", &[]);
        assert!(rock.matches(&stone, &block_tags));
        assert_eq!(
            rock.result(&stone),
            BlockState::new("botania:livingrock", &[])
        );
        assert!(!rock.matches(&BlockState::new("botania:livingrock", &[]), &block_tags));

        let wood = PureDaisyRecipe::parse(
            "botania:pure_daisy/livingwood",
            &parse(r#"{"type":"botania:state_copying_pure_daisy","input":{"type":"tag","tag":"minecraft:logs"},"output":"botania:livingwood_log"}"#).unwrap(),
        )
        .unwrap();
        let oak = BlockState::new("minecraft:oak_log", &[("axis", "z")]);
        assert!(wood.matches(&oak, &block_tags));
        assert_eq!(
            wood.result(&oak),
            BlockState::new("botania:livingwood_log", &[("axis", "z")])
        );
        // Livingwood is a log too: converting it again would change nothing.
        let living = BlockState::new("botania:livingwood_log", &[("axis", "y")]);
        assert!(!wood.matches(&living, &block_tags));

        let ender = PureDaisyRecipe::parse(
            "botania:pure_daisy/end_stone_to_cobbled_deepslate",
            &parse(r#"{"type":"botania:pure_daisy","input":{"type":"block","block":"minecraft:end_stone"},"output":{"name":"minecraft:cobbled_deepslate"},"success_function":"botania:ender_air_release","time":75}"#).unwrap(),
        )
        .unwrap();
        assert_eq!(ender.time, 75);
        assert_eq!(
            ender.success_function.as_deref(),
            Some("botania:ender_air_release")
        );
        // A negative time and an unknown state ingredient type are refused.
        assert!(PureDaisyRecipe::parse("x", &parse(r#"{"type":"botania:pure_daisy","input":{"type":"block","block":"a:b"},"output":{"name":"a:c"},"time":-1}"#).unwrap()).is_none());
        assert!(PureDaisyRecipe::parse("x", &parse(r#"{"type":"botania:pure_daisy","input":{"type":"weird"},"output":{"name":"a:c"}}"#).unwrap()).is_none());
    }

    #[test]
    fn state_ingredients_test_like_botania() {
        let lit = BlockState::new("minecraft:furnace", &[("facing", "north"), ("lit", "true")]);
        let unlit = BlockState::new(
            "minecraft:furnace",
            &[("facing", "north"), ("lit", "false")],
        );
        let state = StateIngredient::parse(
            &parse(r#"{"type":"state","name":"minecraft:furnace","properties":{"lit":"true"}}"#)
                .unwrap(),
        )
        .unwrap();
        assert!(state.test(&lit, &block_tags));
        assert!(!state.test(&unlit, &block_tags));
        let excl = StateIngredient::parse(
            &parse(r#"{"type":"tag_excluding","tag":"minecraft:logs","exclude":[{"type":"blocks","blocks":["minecraft:oak_log"]}]}"#).unwrap(),
        )
        .unwrap();
        assert!(excl.test(&BlockState::new("minecraft:birch_log", &[]), &block_tags));
        assert!(!excl.test(&BlockState::new("minecraft:oak_log", &[]), &block_tags));
        let compound = StateIngredient::parse(
            &parse(r#"{"type":"compound","ingredients":[{"type":"block","block":"minecraft:ice"},{"type":"tag","tag":"minecraft:logs"}]}"#).unwrap(),
        )
        .unwrap();
        assert!(compound.test(&BlockState::new("minecraft:ice", &[]), &block_tags));
        assert!(compound.test(&BlockState::new("minecraft:oak_wood", &[]), &block_tags));
        assert!(!compound.test(&BlockState::new("minecraft:stone", &[]), &block_tags));
    }

    #[test]
    fn infusion_reads_and_prefers_a_matching_catalyst() {
        // mana_infusion/manasteel.json, and the alchemy catalyst's stone → andesite.
        let manasteel = InfusionRecipe::parse(
            "botania:mana_infusion/manasteel",
            &parse(r#"{"type":"botania:mana_infusion","input":{"item":"minecraft:iron_ingot"},"mana":3000,"output":{"item":"botania:manasteel_ingot"}}"#).unwrap(),
        )
        .unwrap();
        assert_eq!(manasteel.mana, 3000);
        assert_eq!(manasteel.output.item, "botania:manasteel_ingot");
        let stone_plain = InfusionRecipe {
            id: "plain".into(),
            input: Ingredient(vec![Choice::Item("minecraft:stone".into())]),
            output: Output::parse(&Value::str("minecraft:cobblestone")).unwrap(),
            mana: 100,
            group: String::new(),
            catalyst: None,
        };
        let stone_alchemy = InfusionRecipe {
            id: "alchemy".into(),
            input: Ingredient(vec![Choice::Item("minecraft:stone".into())]),
            output: Output::parse(&Value::str("minecraft:andesite")).unwrap(),
            mana: 200,
            group: String::new(),
            catalyst: Some(StateIngredient::Block("botania:alchemy_catalyst".into())),
        };
        let stone_conj = InfusionRecipe {
            catalyst: Some(StateIngredient::Block(
                "botania:conjuration_catalyst".into(),
            )),
            id: "conjuration".into(),
            ..stone_alchemy.clone()
        };
        // The plain recipe is first in the list, the catalyst one still wins over it.
        let recipes = vec![manasteel, stone_plain, stone_conj, stone_alchemy];
        let air = BlockState::new("minecraft:air", &[]);
        let alchemy = BlockState::new("botania:alchemy_catalyst", &[]);
        let pick = |item, below| {
            infusion_pick(&recipes, item, below, &item_tags, &block_tags).map(|r| r.id.as_str())
        };
        assert_eq!(pick("minecraft:stone", &air), Some("plain"));
        assert_eq!(pick("minecraft:stone", &alchemy), Some("alchemy"));
        assert_eq!(
            pick("minecraft:iron_ingot", &alchemy),
            Some("botania:mana_infusion/manasteel")
        );
        assert_eq!(pick("minecraft:dirt", &air), None);
        // Zero mana, too much mana, and the legacy catalyst syntax are refused.
        for bad in [
            r#"{"type":"botania:mana_infusion","input":{"item":"a:b"},"mana":0,"output":{"item":"a:c"}}"#,
            r#"{"type":"botania:mana_infusion","input":{"item":"a:b"},"mana":1000002,"output":{"item":"a:c"}}"#,
            r#"{"type":"botania:mana_infusion","input":{"item":"a:b"},"mana":5,"output":{"item":"a:c"},"catalyst":{"name":"a:d"}}"#,
        ] {
            assert!(
                InfusionRecipe::parse("x", &parse(bad).unwrap()).is_none(),
                "{bad}"
            );
        }
    }

    // botania/recipe/mana_spreader.json from Botania's jar.
    const SPREADER: &str = r#"{"type":"botania:gog_alternation","base":{"type":"minecraft:crafting_shaped","category":"redstone","group":"botania:spreader","key":{"C":{"item":"minecraft:copper_ingot"},"P":{"tag":"botania:petals"},"W":{"tag":"botania:livingwood_logs"}},"pattern":["WWW","CP ","WWW"],"result":{"item":"botania:mana_spreader"},"show_notification":true},"gog":{"type":"minecraft:crafting_shaped","category":"redstone","group":"botania:spreader","key":{"P":{"tag":"botania:petals"},"W":{"tag":"botania:livingwood_logs"}},"pattern":["WWW","WP ","WWW"],"result":{"item":"botania:mana_spreader"},"show_notification":true}}"#;

    #[test]
    fn wrappers_unwrap_into_26_3_crafting() {
        let spreader = unwrap(&parse(SPREADER).unwrap()).unwrap();
        assert_eq!(
            spreader.to_json(),
            r##"{"type":"minecraft:crafting_shaped","category":"redstone","group":"botania:spreader","key":{"C":"minecraft:copper_ingot","P":"#botania:petals","W":"#botania:livingwood_logs"},"pattern":["WWW","CP ","WWW"],"result":{"id":"botania:mana_spreader"},"show_notification":true}"##
        );
        // twig_wand.json.
        let wand = unwrap(&parse(r#"{"type":"botania:twig_wand","category":"equipment","group":"botania:twig_wand","key":{"P":{"tag":"botania:petals"},"S":{"item":"botania:livingwood_twig"}},"pattern":[" PS"," SP","S  "],"result":{"item":"botania:twig_wand"},"show_notification":true}"#).unwrap()).unwrap();
        assert_eq!(
            wand.to_json(),
            r##"{"type":"minecraft:crafting_shaped","category":"equipment","group":"botania:twig_wand","key":{"P":"#botania:petals","S":"botania:livingwood_twig"},"pattern":[" PS"," SP","S  "],"result":{"id":"botania:twig_wand"},"show_notification":true}"##
        );
        // flighttiara_1.json: a shapeless recipe whose result keeps its NBT as custom data.
        let tiara = unwrap(&parse(r#"{"type":"botania:nbt_output_wrapper","nbt":"{variant:1}","recipe":{"type":"minecraft:crafting_shapeless","category":"equipment","group":"botania:flight_tiara_wings","ingredients":[{"item":"botania:flight_tiara"},{"item":"minecraft:quartz"}],"result":{"item":"botania:flight_tiara"}}}"#).unwrap()).unwrap();
        assert_eq!(
            tiara.to_json(),
            r#"{"type":"minecraft:crafting_shapeless","category":"equipment","group":"botania:flight_tiara_wings","ingredients":["botania:flight_tiara","minecraft:quartz"],"result":{"id":"botania:flight_tiara","components":{"minecraft:custom_data":"{variant:1}"}}}"#
        );
        // A vanilla recipe with a count, and one already in 26.3's form, come out in 26.3's.
        let planks = unwrap(&parse(r#"{"type":"minecraft:crafting_shapeless","category":"building","group":"planks","ingredients":[{"tag":"botania:livingwood_logs"}],"result":{"count":4,"item":"botania:livingwood_planks"}}"#).unwrap()).unwrap();
        assert_eq!(
            planks.to_json(),
            r##"{"type":"minecraft:crafting_shapeless","category":"building","group":"planks","ingredients":["#botania:livingwood_logs"],"result":{"id":"botania:livingwood_planks","count":4}}"##
        );
        assert_eq!(unwrap(&planks), Some(planks.clone()));
        // Mismatched sub-recipes, a mixed tag list and unknown wrappers are refused.
        assert!(unwrap(&parse(r#"{"type":"botania:gog_alternation","base":{"type":"minecraft:crafting_shaped"},"gog":{"type":"minecraft:crafting_shapeless"}}"#).unwrap()).is_none());
        assert!(unwrap(&parse(r#"{"type":"minecraft:crafting_shapeless","ingredients":[[{"tag":"a:b"},{"item":"a:c"}]],"result":{"item":"a:d"}}"#).unwrap()).is_none());
        assert!(unwrap(&parse(r#"{"type":"botania:armor_upgrade"}"#).unwrap()).is_none());
    }
}
