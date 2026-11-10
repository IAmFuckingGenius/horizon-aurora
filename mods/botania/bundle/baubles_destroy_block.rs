// A player breaks a block: the Ring of Loki's cursors with the Terra Shatterer or Truncator.
fn botania_baubles_destroy_block(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    block_state: horizon_world::registry::BlockStateId,
    canceled: bool,
) {
    crate::game::botania_baubles::destroy_block(game, pos, block_state, canceled);
}
