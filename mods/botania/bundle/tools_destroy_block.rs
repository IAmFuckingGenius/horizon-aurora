// The Terra Truncator starts a felling from the block mined.
fn botania_tools_destroy_block(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    block_state: horizon_world::registry::BlockStateId,
    canceled: bool,
) {
    crate::game::botania_tools::destroy_block(game, pos, block_state, canceled);
}
