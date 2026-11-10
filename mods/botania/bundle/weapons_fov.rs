// A drawn Botania bow zooms as a bow does.
fn botania_weapons_fov(
    game: &crate::game::Game<'_>,
    player: &horizon_client::camera::FovPlayer,
    base: f32,
    modifier: &mut f32,
) {
    crate::game::botania_weapons::fov(game, player, base, modifier);
}
