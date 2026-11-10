// The start of an entity's tick: a pixie's flight and sparkles (`customServerAiStep`).
fn botania_entities_tick_pre(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    entity: u32,
    kind: &str,
    canceled: bool,
) {
    crate::game::botania_entities::tick_pre(game, dimension, entity, kind, canceled);
}
