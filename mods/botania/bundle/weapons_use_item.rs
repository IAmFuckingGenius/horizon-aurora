// A sneaking right click turns the Terra Shatterer's area on or off.
fn botania_weapons_use_item(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    claimed: &mut bool,
) {
    crate::game::botania_weapons::use_item(game, stack, claimed);
}
