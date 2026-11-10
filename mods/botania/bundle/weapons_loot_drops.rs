// The elementium pickaxe leaves the junk a block drops.
fn botania_weapons_loot_drops(
    game: &mut crate::game::Game<'_>,
    context: &horizon_client::mod_api::NativeLootDrops<'_>,
    drops: &mut Vec<horizon_client::items::ItemStack>,
) {
    crate::game::botania_weapons::loot_drops(game, context, drops);
}
