// After a level's tick: comparators of pools whose mana level changed read them again.
fn level_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::botania::level_tick(game, dimension);
}
