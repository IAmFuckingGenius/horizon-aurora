// A Thundercaller hit, for the attacker's tick.
fn botania_weapons_hurt_enemy(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    entity: u32,
) {
    crate::game::botania_weapons::hurt_enemy(game, stack, entity);
}
