// Mana tools mend with mana; the Terra Shatterer spends its mana.
fn botania_weapons_inventory_tick(game: &mut crate::game::Game<'_>, slot: i32) {
    crate::game::botania_weapons::inventory_tick(game, slot);
}
