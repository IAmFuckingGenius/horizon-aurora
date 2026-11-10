// A Botania bow is drawn until it is let go.
fn botania_weapons_use_duration(stack: &horizon_client::items::ItemStack, duration: &mut i32) {
    crate::game::botania_weapons::use_duration(stack, duration);
}
