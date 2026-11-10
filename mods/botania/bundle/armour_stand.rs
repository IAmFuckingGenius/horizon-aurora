// Botania's armour models drawn on an armour stand, over its model and layers.
fn botania_armour_stand(
    game: &crate::game::Game<'_>,
    stand: &crate::game::native_mob_draw::NativeArmorStandRender<'_>,
    parts: &crate::game::native_mob_draw::NativeModelParts<'_>,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    crate::game::botania_armour::stand_layer(game, stand, parts, draw);
}
