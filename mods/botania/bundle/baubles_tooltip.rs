// An item's tooltip: the relics' owner, the Stone of Temperance, the cosmetics.
fn botania_baubles_tooltip(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    crate::game::botania_baubles::tooltip(game, stack, lines);
}
