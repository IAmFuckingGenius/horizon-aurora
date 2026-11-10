//! Botania's armour models as data: the meshes `vazkii.botania.client.model.armor` builds in code
//! (`ManasteelArmorModel`, `ManaweaveRobeModel`, `ElementiumArmorModel`, `TerrasteelArmorModel`,
//! each a `createInsideMesh` and a `createOutsideMesh`, baked on a 64 x 128 texture by
//! `BotaniaLayerDefinitions`), which mesh and parts each worn slot shows (`ArmorModels.make`,
//! `ArmorModel.setPartVisibility`), the texture of each material (`getArmorTexture`) and the
//! armour items they belong to.
//!
//! A part is a `PartDefinition`: its name, its parent's, its `PartPose` and its `CubeListBuilder`
//! boxes. The game bakes these into model parts and draws a worn piece's root parts where the
//! wearer's own head, body, arms and legs are, as `ArmorModel` copies them from the parent
//! `HumanoidModel` (`copyPropertiesTo`).

use std::f32::consts::PI;

/// The armour sets with a model of their own (`ArmorModels.get`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    Manasteel,
    Manaweave,
    Elementium,
    Terrasteel,
}

/// An armour slot (`EquipmentSlot`), head to feet: the order a player's armour slots are kept in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Head,
    Chest,
    Legs,
    Feet,
}

/// Which of a material's two meshes (`ArmorModels.make`): the inner one for the leggings, the
/// outer one for the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layer {
    Inner,
    Outer,
}

/// One box (`CubeListBuilder.addBox` after `texOffs` and `mirror`), in texels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cube {
    /// `texOffs`: the top left corner of the box's faces on the texture.
    pub tex: [i32; 2],
    pub mirror: bool,
    pub origin: [f32; 3],
    pub size: [f32; 3],
    /// The box's `CubeDeformation`: 0.01 for most, none where Botania gives none.
    pub grow: f32,
}

/// One part (`PartDefinition.addOrReplaceChild`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub name: &'static str,
    /// The parent part's name; empty for the mesh's root.
    pub parent: &'static str,
    /// `PartPose`: its offset in texels, then its rotation about x, y and z in radians.
    pub pose: [f32; 6],
    pub cubes: &'static [Cube],
}

/// `LayerDefinition.create(mesh, 64, 128)`.
pub const TEXTURE_SIZE: [u32; 2] = [64, 128];

/// The parts a `HumanoidModel` takes from its root (its constructor's `getChild` calls).
pub const HUMANOID_PARTS: [&str; 7] = [
    "head",
    "hat",
    "body",
    "right_arm",
    "left_arm",
    "right_leg",
    "left_leg",
];

impl Material {
    pub const ALL: [Material; 4] = [
        Material::Manasteel,
        Material::Manaweave,
        Material::Elementium,
        Material::Terrasteel,
    ];

    /// The parts of one of the material's meshes, each after its parent.
    pub fn mesh(self, layer: Layer) -> &'static [Part] {
        match (self, layer) {
            (Material::Manasteel, Layer::Inner) => MANASTEEL_INNER,
            (Material::Manasteel, Layer::Outer) => MANASTEEL_OUTER,
            (Material::Manaweave, Layer::Inner) => MANAWEAVE_INNER,
            (Material::Manaweave, Layer::Outer) => MANAWEAVE_OUTER,
            (Material::Elementium, Layer::Inner) => ELEMENTIUM_INNER,
            (Material::Elementium, Layer::Outer) => ELEMENTIUM_OUTER,
            (Material::Terrasteel, Layer::Inner) => TERRASTEEL_INNER,
            (Material::Terrasteel, Layer::Outer) => TERRASTEEL_OUTER,
        }
    }

    /// The texture the armour is drawn with (`getArmorTextureAfterInk`): Botania's
    /// `textures/model/armor_<material>.png`, which the content mod also carries under
    /// `textures/entity/model/` so the game's atlas packs it, by its atlas name. Manaweave has a
    /// holiday look while `jingleTheBells` (see [`jingle_the_bells`]).
    pub fn texture(self, holiday: bool) -> &'static str {
        match self {
            Material::Manasteel => "botania:entity/model/armor_manasteel",
            Material::Manaweave if holiday => "botania:entity/model/armor_manaweave_holiday",
            Material::Manaweave => "botania:entity/model/armor_manaweave",
            Material::Elementium => "botania:entity/model/armor_elementium",
            Material::Terrasteel => "botania:entity/model/armor_terrasteel",
        }
    }
}

impl Slot {
    pub const ALL: [Slot; 4] = [Slot::Head, Slot::Chest, Slot::Legs, Slot::Feet];

    /// `ArmorModels.make`: `bakeLayer(slot == LEGS ? inner : outer)`.
    pub fn layer(self) -> Layer {
        match self {
            Slot::Legs => Layer::Inner,
            _ => Layer::Outer,
        }
    }

    /// The root parts drawn for the slot (`ArmorModel.setPartVisibility`, a copy of vanilla's
    /// `HumanoidArmorLayer`).
    pub fn parts(self) -> &'static [&'static str] {
        match self {
            Slot::Head => &["head", "hat"],
            Slot::Chest => &["body", "right_arm", "left_arm"],
            Slot::Legs => &["body", "right_leg", "left_leg"],
            Slot::Feet => &["right_leg", "left_leg"],
        }
    }
}

/// The material and slot of a Botania armour item (`ManasteelArmorItem` and the classes that extend
/// it), by its item id.
pub fn piece(item: &str) -> Option<(Material, Slot)> {
    let name = item.strip_prefix("botania:")?;
    let (material, slot) = name.split_once('_')?;
    let material = match material {
        "manasteel" => Material::Manasteel,
        "manaweave" => Material::Manaweave,
        "elementium" => Material::Elementium,
        "terrasteel" => Material::Terrasteel,
        _ => return None,
    };
    let slot = match slot {
        "helmet" => Slot::Head,
        "chestplate" => Slot::Chest,
        "leggings" => Slot::Legs,
        "boots" => Slot::Feet,
        _ => return None,
    };
    Some((material, slot))
}

/// `ClientProxy.jingleTheBells`, from the day (days since 1970-01-01): from December 16 to
/// January 2.
pub fn jingle_the_bells(unix_days: i64) -> bool {
    let (month, day) = month_day(unix_days);
    month == 12 && day >= 16 || month == 1 && day <= 2
}

/// The month (1 to 12) and day of the month of a day counted from 1970-01-01 in the proleptic
/// Gregorian calendar (Howard Hinnant's `civil_from_days`).
fn month_day(unix_days: i64) -> (u32, u32) {
    let z = unix_days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (month, day)
}

// ---- the meshes ---------------------------------------------------------------------------------

/// `new CubeDeformation(0.01F)`, the growth of almost every box.
const D: f32 = 0.01;
const ZERO: [f32; 6] = [0.0; 6];

const fn cube(tex: [i32; 2], mirror: bool, origin: [f32; 3], size: [f32; 3], grow: f32) -> Cube {
    Cube {
        tex,
        mirror,
        origin,
        size,
        grow,
    }
}

const fn part(
    name: &'static str,
    parent: &'static str,
    pose: [f32; 6],
    cubes: &'static [Cube],
) -> Part {
    Part {
        name,
        parent,
        pose,
        cubes,
    }
}

/// `PartPose.offset`.
const fn at(x: f32, y: f32, z: f32) -> [f32; 6] {
    [x, y, z, 0.0, 0.0, 0.0]
}

/// The 2 x 2 x 2 box Botania puts in each root part it draws from.
const fn stub(origin: [f32; 3], mirror: bool) -> Cube {
    cube([0, 0], mirror, origin, [2.0, 2.0, 2.0], D)
}

/// `ManasteelArmorModel.createInsideMesh`.
#[rustfmt::skip]
const MANASTEEL_INNER: &[Part] = &[
    part("head", "", ZERO, &[]),
    part("hat", "", ZERO, &[]),
    part("left_arm", "", ZERO, &[]),
    part("right_arm", "", ZERO, &[]),
    part("body", "", ZERO, &[stub([-1.0, 0.0, -1.0], false)]),
    part("belt", "body", ZERO, &[cube([0, 51], false, [-4.5, 9.0, -3.0], [9.0, 3.0, 6.0], D)]),
    part("left_leg", "", at(1.9, 12.0, 0.0), &[cube([0, 60], true, [-0.39, 0.0, -2.49], [3.0, 6.0, 5.0], D)]),
    part("right_leg", "", at(-1.9, 12.0, 0.0), &[cube([0, 60], false, [-2.61, 0.0, -2.51], [3.0, 6.0, 5.0], D)]),
];

/// `ManasteelArmorModel.createOutsideMesh`.
#[rustfmt::skip]
const MANASTEEL_OUTER: &[Part] = &[
    part("head", "", ZERO, &[stub([-1.0, -2.0, 0.0], false)]),
    part("helm", "head", ZERO, &[cube([0, 0], false, [-4.5, -8.5, -4.5], [9.0, 9.0, 9.0], D)]),
    part("helm_top", "helm", [0.0, -8.5, 3.5, -0.2618, 0.0, 0.0], &[cube([36, 6], false, [-1.5, 0.0, -7.0], [3.0, 3.0, 7.0], D)]),
    part("helm_crystal", "helm", [0.0, -7.5, -4.5, 0.0873, 0.0, 0.0], &[cube([36, 0], false, [-1.5, -4.0, -1.0], [3.0, 5.0, 1.0], D)]),
    part("body", "", ZERO, &[stub([-1.0, 0.0, -1.0], false)]),
    part("body_top", "body", ZERO, &[cube([0, 18], false, [-4.5, -0.5, -3.0], [9.0, 6.0, 6.0], D)]),
    part("body_bottom", "body_top", ZERO, &[cube([0, 30], false, [-2.5, 5.5, -2.5], [5.0, 3.0, 5.0], 0.0)]),
    part("left_arm", "", at(4.0, 2.0, 0.0), &[stub([0.0, -1.0, -1.0], true)]),
    part("left_arm_main", "left_arm", ZERO, &[cube([22, 40], true, [1.5, 2.0, -2.49], [2.0, 6.0, 5.0], D)]),
    part("left_arm_pauldron", "left_arm_main", ZERO, &[cube([0, 40], true, [-0.5, -3.0, -3.0], [5.0, 4.0, 6.0], D)]),
    part("left_arm_crystal", "left_arm_pauldron", [2.0, -2.5, 0.0, 0.0, 0.0, -PI / 36.0], &[cube([36, 40], true, [-0.5, -2.5, -1.5], [2.0, 3.0, 3.0], D)]),
    part("right_arm", "", at(-4.0, 2.0, 0.0), &[stub([-2.0, -1.0, -1.0], true)]),
    part("right_arm_main", "right_arm", ZERO, &[cube([22, 40], false, [-3.5, 2.0, -2.51], [2.0, 6.0, 5.0], D)]),
    part("right_arm_pauldron", "right_arm_main", ZERO, &[cube([0, 40], false, [-4.5, -3.0, -3.0], [5.0, 4.0, 6.0], D)]),
    part("right_arm_crystal", "right_arm_pauldron", [-2.0, -2.5, 0.0, 0.0, 0.0, PI / 36.0], &[cube([36, 40], false, [-1.5, -2.5, -1.5], [2.0, 3.0, 3.0], D)]),
    part("left_leg", "", at(1.9, 12.0, 0.0), &[cube([0, 71], true, [-2.39, 8.5, -2.49], [5.0, 4.0, 5.0], D)]),
    part("left_leg_crystal", "left_leg", [2.5, 9.0, 2.0, 0.0, 0.0, PI / 36.0], &[cube([36, 46], true, [-1.0, -2.0, -1.5], [2.0, 3.0, 3.0], D)]),
    part("right_leg", "", at(-2.0, 12.0, 0.0), &[cube([0, 71], false, [-2.5, 8.5, -2.51], [5.0, 4.0, 5.0], D)]),
    part("right_leg_crystal", "right_leg", [-2.5, 9.0, 2.0, 0.0, 0.0, -PI / 36.0], &[cube([36, 46], false, [-1.0, -2.0, -1.5], [2.0, 3.0, 3.0], D)]),
    part("hat", "", ZERO, &[]),
];

/// `ManaweaveRobeModel.createInsideMesh`.
#[rustfmt::skip]
const MANAWEAVE_INNER: &[Part] = &[
    part("head", "", ZERO, &[]),
    part("hat", "", ZERO, &[]),
    part("left_arm", "", ZERO, &[]),
    part("right_arm", "", ZERO, &[]),
    part("body", "", ZERO, &[]),
    part("left_leg", "", at(1.9, 12.0, 0.0), &[cube([0, 78], true, [-2.39, -0.5, -2.49], [5.0, 6.0, 5.0], D)]),
    part("left_leg_skirt", "left_leg", [-0.5, -2.0, -2.5, 0.0, -PI / 18.0, -PI / 12.0], &[cube([0, 59], true, [-1.0, 0.0, -0.5], [5.0, 13.0, 6.0], D)]),
    part("right_leg", "", at(-1.9, 12.0, 0.0), &[cube([0, 78], false, [-2.61, 0.0, -2.51], [5.0, 6.0, 5.0], D)]),
    part("right_leg_skirt", "right_leg", [0.5, -2.0, -2.5, 0.0, PI / 18.0, PI / 12.0], &[cube([0, 59], false, [-4.0, 0.0, -0.5], [5.0, 13.0, 6.0], D)]),
];

/// `ManaweaveRobeModel.createOutsideMesh`.
#[rustfmt::skip]
const MANAWEAVE_OUTER: &[Part] = &[
    part("head", "", ZERO, &[stub([-1.0, -2.0, 0.0], false)]),
    part("helm", "head", [0.0, 0.0, 0.0, PI / 18.0, 0.0, 0.0], &[cube([0, 0], false, [-4.5, -9.5, -4.0], [9.0, 11.0, 10.0], D)]),
    part("body", "", ZERO, &[stub([-1.0, 0.0, -1.0], false)]),
    part("body_top", "body", ZERO, &[cube([0, 21], false, [-4.5, -0.5, -3.0], [9.0, 7.0, 6.0], 0.0)]),
    part("body_bottom", "body_top", ZERO, &[cube([0, 34], false, [-4.5, 6.5, -2.5], [9.0, 5.0, 5.0], D)]),
    part("left_arm", "", at(4.0, 2.0, 0.0), &[stub([0.0, -1.0, -1.0], true)]),
    part("left_arm_main", "left_arm", ZERO, &[cube([0, 44], true, [-1.5, -2.5, -2.49], [5.0, 10.0, 5.0], 0.0)]),
    part("left_arm_pauldron", "left_arm_main", [0.0, 0.0, 0.0, 0.0, 0.0, -PI / 36.0], &[cube([20, 44], true, [-1.0, -3.0, -3.0], [6.0, 5.0, 6.0], 0.0)]),
    part("right_arm", "", at(-4.0, 2.0, 0.0), &[stub([-2.0, -1.0, -1.0], true)]),
    part("right_arm_main", "right_arm", ZERO, &[cube([0, 44], false, [-3.5, -2.5, -2.51], [5.0, 10.0, 5.0], D)]),
    part("right_arm_pauldron", "right_arm_main", [0.0, 0.0, 0.0, 0.0, 0.0, PI / 36.0], &[cube([20, 44], false, [-5.0, -3.0, -3.0], [6.0, 5.0, 6.0], D)]),
    part("left_leg", "", at(1.9, 12.0, 0.0), &[cube([0, 89], true, [-2.39, 8.5, -2.49], [5.0, 4.0, 5.0], D)]),
    part("right_leg", "", at(-1.9, 12.0, 0.0), &[cube([0, 89], false, [-2.61, 8.5, -2.51], [5.0, 4.0, 5.0], D)]),
    part("hat", "", ZERO, &[]),
];

/// `ElementiumArmorModel.createInsideMesh`.
#[rustfmt::skip]
const ELEMENTIUM_INNER: &[Part] = &[
    part("head", "", ZERO, &[]),
    part("hat", "", ZERO, &[]),
    part("left_arm", "", ZERO, &[]),
    part("right_arm", "", ZERO, &[]),
    part("body", "", ZERO, &[stub([-1.0, 0.0, -1.0], false)]),
    part("belt", "body", ZERO, &[cube([0, 53], false, [-4.5, 8.0, -3.0], [9.0, 5.0, 6.0], D)]),
    part("left_leg", "", at(1.9, 12.0, 0.0), &[cube([0, 64], true, [-2.39, 0.0, -2.49], [5.0, 6.0, 5.0], D)]),
    part("right_leg", "", at(-1.9, 12.0, 0.0), &[cube([0, 64], false, [-2.61, 0.0, -2.51], [5.0, 6.0, 5.0], D)]),
];

/// `ElementiumArmorModel.createOutsideMesh`.
#[rustfmt::skip]
const ELEMENTIUM_OUTER: &[Part] = &[
    part("head", "", ZERO, &[stub([-1.0, -2.0, 0.0], false)]),
    part("helm", "head", ZERO, &[cube([0, 0], false, [-4.5, -9.0, -4.5], [9.0, 9.0, 9.0], D)]),
    part("helm_fairy", "helm", ZERO, &[cube([36, 11], false, [-2.5, -10.0, -5.5], [5.0, 5.0, 5.0], D)]),
    part("helm_wing_1l", "helm", [4.5, -6.0, -0.5, PI / 12.0, PI / 6.0, PI / 12.0], &[cube([36, 0], true, [-1.0, -5.0, 0.0], [1.0, 5.0, 6.0], D)]),
    part("helm_wing_2l", "helm", [4.5, -6.0, -0.5, -PI / 12.0, PI / 12.0, -PI / 12.0], &[cube([50, 0], false, [-1.0, 0.0, 0.0], [1.0, 3.0, 4.0], D)]),
    part("helm_wing_1r", "helm", [-4.5, -6.0, -0.5, PI / 12.0, -PI / 6.0, -PI / 12.0], &[cube([36, 0], false, [0.0, -5.0, 0.0], [1.0, 5.0, 6.0], D)]),
    part("helm_wing_2r", "helm", [-4.5, -6.0, -0.5, -PI / 12.0, -PI / 12.0, PI / 12.0], &[cube([50, 0], true, [0.0, 0.0, 0.0], [1.0, 3.0, 4.0], D)]),
    part("body", "", ZERO, &[stub([-1.0, 0.0, -1.0], false)]),
    part("body_top", "body", ZERO, &[cube([0, 19], false, [-4.5, 0.0, -3.0], [9.0, 6.0, 6.0], D)]),
    part("body_bottom", "body_top", ZERO, &[cube([0, 31], false, [-3.5, 5.0, -2.5], [7.0, 3.0, 5.0], D)]),
    part("left_arm", "", at(4.0, 2.0, 0.0), &[stub([0.0, -1.0, -1.0], true)]),
    part("left_arm_main", "left_arm", ZERO, &[cube([24, 40], true, [0.5, 4.5, -2.49], [3.0, 6.0, 5.0], D)]),
    part("left_arm_pauldron", "left_arm_main", ZERO, &[cube([0, 40], true, [-0.5, -3.0, -3.0], [6.0, 7.0, 6.0], D)]),
    part("left_arm_wing_1", "left_arm_pauldron", [6.0, -1.0, 0.0, PI / 12.0, PI / 6.0, PI / 12.0], &[cube([40, 35], false, [0.0, -4.0, 0.0], [0.0, 4.0, 5.0], D)]),
    part("left_arm_wing_2", "left_arm_pauldron", [6.0, -1.0, 0.0, -PI / 12.0, PI / 12.0, -PI / 12.0], &[cube([40, 40], false, [0.0, 0.0, 0.0], [0.0, 3.0, 4.0], D)]),
    part("right_arm", "", at(-4.0, 2.0, 0.0), &[stub([-2.0, -1.0, -1.0], true)]),
    part("right_arm_main", "right_arm", ZERO, &[cube([24, 40], false, [-3.5, 4.5, -2.51], [3.0, 6.0, 5.0], D)]),
    part("right_arm_pauldron", "right_arm_main", ZERO, &[cube([0, 40], false, [-5.5, -3.0, -3.0], [6.0, 7.0, 6.0], D)]),
    part("right_arm_wing_1", "right_arm_pauldron", [-6.5, -1.0, 0.0, PI / 12.0, -PI / 6.0, -PI / 12.0], &[cube([40, 35], true, [0.0, -4.0, 0.0], [0.0, 4.0, 5.0], D)]),
    part("right_arm_wing_2", "right_arm_pauldron", [-6.5, -1.0, 0.0, -PI / 12.0, -PI / 12.0, PI / 12.0], &[cube([40, 40], true, [0.0, 0.0, 0.0], [0.0, 3.0, 4.0], D)]),
    part("left_leg", "", at(1.9, 12.0, 0.0), &[cube([0, 75], true, [-2.39, 8.5, -2.49], [5.0, 5.0, 5.0], D)]),
    part("left_boot_wing_1", "left_leg", [2.5, 8.5, 0.0, PI / 12.0, PI / 6.0, PI / 12.0], &[cube([40, 40], false, [0.0, -3.0, 0.0], [0.0, 3.0, 4.0], D)]),
    part("left_boot_wing_2", "left_leg", [2.5, 8.5, 0.0, -PI / 12.0, PI / 12.0, -PI / 12.0], &[cube([40, 44], false, [0.0, 0.0, 0.0], [0.0, 2.0, 3.0], D)]),
    part("right_leg", "", at(-1.9, 12.0, 0.0), &[cube([0, 75], false, [-2.61, 8.5, -2.51], [5.0, 5.0, 5.0], D)]),
    part("right_boot_wing_1", "right_leg", [-2.6, 8.5, 0.0, PI / 12.0, -PI / 6.0, -PI / 12.0], &[cube([40, 40], true, [0.0, -3.0, 0.0], [0.0, 3.0, 4.0], D)]),
    part("right_boot_wing_2", "right_leg", [-2.5, 8.5, 0.0, -PI / 12.0, -PI / 12.0, PI / 12.0], &[cube([40, 44], true, [0.0, 0.0, 0.0], [0.0, 2.0, 3.0], D)]),
    part("hat", "", ZERO, &[]),
];

/// `TerrasteelArmorModel.createInsideMesh`.
#[rustfmt::skip]
const TERRASTEEL_INNER: &[Part] = &[
    part("head", "", ZERO, &[]),
    part("hat", "", ZERO, &[]),
    part("left_arm", "", ZERO, &[]),
    part("right_arm", "", ZERO, &[]),
    part("body", "", ZERO, &[stub([-1.0, 0.0, -1.0], false)]),
    part("belt", "body", ZERO, &[cube([0, 65], false, [-4.5, 8.0, -3.0], [9.0, 5.0, 6.0], D)]),
    part("left_leg", "", at(1.9, 12.0, 0.0), &[cube([0, 76], true, [-2.39, -0.01, -2.49], [5.0, 6.0, 5.0], D)]),
    part("right_leg", "", at(-1.9, 12.0, 0.0), &[cube([0, 76], false, [-2.61, -0.01, -2.51], [5.0, 6.0, 5.0], D)]),
];

/// `TerrasteelArmorModel.createOutsideMesh`.
#[rustfmt::skip]
const TERRASTEEL_OUTER: &[Part] = &[
    part("head", "", ZERO, &[stub([-1.0, -2.0, 0.0], false)]),
    part("helm", "head", ZERO, &[cube([0, 0], false, [-4.5, -9.0, -4.5], [9.0, 11.0, 9.0], D)]),
    part("helm_front", "helm", ZERO, &[cube([36, 0], false, [-1.5, -10.0, -5.5], [3.0, 8.0, 7.0], D)]),
    part("helm_leaf_1l", "helm", [4.5, -6.0, -2.5, -PI / 12.0, -PI / 12.0, PI / 6.0], &[cube([56, 14], true, [0.0, -3.0, -1.0], [2.0, 3.0, 1.0], D)]),
    part("helm_leaf_2l", "helm", [4.5, -6.0, -0.5, -PI / 6.0, -PI / 6.0, PI / 4.0], &[cube([56, 8], true, [0.0, -5.0, -1.0], [2.0, 5.0, 1.0], D)]),
    part("helm_leaf_3l", "helm", [4.5, -6.0, 2.5, -PI / 4.0, -PI / 4.0, PI / 4.0], &[cube([56, 0], true, [0.0, -7.0, -1.0], [2.0, 7.0, 1.0], D)]),
    part("helm_leaf_1r", "helm", [-4.5, -6.0, -2.5, -PI / 12.0, PI / 12.0, -PI / 6.0], &[cube([56, 14], false, [-2.0, -3.0, -1.0], [2.0, 3.0, 1.0], D)]),
    part("helm_leaf_2r", "helm", [-4.5, -6.0, -0.5, -PI / 6.0, PI / 6.0, -PI / 4.0], &[cube([56, 8], false, [-2.0, -5.0, -1.0], [2.0, 5.0, 1.0], D)]),
    part("helm_leaf_3r", "helm", [-4.5, -6.0, 2.5, -PI / 4.0, PI / 4.0, -PI / 4.0], &[cube([56, 0], false, [-2.0, -7.0, -1.0], [2.0, 7.0, 1.0], D)]),
    part("helm_branch_1l", "helm", [2.5, -9.0, -3.5, PI / 6.0, PI / 12.0, 0.0], &[cube([36, 15], true, [-1.0, -1.0, 0.0], [2.0, 2.0, 7.0], D)]),
    part("helm_branch_2l", "helm", [4.5, -3.0, -2.5, PI / 36.0, PI / 12.0, 0.0], &[cube([36, 15], true, [-1.0, -1.0, 0.0], [2.0, 2.0, 7.0], D)]),
    part("helm_branch_1r", "helm", [-2.5, -9.0, -3.5, PI / 6.0, -PI / 12.0, 0.0], &[cube([36, 15], false, [-1.0, -1.0, 0.0], [2.0, 2.0, 7.0], D)]),
    part("helm_branch_2r", "helm", [-4.5, -3.0, -2.5, PI / 36.0, -PI / 12.0, 0.0], &[cube([36, 15], false, [-1.0, -1.0, 0.0], [2.0, 2.0, 7.0], D)]),
    part("body", "", ZERO, &[stub([-1.0, 0.0, -1.0], false)]),
    part("body_top", "body", ZERO, &[cube([0, 20], false, [-5.5, 0.0, -3.0], [11.0, 6.0, 6.0], D)]),
    part("body_bottom", "body_top", ZERO, &[cube([0, 32], false, [-4.5, 5.0, -2.5], [9.0, 3.0, 5.0], D)]),
    part("left_arm", "", at(4.0, 2.0, 0.0), &[stub([0.0, -1.0, -1.0], true)]),
    part("left_arm_main", "left_arm", ZERO, &[cube([0, 52], true, [-1.5, 3.0, -2.49], [5.0, 8.0, 5.0], D)]),
    part("left_arm_pauldron", "left_arm_main", at(1.5, 0.0, 0.0), &[cube([0, 40], true, [-1.0, -3.0, -3.0], [6.0, 6.0, 6.0], D)]),
    part("left_arm_branch_1", "left_arm_pauldron", [4.0, -3.0, -1.0, PI / 6.0, PI / 6.0, 0.0], &[cube([36, 15], true, [-1.0, -1.0, 0.0], [2.0, 2.0, 7.0], D)]),
    part("left_arm_branch_2", "left_arm_pauldron", [5.0, -2.0, 0.0, PI / 36.0, PI / 4.0, 0.0], &[cube([36, 24], true, [-1.0, 0.0, 0.0], [2.0, 2.0, 5.0], D)]),
    part("right_arm", "", at(-4.0, 2.0, 0.0), &[stub([-2.0, -1.0, -1.0], true)]),
    part("right_arm_main", "right_arm", ZERO, &[cube([0, 52], false, [-3.5, 3.0, -2.51], [5.0, 8.0, 5.0], D)]),
    part("right_arm_pauldron", "right_arm_main", at(-1.5, 0.0, 0.0), &[cube([0, 40], false, [-5.0, -3.0, -3.0], [6.0, 6.0, 6.0], D)]),
    part("right_arm_branch_1", "right_arm_pauldron", [-4.0, -3.0, -1.0, PI / 6.0, -PI / 6.0, 0.0], &[cube([36, 15], false, [-1.0, -1.0, 0.0], [2.0, 2.0, 7.0], D)]),
    part("right_arm_branch_2", "right_arm_pauldron", [-5.0, -2.0, 0.0, PI / 36.0, -PI / 4.0, 0.0], &[cube([36, 24], false, [-1.0, 0.0, 0.0], [2.0, 2.0, 5.0], D)]),
    part("left_leg", "", at(1.9, 12.0, 0.0), &[cube([0, 94], true, [-2.39, 8.5, -2.49], [5.0, 4.0, 5.0], D)]),
    part("left_boot_top", "left_leg", ZERO, &[cube([0, 87], true, [-2.39, 6.0, -2.49], [6.0, 2.0, 5.0], D)]),
    part("left_boot_branch_1", "left_boot_top", [3.5, 6.0, 0.0, PI / 12.0, PI / 12.0, -PI / 36.0], &[cube([36, 15], true, [-1.0, -1.0, 0.0], [2.0, 2.0, 7.0], D)]),
    part("left_boot_branch_2", "left_boot_top", [2.5, 9.0, 0.0, PI / 36.0, PI / 4.0, 0.0], &[cube([36, 24], true, [-1.0, -1.0, 0.0], [2.0, 2.0, 5.0], D)]),
    part("right_leg", "", at(-1.9, 12.0, 0.0), &[cube([0, 94], false, [-2.61, 8.5, -2.51], [5.0, 4.0, 5.0], D)]),
    part("right_boot_top", "right_leg", ZERO, &[cube([0, 87], false, [-3.61, 6.0, -2.51], [6.0, 2.0, 5.0], D)]),
    part("right_boot_branch_1", "right_boot_top", [-3.5, 6.0, 0.0, PI / 12.0, -PI / 12.0, PI / 36.0], &[cube([36, 15], false, [-1.0, -1.0, 0.0], [2.0, 2.0, 7.0], D)]),
    part("right_boot_branch_2", "right_boot_top", [-2.5, 9.0, 0.5, PI / 36.0, -PI / 4.0, 0.0], &[cube([36, 24], false, [-1.0, -1.0, 0.0], [2.0, 2.0, 5.0], D)]),
    part("hat", "", ZERO, &[]),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn meshes() -> impl Iterator<Item = (Material, Layer, &'static [Part])> {
        Material::ALL.into_iter().flat_map(|m| {
            [Layer::Inner, Layer::Outer]
                .into_iter()
                .map(move |l| (m, l, m.mesh(l)))
        })
    }

    #[test]
    fn every_armour_item_has_its_material_and_slot() {
        let mut found = 0;
        for (material, name) in [
            (Material::Manasteel, "manasteel"),
            (Material::Manaweave, "manaweave"),
            (Material::Elementium, "elementium"),
            (Material::Terrasteel, "terrasteel"),
        ] {
            for (slot, piece_name) in [
                (Slot::Head, "helmet"),
                (Slot::Chest, "chestplate"),
                (Slot::Legs, "leggings"),
                (Slot::Feet, "boots"),
            ] {
                assert_eq!(
                    piece(&format!("botania:{name}_{piece_name}")),
                    Some((material, slot))
                );
                found += 1;
            }
        }
        assert_eq!(found, 16);
        // Tools of the same materials, other mods' armour and Botania's other items have none.
        for other in [
            "botania:manasteel_sword",
            "botania:terrasteel_ingot",
            "botania:elementium_shears",
            "minecraft:diamond_helmet",
            "other:manasteel_helmet",
            "botania:manasteel_helmet_extra",
            "botania:twig_wand",
        ] {
            assert_eq!(piece(other), None, "{other}");
        }
    }

    #[test]
    fn only_the_leggings_use_the_inner_mesh() {
        assert_eq!(Slot::Legs.layer(), Layer::Inner);
        for slot in [Slot::Head, Slot::Chest, Slot::Feet] {
            assert_eq!(slot.layer(), Layer::Outer);
        }
    }

    #[test]
    fn each_slot_shows_the_parts_humanoid_armour_shows() {
        // HumanoidArmorLayer.setPartVisibility, as ArmorModel copies it.
        assert_eq!(Slot::Head.parts(), ["head", "hat"]);
        assert_eq!(Slot::Chest.parts(), ["body", "right_arm", "left_arm"]);
        assert_eq!(Slot::Legs.parts(), ["body", "right_leg", "left_leg"]);
        assert_eq!(Slot::Feet.parts(), ["right_leg", "left_leg"]);
    }

    #[test]
    fn every_mesh_has_the_parts_a_humanoid_model_needs() {
        // `new HumanoidModel(root)` fails on a root without any of them.
        for (material, layer, mesh) in meshes() {
            for name in HUMANOID_PARTS {
                assert!(
                    mesh.iter().any(|p| p.name == name && p.parent.is_empty()),
                    "{material:?} {layer:?}: no root part {name}"
                );
            }
        }
    }

    #[test]
    fn parts_have_unique_names_and_come_after_their_parents() {
        for (material, layer, mesh) in meshes() {
            for (i, part) in mesh.iter().enumerate() {
                assert!(
                    mesh[..i].iter().all(|p| p.name != part.name),
                    "{material:?} {layer:?}: {} twice",
                    part.name
                );
                assert!(
                    part.parent.is_empty() || mesh[..i].iter().any(|p| p.name == part.parent),
                    "{material:?} {layer:?}: {} before its parent {}",
                    part.name,
                    part.parent
                );
            }
        }
    }

    #[test]
    fn every_box_takes_its_faces_from_inside_the_texture() {
        // A box's faces cover 2 * (depth + width) by depth + height texels from its texOffs.
        let [tw, th] = TEXTURE_SIZE.map(|s| s as f32);
        for (material, layer, mesh) in meshes() {
            for part in mesh {
                for cube in part.cubes {
                    let [w, h, d] = cube.size;
                    let [u, v] = cube.tex.map(|t| t as f32);
                    assert!(
                        u + 2.0 * (d + w) <= tw && v + d + h <= th,
                        "{material:?} {layer:?}: {} reads outside the texture",
                        part.name
                    );
                }
            }
        }
    }

    #[test]
    fn the_slots_draw_parts_that_hold_boxes() {
        // Every piece shows something: one of its slot's root parts has boxes in or under it.
        for material in Material::ALL {
            for slot in Slot::ALL {
                let mesh = material.mesh(slot.layer());
                let holds = |root: &str| {
                    mesh.iter()
                        .any(|p| !p.cubes.is_empty() && (p.name == root || under(mesh, p, root)))
                };
                assert!(
                    slot.parts().iter().any(|root| holds(root)),
                    "{material:?} {slot:?} draws nothing"
                );
            }
        }
    }

    fn under(mesh: &[Part], part: &Part, root: &str) -> bool {
        let mut parent = part.parent;
        while !parent.is_empty() {
            if parent == root {
                return true;
            }
            parent = mesh
                .iter()
                .find(|p| p.name == parent)
                .map_or("", |p| p.parent);
        }
        false
    }

    #[test]
    fn manaweave_alone_has_a_holiday_texture() {
        assert_eq!(
            Material::Manaweave.texture(true),
            "botania:entity/model/armor_manaweave_holiday"
        );
        assert_eq!(
            Material::Manaweave.texture(false),
            "botania:entity/model/armor_manaweave"
        );
        for material in [
            Material::Manasteel,
            Material::Elementium,
            Material::Terrasteel,
        ] {
            assert_eq!(material.texture(true), material.texture(false));
        }
    }

    #[test]
    fn the_bells_jingle_from_december_16_to_january_2() {
        // Days since 1970-01-01.
        assert!(!jingle_the_bells(20_802)); // 2026-12-15
        assert!(jingle_the_bells(20_803)); // 2026-12-16
        assert!(jingle_the_bells(20_453)); // 2025-12-31
        assert!(jingle_the_bells(20_820)); // 2027-01-02
        assert!(!jingle_the_bells(20_821)); // 2027-01-03
        assert!(!jingle_the_bells(20_767)); // 2026-11-10
        assert!(!jingle_the_bells(19_782)); // 2024-02-29
        assert!(jingle_the_bells(0)); // 1970-01-01
        assert!(!jingle_the_bells(2)); // 1970-01-03
    }

    #[test]
    fn days_become_calendar_dates() {
        assert_eq!(month_day(0), (1, 1));
        assert_eq!(month_day(19_782), (2, 29));
        assert_eq!(month_day(20_453), (12, 31));
        assert_eq!(month_day(20_767), (11, 10));
        assert_eq!(month_day(-1), (12, 31));
    }
}
