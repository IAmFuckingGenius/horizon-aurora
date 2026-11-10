// After a level's tick: the heads of the players the elementium axe killed.
fn botania_tool_uses_level_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::botania_tool_uses::level_tick(game, dimension);
}
