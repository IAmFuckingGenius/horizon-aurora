// A mana item's tooltip makes room for its mana bar.
fn botania_mana_component(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    size: &mut Option<[f32; 2]>,
) {
    crate::game::botania_mana::tooltip_component(game, stack, size);
}
