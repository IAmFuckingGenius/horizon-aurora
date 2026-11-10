// After a level's tick: the clouds age, pixies strike, and what left the level is forgotten.
fn botania_entities_level_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::botania_entities::level_tick(game, dimension);
}
