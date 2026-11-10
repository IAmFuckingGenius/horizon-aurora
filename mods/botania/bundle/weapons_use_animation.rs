// A Botania bow draws in the bow's pose.
fn botania_weapons_use_animation(
    stack: &horizon_client::items::ItemStack,
    animation: &mut horizon_world::consumable::UseAnimation,
) {
    crate::game::botania_weapons::use_animation(stack, animation);
}
