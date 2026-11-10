// Comparators read mana pools.
fn has_output(
    level: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    output: &mut bool,
) {
    crate::game::botania::has_output(level, state, output);
}
