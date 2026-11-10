// A client tick: dodges, air jumps, the sashes.
fn botania_baubles_tick(game: &mut crate::game::Game<'_>) {
    crate::game::botania_baubles::client_tick(game);
}
