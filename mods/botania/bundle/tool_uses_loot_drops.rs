// A player killed by a player: noted for the elementium axe's beheading.
fn botania_tool_uses_loot_drops(
    game: &mut crate::game::Game<'_>,
    context: &horizon_client::mod_api::NativeLootDrops<'_>,
    drops: &mut Vec<horizon_client::items::ItemStack>,
) {
    crate::game::botania_tool_uses::loot_drops(game, context, drops);
}
