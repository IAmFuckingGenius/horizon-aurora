// Botania's blocks get their block entities (`EntityBlock.newBlockEntity`).
fn block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::botania::block_entity(block, entity);
}
