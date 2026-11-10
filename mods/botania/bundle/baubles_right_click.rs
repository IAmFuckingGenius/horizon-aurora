// A right click on a block: the Ring of Loki's selection and copies.
fn botania_baubles_right_click(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    crate::game::botania_baubles::right_click_block(game, pos, face, canceled);
}
