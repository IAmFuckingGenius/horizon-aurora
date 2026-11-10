// The manasteel and elementium pickaxes and axes, the vitreous pickaxe and the Terra tools placing a
// torch or sapling from the inventory.
fn botania_tool_uses_use_on(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    crate::game::botania_tool_uses::use_on(game, stack, pos, face, claimed);
}
