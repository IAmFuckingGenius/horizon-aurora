// The Wand of the Forest used in the air: a sneak-click switches its mode.
fn botania_use_item(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    claimed: &mut bool,
) {
    crate::game::botania::use_item(game, stack, claimed);
}
