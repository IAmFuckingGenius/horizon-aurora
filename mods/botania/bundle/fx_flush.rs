// After a level's tick, and after the others' (its id sorts last): the Botania particles the
// level's tick queued go to the players near them.
fn botania_fx_flush(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::botania_particles::flush(game, dimension);
}
