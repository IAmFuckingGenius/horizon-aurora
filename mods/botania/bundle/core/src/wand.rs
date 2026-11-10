//! The Wand of the Forest (`WandOfTheForestItem`): what a click with it does on a block, a
//! spreader turned by a sneak-click in function mode (`ManaSpreaderBlockEntity.onUsedByWand`),
//! the look ray that click is found along (`Entity.calculateViewVector`), and the rainbow beam
//! drawn when a binding succeeds (`doParticleBeam`, `Mth.hsvToRgb`).

use crate::burst::Aabb;
use crate::mth;

/// What the wand uses when it doesn't bind (`Wandable`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wandable {
    Spreader,
    Pool,
}

/// A right click with the wand on a block, as `useOn` sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    /// The block clicked.
    pub pos: [i32; 3],
    /// `isSecondaryUseActive` / `isShiftKeyDown`.
    pub sneaking: bool,
    /// `getBindMode`: true unless switched to function mode.
    pub bind_mode: bool,
    /// `getBindingAttempt`: the block selected by an earlier click.
    pub selected: Option<[i32; 3]>,
    /// The selected block still has a block entity the wand binds (`WandBindable`).
    pub selected_bindable: bool,
    /// The clicked block has one (`WandBindable.canSelect` is always true for those ported).
    pub bindable: bool,
    /// The clicked block's `Wandable`, if any.
    pub wandable: Option<Wandable>,
}

/// What the click does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseOn {
    /// `tryCompleteBinding`: `from` binds to `to` (`WandBindable.bindTo`).
    Bind { from: [i32; 3], to: [i32; 3] },
    /// The selection becomes this block, or none (`setBindingAttempt`), with a `ding`.
    Select(Option<[i32; 3]>),
    /// A sneak-click on a spreader: it turns away from the point clicked.
    AimSpreader,
    /// A plain click on a spreader: it is sent to the players again.
    SyncSpreader,
    /// A sneak-click on a pool: it toggles `outputting`.
    TogglePool,
    /// A plain click on a pool: used, nothing changes.
    Used,
    /// The wand does nothing here (`InteractionResult.PASS`): the item's `use` may follow.
    Pass,
}

/// The click's result: what it does and the wand's selection afterwards, when that changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub action: UseOn,
    pub selection: Option<Option<[i32; 3]>>,
}

/// `WandOfTheForestItem.useOn` for the blocks ported: completing a binding, then selecting a
/// bindable block in bind mode, then the block's own wand use. Rotating other blocks on a
/// sneak-click (`manipulateBlockstate`) isn't ported: such a click passes.
pub fn use_on(c: &Click) -> Outcome {
    let mut selection = None;
    if c.sneaking
        && let Some(from) = c.selected
        && from != c.pos
    {
        // The selection is cleared whether or not the selected block can still bind.
        selection = Some(None);
        if c.selected_bindable {
            return Outcome {
                action: UseOn::Bind { from, to: c.pos },
                selection,
            };
        }
    }
    if c.bind_mode && c.bindable && c.sneaking {
        // The selection read before this click: clicking the selected block deselects it.
        let next = if c.selected == Some(c.pos) {
            None
        } else {
            Some(c.pos)
        };
        return Outcome {
            action: UseOn::Select(next),
            selection: Some(next),
        };
    }
    let action = match c.wandable {
        Some(Wandable::Spreader) if c.sneaking => UseOn::AimSpreader,
        Some(Wandable::Spreader) => UseOn::SyncSpreader,
        Some(Wandable::Pool) if c.sneaking => UseOn::TogglePool,
        Some(Wandable::Pool) => UseOn::Used,
        None => UseOn::Pass,
    };
    Outcome { action, selection }
}

/// `Entity.calculateViewVector(xRot, yRot)` for a player looking along `yaw` and `pitch`
/// (degrees), with `Mth`'s table sine.
pub fn view_vector(yaw: f32, pitch: f32) -> [f64; 3] {
    let rad = std::f32::consts::PI / 180.0;
    let f = pitch * rad;
    let g = -yaw * rad;
    let h = mth::cos(g);
    let i = mth::sin(g);
    let j = mth::cos(f);
    let k = mth::sin(f);
    [f64::from(i * j), f64::from(-k), f64::from(h * j)]
}

/// Where the look ray from `eye` along `dir` meets the face `face` (`[0, 1, 0]` is the top) of
/// the box `bounds` (absolute): the point `LexicaBotaniaItem.doRayTrace` finds for a click on that
/// face. The crossing of the face's plane, kept on the face; the face's centre when the ray runs
/// along the plane.
pub fn face_hit(eye: [f64; 3], dir: [f64; 3], face: [i32; 3], bounds: Aabb) -> [f64; 3] {
    let axis = (0..3).find(|&i| face[i] != 0).unwrap_or(1);
    let plane = if face[axis] > 0 {
        bounds[axis + 3]
    } else {
        bounds[axis]
    };
    let mut p = [
        (bounds[0] + bounds[3]) / 2.0,
        (bounds[1] + bounds[4]) / 2.0,
        (bounds[2] + bounds[5]) / 2.0,
    ];
    if dir[axis].abs() > 1.0e-9 {
        let t = (plane - eye[axis]) / dir[axis];
        for i in 0..3 {
            p[i] = (eye[i] + t * dir[i]).clamp(bounds[i], bounds[i + 3]);
        }
    }
    p[axis] = plane;
    p
}

/// `ManaSpreaderBlockEntity.onUsedByWand` while sneaking: the spreader at `spreader` turns to face
/// away from `hit`, the point clicked on its face `face`. A side face sets the yaw from the
/// point's angle around the block's centre; any face sets the pitch from its height
/// (`rotationY = -y * 180`). Returns `(rotationX, rotationY)`; `rotation_x` is kept for the top
/// and bottom.
pub fn aim_from_click(
    spreader: [i32; 3],
    hit: [f64; 3],
    face: [i32; 3],
    rotation_x: f32,
) -> (f32, f32) {
    let x = hit[0] - f64::from(spreader[0]) - 0.5;
    let y = hit[1] - f64::from(spreader[1]) - 0.5;
    let z = hit[2] - f64::from(spreader[2]) - 0.5;
    let mut rx = rotation_x;
    if face[1] == 0 {
        // Vec3(x, 0, z) against Vec3(-0.5, 0, 0).
        let dot = x * -0.5 + 0.0 * 0.0 + z * 0.0;
        let length = (x * x + z * z).sqrt();
        let angle = (dot / (0.5 * length)).acos() * 180.0 / std::f64::consts::PI;
        rx = angle as f32 + 180.0;
        if z < 0.0 {
            rx = 360.0 - rx;
        }
    }
    let ry = -((y * 180.0) as f32);
    (rx, ry)
}

/// `Mth.hsvToRgb(hue, saturation, value)` as `0xRRGGBB`.
pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> u32 {
    let i = ((hue * 6.0) as i32) % 6;
    let f = hue * 6.0 - i as f32;
    let g = value * (1.0 - saturation);
    let h = value * (1.0 - f * saturation);
    let j = value * (1.0 - (1.0 - f) * saturation);
    let (k, l, m) = match i {
        0 => (value, j, g),
        1 => (h, value, g),
        2 => (g, value, j),
        3 => (g, h, value),
        4 => (j, g, value),
        _ => (value, g, h),
    };
    let byte = |v: f32| ((v * 255.0) as i32).clamp(0, 255) as u32;
    (byte(k) << 16) | (byte(l) << 8) | byte(m)
}

/// `doParticleBeam` from `from` to `to`: a sparkle every `step` blocks, its colour going once
/// round the rainbow from `hue` along the beam. Botania steps 0.05 blocks; the port passes a
/// longer step and at most `max` points, for phones.
pub fn beam(from: [f64; 3], to: [f64; 3], step: f64, max: usize, hue: f32) -> Vec<([f64; 3], u32)> {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if step <= 0.0 || !length.is_finite() {
        return Vec::new();
    }
    let n = ((length / step) as usize).min(max);
    (0..n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let at = [from[0] + d[0] * t, from[1] + d[1] * t, from[2] + d[2] * t];
            let h = i as f32 / n as f32 + hue;
            (at, hsv_to_rgb(h - h.floor(), 1.0, 1.0))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(pos: [i32; 3]) -> Click {
        Click {
            pos,
            sneaking: true,
            bind_mode: true,
            selected: None,
            selected_bindable: false,
            bindable: false,
            wandable: None,
        }
    }

    #[test]
    fn bind_mode_selects_then_binds_on_the_next_sneak_click() {
        // A sneak-click on a flower selects it (bind mode is the default).
        let flower = [0, 64, 0];
        let spreader = [3, 64, 0];
        let first = use_on(&Click {
            bindable: true,
            ..click(flower)
        });
        assert_eq!(first.action, UseOn::Select(Some(flower)));
        assert_eq!(first.selection, Some(Some(flower)));
        // A sneak-click elsewhere completes the binding and clears the selection, whatever
        // the clicked block is (`bindTo` judges it).
        let second = use_on(&Click {
            selected: Some(flower),
            selected_bindable: true,
            wandable: Some(Wandable::Spreader),
            bindable: true,
            ..click(spreader)
        });
        assert_eq!(
            second.action,
            UseOn::Bind {
                from: flower,
                to: spreader
            }
        );
        assert_eq!(second.selection, Some(None));
        // Clicking the selected block again deselects it.
        let again = use_on(&Click {
            selected: Some(flower),
            selected_bindable: true,
            bindable: true,
            ..click(flower)
        });
        assert_eq!(again.action, UseOn::Select(None));
    }

    #[test]
    fn a_selection_that_lost_its_block_is_dropped() {
        // The selected flower was broken: the selection clears, and a bindable block clicked
        // is selected instead; a plain block passes, still clearing it.
        let gone = [9, 64, 9];
        let out = use_on(&Click {
            selected: Some(gone),
            bindable: true,
            ..click([0, 64, 0])
        });
        assert_eq!(out.action, UseOn::Select(Some([0, 64, 0])));
        let out = use_on(&Click {
            selected: Some(gone),
            ..click([0, 64, 0])
        });
        assert_eq!(out.action, UseOn::Pass);
        assert_eq!(out.selection, Some(None));
    }

    #[test]
    fn without_sneaking_or_in_function_mode_the_block_uses_the_wand() {
        let spreader = Click {
            bindable: true,
            wandable: Some(Wandable::Spreader),
            ..click([0, 64, 0])
        };
        // Function mode, sneaking: the spreader is aimed.
        let out = use_on(&Click {
            bind_mode: false,
            ..spreader
        });
        assert_eq!(out.action, UseOn::AimSpreader);
        assert_eq!(out.selection, None);
        // Not sneaking: neither selection nor aim.
        let out = use_on(&Click {
            sneaking: false,
            ..spreader
        });
        assert_eq!(out.action, UseOn::SyncSpreader);
        // A selection isn't completed without sneaking.
        let out = use_on(&Click {
            sneaking: false,
            selected: Some([5, 64, 5]),
            selected_bindable: true,
            ..spreader
        });
        assert_eq!(out.action, UseOn::SyncSpreader);
        // Pools toggle `outputting` on a sneak-click and are only used otherwise.
        let pool = Click {
            wandable: Some(Wandable::Pool),
            ..click([0, 64, 0])
        };
        assert_eq!(use_on(&pool).action, UseOn::TogglePool);
        assert_eq!(
            use_on(&Click {
                sneaking: false,
                ..pool
            })
            .action,
            UseOn::Used
        );
        assert_eq!(use_on(&click([0, 64, 0])).action, UseOn::Pass);
    }

    #[test]
    fn view_vector_is_vanillas() {
        let close = |v: [f64; 3], e: [f64; 3]| (0..3).all(|i| (v[i] - e[i]).abs() < 1e-6);
        // Yaw 0 looks south (+Z), 90 west (-X), -90 east; pitch 90 looks down.
        assert!(close(view_vector(0.0, 0.0), [0.0, 0.0, 1.0]));
        assert!(close(view_vector(90.0, 0.0), [-1.0, 0.0, 0.0]));
        assert!(close(view_vector(-90.0, 0.0), [1.0, 0.0, 0.0]));
        assert!(close(view_vector(0.0, 90.0), [0.0, -1.0, 0.0]));
        let v = view_vector(0.0, 45.0);
        assert!((v[1] + std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-4);
    }

    #[test]
    fn a_sneak_click_turns_the_spreader_away_from_the_point() {
        let s = [0, 64, 0];
        let full = [0.0, 64.0, 0.0, 1.0, 65.0, 1.0];
        // The middle of each side: the spreader faces the opposite way, level.
        // East face (+X): faces west, rotationX 360 (= 0).
        assert_eq!(
            aim_from_click(s, [1.0, 64.5, 0.5], [1, 0, 0], 90.0),
            (360.0, -0.0)
        );
        // West face: faces east (180).
        assert_eq!(
            aim_from_click(s, [0.0, 64.5, 0.5], [-1, 0, 0], 90.0),
            (180.0, -0.0)
        );
        // South face (+Z): faces north (270); north face: faces south (90).
        assert_eq!(
            aim_from_click(s, [0.5, 64.5, 1.0], [0, 0, 1], 0.0),
            (270.0, -0.0)
        );
        assert_eq!(
            aim_from_click(s, [0.5, 64.5, 0.0], [0, 0, -1], 0.0),
            (90.0, -0.0)
        );
        // The top keeps the yaw and points the spreader straight down; low on a side face
        // points it up.
        assert_eq!(
            aim_from_click(s, [0.3, 65.0, 0.7], [0, 1, 0], 123.0),
            (123.0, -90.0)
        );
        let (_, ry) = aim_from_click(s, [1.0, 64.25, 0.5], [1, 0, 0], 0.0);
        assert_eq!(ry, 45.0);
        // A click on the east face's south edge: it faces north-west.
        let (rx, _) = aim_from_click(s, [1.0, 64.5, 1.0], [1, 0, 0], 0.0);
        assert!((rx - 315.0).abs() < 1e-3, "{rx}");
        // The point is found along the player's look: from the east, level with the centre.
        let eye = [4.0, 64.5, 0.5];
        let hit = face_hit(eye, view_vector(90.0, 0.0), [1, 0, 0], full);
        assert!((hit[0] - 1.0).abs() < 1e-9 && (hit[1] - 64.5).abs() < 1e-6);
        assert!((hit[2] - 0.5).abs() < 1e-6, "{hit:?}");
        // Looking down at the top from above its corner: the crossing is kept on the face.
        let hit = face_hit([3.0, 70.0, 3.0], [0.0, -1.0, 0.0], [0, 1, 0], full);
        assert_eq!(hit, [1.0, 65.0, 1.0]);
    }

    #[test]
    fn the_beam_goes_round_the_rainbow() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), 0xFF0000);
        assert_eq!(hsv_to_rgb(1.0 / 3.0, 1.0, 1.0), 0x00FF00);
        assert_eq!(hsv_to_rgb(0.5, 1.0, 1.0), 0x00FFFF);
        assert_eq!(hsv_to_rgb(2.0 / 3.0, 1.0, 1.0), 0x0000FF);
        assert_eq!(hsv_to_rgb(0.25, 0.0, 1.0), 0xFFFFFF);
        let b = beam([0.5, 64.5, 0.5], [6.5, 64.5, 0.5], 0.25, 64, 0.0);
        assert_eq!(b.len(), 24);
        assert_eq!(b[0], ([0.5, 64.5, 0.5], 0xFF0000));
        assert!(b.iter().all(|(p, _)| p[0] < 6.5));
        // Capped for long beams.
        assert_eq!(beam([0.0; 3], [100.0, 0.0, 0.0], 0.25, 64, 0.5).len(), 64);
        assert!(beam([0.0; 3], [0.0; 3], 0.25, 64, 0.0).is_empty());
    }
}
