// A client's message: a dodge, an air jump.
fn botania_baubles_net(game: &mut crate::game::Game<'_>, player: u128, channel: &str) {
    crate::game::botania_baubles::net_from_client(game, player, channel);
}
