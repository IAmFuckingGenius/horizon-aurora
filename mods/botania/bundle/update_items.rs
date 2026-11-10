// An apothecary's items, for the client to draw (after `update_tag`).
fn botania_update_items(
    entity: &horizon_client::containers::ModBlockEntity,
    tag: &mut Option<horizon_world::nbt::Compound>,
) {
    crate::game::botania_render::update_tag(entity, tag);
}
