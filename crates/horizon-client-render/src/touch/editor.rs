use super::*;

pub struct Editor {
    pub selected: Option<Control>,
    option: Option<usize>,
    apply_all: bool,
    original: Settings,
    drag: Option<(u64, Drag)>,
    error: Option<String>,
    palette: bool,
    selected_custom: Option<usize>,
    key_picker: bool,
    key_page: usize,
    raw_keys: bool,
}

enum Drag {
    Control([f32; 2]),
    Slider(usize),
}

impl TouchControls {
    pub fn open_editor(&mut self) {
        self.clear();
        self.editor = Some(Editor {
            selected: None,
            option: None,
            apply_all: false,
            original: self.settings.clone(),
            drag: None,
            error: None,
            palette: false,
            selected_custom: None,
            key_picker: false,
            key_page: 0,
            raw_keys: false,
        });
    }
    pub fn cancel_editor(&mut self) {
        if let Some(e) = self.editor.take() {
            self.settings = e.original;
        }
        self.clear();
    }
    /// The editor owns every finger: editing cannot move, mine or open menus.
    /// Returns true when Done/Cancel closed the editor.
    pub fn edit_touch(&mut self, id: u64, p: [f32; 2], started: bool, ended: bool) -> bool {
        let Some(mut e) = self.editor.take() else {
            return false;
        };
        self.clicked |= started;
        if self.edit_custom(&mut e, id, p, started, ended) {
            self.editor = Some(e);
            return false;
        }
        if started {
            if layout::contains([0.02, 0.04, 0.105, 0.075], p) {
                match self.settings.save() {
                    Ok(()) => {
                        self.clear();
                        return true;
                    }
                    Err(error) => e.error = Some(error.to_string()),
                }
            } else if layout::contains([0.135, 0.04, 0.105, 0.075], p) {
                self.settings = e.original;
                self.clear();
                return true;
            } else if layout::contains([0.25, 0.04, 0.105, 0.075], p) {
                self.settings.layouts[self.settings.mode.index()] =
                    Settings::default().layouts[self.settings.mode.index()];
                self.settings.button_visibility = [0; 14];
                // Custom buttons go back to free space, one after another, instead of
                // onto a fixed grid over the action buttons.
                let mode = self.settings.mode.index();
                for i in 0..self.settings.custom_buttons.len() {
                    let [x, y] = self.free_custom_spot(mode, i);
                    self.settings.custom_buttons[i].layouts[mode] =
                        layout::Placement::new(x, y, Self::NEW_CUSTOM_SIZE);
                }
            } else if layout::contains([0.02, 0.13, 0.335, 0.075], p) {
                e.palette = !e.palette;
                e.selected = None;
                e.selected_custom = None;
                e.key_picker = false;
                e.drag = None;
            } else if e.palette {
                for (index, control) in Control::ALL.into_iter().enumerate() {
                    if layout::contains(palette_rect(index), p) {
                        let show = !self.visible(control);
                        self.settings.button_visibility[index] = if show { 1 } else { 2 };
                    }
                }
            } else if layout::contains([0.32, 0.88, 0.36, 0.07], p) && e.selected.is_some() {
                self.settings.button_visibility[e.selected.unwrap() as usize] = 2;
                e.selected = None;
                e.drag = None;
            } else if layout::contains([0.32, 0.61, 0.36, 0.06], p)
                && e.selected.is_some()
                && e.option.is_some()
            {
                e.apply_all = !e.apply_all;
            } else if layout::contains([0.32, 0.68, 0.36, 0.07], p)
                && e.selected.is_some()
                && e.option.is_some()
            {
                e.drag = Some((id, Drag::Slider(e.option.unwrap())));
            } else if layout::contains([0.32, 0.79, 0.17, 0.07], p) && e.selected.is_some() {
                e.option = Some(1);
            } else if layout::contains([0.51, 0.79, 0.17, 0.07], p) && e.selected.is_some() {
                e.option = Some(0);
            } else if let Some(c) = Control::ALL.into_iter().rev().find(|c| {
                self.visible(*c) && layout::contains(self.placement(*c).rect(self.aspect()), p)
            }) {
                e.selected = Some(c);
                e.option = None;
                let center = self.placement(c).center;
                e.drag = Some((id, Drag::Control([p[0] - center[0], p[1] - center[1]])));
            } else {
                e.selected = None;
                e.option = None;
            }
        }
        if let Some((owner, drag)) = &e.drag
            && *owner == id
            && let Some(selected) = e.selected
        {
            let aspect = self.aspect();
            let slot = self.settings.slot(selected);
            let place = &mut self.settings.layouts[self.settings.mode.index()][slot];
            match drag {
                Drag::Control(offset) => {
                    let mut center = [p[0] - offset[0], p[1] - offset[1]];
                    if self.settings.left_handed {
                        center[0] = 1.0 - center[0];
                    }
                    // Match the displayed size on narrow portrait surfaces.
                    // A stored button may be wider than the screen; its raw
                    // size would otherwise produce inverted clamp bounds.
                    let size = (place.size * self.settings.button_scale)
                        .min(aspect)
                        .min(1.0);
                    place.center = [
                        center[0].clamp(size / aspect / 2.0, 1.0 - size / aspect / 2.0),
                        center[1].clamp(size / 2.0, 1.0 - size / 2.0),
                    ];
                }
                Drag::Slider(index) => {
                    let value = ((p[0] - 0.335) / 0.33).clamp(0.0, 1.0);
                    let layout = &mut self.settings.layouts[self.settings.mode.index()];
                    let size = 0.045 + value * 0.405;
                    // "Apply to all" scales every control by the same factor, so the
                    // joystick stays bigger than a button instead of all becoming equal.
                    let factor = size / layout[slot].size.max(0.001);
                    for (i, place) in layout.iter_mut().enumerate() {
                        if i == slot {
                            if *index == 0 {
                                place.size = size;
                            } else {
                                place.opacity = value;
                            }
                        } else if e.apply_all {
                            if *index == 0 {
                                place.size = (place.size * factor).clamp(0.045, 0.45);
                            } else {
                                place.opacity = value;
                            }
                        }
                    }
                }
            }
            if ended {
                e.drag = None;
            }
        }

        self.editor = Some(e);
        false
    }
    pub(super) fn draw_editor_toolbar(&self, b: &mut HudBatch, a: &HudAtlas) {
        for (rect, label) in [
            ([0.02, 0.04, 0.105, 0.075], "Done"),
            ([0.135, 0.04, 0.105, 0.075], "Cancel"),
            ([0.25, 0.04, 0.105, 0.075], "Reset"),
            ([0.02, 0.13, 0.335, 0.075], "Add buttons..."),
        ] {
            plate(b, a, rect, label, false);
        }
    }

    pub(super) fn draw_editor(&self, b: &mut HudBatch, a: &HudAtlas) {
        let e = self.editor.as_ref().unwrap();
        let (w, h) = (b.width, b.height);
        let text = rgba(245, 245, 245, 255);
        if let Some(selected) = e.selected {
            plate(b, a, [0.32, 0.88, 0.36, 0.07], "Remove button", false);
            let p = self.placement(selected);
            plate(
                b,
                a,
                [0.32, 0.79, 0.17, 0.07],
                "Opacity",
                e.option == Some(1),
            );
            plate(b, a, [0.51, 0.79, 0.17, 0.07], "Size", e.option == Some(0));
            if let Some(option) = e.option {
                b.fill(
                    a,
                    w * 0.31,
                    h * 0.59,
                    w * 0.38,
                    h * 0.18,
                    rgba(15, 15, 15, 235),
                );
                plate(
                    b,
                    a,
                    [0.32, 0.61, 0.36, 0.06],
                    if e.apply_all {
                        "Apply to all: On"
                    } else {
                        "Apply to all: Off"
                    },
                    e.apply_all,
                );
                let value = if option == 0 {
                    (p.size - 0.045) / 0.405
                } else {
                    p.opacity
                };
                let percent = if option == 0 {
                    p.size * 100.0
                } else {
                    p.opacity * 100.0
                };
                b.text(
                    a,
                    &format!("{} {:.0}%", selected.name(), percent),
                    w * 0.325,
                    h * 0.68,
                    text,
                    true,
                );
                nine(
                    b,
                    a,
                    "ui/slider_background",
                    1.0,
                    [w * 0.335, h * 0.72, w * 0.33, h * 0.025],
                    rgba(255, 255, 255, 255),
                );
                nine(
                    b,
                    a,
                    "ui/slider_button_default",
                    2.0,
                    [w * (0.335 + value * 0.33) - 3.0, h * 0.707, 6.0, h * 0.05],
                    rgba(255, 255, 255, 255),
                );
            }
        }
        if e.palette {
            let [x, y, pw, ph] = PALETTE_PANEL;
            b.fill(a, w * x, h * y, w * pw, h * ph, rgba(15, 15, 15, 245));
            for (index, control) in Control::ALL.into_iter().enumerate() {
                let shown = self.visible(control);
                plate(
                    b,
                    a,
                    palette_rect(index),
                    &format!("{} {}", if shown { "[-]" } else { "[+]" }, control.name()),
                    shown,
                );
            }
        }
        self.draw_custom_editor(b, a, e);
        if e.error.is_some() {
            b.text(
                a,
                "Save failed. Retry Done.",
                w * 0.33,
                h * 0.88,
                rgba(255, 120, 120, 255),
                true,
            );
        }
    }
}
/// Two columns of seven rows that end above [`CREATE_RECT`], all inside the panel.
fn palette_rect(index: usize) -> [f32; 4] {
    [
        0.22 + (index % 2) as f32 * 0.29,
        0.16 + (index / 2) as f32 * 0.092,
        0.27,
        0.08,
    ]
}
const PALETTE_PANEL: [f32; 4] = [0.2, 0.14, 0.6, 0.76];
/// Hold or Toggle, above the selected custom button's Action row.
const MODE_RECT: [f32; 4] = [0.32, 0.46, 0.36, 0.07];
const CREATE_RECT: [f32; 4] = [0.22, 0.81, 0.56, 0.07];
const TOOLBAR: [[f32; 4]; 4] = [
    [0.02, 0.04, 0.105, 0.075],
    [0.135, 0.04, 0.105, 0.075],
    [0.25, 0.04, 0.105, 0.075],
    [0.02, 0.13, 0.335, 0.075],
];
const REMOVE_RECT: [f32; 4] = [0.32, 0.88, 0.36, 0.07];
const OPACITY_RECT: [f32; 4] = [0.32, 0.79, 0.17, 0.07];
const SIZE_RECT: [f32; 4] = [0.51, 0.79, 0.17, 0.07];
const APPLY_ALL_RECT: [f32; 4] = [0.32, 0.61, 0.36, 0.06];
const SLIDER_RECT: [f32; 4] = [0.32, 0.68, 0.36, 0.07];
fn toolbar_at(p: [f32; 2]) -> bool {
    TOOLBAR.iter().any(|r| layout::contains(*r, p))
}
/// The option panel of a selected built-in control takes its taps before any custom
/// button drawn underneath it.
fn builtin_panel_at(e: &Editor, p: [f32; 2]) -> bool {
    e.selected.is_some()
        && ([REMOVE_RECT, OPACITY_RECT, SIZE_RECT]
            .iter()
            .any(|r| layout::contains(*r, p))
            || (e.option.is_some()
                && [APPLY_ALL_RECT, SLIDER_RECT]
                    .iter()
                    .any(|r| layout::contains(*r, p))))
}
fn plate(b: &mut HudBatch, a: &HudAtlas, [x, y, w, h]: [f32; 4], label: &str, on: bool) {
    let (bw, bh) = (b.width, b.height);
    nine(
        b,
        a,
        "ui/button_borderless_light",
        1.0,
        [x * bw, y * bh, w * bw, h * bh],
        if on {
            rgba(100, 210, 90, 255)
        } else {
            rgba(255, 255, 255, 255)
        },
    );
    if let Some((first, second)) = label.split_once(" & ") {
        b.text(
            a,
            &format!("{first} &"),
            x * bw + 3.0,
            y * bh + (h * bh - 20.0) / 2.0,
            rgba(35, 35, 35, 255),
            false,
        );
        b.text(
            a,
            second,
            x * bw + 3.0,
            y * bh + (h * bh - 20.0) / 2.0 + 10.0,
            rgba(35, 35, 35, 255),
            false,
        );
    } else {
        let mut display = label.to_owned();
        while HudBatch::text_width(a, &display) > w * bw - 6.0 && !display.is_empty() {
            display.pop();
        }
        b.text(
            a,
            &display,
            x * bw + 3.0,
            y * bh + (h * bh - 8.0) / 2.0,
            rgba(35, 35, 35, 255),
            false,
        );
    }
}

impl TouchControls {
    fn edit_custom(
        &mut self,
        e: &mut Editor,
        id: u64,
        p: [f32; 2],
        started: bool,
        ended: bool,
    ) -> bool {
        // Only the toolbar and an open built-in panel outrank custom buttons; a custom
        // button anywhere else, the top of the screen included, stays selectable.
        if started && (toolbar_at(p) || builtin_panel_at(e, p)) {
            return false;
        }
        if e.key_picker {
            if started {
                if layout::contains([0.15, 0.16, 0.33, 0.06], p) {
                    e.raw_keys = false;
                    e.key_page = 0;
                } else if layout::contains([0.52, 0.16, 0.33, 0.06], p) {
                    e.raw_keys = true;
                    e.key_page = 0;
                } else if layout::contains([0.2, 0.86, 0.18, 0.07], p) {
                    e.key_page = e.key_page.saturating_sub(1);
                } else if layout::contains([0.41, 0.86, 0.18, 0.07], p) {
                    e.key_picker = false;
                } else if layout::contains([0.62, 0.86, 0.18, 0.07], p) {
                    e.key_page = (e.key_page + 1)
                        .min(super::custom::choices(e.raw_keys).len().saturating_sub(1) / 8);
                } else {
                    for (i, (token, _)) in super::custom::choices(e.raw_keys)
                        .into_iter()
                        .skip(e.key_page * 8)
                        .take(8)
                        .enumerate()
                    {
                        if layout::contains(key_rect(i), p)
                            && let Some(index) = e.selected_custom
                        {
                            self.settings.custom_buttons[index].key = token;
                            e.key_picker = false;
                            break;
                        }
                    }
                }
            }
            return true;
        }
        if e.palette && started && layout::contains(CREATE_RECT, p) {
            if self.settings.custom_buttons.len() < 32 {
                let index = self.settings.custom_buttons.len();
                let button = self.new_custom_button("key.keyboard.unknown".into());
                self.settings.custom_buttons.push(button);
                e.selected = None;
                e.selected_custom = Some(index);
                e.key_picker = true;
                e.raw_keys = false;
                e.key_page = 0;
                e.palette = false;
                e.option = None;
                e.drag = None;
            }
            return true;
        }
        if e.palette {
            return false;
        }
        if started && e.selected_custom.is_some() {
            if layout::contains(MODE_RECT, p) {
                let button = &mut self.settings.custom_buttons[e.selected_custom.unwrap()];
                button.toggle ^= true;
                e.drag = None;
                return true;
            }
            if layout::contains([0.32, 0.54, 0.36, 0.07], p) {
                e.key_picker = true;
                e.key_page = 0;
                e.drag = None;
                return true;
            }
            if layout::contains([0.32, 0.88, 0.36, 0.07], p) {
                self.settings
                    .custom_buttons
                    .remove(e.selected_custom.unwrap());
                e.selected_custom = None;
                e.drag = None;
                return true;
            }
            if layout::contains([0.32, 0.79, 0.17, 0.07], p) {
                e.option = Some(1);
                return true;
            }
            if layout::contains([0.51, 0.79, 0.17, 0.07], p) {
                e.option = Some(0);
                return true;
            }
            if layout::contains([0.32, 0.68, 0.36, 0.07], p)
                && let Some(option) = e.option
            {
                e.drag = Some((id, Drag::Slider(option)));
            } else {
                e.selected_custom = None;
                e.drag = None;
            }
        }
        if started
            && e.drag.is_none()
            && let Some(index) = self.custom_at(p)
        {
            let center = self.custom_placement(index).center;
            e.selected = None;
            e.selected_custom = Some(index);
            e.option = None;
            e.apply_all = false;
            e.drag = Some((id, Drag::Control([p[0] - center[0], p[1] - center[1]])));
        }
        let Some(index) = e.selected_custom else {
            return false;
        };
        if let Some((owner, drag)) = &e.drag
            && *owner == id
        {
            let aspect = self.aspect();
            let scale = self.settings.button_scale;
            let mirror = self.settings.left_handed;
            let place =
                &mut self.settings.custom_buttons[index].layouts[self.settings.mode.index()];
            match drag {
                Drag::Control(offset) => {
                    let mut center = [p[0] - offset[0], p[1] - offset[1]];
                    if mirror {
                        center[0] = 1.0 - center[0];
                    }
                    let size = (place.size * scale).min(aspect).min(1.0);
                    place.center = [
                        center[0].clamp(size / aspect / 2.0, 1.0 - size / aspect / 2.0),
                        center[1].clamp(size / 2.0, 1.0 - size / 2.0),
                    ];
                }
                Drag::Slider(option) => {
                    let value = ((p[0] - 0.335) / 0.33).clamp(0.0, 1.0);
                    if *option == 0 {
                        place.size = 0.045 + value * 0.405;
                    } else {
                        place.opacity = value;
                    }
                }
            }
            if ended {
                e.drag = None;
            }
        }
        true
    }
    pub fn capture_custom_key(&mut self, name: &str) -> bool {
        if crate::keybinds::key_code(name).is_none() {
            return false;
        }
        if let Some(e) = &mut self.editor
            && e.key_picker
            && let Some(i) = e.selected_custom
        {
            self.settings.custom_buttons[i].key = name.into();
            e.key_picker = false;
            true
        } else {
            false
        }
    }
    fn draw_custom_editor(&self, b: &mut HudBatch, a: &HudAtlas, e: &Editor) {
        let (w, h) = (b.width, b.height);
        if e.palette {
            plate(b, a, CREATE_RECT, "Create button...", false);
        }
        if let Some(i) = e.selected_custom
            && !e.palette
            && !e.key_picker
        {
            let p = self.settings.custom_buttons[i].layouts[self.settings.mode.index()];
            plate(
                b,
                a,
                MODE_RECT,
                if self.settings.custom_buttons[i].toggle {
                    "Mode: Toggle (tap on/off)"
                } else {
                    "Mode: Hold"
                },
                self.settings.custom_buttons[i].toggle,
            );
            plate(
                b,
                a,
                [0.32, 0.54, 0.36, 0.07],
                &format!("Action: {}", self.settings.custom_buttons[i].label()),
                false,
            );
            plate(
                b,
                a,
                [0.32, 0.79, 0.17, 0.07],
                "Opacity",
                e.option == Some(1),
            );
            plate(b, a, [0.51, 0.79, 0.17, 0.07], "Size", e.option == Some(0));
            plate(b, a, [0.32, 0.88, 0.36, 0.07], "Remove button", false);
            if let Some(option) = e.option {
                let value = if option == 0 {
                    (p.size - 0.045) / 0.405
                } else {
                    p.opacity
                };
                b.fill(
                    a,
                    w * 0.3,
                    h * 0.66,
                    w * 0.4,
                    h * 0.11,
                    rgba(15, 15, 15, 235),
                );
                let percent = if option == 0 {
                    p.size * 100.0
                } else {
                    p.opacity * 100.0
                };
                b.text(
                    a,
                    &format!(
                        "{} {:.0}%",
                        if option == 0 { "Size" } else { "Opacity" },
                        percent
                    ),
                    w * 0.325,
                    h * 0.68,
                    rgba(245, 245, 245, 255),
                    true,
                );
                nine(
                    b,
                    a,
                    "ui/slider_background",
                    1.0,
                    [w * 0.335, h * 0.72, w * 0.33, h * 0.025],
                    rgba(255, 255, 255, 255),
                );
                nine(
                    b,
                    a,
                    "ui/slider_button_default",
                    2.0,
                    [w * (0.335 + value * 0.33) - 3.0, h * 0.707, 6.0, h * 0.05],
                    rgba(255, 255, 255, 255),
                );
            }
        }
        if e.key_picker {
            b.fill(
                a,
                w * 0.12,
                h * 0.14,
                w * 0.76,
                h * 0.81,
                rgba(15, 15, 15, 250),
            );
            plate(b, a, [0.15, 0.16, 0.33, 0.06], "Game action", !e.raw_keys);
            plate(b, a, [0.52, 0.16, 0.33, 0.06], "Keyboard key", e.raw_keys);
            for (i, (_, label)) in super::custom::choices(e.raw_keys)
                .iter()
                .skip(e.key_page * 8)
                .take(8)
                .enumerate()
            {
                plate(b, a, key_rect(i), label, false);
            }
            plate(b, a, [0.2, 0.86, 0.18, 0.07], "Previous", false);
            plate(b, a, [0.41, 0.86, 0.18, 0.07], "Back", false);
            plate(b, a, [0.62, 0.86, 0.18, 0.07], "Next", false);
        }
    }
}
fn key_rect(i: usize) -> [f32; 4] {
    [0.15, 0.25 + i as f32 * 0.072, 0.7, 0.06]
}
