// An apothecary's item slots.
fn botania_container(
    block: &str,
    container: &mut Option<horizon_client::containers::ModContainer>,
) {
    crate::game::botania::container(block, container);
}
