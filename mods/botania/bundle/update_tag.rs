// What clients get of a Botania block entity.
fn botania_update_tag(
    entity: &horizon_client::containers::ModBlockEntity,
    tag: &mut Option<horizon_world::nbt::Compound>,
) {
    crate::game::botania::update_tag(entity, tag);
}
