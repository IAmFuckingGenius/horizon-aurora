// After a level's tick: fellings, shearings and the break events of area tools.
fn botania_tools_level_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::botania_tools::level_tick(game, dimension);
}
