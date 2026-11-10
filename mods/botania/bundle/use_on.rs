// The Wand of the Forest used on a block (`WandOfTheForestItem.useOn`).
fn botania_use_on(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    crate::game::botania::use_on(game, stack, pos, face, claimed);
}
