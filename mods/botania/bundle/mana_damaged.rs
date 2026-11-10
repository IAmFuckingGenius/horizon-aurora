// A server player's item wears: Botania's tools, bows and armour pay with mana instead.
fn botania_mana_damaged(
    game: &mut crate::game::Game<'_>,
    player: u128,
    item: &str,
    amount: &mut i32,
    cancel: &mut bool,
) {
    crate::game::botania_mana::item_damaged(game, player, item, amount, cancel);
}
