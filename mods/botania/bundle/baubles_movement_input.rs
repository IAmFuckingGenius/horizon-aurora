// The movement keys of a client tick.
fn botania_baubles_movement_input(
    game: &crate::game::Game<'_>,
    forward: f32,
    strafe: f32,
    jump: bool,
) {
    crate::game::botania_baubles::movement_input(game, forward, strafe, jump);
}
