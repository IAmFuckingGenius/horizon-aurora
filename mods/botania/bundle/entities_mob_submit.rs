// A pixie is drawn with `PixieModel`.
fn botania_entities_mob_submit(
    game: &crate::game::Game<'_>,
    mob: &crate::game::native_mob_draw::NativeMobRender<'_>,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
    drawn: &mut bool,
) {
    crate::game::botania_entities::mob_submit(game, mob, draw, drawn);
}
