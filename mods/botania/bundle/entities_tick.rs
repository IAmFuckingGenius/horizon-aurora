// Each game tick: the window's pixies get their box, and old hurts are forgotten.
fn botania_entities_tick(game: &mut crate::game::Game<'_>) {
    crate::game::botania_entities::tick(game);
}
