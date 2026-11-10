// Botania's armour models drawn on the player the inventory shows, over its model and layers.
fn botania_armour_preview(
    game: &crate::game::Game<'_>,
    parts: &crate::game::native_mob_draw::NativeModelParts<'_>,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    crate::game::botania_armour::preview_layer(game, parts, draw);
}
