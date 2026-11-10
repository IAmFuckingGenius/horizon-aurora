// A tiny potato petted with an empty hand.
fn botania_tool_uses_right_click(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    canceled: &mut bool,
) {
    crate::game::botania_tool_uses::right_click(game, pos, canceled);
}
