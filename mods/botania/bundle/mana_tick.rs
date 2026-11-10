// A carried stack's server tick: Botania's mana items, tools and armour (`inventoryTick`).
fn botania_mana_tick(game: &mut crate::game::Game<'_>, slot: i32, selected: bool) {
    crate::game::botania_mana::inventory_tick(game, slot, selected);
}
