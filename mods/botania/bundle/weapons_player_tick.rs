// The end of a player's tick: Thundercaller hits, the swing's beam or star.
fn botania_weapons_player_tick(game: &mut crate::game::Game<'_>) {
    crate::game::botania_weapons::player_tick(game);
}
