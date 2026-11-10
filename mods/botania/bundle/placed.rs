// A placed spreader faces the player who placed it.
fn botania_placed(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    state: horizon_world::registry::BlockStateId,
) {
    crate::game::botania::placed(game, pos, state);
}
