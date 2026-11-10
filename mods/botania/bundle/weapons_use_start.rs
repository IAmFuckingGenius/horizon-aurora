// A survival player draws a Botania bow with an arrow, or the mana for one.
fn botania_weapons_use_start(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    entity: Option<i32>,
    canceled: &mut bool,
) {
    crate::game::botania_weapons::use_start(game, stack, entity, canceled);
}
