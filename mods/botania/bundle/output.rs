// A mana pool's comparator level.
fn output(
    level: &crate::game::GameLevel<'_>,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    output: &mut i32,
) {
    crate::game::botania::output(level, pos, state, output);
}
