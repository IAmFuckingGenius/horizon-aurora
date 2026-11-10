// A Botania bow let go shoots its arrow.
fn botania_weapons_release(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    entity: Option<i32>,
    time_left: i32,
    used: &mut bool,
) {
    crate::game::botania_weapons::release(game, stack, entity, time_left, used);
}
