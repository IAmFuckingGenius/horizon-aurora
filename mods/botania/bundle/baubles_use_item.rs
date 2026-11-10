// An item used in the air: the Stone of Temperance turns on or off.
fn botania_baubles_use_item(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    claimed: &mut bool,
) {
    crate::game::botania_baubles::use_item(game, stack, claimed);
}
