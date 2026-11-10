// The window's game: the server's Botania particles (`botania:fx`) become wisps and sparkles.
fn botania_fx_from_server(game: &mut crate::game::Game<'_>, channel: &str, data: &[u8]) {
    crate::game::botania_particles::from_server(game, channel, data);
}
