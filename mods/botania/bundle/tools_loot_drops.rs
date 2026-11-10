// The elementium axe's beheading.
fn botania_tools_loot_drops(
    game: &mut crate::game::Game<'_>,
    context: &horizon_client::mod_api::NativeLootDrops<'_>,
    drops: &mut Vec<horizon_client::items::ItemStack>,
) {
    crate::game::botania_tools::loot_drops(game, context, drops);
}
