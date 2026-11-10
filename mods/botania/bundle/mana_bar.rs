// The bar under an item: a mana item's mana.
fn botania_mana_bar(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    bar: &mut Option<(i32, u32)>,
) {
    crate::game::botania_mana::item_bar(game, stack, bar);
}
