// A right click on a Petal Apothecary (`PetalApothecaryBlock.use`).
fn botania_right_click(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::botania::right_click(game, pos, canceled);
}
