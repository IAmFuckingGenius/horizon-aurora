// The Lexica Botania opened: its advancement and sound.
fn botania_tool_uses_use_item(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    claimed: &mut bool,
) {
    crate::game::botania_tool_uses::use_item(game, stack, claimed);
}
