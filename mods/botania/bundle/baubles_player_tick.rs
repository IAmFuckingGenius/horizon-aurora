// The end of a player's tick: their trinkets (`onWornTick`).
fn botania_baubles_player_tick(game: &mut crate::game::Game<'_>) {
    crate::game::botania_baubles::player_tick(game);
}
