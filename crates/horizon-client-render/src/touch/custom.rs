//! User-added keys share drawing/hit geometry and retain per-finger ownership.
use super::*;
use layout::Placement;
use serde::{Deserialize, Serialize};
/// How a custom button looks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Shape {
    /// The game's own button frame, tinted by the colour.
    #[default]
    Classic,
    /// A flat plate with pixel-stepped corners.
    Flat,
    /// A circle, or a capsule when the button is wider than tall.
    Round,
}
/// The colours a custom button can take (RGB).
pub const COLORS: [u32; 8] = [
    0x1d1a2e, 0x7656fa, 0x3b82f6, 0x22c55e, 0xeab308, 0xf97316, 0xef4444, 0xe5e7eb,
];
/// The most keys one button presses together.
pub const MAX_KEYS: usize = 4;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomButton {
    pub key: String,
    /// Keys pressed after `key` and released before it: a combination such as
    /// Ctrl+Q. Older files have none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub more_keys: Vec<String>,
    /// The player's own label; empty shows the keys' names.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    pub layouts: [Placement; 3],
    /// A tap switches the key on, the next tap off (#96); otherwise the key is held
    /// while the finger is down.
    #[serde(default)]
    pub toggle: bool,
    /// A finger that slides onto it from another sliding button presses it, and
    /// releases it when it slides off (combos, quick hotkeys).
    #[serde(default)]
    pub swipe: bool,
    /// Where it shows ([`Show`] as a byte; Auto means while playing).
    #[serde(default)]
    pub show: u8,
    #[serde(default)]
    pub shape: Shape,
    /// An index into [`COLORS`].
    #[serde(default)]
    pub color: u8,
    /// Width as a multiple of the height (1 is square).
    #[serde(default = "one")]
    pub width: f32,
}
fn one() -> f32 {
    1.0
}
impl CustomButton {
    pub fn new(key: String, index: usize) -> Self {
        Self {
            key,
            more_keys: Vec::new(),
            label: String::new(),
            layouts: [Placement::new(
                0.68 + (index % 3) as f32 * 0.1,
                0.35 + (index / 3 % 3) as f32 * 0.12,
                0.105,
            ); 3],
            toggle: false,
            swipe: false,
            show: 0,
            shape: Shape::Classic,
            color: 0,
            width: 1.0,
        }
    }
    pub fn valid_token(token: &str) -> bool {
        token == "key.keyboard.unknown"
            || crate::keybinds::key_code(token).is_some()
            || token
                .strip_prefix("action:")
                .is_some_and(|name| crate::keybinds::MAPPINGS.iter().any(|m| m.0 == name))
    }
    /// Every key it presses, in order; unassigned ones are left out.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.key.as_str())
            .chain(self.more_keys.iter().map(String::as_str))
            .filter(|k| *k != "key.keyboard.unknown")
    }
    /// The key slots shown in the editor: `key` and the extra ones.
    pub fn slots(&self) -> Vec<&str> {
        std::iter::once(self.key.as_str())
            .chain(self.more_keys.iter().map(String::as_str))
            .collect()
    }
    /// Puts `token` in slot `slot` (0 is `key`), appending a new slot past the end.
    pub fn set_slot(&mut self, slot: usize, token: String) {
        if slot == 0 {
            self.key = token;
        } else if let Some(k) = self.more_keys.get_mut(slot - 1) {
            *k = token;
        } else if self.more_keys.len() + 1 < MAX_KEYS {
            self.more_keys.push(token);
        }
    }
    /// Removes slot `slot`; the first remaining key becomes `key`.
    pub fn remove_slot(&mut self, slot: usize) {
        if slot == 0 {
            if self.more_keys.is_empty() {
                self.key = "key.keyboard.unknown".into();
            } else {
                self.key = self.more_keys.remove(0);
            }
        } else if slot - 1 < self.more_keys.len() {
            self.more_keys.remove(slot - 1);
        }
    }
    pub fn show(&self) -> Show {
        Show::from_u8(self.show)
    }
    /// Shown while playing, and over screens.
    pub fn visible(&self, menu: bool) -> bool {
        let show = self.show();
        if menu {
            show.in_menus().unwrap_or(false)
        } else {
            show.in_game().unwrap_or(true)
        }
    }
    /// One key's name, in the game's language.
    pub fn token_label(token: &str) -> String {
        if token == "key.keyboard.unknown" {
            return crate::lang::get("horizon.touch.custom.unassigned");
        }
        if let Some(name) = token.strip_prefix("action:") {
            return crate::text::Text::translate(name, &[]).plain();
        }
        crate::keybinds::display_name(token).plain()
    }
    /// The label drawn on the button: the player's own, or its keys joined by `+`.
    pub fn label(&self) -> String {
        if !self.label.trim().is_empty() {
            return self.label.clone();
        }
        let names: Vec<String> = self.keys().map(Self::token_label).collect();
        if names.is_empty() {
            Self::token_label("key.keyboard.unknown")
        } else {
            names.join("+")
        }
    }
    pub fn sanitize(&mut self) {
        self.more_keys.truncate(MAX_KEYS - 1);
        self.more_keys.retain(|k| Self::valid_token(k));
        if !self.width.is_finite() {
            self.width = 1.0;
        }
        self.width = self.width.clamp(1.0, 4.0);
        self.color = self.color.min(COLORS.len() as u8 - 1);
        if self.show > 4 {
            self.show = 0;
        }
        self.label = self.label.chars().take(24).collect();
        for p in &mut self.layouts {
            p.sanitize();
        }
    }
}
pub fn choices(raw_keys: bool) -> Vec<(String, String)> {
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
/// What a finger holds down.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Held {
    /// A custom button, by index.
    Button(usize),
    /// A built-in control's key over a screen (Shift, a mouse button...).
    Token(&'static str),
    /// A sliding finger between sliding buttons: it holds nothing until it is on one.
    Slid,
}
impl TouchControls {
    pub(super) fn inventory_token(control: Control) -> Option<&'static str> {
        match control {
            Control::Sneak => Some("key.keyboard.left.shift"),
            Control::Attack => Some("action:key.attack"),
            Control::Use => Some("action:key.use"),
            Control::Offhand => Some("action:key.swapOffhand"),
            _ => None,
        }
    }
    /// A finger over a screen: custom buttons shown there, and the modifier and
    /// mouse keys of built-in controls. Returns whether the touch was taken.
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
        if started && self.custom_update(id, point, true, ended) {
            return true;
        }
        if started
            && self.custom_at(point).is_none()
            && let Some(token) = self.control_at(point).and_then(Self::inventory_token)
        {
            let before = self.held_token_list();
            self.custom_fingers.insert(id, Held::Token(token));
            self.fingers.insert(id, (Role::Ignore, point));
            self.emit_custom_changes(before);
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
    /// A custom button's width as a fraction of the surface width.
    pub(super) fn custom_width(&self, index: usize, size: f32) -> f32 {
        let wide = self.settings.custom_buttons[index].width.clamp(1.0, 4.0);
        (size * wide / self.aspect()).min(1.0)
    }
    /// Where custom button `index` is drawn: mirrored for the left hand, scaled by
    /// the global button size and kept on the surface, its width included.
    pub fn custom_placement(&self, index: usize) -> Placement {
        let mut p = self.settings.custom_buttons[index].layouts[self.settings.mode.index()];
        if self.settings.left_handed {
            p.center[0] = 1.0 - p.center[0];
        }
        p.size = (p.size * self.settings.button_scale)
            .min(self.aspect())
            .min(1.0);
        let half = [self.custom_width(index, p.size) / 2.0, p.size / 2.0];
        for (i, half) in half.into_iter().enumerate() {
            p.center[i] = p.center[i].clamp(half, 1.0 - half);
        }
        p
    }
    /// Custom button `index`'s rectangle, in surface fractions.
    pub fn custom_rect(&self, index: usize) -> [f32; 4] {
        let p = self.custom_placement(index);
        let w = self.custom_width(index, p.size);
        [p.center[0] - w / 2.0, p.center[1] - p.size / 2.0, w, p.size]
    }
    /// Whether custom button `index` is on screen now: in the editor's layer, over a
    /// screen when one is open, or while playing.
    pub fn custom_visible(&self, index: usize) -> bool {
        let Some(button) = self.settings.custom_buttons.get(index) else {
            return false;
        };
        let menu = match self.editing {
            Some(layer) => layer == Layer::Menus,
            None => self.inventory_mode,
        };
        button.visible(menu)
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
            .find(|&i| self.custom_visible(i) && layout::contains(self.custom_rect(i), point))
    }
    /// The sliding button under `point`, if any.
    fn swipe_at(&self, point: [f32; 2]) -> Option<usize> {
        self.custom_at(point).filter(|&i| {
            let b = &self.settings.custom_buttons[i];
            b.swipe && !b.toggle
        })
    }
    pub(super) fn custom_update(
        &mut self,
        id: u64,
        point: [f32; 2],
        started: bool,
        ended: bool,
    ) -> bool {
        if let Some(held) = self.custom_fingers.get(&id).cloned() {
            if ended {
                let before = self.held_token_list();
                self.custom_fingers.remove(&id);
                self.fingers.remove(&id);
                self.emit_custom_changes(before);
                return true;
            }
            // A sliding finger moves between sliding buttons.
            let sliding = match held {
                Held::Button(i) => self
                    .settings
                    .custom_buttons
                    .get(i)
                    .is_some_and(|b| b.swipe && !b.toggle),
                Held::Slid => true,
                Held::Token(_) => false,
            };
            if sliding {
                let now = self.swipe_at(point).map_or(Held::Slid, Held::Button);
                if now != held {
                    let before = self.held_token_list();
                    if let Some(finger) = self.fingers.get_mut(&id) {
                        finger.1 = point;
                    }
                    self.custom_fingers.insert(id, now.clone());
                    self.emit_custom_changes(before);
                    if now != Held::Slid {
                        self.clicked = true;
                    }
                }
            }
            return true;
        }
        if started && let Some(index) = self.custom_at(point) {
            let button = &self.settings.custom_buttons[index];
            if button.keys().next().is_none() {
                return false;
            }
            let before = self.held_token_list();
            if button.toggle {
                // The finger only flips the switch; its lift does nothing.
                self.fingers.insert(id, (Role::Ignore, point));
                if !self.custom_latched.remove(&index) {
                    self.custom_latched.insert(index);
                }
            } else {
                self.custom_fingers.insert(id, Held::Button(index));
                self.fingers.insert(id, (Role::Ignore, point));
            }
            self.emit_custom_changes(before);
            self.clicked = true;
            return true;
        }
        false
    }
    /// The keys held by fingers and switched-on toggles, each once, in the order
    /// their buttons list them.
    fn held_token_list(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for token in self.held_custom_tokens() {
            if !out.iter().any(|t| t == token) {
                out.push(token.to_owned());
            }
        }
        out
    }
    /// Presses keys that became held and releases those no longer held: a key two
    /// buttons share stays down until the last one lets go. Releases go in reverse
    /// order, so a combination lets go of its modifier last.
    fn emit_custom_changes(&mut self, before: Vec<String>) {
        let after = self.held_token_list();
        for token in before.iter().rev() {
            if !after.contains(token) {
                self.custom_events.push((token.clone(), false));
            }
        }
        for token in after {
            if !before.contains(&token) {
                self.custom_events.push((token, true));
            }
        }
    }
    pub fn held_custom_tokens(&self) -> impl Iterator<Item = &str> {
        let buttons = &self.settings.custom_buttons;
        let mut out: Vec<&str> = Vec::new();
        for held in self.custom_fingers.values() {
            match held {
                Held::Button(i) => out.extend(buttons.get(*i).into_iter().flat_map(|b| b.keys())),
                Held::Token(token) => out.push(token),
                Held::Slid => {}
            }
        }
        for i in &self.custom_latched {
            out.extend(buttons.get(*i).into_iter().flat_map(|b| b.keys()));
        }
        out.into_iter()
    }
    /// Whether custom button `index` is down: held by a finger or switched on.
    fn custom_pressed(&self, index: usize) -> bool {
        self.custom_latched.contains(&index)
            || self
                .custom_fingers
                .values()
                .any(|h| *h == Held::Button(index))
    }
    pub fn key_events(&mut self) -> Vec<(String, bool)> {
        std::mem::take(&mut self.custom_events)
    }
    /// Releases every held custom key, switched-on toggles included: a screen or
    /// focus loss releases them, as it releases held keys.
    pub(super) fn clear_custom(&mut self) {
        let before = self.held_token_list();
        self.custom_latched.clear();
        self.custom_fingers.clear();
        self.emit_custom_changes(before);
    }
    pub(super) fn draw_custom(&self, b: &mut HudBatch, a: &HudAtlas) {
        for (i, button) in self.settings.custom_buttons.iter().enumerate() {
            if !self.custom_visible(i) {
                continue;
            }
            let p = self.custom_placement(i);
            let [x, y, w, h] = self.custom_rect(i);
            let rect = [x * b.width, y * b.height, w * b.width, h * b.height];
            let pressed = self.custom_pressed(i);
            let opacity = if self.editor.is_some() {
                p.opacity.max(0.35)
            } else {
                p.opacity
            };
            if opacity <= 0.0 {
                continue;
            }
            draw_custom_button(b, a, button, rect, pressed, opacity);
        }
    }
}
/// One custom button in its shape and colour, its label fitted inside.
fn draw_custom_button(
    b: &mut HudBatch,
    a: &HudAtlas,
    button: &CustomButton,
    rect: [f32; 4],
    pressed: bool,
    opacity: f32,
) {
    let alpha = (opacity * 255.0).round() as u8;
    let rgb = COLORS[(button.color as usize).min(COLORS.len() - 1)];
    let [r, g, bl] = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
    let label = button.label();
    match button.shape {
        Shape::Classic => {
            // The game's frame, tinted: the default colour keeps it white.
            let tint = if button.color == 0 {
                rgba(255, 255, 255, alpha)
            } else {
                rgba(r / 2 + 128, g / 2 + 128, bl / 2 + 128, alpha)
            };
            text_button(b, a, rect, &label, pressed, tint, true);
            return;
        }
        Shape::Flat => {
            let fill = shade(rgb, if pressed { 0.7 } else { 1.0 }, alpha);
            let edge = shade(rgb, if pressed { 0.85 } else { 1.35 }, alpha);
            let s = (rect[3] / 12.0).clamp(1.0, 4.0);
            stepped_plate(b, a, rect, s, edge);
            stepped_plate(
                b,
                a,
                [
                    rect[0] + s,
                    rect[1] + s,
                    rect[2] - 2.0 * s,
                    rect[3] - 2.0 * s,
                ],
                s,
                fill,
            );
        }
        Shape::Round => {
            let fill = shade(rgb, if pressed { 0.7 } else { 1.0 }, alpha);
            let edge = shade(rgb, if pressed { 0.85 } else { 1.35 }, alpha);
            capsule(b, a, rect, edge);
            let inset = (rect[3] / 14.0).clamp(1.0, 3.0);
            capsule(
                b,
                a,
                [
                    rect[0] + inset,
                    rect[1] + inset,
                    rect[2] - 2.0 * inset,
                    rect[3] - 2.0 * inset,
                ],
                fill,
            );
        }
    }
    // Dark text on light colours, light text on dark ones.
    let light = (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(bl) * 114) / 1000 > 150;
    let ink = if light {
        rgba(20, 20, 28, alpha.max(200))
    } else {
        rgba(245, 245, 250, alpha.max(200))
    };
    let h = rect[3];
    let offset = if pressed { (h / 18.0).min(2.0) } else { 0.0 };
    let pad = if button.shape == Shape::Round {
        h * 0.3
    } else {
        6.0
    };
    super::draw_label(b, a, rect, &label, ink, offset, pad);
}
/// `rgb` lightened (> 1) or darkened (< 1), with `alpha`.
fn shade(rgb: u32, factor: f32, alpha: u8) -> u32 {
    let c = |shift: u32| {
        let v = ((rgb >> shift) & 0xff) as f32;
        let v = if factor > 1.0 {
            v + (255.0 - v) * (factor - 1.0)
        } else {
            v * factor
        };
        v.round().clamp(0.0, 255.0) as u8
    };
    rgba(c(16), c(8), c(0), alpha)
}
/// A rectangle with one-step pixel corners `s` wide.
fn stepped_plate(b: &mut HudBatch, a: &HudAtlas, [x, y, w, h]: [f32; 4], s: f32, color: u32) {
    if w <= 2.0 * s || h <= 2.0 * s {
        return;
    }
    b.fill(a, x + s, y, w - 2.0 * s, h, color);
    b.fill(a, x, y + s, s, h - 2.0 * s, color);
    b.fill(a, x + w - s, y + s, s, h - 2.0 * s, color);
}
/// A filled circle, or a capsule when wider than tall.
fn capsule(b: &mut HudBatch, a: &HudAtlas, [x, y, w, h]: [f32; 4], color: u32) {
    let Some(white) = a.sprites.get("horizon/white") else {
        return;
    };
    let uv = [[(white.u0 + white.u1) / 2.0, (white.v0 + white.v1) / 2.0]; 4];
    let r = h / 2.0;
    let middle = (w - h).max(0.0);
    if middle > 0.0 {
        b.fill(a, x + r, y, middle, h, color);
    }
    for (cx, from, to) in [(x + r, 16usize, 48usize), (x + r + middle, 48, 80)] {
        let center = [cx, y + r];
        let points = circle_points();
        for i in from..to {
            let p = points[i % 64];
            let q = points[(i + 1) % 64];
            let p = [center[0] + p[0] * r, center[1] + p[1] * r];
            let q = [center[0] + q[0] * r, center[1] + q[1] * r];
            b.quad([center, p, q, center], uv, color);
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
        t.inventory_screen = true;
        t.settings.inventory_controls = true;
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
    fn screens_show_only_controls_chosen_for_menus_at_their_places() {
        let mut touch = TouchControls::default();
        touch.viewport(1920, 1080);
        let place = touch.placement(Control::Sneak).center;
        touch.inventory_mode = true;
        assert!(
            !touch.any_visible(),
            "nothing shows over screens by default"
        );
        touch.inventory_screen = true;
        touch.settings.inventory_controls = true;
        for c in [
            Control::Sneak,
            Control::Attack,
            Control::Use,
            Control::Inventory,
        ] {
            assert!(touch.visible(c), "{c:?} in the inventory set");
        }
        assert!(!touch.visible(Control::Move) && !touch.visible(Control::Jump));
        touch.inventory_screen = false;
        assert!(
            !touch.visible(Control::Sneak),
            "a chest is not the inventory"
        );
        touch.settings.set_show(Control::Sneak, Show::Menus);
        assert!(touch.visible(Control::Sneak));
        assert_eq!(touch.placement(Control::Sneak).center, place);
        touch.settings.set_show(Control::Jump, Show::Always);
        assert!(
            !touch.visible(Control::Jump),
            "jumping means nothing over a screen"
        );
        touch.inventory_mode = false;
        assert!(touch.visible(Control::Jump));
        assert!(!touch.visible(Control::Sneak), "shown in menus only");
        let restored: Settings =
            serde_json::from_slice(&serde_json::to_vec(&touch.settings).unwrap()).unwrap();
        assert_eq!(restored.show(Control::Sneak), Show::Menus);
        assert_eq!(restored.show(Control::Jump), Show::Always);
    }
    #[test]
    fn custom_buttons_show_in_game_menus_or_both() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        for show in [Show::Auto, Show::Menus, Show::Always] {
            let mut button = t.new_custom_button("key.keyboard.q".into());
            button.show = show.to_u8();
            t.settings.custom_buttons.push(button);
        }
        let at = |t: &TouchControls, i: usize| t.custom_placement(i).center;
        assert_eq!(t.custom_at(at(&t, 0)), Some(0));
        assert_eq!(t.custom_at(at(&t, 1)), None);
        assert_eq!(t.custom_at(at(&t, 2)), Some(2));
        t.inventory_mode = true;
        assert_eq!(t.custom_at(at(&t, 0)), None);
        assert_eq!(t.custom_at(at(&t, 1)), Some(1));
        assert!(t.inventory_update(5, at(&t, 1), true, false));
        assert_eq!(t.key_events(), [("key.keyboard.q".into(), true)]);
        assert!(t.inventory_update(5, at(&t, 1), false, true));
        assert_eq!(t.key_events(), [("key.keyboard.q".into(), false)]);
    }
    #[test]
    fn a_combination_presses_in_order_and_releases_in_reverse() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        let mut button = t.new_custom_button("key.keyboard.left.control".into());
        button.set_slot(1, "key.keyboard.q".into());
        t.settings.custom_buttons.push(button);
        let p = t.custom_placement(0).center;
        t.update(1, p, true, false);
        assert_eq!(
            t.key_events(),
            [
                ("key.keyboard.left.control".into(), true),
                ("key.keyboard.q".into(), true)
            ]
        );
        t.update(1, p, false, true);
        assert_eq!(
            t.key_events(),
            [
                ("key.keyboard.q".into(), false),
                ("key.keyboard.left.control".into(), false)
            ]
        );
    }
    #[test]
    fn a_sliding_finger_moves_between_sliding_buttons() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        for key in ["key.keyboard.1", "key.keyboard.2"] {
            let mut button = t.new_custom_button(key.into());
            button.swipe = true;
            t.settings.custom_buttons.push(button);
        }
        let (a, b) = (t.custom_placement(0).center, t.custom_placement(1).center);
        t.update(1, a, true, false);
        assert_eq!(t.key_events(), [("key.keyboard.1".into(), true)]);
        t.update(1, b, false, false);
        assert_eq!(
            t.key_events(),
            [
                ("key.keyboard.1".into(), false),
                ("key.keyboard.2".into(), true)
            ]
        );
        t.update(1, [0.5, 0.5], false, false);
        assert_eq!(t.key_events(), [("key.keyboard.2".into(), false)]);
        t.update(1, a, false, false);
        assert_eq!(t.key_events(), [("key.keyboard.1".into(), true)]);
        t.update(1, a, false, true);
        assert_eq!(t.key_events(), [("key.keyboard.1".into(), false)]);
    }
    #[test]
    fn older_custom_buttons_load_with_the_new_options_at_their_defaults() {
        let old = r#"{"key":"key.keyboard.z","layouts":[{"center":[0.5,0.5],"size":0.1,"opacity":0.65},{"center":[0.5,0.5],"size":0.1,"opacity":0.65},{"center":[0.5,0.5],"size":0.1,"opacity":0.65}],"toggle":true}"#;
        let button: CustomButton = serde_json::from_str(old).unwrap();
        assert!(button.toggle && !button.swipe);
        assert_eq!(button.width, 1.0);
        assert_eq!(button.shape, Shape::Classic);
        assert!(button.visible(false) && !button.visible(true));
        assert_eq!(button.keys().collect::<Vec<_>>(), ["key.keyboard.z"]);
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
