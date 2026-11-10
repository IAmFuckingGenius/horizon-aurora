// A player's blow: a pixie its summoner hits takes no damage.
fn botania_entities_attack(
    game: &mut crate::game::Game<'_>,
    entity: i32,
    kind: &str,
    canceled: &mut bool,
) {
    crate::game::botania_entities::attack_entity(game, entity, kind, canceled);
}
