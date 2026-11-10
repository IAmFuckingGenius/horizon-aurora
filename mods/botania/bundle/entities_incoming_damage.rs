// Damage a living entity takes: a player hurt by a mob, a pixie hurt by its summoner.
fn botania_entities_incoming_damage(
    entity: i32,
    kind: &str,
    damage_type: &str,
    causing: i32,
    canceled: &mut bool,
) {
    crate::game::botania_entities::incoming_damage(entity, kind, damage_type, causing, canceled);
}
