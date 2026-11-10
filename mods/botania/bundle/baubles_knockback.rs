// A blast pushes an entity: the Tectonic Girdle.
fn botania_baubles_knockback(
    game: &mut crate::game::Game<'_>,
    entity: Option<u32>,
    knockback_velocity: &mut [f64; 3],
) {
    crate::game::botania_baubles::explosion_knockback(game, entity, knockback_velocity);
}
