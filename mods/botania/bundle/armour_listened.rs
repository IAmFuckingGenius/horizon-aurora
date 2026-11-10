// Botania's armour models: the game poses mobs and players for the armour layer only while
// somebody wears a piece.
fn botania_armour_listened(game: &crate::game::Game<'_>, listened: &mut bool) {
    crate::game::botania_armour::listened(game, listened);
}
