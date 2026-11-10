// The world frame's rims: the Ring of Loki's cursors.
fn botania_baubles_visuals(
    game: &crate::game::Game<'_>,
    rims: &mut Vec<horizon_client::mod_api::NativeBlockRim>,
) {
    crate::game::botania_baubles::visuals(game, rims);
}
