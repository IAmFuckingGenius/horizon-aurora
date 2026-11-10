// Botania's armour models drawn on whoever wears the armour: a modified port of Vazkii's Botania
// `vazkii.botania.client.model.armor` (Botania License, see LICENSE and NOTICE.md in this bundle),
// added to `crates/horizon-client/src/game.rs` as `mod botania_armour` by horizon-mixins.
//
// The meshes are data in `botania_core::armor_model`; they are baked into model parts once. Each
// frame `native_mob_layer` draws a worn piece's root parts (`ArmorModel.setPartVisibility`) where
// the wearer's own head, body, arms and legs are drawn this frame, which is what Botania's
// `ArmorModel` gets from the parent `HumanoidModel` (`copyPropertiesTo`, each part's `copyFrom`):
// sneaking, swimming, flying with an elytra, riding, swinging and aiming all come with the pose.
// The content items carry an `equippable` without an equipment asset, so the vanilla armour layer
// draws nothing for them.
//
// Inner attributes and `//!` comments are not allowed in a horizon-mixins module payload.

use super::Game;
use super::native_block_entity_draw::BlockEntityDraw;
use super::native_mob_draw::{NativeMobRender, NativePlayerRender};
use botania_core::armor_model::{self, HUMANOID_PARTS, Layer, Material, Slot};
use glam::{DVec3, Mat4};
use horizon_client::items::ItemStack;
use horizon_client::mobs::Mob;
use horizon_client::mobs::model_layers::{
    CubeDeformation, CubeListBuilder, LayerDefinition, MeshDefinition, ModelPart, PartPose,
};
use horizon_world::nbt::{JavaString, Tag};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// What one armour slot holds: the item id and, when known, its stack.
type Worn<'a> = Option<(&'a str, Option<&'a ItemStack>)>;

// ---- hooks ---------------------------------------------------------------------------------------

// `native_player_render_listened`: the game poses mobs and players for `native_mob_layer` only
// while somebody in the level wears a piece.
pub(super) fn listened(game: &Game<'_>, out: &mut bool) {
    if with_wearers(game, |w| w.local || !w.mobs.is_empty()) {
        *out = true;
    }
}

// `native_mob_layer` (`HumanoidArmorLayer.render` with Botania's `getHumanoidArmorModel`): the
// pieces `mob` wears, drawn over its vanilla model and layers.
pub(super) fn layer(
    game: &Game<'_>,
    mob: &NativeMobRender<'_>,
    who: Option<&NativePlayerRender<'_>>,
    draw: &mut BlockEntityDraw,
) {
    if who.is_some_and(|w| w.local) {
        // This window's player, in third person: its own armour slots.
        let Some(m) = game.player_model.as_ref() else {
            return;
        };
        let worn = game
            .armor
            .each_ref()
            .map(|s| s.as_ref().map(|s| (&*s.item, Some(s))));
        draw_worn(game, m, mob, worn, draw);
        return;
    }
    if !with_wearers(game, |w| w.mobs.contains(&mob.id)) {
        return;
    }
    let Some(m) = game.mobs.iter().find(|m| m.id == mob.id) else {
        return;
    };
    if !humanoid_armour(m) {
        return;
    }
    let e = &m.extra;
    let worn = [
        (&e.head, &e.armor[0]),
        (&e.chest, &e.armor[1]),
        (&e.legs, &e.armor[2]),
        (&e.feet, &e.armor[3]),
    ]
    .map(|(item, stack)| item.as_deref().map(|item| (item, stack.as_ref())));
    draw_worn(game, m, mob, worn, draw);
}

// ---- who wears what ------------------------------------------------------------------------------

// Who wore a piece at the last look: this window's player, and the mobs and other players by id.
struct Wearers {
    at: Instant,
    local: bool,
    mobs: Vec<u32>,
}

// How long a look at everybody's armour holds: `native_player_render_listened` and the layer are
// asked for every mob in view, each frame.
const LOOK_AGAIN: Duration = Duration::from_millis(200);

static WEARERS: Mutex<Option<Wearers>> = Mutex::new(None);

fn with_wearers<R>(game: &Game<'_>, read: impl FnOnce(&Wearers) -> R) -> R {
    let mut wearers = WEARERS.lock().unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    if wearers
        .as_ref()
        .is_none_or(|w| now.duration_since(w.at) >= LOOK_AGAIN)
    {
        let botania = |item: Option<&str>| item.is_some_and(|i| armor_model::piece(i).is_some());
        *wearers = Some(Wearers {
            at: now,
            local: game.armor.iter().flatten().any(|s| botania(Some(&*s.item))),
            mobs: game
                .mobs
                .iter()
                .filter(|m| {
                    let e = &m.extra;
                    [&e.head, &e.chest, &e.legs, &e.feet]
                        .into_iter()
                        .any(|i| botania(i.as_deref()))
                })
                .map(|m| m.id)
                .collect(),
        });
    }
    read(wearers.get_or_insert_with(|| Wearers {
        at: now,
        local: false,
        mobs: Vec::new(),
    }))
}

// Whether vanilla draws armour on `m` (a `HumanoidArmorLayer`): players and mannequins, and the
// grown mobs whose model has a helmet layer (zombies, skeletons, piglins...). A baby's smaller
// model would leave Botania's grown-size pieces floating, so babies get none.
fn humanoid_armour(m: &Mob) -> bool {
    if matches!(&*m.kind.id, "minecraft:player" | "minecraft:mannequin") {
        return true;
    }
    if m.is_baby() {
        return false;
    }
    let (model, _) = horizon_client::mobs::render::body(m);
    model
        .strip_suffix("#main")
        .is_some_and(|base| m.kind.layer(&format!("{base}#helmet")).is_some())
}

// ---- drawing -------------------------------------------------------------------------------------

// The pieces of `worn` (head to feet) Botania draws, each in its own slot only
// (`HumanoidArmorLayer.renderArmorPiece`), under `draw` (the frame at the feet, turned with the
// body).
fn draw_worn(
    game: &Game<'_>,
    m: &Mob,
    mob: &NativeMobRender<'_>,
    worn: [Worn<'_>; 4],
    draw: &mut BlockEntityDraw,
) {
    let a = f64::from(mob.partial);
    // Where the model's frame starts: the feet, as `base_transform` places them.
    let feet = DVec3::new(
        m.previous.x + (m.position.x - m.previous.x) * a,
        m.previous.y + (m.position.y - m.previous.y) * a,
        m.previous.z + (m.position.z - m.previous.z) * a,
    );
    let data = game.res.mob_data();
    // Each part of the wearer's model where it is drawn now, from the feet, as it is asked for.
    let mut parts: [Option<Option<Mat4>>; HUMANOID_PARTS.len()] = [None; HUMANOID_PARTS.len()];
    let mut turned = false;
    for (slot, worn) in Slot::ALL.into_iter().zip(worn) {
        let Some((item, stack)) = worn else {
            continue;
        };
        let Some((material, own)) = armor_model::piece(item) else {
            continue;
        };
        // Phantom ink makes a piece invisible (`getArmorTexture` gives `armor_invisible.png`).
        if own != slot || stack.is_some_and(phantom_ink) {
            continue;
        }
        if !turned {
            // Back to the frame at the feet, unturned: the parts' places come with the turn.
            draw.push();
            draw.rotate_about([0.0, 1.0, 0.0], mob.body_yaw, [0.0; 3]);
            turned = true;
        }
        let sprite = material.texture(holiday());
        let model = model(material, slot.layer());
        let start = draw.vertices_mut().len();
        for &name in slot.parts() {
            let Some(part) = model
                .get_child(name)
                .filter(|p| !p.is_empty() || p.children().next().is_some())
            else {
                continue;
            };
            let Some(i) = HUMANOID_PARTS.iter().position(|n| *n == name) else {
                continue;
            };
            let at = *parts[i].get_or_insert_with(|| {
                horizon_client::mobs::render::part_transform(m, mob.partial, feet, data, name)
            });
            let Some(at) = at else {
                continue;
            };
            draw.push();
            place(draw, at);
            game.native_draw_model_part(draw, part, sprite, 0xFF_FFFF);
            draw.pop();
        }
        if foil(game, item, stack) {
            glint(game, draw, start, sprite);
        }
    }
    if turned {
        draw.pop();
    }
}

// Moves `draw`'s pose by `at`: a turn, a scale and a move, which is all a model part's place is.
fn place(draw: &mut BlockEntityDraw, at: Mat4) {
    let (scale, rotation, translation) = at.to_scale_rotation_translation();
    draw.translate(translation.to_array());
    let (axis, angle) = rotation.to_axis_angle();
    draw.rotate_about(axis.to_array(), angle.to_degrees(), [0.0; 3]);
    draw.scale(scale.to_array());
}

// `ItemStack.hasFoil`: vanilla draws an enchanted piece with the armour glint, Botania's too.
fn foil(game: &Game<'_>, item: &str, stack: Option<&ItemStack>) -> bool {
    let item_glint = game
        .res
        .items()
        .enchantments
        .props
        .get(item)
        .is_some_and(|p| p.glint);
    stack.is_some_and(|s| s.has_foil(item_glint))
}

// The armour glint over what was drawn from `start`: the glint scrolls over the texture's own
// coordinates, as the vanilla armour layer's does.
fn glint(game: &Game<'_>, draw: &mut BlockEntityDraw, start: usize, sprite: &str) {
    let Some(r) = game.gui.atlas.sprites.get(sprite).copied() else {
        return;
    };
    let (w, h) = (r.u1 - r.u0, r.v1 - r.v0);
    if w == 0.0 || h == 0.0 {
        return;
    }
    for v in &mut draw.vertices_mut()[start..] {
        v.layer = horizon_client::mobs::render::ARMOR_GLINT;
        v.glint_uv = [(v.uv[0] - r.u0) / w, (v.uv[1] - r.v0) / h];
    }
}

// `ManasteelArmorItem.hasPhantomInk`: the stack's `phantomInk` flag.
fn phantom_ink(stack: &ItemStack) -> bool {
    let flag = stack
        .custom_data()
        .and_then(|c| c.get(&JavaString::from("phantomInk")));
    match flag {
        Some(Tag::Byte(v)) => *v != 0,
        Some(Tag::Short(v)) => *v != 0,
        Some(Tag::Int(v)) => *v != 0,
        Some(Tag::Long(v)) => *v != 0,
        _ => false,
    }
}

// `ClientProxy.jingleTheBells`, settled once as Botania settles it at start (by the UTC date).
fn holiday() -> bool {
    static HOLIDAY: OnceLock<bool> = OnceLock::new();
    *HOLIDAY.get_or_init(|| {
        let days = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() / 86_400);
        armor_model::jingle_the_bells(i64::try_from(days).unwrap_or(0))
    })
}

// ---- models --------------------------------------------------------------------------------------

// A material's mesh baked (`ArmorModels.make`, `bakeLayer`), with its root parts' own poses
// cleared: the wearer's parts take their place, as `copyFrom` overwrites them.
fn model(material: Material, layer: Layer) -> &'static ModelPart {
    static MODELS: OnceLock<HashMap<(Material, Layer), ModelPart>> = OnceLock::new();
    let models = MODELS.get_or_init(|| {
        Material::ALL
            .into_iter()
            .flat_map(|m| [Layer::Inner, Layer::Outer].map(|l| ((m, l), bake(m.mesh(l)))))
            .collect()
    });
    &models[&(material, layer)]
}

fn bake(parts: &[armor_model::Part]) -> ModelPart {
    let mut mesh = MeshDefinition::new();
    let root = mesh.root();
    let mut ids = HashMap::new();
    for p in parts {
        let parent = ids.get(p.parent).copied().unwrap_or(root);
        let mut cubes = CubeListBuilder::create();
        for c in p.cubes {
            cubes = cubes
                .tex_offs(c.tex[0], c.tex[1])
                .set_mirror(c.mirror)
                .add_box_with(c.origin, c.size, CubeDeformation::new(c.grow));
        }
        let [x, y, z, x_rot, y_rot, z_rot] = p.pose;
        let pose = PartPose::offset_and_rotation(x, y, z, x_rot, y_rot, z_rot);
        let id = mesh.add_or_replace_child(parent, p.name, cubes, pose);
        ids.insert(p.name, id);
    }
    let [width, height] = armor_model::TEXTURE_SIZE;
    let mut baked = LayerDefinition::create(mesh, width, height).bake_root();
    for (_, part) in baked.children_mut() {
        part.load_pose(PartPose::ZERO);
    }
    baked
}
