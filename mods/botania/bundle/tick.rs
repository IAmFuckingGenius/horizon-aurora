// A server tick of a Botania block entity.
fn tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
) {
    crate::game::botania::tick(game, pos, entity);
}
