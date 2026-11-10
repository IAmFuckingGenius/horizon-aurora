//! Mobs drawn by native mods on the client: `native_mob_submit` (vanilla's
//! `EntityRenderer.submit` for a mob, NeoForge's `RenderLivingEvent.Pre` when
//! it cancels) gets the mob's render state and a [`BlockEntityDraw`] posed at
//! its feet and turned with its body, so a mod draws a model of its own with
//! the helpers block entities draw with (`native_draw_cube`,
//! `native_draw_quad`, `native_draw_block`, `native_draw_item`). A player
//! has `native_player_submit` (`RenderPlayerEvent.Pre`), and every mob and
//! player in view gets `native_mob_layer` after its vanilla model
//! (`EntityRenderersEvent.AddLayers`), in the same frame. Armour stands get
//! `native_armor_stand_layer`, and the player drawn in the inventory
//! `native_player_preview_layer`, with where their parts are drawn
//! ([`NativeModelParts`]).

use super::{Dim, Game, native_block_entity_draw::BlockEntityDraw};
use glam::{DVec3, Mat4, Vec3};
use horizon_client::{
    block_entity_render::diffuse_lights,
    items::ItemStack,
    mesher::Vertex,
    mobs::{Mob, MobData, Part, anim, anim::Poses, render},
};
use std::ops::Range;

/// One mob's frame, as `EntityRenderer.extractRenderState` fills a
/// `LivingEntityRenderState`: what a model reads to pose and animate itself.
/// Angles are in degrees, interpolated to the frame's partial tick.
// Native mods read it; nothing in Horizon does.
#[derive(Clone, Debug)]
pub struct NativeMobRender<'a> {
    /// The mob's network id, as the mob helpers and mod messages name it.
    pub id: u32,
    /// Its entity type (`minecraft:villager`).
    pub kind: &'a str,
    /// Its `CustomName` as plain text, when it has one.
    pub custom_name: Option<&'a str>,
    /// `isBaby`.
    pub baby: bool,
    /// The frame's partial tick, 0 to 1.
    pub partial: f32,
    /// `ageInTicks`: ticks lived plus the partial tick, for idle motion.
    pub age: f32,
    /// `bodyRot`: the way the body faces (0 is south, 90 west).
    pub body_yaw: f32,
    /// `yRot`: the head's turn from the body, -180 to 180.
    pub head_yaw: f32,
    /// `xRot`: the head's pitch, positive looking down.
    pub pitch: f32,
    /// `walkAnimationPos`: how far the legs have swung, for a walk cycle
    /// (a vanilla quadruped's legs swing `cos(pos * 0.6662) * 1.4 * speed` radians).
    pub walk_position: f32,
    /// `walkAnimationSpeed`: how fast it walks, 0 standing to about 1.
    pub walk_speed: f32,
    /// Ticks left of the red hurt flash (10 when hit).
    pub hurt_time: i32,
    /// Ticks since it died, 0 while alive (vanilla tips the body over 20).
    pub death_time: i32,
    pub on_ground: bool,
}

impl<'a> NativeMobRender<'a> {
    /// The render state of `m` at `partial` (`LivingEntityRenderer.extractRenderState`).
    pub(super) fn of(m: &'a Mob, partial: f32) -> Self {
        let body = anim::body_rot(m, partial);
        // The walk cycle, head turn and age as the vanilla models read them:
        // no walking while riding or dead, a baby's legs three times as far
        // (`WalkAnimationState.position(partial)`, `positionScale`).
        let pose = anim::state(m, partial);
        Self {
            id: m.id,
            kind: &m.kind.id,
            custom_name: m.custom_name.as_deref(),
            baby: m.is_baby(),
            partial,
            age: pose.age,
            body_yaw: body,
            head_yaw: pose.y_rot,
            pitch: pose.x_rot,
            walk_position: pose.pos,
            walk_speed: pose.speed,
            hurt_time: m.hurt_time,
            death_time: m.death_time,
            on_ground: m.on_ground,
        }
    }
}

/// Which player a [`NativeMobRender`] is, for `native_player_submit` and
/// `native_mob_layer`.
// Native mods read it; nothing in Horizon does.
#[derive(Clone, Copy, Debug)]
pub struct NativePlayerRender<'a> {
    /// The player's profile name.
    pub name: &'a str,
    /// Whether it is the player of this window, drawn in third person.
    pub local: bool,
}

/// One armour stand's frame, for `native_armor_stand_layer`. API 416.
// Native mods read it; nothing in Horizon does.
#[derive(Clone, Copy, Debug)]
pub struct NativeArmorStandRender<'a> {
    /// The stand's network id.
    pub id: u32,
    /// `isSmall`: the half-size stand, whose armour model is half size too.
    pub small: bool,
    /// `isInvisible`: its own model is left out; its layers are drawn.
    pub invisible: bool,
    /// The frame's partial tick, 0 to 1.
    pub partial: f32,
    /// The way it faces (0 is south, 90 west), with a hit's wobble: `draw`'s turn.
    pub body_yaw: f32,
    /// Main hand, off hand, feet, legs, chest, head (`EquipmentSlot` order).
    pub equipment: &'a [Option<ItemStack>; 6],
}

/// Where a humanoid model's parts are drawn this frame, for the layers of
/// `native_armor_stand_layer` and `native_player_preview_layer`: a layer's model
/// follows the head, body, arms and legs as a vanilla armour layer follows its
/// parent model (`HumanoidModel.copyPropertiesTo`). API 416.
// Native mods read it; nothing in Horizon does.
pub struct NativeModelParts<'a> {
    /// The hook's frame (`draw` as the hook gets it), undone.
    from: Mat4,
    of: PartsOf<'a>,
}

/// What a [`NativeModelParts`] walks.
enum PartsOf<'a> {
    /// A mob's posed body at a partial tick, from the camera (`part_transform`).
    Mob(&'a Mob, f32, DVec3, &'a MobData),
    /// A model posed by `Poses` under its `setupRotations` (`part_matrix`).
    Model(&'a Part, Poses, Mat4),
}

impl<'a> NativeModelParts<'a> {
    /// The parts of `m`'s body at `partial`, from `camera`, for a hook drawn at `pose`.
    pub(super) fn of_mob(
        m: &'a Mob,
        partial: f32,
        camera: DVec3,
        data: &'a MobData,
        pose: Mat4,
    ) -> Self {
        let of = PartsOf::Mob(m, partial, camera, data);
        Self {
            from: pose.inverse(),
            of,
        }
    }
    /// The parts of `model` posed by `poses` under `base`, for a hook drawn at `pose`.
    pub(super) fn of_model(model: &'a Part, poses: Poses, base: Mat4, pose: Mat4) -> Self {
        let of = PartsOf::Model(model, poses, base);
        Self {
            from: pose.inverse(),
            of,
        }
    }
    /// Moves `draw`'s pose onto the part `name` (`head`, `hat`, `body`,
    /// `right_arm`, `left_arm`, `right_leg`, `left_leg`) where it is drawn this
    /// frame (`ModelPart.translateAndRotate` down the tree): its pivot at the
    /// origin, turned and scaled as the part, +y down as in a model's own
    /// space, so a `ModelPart` drawn there (`native_draw_model_part`) sits as
    /// the vanilla part does. Call it in the hook's frame, between
    /// `draw.push()` and `draw.pop()`. False, `draw` unchanged, for a part the
    /// model lacks.
    pub fn place(&self, name: &str, draw: &mut BlockEntityDraw) -> bool {
        let at = match &self.of {
            PartsOf::Mob(m, partial, camera, data) => {
                render::part_transform(m, *partial, *camera, data, name)
            }
            PartsOf::Model(model, poses, base) => {
                render::part_matrix(model, *base, poses, name, 0.0)
            }
        };
        at.map(|at| draw.apply(self.from * at)).is_some()
    }
}

/// The pose a mod's mob model starts from: at the feet, `at` (relative to the
/// camera), turned with the body so +z is the way it faces, +x its left and +y
/// up, in blocks.
pub(super) fn mob_pose(at: DVec3, body_yaw: f32) -> Mat4 {
    Mat4::from_translation(at.as_vec3()) * Mat4::from_rotation_y((-body_yaw).to_radians())
}

impl Game<'_> {
    /// `native_mob_submit` for `mob`, lit by `light` (packed as the entity
    /// pass packs it): true when a mod drew it, and its vanilla model, armour
    /// and held items are left out. The name tag, shadow, fire and a glowing
    /// outline still draw as for any mob.
    pub(super) fn mod_mob_geometry(
        &self,
        mob: &Mob,
        partial: f32,
        camera: DVec3,
        light: u32,
        out: &mut Vec<Vertex>,
    ) -> bool {
        self.mod_entity_draw(mob, partial, camera, light, out, |render, draw| {
            let mut drawn = false;
            self.native_mob_submit(render, draw, &mut drawn);
            drawn
        })
    }
    /// `native_player_submit` for the player `mob` (`who` says which): true
    /// when a mod drew it and the vanilla model, armour, held items, cape and
    /// other layers are left out.
    pub(super) fn mod_player_geometry(
        &self,
        mob: &Mob,
        who: NativePlayerRender<'_>,
        partial: f32,
        camera: DVec3,
        light: u32,
        out: &mut Vec<Vertex>,
    ) -> bool {
        // Posing a player for the mods costs, each frame: only when one listens.
        let mut listened = false;
        self.native_player_render_listened(&mut listened);
        if !listened {
            return false;
        }
        self.mod_entity_draw(mob, partial, camera, light, out, |render, draw| {
            let mut drawn = false;
            self.native_player_submit(render, &who, draw, &mut drawn);
            drawn
        })
    }
    /// `native_mob_layer` for `mob` (`who` set for a player): what a mod adds
    /// is kept, drawn after the vanilla model and its layers.
    pub(super) fn mod_mob_layer_geometry(
        &self,
        mob: &Mob,
        who: Option<NativePlayerRender<'_>>,
        partial: f32,
        camera: DVec3,
        light: u32,
        out: &mut Vec<Vertex>,
    ) {
        // Every mob in view each frame: only when a mod listens.
        let mut listened = false;
        self.native_player_render_listened(&mut listened);
        if !listened {
            return;
        }
        self.mod_entity_draw(mob, partial, camera, light, out, |render, draw| {
            self.native_mob_layer(render, who.as_ref(), draw);
            true
        });
    }
    /// `native_player_preview_layer` for the player `m` posed for a screen
    /// (`camera` at its feet), lit by the screen's `lights` turned into its
    /// frame: the range of `out` the mods added, its faces lit already.
    pub(super) fn mod_preview_layer(
        &self,
        m: &Mob,
        camera: DVec3,
        lights: (Vec3, Vec3),
        out: &mut Vec<Vertex>,
    ) -> Range<usize> {
        let render = NativeMobRender::of(m, 1.0);
        let feet = DVec3::new(m.position.x, m.position.y, m.position.z);
        let pose = mob_pose(feet - camera, render.body_yaw);
        let parts = NativeModelParts::of_mob(m, 1.0, camera, self.res.mob_data(), pose);
        let start = out.len();
        let mut draw = BlockEntityDraw::posed(
            1.0,
            self.world.game_time,
            240 | 240 << 8,
            [feet.x, feet.y, feet.z].map(|c| c.floor() as i32),
            pose,
            lights,
            std::mem::take(out),
        );
        self.native_player_preview_layer(&render, &parts, &mut draw);
        *out = draw.into_vertices();
        start..out.len()
    }
    /// Gives `hook` the frame of `mob` and a draw posed at its feet; what it
    /// added to `out` stays when it answers true.
    fn mod_entity_draw(
        &self,
        mob: &Mob,
        partial: f32,
        camera: DVec3,
        light: u32,
        out: &mut Vec<Vertex>,
        hook: impl FnOnce(&NativeMobRender<'_>, &mut BlockEntityDraw) -> bool,
    ) -> bool {
        let render = NativeMobRender::of(mob, partial);
        let a = f64::from(partial);
        let feet = mob.previous.lerp(mob.position, a);
        let feet = DVec3::new(feet.x, feet.y, feet.z);
        let start = out.len();
        let mut draw = BlockEntityDraw::posed(
            partial,
            self.world.game_time,
            light,
            [feet.x, feet.y, feet.z].map(|c| c.floor() as i32),
            mob_pose(feet - camera, render.body_yaw),
            diffuse_lights(self.dim == Dim::Nether),
            std::mem::take(out),
        );
        let keep = hook(&render, &mut draw);
        *out = draw.into_vertices();
        if !keep {
            out.truncate(start);
        }
        keep
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    #[test]
    fn the_pose_faces_where_the_body_faces() {
        // Entity yaw 0 faces south (+z), 90 west (-x), 180 north, 270 east.
        for (yaw, ahead, left) in [
            (0.0, Vec3::Z, Vec3::X),
            (90.0, Vec3::NEG_X, Vec3::Z),
            (180.0, Vec3::NEG_Z, Vec3::NEG_X),
            (270.0, Vec3::X, Vec3::NEG_Z),
        ] {
            let pose = mob_pose(DVec3::ZERO, yaw);
            assert!(close(pose.transform_vector3(Vec3::Z), ahead), "{yaw}");
            assert!(close(pose.transform_vector3(Vec3::X), left), "{yaw}");
            assert!(close(pose.transform_vector3(Vec3::Y), Vec3::Y), "{yaw}");
        }
    }

    #[test]
    fn the_pose_starts_at_the_feet() {
        let at = DVec3::new(3.0, -1.5, 0.25);
        let pose = mob_pose(at, 37.0);
        assert!(close(pose.transform_point3(Vec3::ZERO), at.as_vec3()));
    }
}
