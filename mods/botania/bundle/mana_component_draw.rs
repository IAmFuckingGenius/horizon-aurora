// The mana bar in a mana item's tooltip.
fn botania_mana_component_draw(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
    size: [f32; 2],
) {
    crate::game::botania_mana::tooltip_component_draw(game, stack, batch, origin, size);
}
