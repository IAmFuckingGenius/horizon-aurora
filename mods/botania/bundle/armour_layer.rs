// Botania's armour models drawn over a mob's or player's vanilla model and layers.
fn botania_armour_layer(
    game: &crate::game::Game<'_>,
    mob: &crate::game::native_mob_draw::NativeMobRender<'_>,
    who: Option<&crate::game::native_mob_draw::NativePlayerRender<'_>>,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    crate::game::botania_armour::layer(game, mob, who, draw);
}
