//! User-added keys share drawing/hit geometry and retain per-finger ownership.
use super::*;
use layout::Placement;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomButton {
    pub key: String,
    pub layouts: [Placement; 3],
    /// A tap switches the key on, the next tap off (#96); otherwise the key is held
    /// while the finger is down.
    #[serde(default)]
    pub toggle: bool,
}
impl CustomButton {
    pub fn new(key: String, index: usize) -> Self {
        Self {
            key,
            layouts: [Placement::new(
                0.68 + (index % 3) as f32 * 0.1,
                0.35 + (index / 3 % 3) as f32 * 0.12,
                0.105,
            ); 3],
            toggle: false,
        }
    }
    pub fn valid_token(token: &str) -> bool {
        token == "key.keyboard.unknown"
            || crate::keybinds::key_code(token).is_some()
            || token
                .strip_prefix("action:")
                .is_some_and(|name| crate::keybinds::MAPPINGS.iter().any(|m| m.0 == name))
    }
    pub fn label(&self) -> String {
        if self.key == "key.keyboard.unknown" {
            return "Button".into();
        }
        if let Some(name) = self.key.strip_prefix("action:") {
            return crate::text::Text::translate(name, &[]).plain();
        }
        crate::keybinds::display_name(&self.key).plain()
    }
}
pub(super) fn choices(raw_keys: bool) -> Vec<(String, String)> {
    if raw_keys {
        crate::keybinds::available_keys()
            .iter()
            .map(|(name, _)| ((*name).into(), crate::keybinds::display_name(name).plain()))
            .collect()
    } else {
        crate::keybinds::MAPPINGS
            .iter()
            .filter(|m| m.1 != "key.keyboard.unknown")
            .map(|(name, _, _)| {
                (
                    format!("action:{name}"),
                    crate::text::Text::translate(name, &[]).plain(),
                )
            })
            .collect()
    }
}
impl TouchControls {
    pub(super) fn inventory_token(control: Control) -> Option<&'static str> {
        match control {
            Control::Sneak => Some("key.keyboard.left.shift"),
            Control::Attack => Some("action:key.attack"),
            Control::Use => Some("action:key.use"),
            _ => None,
        }
    }
    pub fn inventory_update(
        &mut self,
        id: u64,
        point: [f32; 2],
        started: bool,
        ended: bool,
    ) -> bool {
        if self.custom_fingers.contains_key(&id) {
            return self.custom_update(id, point, false, ended);
        }
        if started
            && self.custom_at(point).is_none()
            && let Some(token) = self.control_at(point).and_then(Self::inventory_token)
        {
            let pressed = !self.custom_fingers.values().any(|v| v == token);
            self.custom_fingers.insert(id, token.into());
            self.fingers.insert(id, (Role::Ignore, point));
            if pressed {
                self.custom_events.push((token.into(), true));
            }
            return true;
        }
        false
    }
    pub(super) fn fit_placement(&self, mut p: Placement) -> Placement {
        p.size = (p.size * self.settings.button_scale)
            .min(self.aspect())
            .min(1.0);
        let half = [p.size / self.aspect() / 2.0, p.size / 2.0];
        for (i, half) in half.into_iter().enumerate() {
            p.center[i] = p.center[i].clamp(half, 1.0 - half);
        }
        p
    }
    pub(super) fn custom_placement(&self, index: usize) -> Placement {
        let mut p = self.settings.custom_buttons[index].layouts[self.settings.mode.index()];
        if self.settings.left_handed {
            p.center[0] = 1.0 - p.center[0];
        }
        self.fit_placement(p)
    }
    /// Size of a newly created custom button, before the global button scale.
    pub(super) const NEW_CUSTOM_SIZE: f32 = 0.105;
    /// Where a new custom button goes in layout `mode`: the first spot overlapping no
    /// visible control and none of the first `customs` custom buttons. Scans above the
    /// action buttons, then above the joystick, then the whole screen; the stored
    /// centre is unmirrored like every saved layout.
    pub(super) fn free_custom_spot(&mut self, mode: usize, customs: usize) -> [f32; 2] {
        let saved = self.settings.mode;
        self.settings.mode = Mode::ALL[mode];
        let aspect = self.aspect();
        let mut taken: Vec<[f32; 4]> = Control::ALL
            .into_iter()
            .filter(|c| self.settings.visible(*c))
            .map(|c| self.fit_placement(self.placement(c)).rect(aspect))
            .collect();
        taken.extend(
            (0..customs.min(self.settings.custom_buttons.len()))
                .map(|i| self.custom_placement(i).rect(aspect)),
        );
        self.settings.mode = saved;
        let size = (Self::NEW_CUSTOM_SIZE * self.settings.button_scale)
            .min(aspect)
            .min(1.0);
        let free = |center: [f32; 2]| {
            let [x, y, w, h] = Placement::new(center[0], center[1], size).rect(aspect);
            let gap = 0.01;
            x >= gap
                && y >= gap
                && x + w <= 1.0 - gap
                && y + h <= 1.0 - gap
                && taken.iter().all(|[tx, ty, tw, th]| {
                    x + w + gap <= *tx
                        || *tx + tw + gap <= x
                        || y + h + gap <= *ty
                        || *ty + th + gap <= y
                })
        };
        let step = [size / aspect / 2.0, size / 2.0];
        let regions = [
            ([0.62, 0.95], [0.14, 0.50], true),
            ([0.05, 0.38], [0.14, 0.50], false),
            ([0.05, 0.95], [0.10, 0.90], true),
        ];
        for (xs, ys, from_right) in regions {
            let mut y = ys[0];
            while y <= ys[1] {
                let mut i = 0.0;
                loop {
                    let x = if from_right {
                        xs[1] - i * step[0]
                    } else {
                        xs[0] + i * step[0]
                    };
                    if !(xs[0]..=xs[1]).contains(&x) {
                        break;
                    }
                    if free([x, y]) {
                        let x = if self.settings.left_handed {
                            1.0 - x
                        } else {
                            x
                        };
                        return [x, y];
                    }
                    i += 1.0;
                }
                y += step[1];
            }
        }
        // A screen full of buttons: fall back to the old fixed grid.
        CustomButton::new(String::new(), customs).layouts[mode].center
    }
    /// A new custom button placed in free space in every scheme's layout.
    pub(super) fn new_custom_button(&mut self, key: String) -> CustomButton {
        let customs = self.settings.custom_buttons.len();
        let mut button = CustomButton::new(key, customs);
        for mode in 0..3 {
            let [x, y] = self.free_custom_spot(mode, customs);
            button.layouts[mode] = Placement::new(x, y, Self::NEW_CUSTOM_SIZE);
        }
        button
    }
    pub fn custom_at(&self, point: [f32; 2]) -> Option<usize> {
        (0..self.settings.custom_buttons.len())
            .rev()
            .find(|&i| layout::contains(self.custom_placement(i).rect(self.aspect()), point))
    }
    pub(super) fn custom_update(
        &mut self,
        id: u64,
        point: [f32; 2],
        started: bool,
        ended: bool,
    ) -> bool {
        if let Some(key) = self.custom_fingers.get(&id).cloned() {
            if ended {
                self.custom_fingers.remove(&id);
                self.fingers.remove(&id);
                if !self.custom_key_held(&key, None) {
                    self.custom_events.push((key, false));
                }
            }
            return true;
        }
        if started && let Some(index) = self.custom_at(point) {
            let key = self.settings.custom_buttons[index].key.clone();
            if key == "key.keyboard.unknown" {
                return false;
            }
            if self.settings.custom_buttons[index].toggle {
                // The finger only flips the switch; its lift does nothing.
                self.fingers.insert(id, (Role::Ignore, point));
                let on = !self.custom_latched.remove(&index);
                if on {
                    self.custom_latched.insert(index);
                }
                if !self.custom_key_held(&key, Some(index)) {
                    self.custom_events.push((key, on));
                }
                self.clicked = true;
                return true;
            }
            let pressed = !self.custom_key_held(&key, None);
            self.custom_fingers.insert(id, key.clone());
            self.fingers.insert(id, (Role::Ignore, point));
            if pressed {
                self.custom_events.push((key, true));
            }
            self.clicked = true;
            return true;
        }
        false
    }
    /// Whether a finger or a switched-on toggle other than `except` holds `key`.
    fn custom_key_held(&self, key: &str, except: Option<usize>) -> bool {
        self.custom_fingers.values().any(|k| k == key)
            || self.custom_latched.iter().any(|&i| {
                Some(i) != except
                    && self
                        .settings
                        .custom_buttons
                        .get(i)
                        .is_some_and(|b| b.key == key)
            })
    }
    pub fn held_custom_tokens(&self) -> impl Iterator<Item = &str> {
        let latched = self
            .custom_latched
            .iter()
            .filter_map(|&i| self.settings.custom_buttons.get(i))
            .map(|b| b.key.as_str());
        self.custom_fingers
            .values()
            .map(String::as_str)
            .chain(latched)
    }
    pub fn key_events(&mut self) -> Vec<(String, bool)> {
        std::mem::take(&mut self.custom_events)
    }
    /// Releases every held custom key, switched-on toggles included: a screen or
    /// focus loss releases them, as it releases held keys.
    pub(super) fn clear_custom(&mut self) {
        let latched: Vec<String> = std::mem::take(&mut self.custom_latched)
            .into_iter()
            .filter_map(|i| self.settings.custom_buttons.get(i))
            .map(|b| b.key.clone())
            .collect();
        let keys: std::collections::BTreeSet<_> = std::mem::take(&mut self.custom_fingers)
            .into_values()
            .chain(latched)
            .collect();
        self.custom_events
            .extend(keys.into_iter().map(|k| (k, false)));
    }
    pub(super) fn draw_custom(&self, b: &mut HudBatch, a: &HudAtlas) {
        for (i, button) in self.settings.custom_buttons.iter().enumerate() {
            let p = self.custom_placement(i);
            let size = p.size * b.height;
            let rect = [
                p.center[0] * b.width - size / 2.0,
                p.center[1] * b.height - size / 2.0,
                size,
                size,
            ];
            let pressed = self.custom_latched.contains(&i)
                || self.custom_fingers.values().any(|key| *key == button.key);
            let opacity = if self.editor.is_some() {
                p.opacity.max(0.25)
            } else {
                p.opacity
            };
            if opacity <= 0.0 {
                continue;
            }
            text_button(
                b,
                a,
                rect,
                &button.label(),
                pressed,
                rgba(255, 255, 255, (opacity * 255.0) as u8),
                true,
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_toggle_button_switches_its_key_on_and_off() {
        let mut t = TouchControls::default();
        t.viewport(2340, 1080);
        let mut button = t.new_custom_button("key.keyboard.z".into());
        button.toggle = true;
        t.settings.custom_buttons.push(button);
        let at = t.custom_placement(0).center;
        t.update(1, at, true, false);
        t.update(1, at, false, true);
        assert_eq!(t.key_events(), vec![("key.keyboard.z".to_owned(), true)]);
        assert!(t.held_custom_tokens().any(|k| k == "key.keyboard.z"));
        t.update(2, at, true, false);
        t.update(2, at, false, true);
        assert_eq!(t.key_events(), vec![("key.keyboard.z".to_owned(), false)]);
        t.update(3, at, true, false);
        t.clear();
        assert_eq!(
            t.key_events(),
            vec![
                ("key.keyboard.z".to_owned(), true),
                ("key.keyboard.z".to_owned(), false)
            ],
            "a screen releases a switched-on toggle"
        );
    }
    #[test]
    fn new_custom_buttons_avoid_controls_and_each_other() {
        let mut t = TouchControls::default();
        t.viewport(2340, 1080);
        let aspect = 2340.0 / 1080.0;
        for _ in 0..12 {
            let button = t.new_custom_button("key.keyboard.z".into());
            t.settings.custom_buttons.push(button);
        }
        let mode = t.settings.mode.index();
        let rects: Vec<[f32; 4]> = (0..12)
            .map(|i| t.custom_placement(i).rect(aspect))
            .collect();
        let controls: Vec<[f32; 4]> = Control::ALL
            .into_iter()
            .filter(|c| t.settings.visible(*c))
            .map(|c| t.fit_placement(t.placement(c)).rect(aspect))
            .collect();
        let overlap = |a: &[f32; 4], b: &[f32; 4]| {
            a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
        };
        for (i, a) in rects.iter().enumerate() {
            for c in &controls {
                assert!(
                    !overlap(a, c),
                    "custom {i} covers a control in layout {mode}"
                );
            }
            for b in &rects[..i] {
                assert!(
                    !overlap(a, b),
                    "custom {i} is stacked on another custom button"
                );
            }
        }
    }
    #[test]
    fn inventory_modifier_and_mouse_hold_independently_and_cancel_cleanly() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        t.inventory_mode = true;
        t.settings.mode = Mode::Crosshair;
        for (id, control, token) in [
            (1, Control::Sneak, "key.keyboard.left.shift"),
            (2, Control::Attack, "action:key.attack"),
            (3, Control::Use, "action:key.use"),
        ] {
            assert!(t.visible(control));
            assert!(t.inventory_update(id, t.placement(control).center, true, false));
            assert_eq!(t.key_events(), [(token.into(), true)]);
        }
        t.clear_world_gesture();
        assert_eq!(t.held_custom_tokens().count(), 3);
        assert_eq!(t.poll_state(), PadState::default());
        assert!(t.inventory_update(2, [0.0, 0.0], false, true));
        assert_eq!(t.key_events(), [("action:key.attack".into(), false)]);
        t.clear();
        let released = t.key_events();
        assert!(released.contains(&("key.keyboard.left.shift".into(), false)));
        assert!(released.contains(&("action:key.use".into(), false)));
    }
    #[test]
    fn inventory_overlay_preserves_configured_visibility_and_placements() {
        let mut touch = TouchControls::default();
        for mode in Mode::ALL {
            touch.settings.mode = mode;
            touch.settings.button_visibility[Control::Attack as usize] = 2;
            let before: Vec<_> = Control::ALL
                .into_iter()
                .map(|c| (c, touch.visible(c), touch.placement(c).center))
                .collect();
            touch.inventory_mode = true;
            for (control, visible, center) in before {
                assert_eq!(touch.visible(control), visible);
                assert_eq!(touch.placement(control).center, center);
            }
            touch.inventory_mode = false;
        }
    }
    #[test]
    fn unassigned_button_emits_no_input() {
        let mut touch = TouchControls::default();
        touch.viewport(1920, 1080);
        touch
            .settings
            .custom_buttons
            .push(CustomButton::new("key.keyboard.unknown".into(), 0));
        let point = touch.custom_placement(0).center;
        touch.update(1, point, true, false);
        assert!(touch.key_events().is_empty());
    }
    #[test]
    fn custom_keys_hold_through_motion_and_shared_keys_release_only_after_last_finger() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        t.settings
            .custom_buttons
            .push(CustomButton::new("key.keyboard.z".into(), 0));
        let p = t.custom_placement(0).center;
        t.update(1, p, true, false);
        t.update(2, p, true, false);
        assert_eq!(t.key_events(), [("key.keyboard.z".into(), true)]);
        t.update(1, [0.99, 0.1], false, false);
        assert!(t.key_events().is_empty());
        t.update(1, p, false, true);
        assert!(t.key_events().is_empty());
        t.update(2, p, false, true);
        assert_eq!(t.key_events(), [("key.keyboard.z".into(), false)]);
    }
    #[test]
    fn cancellation_releases_all_custom_keys_and_scaling_preserves_saved_sizes() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        t.settings
            .custom_buttons
            .push(CustomButton::new("key.keyboard.z".into(), 0));
        let stored = t.settings.custom_buttons[0].layouts[0].size;
        t.settings.button_scale = 2.0;
        assert_eq!(t.custom_placement(0).size, stored * 2.0);
        t.update(1, t.custom_placement(0).center, true, false);
        t.key_events();
        t.clear();
        assert_eq!(t.key_events(), [("key.keyboard.z".into(), false)]);
        assert_eq!(t.settings.custom_buttons[0].layouts[0].size, stored);
    }
}
