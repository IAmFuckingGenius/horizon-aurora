// A blast about to break blocks: the Benevolent Goddess' Charm.
fn botania_baubles_explosion(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    center: [f64; 3],
    blocks: &mut Vec<[i32; 3]>,
) {
    crate::game::botania_baubles::explosion_detonate(game, dimension, center, blocks);
}
