// Whether entity ticks are reported: while pixies fly.
fn botania_entities_tick_listened(listened: &mut bool) {
    crate::game::botania_entities::tick_listened(listened);
}
