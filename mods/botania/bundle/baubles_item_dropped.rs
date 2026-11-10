// A player threw an item: the Ring of Magnetization rests.
fn botania_baubles_item_dropped(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::botania_baubles::item_dropped(game, player);
}
