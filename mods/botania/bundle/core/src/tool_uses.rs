//! What Botania's pickaxes and axes place from their user's inventory (`ManasteelPickaxeItem.useOn`,
//! `ManasteelAxeItem.useOn`), the advancements Botania's code grants (`PlayerHelper.grantCriterion`)
//! and the Lexica Botania's use.

use crate::{tools, weapons};

/// What a tool places from the inventory when used on a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placer {
    /// `ManasteelPickaxeItem.TORCH_PATTERN`: the manasteel, elementium and vitreous pickaxes and
    /// the Terra Shatterer.
    Torch,
    /// `ManasteelAxeItem.SAPLING_PATTERN`: the manasteel and elementium axes and the Terra
    /// Truncator.
    Sapling,
}

pub const MANASTEEL_PICK: &str = "botania:manasteel_pick";
pub const VITREOUS_PICK: &str = "botania:glass_pickaxe";
pub const MANASTEEL_AXE: &str = "botania:manasteel_axe";
pub const LEXICON: &str = "botania:lexicon";
pub const TINY_POTATO: &str = "botania:tiny_potato";

/// `ManasteelPickaxeItem.TIME`: the ticks a pickaxe waits after placing a torch.
pub const TORCH_COOLDOWN: i32 = 5;

/// The rank the Terra Shatterer's area grants `challenge/rank_ss_pick` at (`origLevel == 5`).
pub const RANK_SS: i32 = 5;
/// The advancements Botania grants from its code, by their `code_triggered` criterion.
pub const RANK_SS_PICK: &str = "botania:challenge/rank_ss_pick";
pub const TINY_POTATO_PET: &str = "botania:main/tiny_potato_pet";
pub const CODE_TRIGGERED: &str = "code_triggered";
/// `UseItemSuccessTrigger`'s id, fired by the Lexica Botania's use.
pub const USE_ITEM_SUCCESS: &str = "botania:use_item_success";

impl Placer {
    /// The tool's `useOn` placement, if it has one.
    pub fn of(item: &str) -> Option<Placer> {
        match item {
            MANASTEEL_PICK | weapons::ELEMENTIUM_PICK | VITREOUS_PICK | weapons::TERRA_PICK => {
                Some(Placer::Torch)
            }
            MANASTEEL_AXE | tools::ELEMENTIUM_AXE | tools::TERRA_AXE => Some(Placer::Sapling),
            _ => None,
        }
    }

    /// Whether an item whose description id is `description_id` is what this tool places.
    pub fn places(self, description_id: &str) -> bool {
        match self {
            Placer::Torch => names_word(description_id, "torch", "Torch"),
            Placer::Sapling => names_word(description_id, "sapling", "Sapling"),
        }
    }
}

/// `Item.getDescriptionId`: a block item's is its block's (`block.minecraft.torch`), any other
/// item's `item.<namespace>.<path>`, with the path's `/` as `.`.
pub fn description_id(item: &str, block: bool) -> String {
    let (namespace, path) = item.split_once(':').unwrap_or(("minecraft", item));
    let kind = if block { "block" } else { "item" };
    format!("{kind}.{namespace}.{}", path.replace('/', "."))
}

/// `Pattern.compile("(?:(?:(?:[A-Z-_.:]|^)<lower>)|(?:(?:[a-z-_.:]|^)<upper>))(?:[A-Z-_.:]|$)")
/// .matcher(text).find()`: `lower` after an upper case letter, `-`, `_`, `.`, `:` or the start,
/// or `upper` after a lower case one of those, and either followed by an upper case letter, one
/// of `-_.:` or the end.
pub fn names_word(text: &str, lower: &str, upper: &str) -> bool {
    let bytes = text.as_bytes();
    let mark = |b: u8| matches!(b, b'-' | b'_' | b'.' | b':');
    let after =
        |end: usize| end == bytes.len() || bytes[end].is_ascii_uppercase() || mark(bytes[end]);
    let found = |word: &str, before: &dyn Fn(u8) -> bool| {
        text.match_indices(word)
            .any(|(at, w)| (at == 0 || before(bytes[at - 1])) && after(at + w.len()))
    };
    found(lower, &|b| b.is_ascii_uppercase() || mark(b))
        || found(upper, &|b| b.is_ascii_lowercase() || mark(b))
}

/// `TerraShattererItem.useOn` before the pickaxe's: with something in the off hand the main hand's
/// use passes, and so does a sneaking use of a Shatterer with a rank (its sneaking `use` turns the
/// area on or off instead).
pub fn shatterer_places(main_hand: bool, off_hand_full: bool, sneaking: bool, level: i32) -> bool {
    !(main_hand && off_hand_full) && (!sneaking || level == 0)
}

/// `ManasteelPickaxeItem.useOn` and `ManasteelAxeItem.useOn` before the search: the main hand's
/// use passes while the off hand holds a block item, which places itself.
pub fn tool_places(main_hand: bool, off_hand_block: bool) -> bool {
    !(main_hand && off_hand_block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torches_and_saplings_are_found_by_their_description_ids() {
        let torch = |id: &str, block: bool| Placer::Torch.places(&description_id(id, block));
        // Every vanilla torch item, a block item named `block.minecraft.*torch`.
        assert!(torch("minecraft:torch", true));
        assert!(torch("minecraft:soul_torch", true));
        assert!(torch("minecraft:redstone_torch", true));
        assert!(torch("minecraft:copper_torch", true));
        // `torch` must end the word: the torchflower and its seeds aren't torches.
        assert!(!torch("minecraft:torchflower", true));
        assert!(!torch("minecraft:torchflower_seeds", false));
        assert!(!torch("minecraft:stone", true));
        let sapling = |id: &str| Placer::Sapling.places(&description_id(id, true));
        assert!(sapling("minecraft:oak_sapling"));
        assert!(sapling("minecraft:cherry_sapling"));
        // Not named a sapling: Botania's pattern leaves them.
        assert!(!sapling("minecraft:mangrove_propagule"));
        assert!(!sapling("minecraft:azalea"));
    }

    #[test]
    fn the_pattern_reads_camel_case_and_marks_as_java_does() {
        // `[A-Z-_.:]` before the lower case word, `[a-z-_.:]` before the capitalised one.
        assert!(names_word("tile.MyTorch", "torch", "Torch"));
        assert!(names_word("item.mod.BIGtorch", "torch", "Torch"));
        assert!(names_word("item.mod.Torch", "torch", "Torch"));
        assert!(names_word("item.mod.bigTorchHolder", "torch", "Torch"));
        assert!(names_word("torch", "torch", "Torch"));
        assert!(!names_word("item.mod.torches", "torch", "Torch"));
        assert!(!names_word("item.mod.blowtorch", "torch", "Torch"));
    }

    #[test]
    fn description_ids_follow_the_item_kind() {
        assert_eq!(
            description_id("minecraft:torch", true),
            "block.minecraft.torch"
        );
        assert_eq!(
            description_id("botania:lexicon", false),
            "item.botania.lexicon"
        );
        assert_eq!(description_id("ruby:gem/red", false), "item.ruby.gem.red");
    }

    #[test]
    fn which_tools_place_what() {
        assert_eq!(Placer::of("botania:manasteel_pick"), Some(Placer::Torch));
        assert_eq!(
            Placer::of("botania:elementium_pickaxe"),
            Some(Placer::Torch)
        );
        assert_eq!(Placer::of("botania:glass_pickaxe"), Some(Placer::Torch));
        assert_eq!(Placer::of("botania:terra_pick"), Some(Placer::Torch));
        assert_eq!(Placer::of("botania:manasteel_axe"), Some(Placer::Sapling));
        assert_eq!(Placer::of("botania:elementium_axe"), Some(Placer::Sapling));
        assert_eq!(Placer::of("botania:terra_axe"), Some(Placer::Sapling));
        assert_eq!(Placer::of("botania:manasteel_shovel"), None);
    }

    #[test]
    fn the_shatterer_passes_to_its_mode_switch() {
        // Not sneaking: torches, whatever the rank.
        assert!(shatterer_places(true, false, false, 5));
        // Sneaking with a rank: the area's switch; without one: torches.
        assert!(!shatterer_places(true, false, true, 1));
        assert!(shatterer_places(true, false, true, 0));
        // Anything in the off hand stops the main hand's use; the off hand's own use goes on.
        assert!(!shatterer_places(true, true, false, 0));
        assert!(shatterer_places(false, true, false, 0));
        assert!(!tool_places(true, true));
        assert!(tool_places(true, false));
        assert!(tool_places(false, true));
    }
}
