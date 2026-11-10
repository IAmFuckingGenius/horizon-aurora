// The elementium hoe's till leaves wet farmland.
fn botania_tools_use_on(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    crate::game::botania_tools::use_on(game, stack, pos, face, claimed);
}
