// Once a tick: the window's game keeps the pools' charging wisps flowing.
fn botania_fx_tick(game: &mut crate::game::Game<'_>) {
    crate::game::botania_particles::client_tick(game);
}
