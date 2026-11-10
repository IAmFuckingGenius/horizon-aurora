// The Terra Blade's beam on an attack at full strength.
fn botania_weapons_attack(game: &mut crate::game::Game<'_>, canceled: bool) {
    crate::game::botania_weapons::attack_entity(game, canceled);
}
