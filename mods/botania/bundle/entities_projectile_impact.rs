// A projectile hits: an Ender Air Bottle's splash, or a pixie's summoner shooting it.
fn botania_entities_projectile_impact(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    projectile: u32,
    kind: &str,
    owner: u32,
    entity: Option<u32>,
    entity_kind: &str,
    block: [i32; 3],
    at: [f64; 3],
    canceled: &mut bool,
) {
    crate::game::botania_entities::projectile_impact(
        game,
        dimension,
        projectile,
        kind,
        owner,
        entity,
        entity_kind,
        block,
        at,
        canceled,
    );
}
