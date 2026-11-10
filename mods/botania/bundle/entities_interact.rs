// A right click on a pixie opens no trades.
fn botania_entities_interact(kind: &str, claimed: &mut bool) {
    crate::game::botania_entities::interact_entity(kind, claimed);
}
