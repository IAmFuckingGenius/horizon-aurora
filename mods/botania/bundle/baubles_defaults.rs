// Items' default components (in horizon-world, so nothing of the game's module): the trinkets,
// the relics and the Stone of Temperance stack to 1 (`unstackable()`), the relics are epic and
// survive fire, the Ring of Chordata is rare and the Charm of the Diva and the Flügel Tiara
// uncommon (`BotaniaItems`). The bands of mana are the mana module's.
fn botania_baubles_defaults(
    item: &str,
    components: &mut serde_json::Map<String, serde_json::Value>,
) {
    let relic = matches!(
        item,
        "botania:thor_ring"
            | "botania:odin_ring"
            | "botania:loki_ring"
            | "botania:dice"
            | "botania:flugel_eye"
            | "botania:infinite_fruit"
            | "botania:king_key"
    );
    let bauble = matches!(
        item,
        "botania:aura_ring"
            | "botania:aura_ring_greater"
            | "botania:magnet_ring"
            | "botania:magnet_ring_greater"
            | "botania:water_ring"
            | "botania:swap_ring"
            | "botania:dodge_ring"
            | "botania:mining_ring"
            | "botania:pixie_ring"
            | "botania:reach_ring"
            | "botania:travel_belt"
            | "botania:super_travel_belt"
            | "botania:speed_up_belt"
            | "botania:knockback_belt"
            | "botania:ice_pendant"
            | "botania:lava_pendant"
            | "botania:super_lava_pendant"
            | "botania:cloud_pendant"
            | "botania:super_cloud_pendant"
            | "botania:holy_cloak"
            | "botania:unholy_cloak"
            | "botania:balance_cloak"
            | "botania:invisibility_cloak"
            | "botania:third_eye"
            | "botania:monocle"
            | "botania:tiny_planet"
            | "botania:goddess_charm"
            | "botania:diva_charm"
            | "botania:itemfinder"
            | "botania:flight_tiara"
            | "botania:blood_pendant"
            | "botania:temperance_stone"
    ) || item.starts_with("botania:cosmetic_");
    if !relic && !bauble {
        return;
    }
    components.insert(
        "minecraft:max_stack_size".to_owned(),
        serde_json::Value::from(1),
    );
    let rarity = match item {
        _ if relic => Some("epic"),
        "botania:water_ring" => Some("rare"),
        "botania:diva_charm" | "botania:flight_tiara" => Some("uncommon"),
        _ => None,
    };
    if let Some(rarity) = rarity {
        components.insert(
            "minecraft:rarity".to_owned(),
            serde_json::Value::from(rarity),
        );
    }
    if relic {
        components.insert(
            "minecraft:damage_resistant".to_owned(),
            serde_json::json!({ "types": "#minecraft:is_fire" }),
        );
    }
}
