// An item's tooltip lines: the creative tablet's and the Terra Shatterer's.
fn botania_mana_tooltip(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    crate::game::botania_mana::tooltip(game, stack, lines);
}
