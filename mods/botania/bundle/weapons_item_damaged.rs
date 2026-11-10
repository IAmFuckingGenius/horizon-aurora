// A mana tool's wear is paid with mana.
fn botania_weapons_item_damaged(
    game: &mut crate::game::Game<'_>,
    player: u128,
    item: &str,
    amount: &mut i32,
    cancel: bool,
) {
    crate::game::botania_weapons::item_damaged(game, player, item, amount, cancel);
}
