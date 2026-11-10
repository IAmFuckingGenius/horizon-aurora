// The Terra Shatterer's area and the elementium shovel's column.
fn botania_weapons_destroy_block(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    block_state: horizon_world::registry::BlockStateId,
    canceled: bool,
) {
    crate::game::botania_weapons::destroy_block(game, pos, block_state, canceled);
}
