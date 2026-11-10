// Items' default components (in horizon-world, so nothing of the game's module): the mana items
// stack to 1 (`stacksTo(1)`), and each Terrasteel armour piece resists knockback by its defence
// / 20 (`TerrasteelArmorItem.getDefaultAttributeModifiers`: 3, 8, 6 and 3 points).
fn botania_mana_defaults(item: &str, components: &mut serde_json::Map<String, serde_json::Value>) {
    if matches!(
        item,
        "botania:mana_tablet"
            | "botania:mana_ring"
            | "botania:mana_ring_greater"
            | "botania:mana_mirror"
    ) {
        components.insert(
            "minecraft:max_stack_size".to_owned(),
            serde_json::Value::from(1),
        );
        return;
    }
    let (defence, slot, piece) = match item {
        "botania:terrasteel_helmet" => (3.0, "head", "helmet"),
        "botania:terrasteel_chestplate" => (8.0, "chest", "chestplate"),
        "botania:terrasteel_leggings" => (6.0, "legs", "leggings"),
        "botania:terrasteel_boots" => (3.0, "feet", "boots"),
        _ => return,
    };
    let modifiers = components
        .entry("minecraft:attribute_modifiers".to_owned())
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    let serde_json::Value::Array(list) = modifiers else {
        return;
    };
    let has = list.iter().any(|m| {
        m.get("type").and_then(serde_json::Value::as_str) == Some("minecraft:knockback_resistance")
    });
    if !has {
        let amount: f64 = defence / 20.0;
        list.push(serde_json::json!({
            "type": "minecraft:knockback_resistance",
            "id": format!("minecraft:armor.{piece}"),
            "amount": amount,
            "operation": "add_value",
            "slot": slot,
        }));
    }
}
