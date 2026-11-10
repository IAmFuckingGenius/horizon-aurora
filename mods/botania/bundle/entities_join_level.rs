// An entity joins a level: Ender Air clouds and thrown bottles are tracked, stray pixies noted.
fn botania_entities_join_level(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    entity: u32,
    kind: &str,
    canceled: bool,
) {
    crate::game::botania_entities::join_level(game, dimension, entity, kind, canceled);
}
