// A dispenser fills a glass bottle with Ender Air or throws an Ender Air Bottle.
fn botania_entities_dispense_item(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    pos: [i32; 3],
    facing: horizon_core::geometry::Direction,
    stack: &horizon_client::items::ItemStack,
    answer: &mut Option<horizon_client::mod_api::NativeDispensed>,
) {
    crate::game::botania_entities::dispense_item(game, dimension, pos, facing, stack, answer);
}
