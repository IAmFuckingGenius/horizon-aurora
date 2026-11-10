// After a level's tick: the Terra Blade's beams and the falling stars fly.
fn botania_weapons_level_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::botania_weapons::level_tick(game, dimension);
}
