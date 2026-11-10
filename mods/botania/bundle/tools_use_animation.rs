// The elementium shears are used in the bow's pose.
fn botania_tools_use_animation(
    stack: &horizon_client::items::ItemStack,
    animation: &mut horizon_world::consumable::UseAnimation,
) {
    crate::game::botania_tools::use_animation(stack, animation);
}
