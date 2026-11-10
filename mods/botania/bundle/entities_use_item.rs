// A right click with an item: a glass bottle fills with Ender Air, an Ender Air Bottle is thrown.
fn botania_entities_use_item(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    claimed: &mut bool,
) {
    crate::game::botania_entities::use_item(game, stack, claimed);
}
