// A player about to be hurt: the Ring of Odin, the Crimson Pendant, the sashes, the cloaks, the
// pixies.
fn botania_baubles_damage(
    entity: i32,
    kind: &str,
    damage_type: &str,
    causing: i32,
    amount: &mut f32,
    canceled: &mut bool,
) {
    crate::game::botania_baubles::incoming_damage(
        entity,
        kind,
        damage_type,
        causing,
        amount,
        canceled,
    );
}
