// A tick of the elementium shears' use: they shear around their user.
fn botania_tools_use_tick(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    entity: Option<i32>,
    remaining: i32,
) {
    crate::game::botania_tools::use_tick(game, stack, entity, remaining);
}
