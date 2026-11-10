// Botania's block entities tick (`EntityBlock.getTicker`).
fn botania_ticks(block: &str, ticks: &mut bool) {
    crate::game::botania::ticks(block, ticks);
}
