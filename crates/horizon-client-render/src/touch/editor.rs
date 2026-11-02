//! The touch layout editor's model: what is selected, dragging with snapping and
//! alignment guides, pinch resizing, undo and redo, and the actions its panels
//! send. The panels are drawn by `horizon_client::touch_editor`, which records
//! each tappable rectangle (in dp) with its [`Act`] in [`Editor::targets`]; taps
//! are matched against the last frame's rectangles here.
use super::*;
use layout::Placement;

/// Something on the layout that can be selected and moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Control(Control),
    Custom(usize),
    Utility(Utility),
}
/// Which controls the editor shows: those shown while playing, or over screens.
/// Both share one position per control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layer {
    #[default]
    Game,
    Menus,
}
/// The editor's open panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Panel {
    #[default]
    None,
    /// The selected control's properties.
    Props,
    /// Every control, with where it shows.
    List,
    /// Choosing the key for slot `slot` of the selected custom button: game
    /// actions, or keyboard keys when `raw`.
    Keys { slot: usize, raw: bool },
    /// Editor-wide options.
    More,
    /// Asks before resetting the layout.
    Reset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slider {
    /// The selected control's size.
    Size,
    Opacity,
    /// A custom button's width, as a multiple of its height.
    Width,
    /// Every control's size at once (the global button scale).
    Scale,
}
/// What a tap on the editor's panels does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Done,
    Cancel,
    Undo,
    Redo,
    /// Opens a panel, or closes it when it is already open.
    Open(Panel),
    /// Closes the open panel (the key picker goes back to the properties).
    Close,
    Layer(Layer),
    Snap,
    Select(Target),
    Show(Show),
    /// The list's eye: shows or hides a control in the current layer.
    Eye(Target),
    Slider(Slider),
    Step(Slider, i8),
    Nudge(i8, i8),
    OpacityToAll,
    Toggle(bool),
    Swipe,
    Shape(Shape),
    Color(u8),
    /// Starts typing the custom button's label.
    Label,
    /// Picks the key of a slot (`MAX_KEYS` adds one).
    Slot(usize),
    RemoveSlot(usize),
    /// The key at this index of the open picker's list.
    Pick(usize),
    Add,
    Duplicate,
    Delete,
    /// Puts a built-in control or a utility button back where it started.
    Restore,
    ResetLayout,
    CopyToSchemes,
    /// A panel's background: taps there do nothing, drags scroll it.
    Sheet,
}
/// A finger on a panel.
#[derive(Clone, Copy, Debug)]
pub struct Press {
    pub id: u64,
    pub start: [f32; 2],
    pub at: [f32; 2],
    pub act: Act,
    pub rect: [f32; 4],
    pub moved: bool,
    scroll: f32,
}
struct Drag {
    id: u64,
    target: Target,
    /// From the finger to the control's centre, in surface fractions.
    offset: [f32; 2],
    at: [f32; 2],
    moved: bool,
}
struct Pinch {
    id: u64,
    at: [f32; 2],
    distance: f32,
    size: f32,
}
pub struct Editor {
    pub selected: Option<Target>,
    pub panel: Panel,
    /// The open panel's scroll offset and its largest value (the view sets it), dp.
    pub scroll: f32,
    pub scroll_max: f32,
    /// The scrolling part of the open panel, dp (the view sets it).
    pub sheet: Option<[f32; 4]>,
    /// Tappable rectangles of the last frame, in dp; the last one on top.
    pub targets: Vec<([f32; 4], Act)>,
    /// Physical pixels per dp (the view sets it).
    pub density: f32,
    /// Alignment lines while dragging, in surface fractions: vertical lines' x,
    /// horizontal lines' y.
    pub guides: (Vec<f32>, Vec<f32>),
    pub press: Option<Press>,
    /// Typing the selected custom button's label.
    pub typing: bool,
    /// The keyboard should open with this text (the game takes it).
    pub text_request: Option<String>,
    /// The keyboard should close (the game takes it).
    pub hide_keyboard: bool,
    /// Shown until the first touch on the layout.
    pub hint: bool,
    /// Saving failed: Done can be tried again.
    pub save_failed: bool,
    original: Settings,
    undo: Vec<Settings>,
    redo: Vec<Settings>,
    drag: Option<Drag>,
    pinch: Option<Pinch>,
}
impl Editor {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// A control is being dragged: the panels step aside.
    pub fn dragging(&self) -> bool {
        self.drag.as_ref().is_some_and(|d| d.moved) || self.pinch.is_some()
    }
    /// Where the finger on a panel is, while it has not moved off its control.
    pub fn pressed_at(&self) -> Option<[f32; 2]> {
        self.press.filter(|p| !p.moved).map(|p| p.at)
    }
}
const MIN_SIZE: f32 = 0.045;
/// The largest size of most controls; [`max_size`] lets the movement control and
/// the utility buttons differ.
const MAX_SIZE: f32 = 0.45;
/// The largest size a target may take, as a fraction of the surface height.
fn max_size(target: Target) -> f32 {
    match target {
        // The D-pad packs four arrows into one square: it needs room to grow.
        Target::Control(Control::Move) => 0.9,
        Target::Utility(_) => 0.2,
        _ => MAX_SIZE,
    }
}
/// How close (dp) an edge or centre has to come to another one to snap to it.
const SNAP: f32 = 6.0;
const GRID: f32 = 8.0;
const UNDO_LIMIT: usize = 64;

impl TouchControls {
    pub fn open_editor(&mut self) {
        self.clear();
        self.editing = Some(Layer::Game);
        self.editor = Some(Editor {
            selected: None,
            panel: Panel::None,
            scroll: 0.0,
            scroll_max: 0.0,
            sheet: None,
            targets: Vec::new(),
            density: 0.0,
            guides: (Vec::new(), Vec::new()),
            press: None,
            typing: false,
            text_request: None,
            hide_keyboard: false,
            hint: true,
            save_failed: false,
            original: self.settings.clone(),
            undo: Vec::new(),
            redo: Vec::new(),
            drag: None,
            pinch: None,
        });
    }
    pub fn cancel_editor(&mut self) {
        if let Some(e) = self.editor.take() {
            self.settings = e.original;
        }
        self.editing = None;
        self.clear();
    }
    /// The editor's layer: the controls shown while playing, or over screens.
    pub fn editor_layer(&self) -> Layer {
        self.editing.unwrap_or_default()
    }
    /// The editor wants the keyboard opened with this text.
    pub fn take_text_request(&mut self) -> Option<String> {
        self.editor.as_mut()?.text_request.take()
    }
    /// The editor is done with the keyboard.
    pub fn take_hide_keyboard(&mut self) -> bool {
        self.editor
            .as_mut()
            .is_some_and(|e| std::mem::take(&mut e.hide_keyboard))
    }
    pub fn editor_typing(&self) -> bool {
        self.editor.as_ref().is_some_and(|e| e.typing)
    }
    /// Text typed into the custom button's label; `backspace` deletes a character
    /// and `enter` finishes.
    pub fn editor_type(&mut self, text: &str, backspace: bool, enter: bool) {
        let Some(e) = self.editor.as_mut() else {
            return;
        };
        if !e.typing {
            return;
        }
        let Some(Target::Custom(i)) = e.selected else {
            e.typing = false;
            return;
        };
        let Some(button) = self.settings.custom_buttons.get_mut(i) else {
            return;
        };
        if backspace {
            button.label.pop();
        }
        for c in text.chars().filter(|c| !c.is_control()) {
            if button.label.chars().count() < 24 {
                button.label.push(c);
            }
        }
        if enter {
            e.typing = false;
            e.hide_keyboard = true;
        }
    }
    /// A key pressed on a physical keyboard while the key picker is open fills
    /// the slot being picked.
    pub fn capture_custom_key(&mut self, name: &str) -> bool {
        if crate::keybinds::key_code(name).is_none() {
            return false;
        }
        let Some(mut e) = self.editor.take() else {
            return false;
        };
        let taken = if let Panel::Keys { slot, .. } = e.panel
            && let Some(Target::Custom(i)) = e.selected
            && i < self.settings.custom_buttons.len()
        {
            Self::snapshot(&self.settings, &mut e);
            self.settings.custom_buttons[i].set_slot(slot, name.into());
            e.panel = Panel::Props;
            e.scroll = 0.0;
            true
        } else {
            false
        };
        self.editor = Some(e);
        taken
    }

    /// The editor owns every finger: editing cannot move, mine or open menus.
    /// `p` is in surface fractions. Returns true when Done or Cancel closed it.
    pub fn edit_touch(&mut self, id: u64, p: [f32; 2], started: bool, ended: bool) -> bool {
        let Some(mut e) = self.editor.take() else {
            return false;
        };
        let closed = self.edit(&mut e, id, p, started, ended);
        if closed {
            self.editing = None;
            self.clear();
        } else {
            self.editor = Some(e);
        }
        closed
    }

    fn dp(&self, e: &Editor, p: [f32; 2]) -> [f32; 2] {
        let [w, h] = self.surface_dp(e);
        [p[0] * w, p[1] * h]
    }
    /// The surface in dp.
    fn surface_dp(&self, e: &Editor) -> [f32; 2] {
        let density = if e.density > 0.0 { e.density } else { 1.0 };
        let [w, h] = if self.surface[0] > 0.0 {
            self.surface
        } else {
            [self.aspect() * 1000.0, 1000.0]
        };
        [w / density, h / density]
    }

    fn edit(&mut self, e: &mut Editor, id: u64, p: [f32; 2], started: bool, ended: bool) -> bool {
        let at = self.dp(e, p);
        if started {
            e.hint = false;
            // A second finger while a control is dragged resizes it.
            if let Some(drag) = &e.drag {
                if drag.id != id && e.pinch.is_none() {
                    let a = self.dp(e, drag.at);
                    e.pinch = Some(Pinch {
                        id,
                        at: p,
                        distance: (at[0] - a[0]).hypot(at[1] - a[1]).max(8.0),
                        size: self.stored(drag.target).size,
                    });
                }
                return false;
            }
            if e.press.is_some() {
                return false;
            }
            if let Some((rect, act)) = e
                .targets
                .iter()
                .rev()
                .find(|(r, _)| layout::contains(*r, at))
                .copied()
            {
                if e.typing && act != Act::Label {
                    e.typing = false;
                    e.hide_keyboard = true;
                }
                e.press = Some(Press {
                    id,
                    start: at,
                    at,
                    act,
                    rect,
                    moved: false,
                    scroll: e.scroll,
                });
                if let Act::Slider(kind) = act {
                    Self::snapshot(&self.settings, e);
                    self.set_slider(e, kind, rect, at);
                }
                return false;
            }
            if e.typing {
                e.typing = false;
                e.hide_keyboard = true;
            }
            if let Some(target) = self.target_at(p) {
                if e.selected != Some(target) && e.panel != Panel::List {
                    e.panel = Panel::None;
                }
                e.selected = Some(target);
                let center = self.shown_center(target);
                e.drag = Some(Drag {
                    id,
                    target,
                    offset: [p[0] - center[0], p[1] - center[1]],
                    at: p,
                    moved: false,
                });
                self.clicked = true;
            } else {
                e.selected = None;
                e.panel = Panel::None;
            }
            return false;
        }
        if let Some(press) = e.press.as_mut()
            && press.id == id
        {
            press.at = at;
            let moved = (at[0] - press.start[0]).hypot(at[1] - press.start[1]) > 8.0;
            if let Act::Slider(kind) = press.act {
                let rect = press.rect;
                self.set_slider(e, kind, rect, at);
            } else if moved || press.moved {
                let scrolls = e.sheet.is_some_and(|s| layout::contains(s, press.start));
                press.moved = true;
                if scrolls {
                    e.scroll = (press.scroll - (at[1] - press.start[1])).clamp(0.0, e.scroll_max);
                }
            }
            if ended {
                let press = e.press.take().unwrap();
                if !press.moved
                    && !matches!(press.act, Act::Slider(_) | Act::Sheet)
                    && layout::contains(press.rect, at)
                {
                    self.clicked = true;
                    return self.apply(e, press.act);
                }
            }
            return false;
        }
        if e.pinch.as_ref().is_some_and(|pinch| pinch.id == id) {
            if ended {
                e.pinch = None;
            } else if let Some(pinch) = e.pinch.as_mut() {
                pinch.at = p;
                self.pinch(e);
            }
            return false;
        }
        if let Some(drag) = e.drag.as_mut()
            && drag.id == id
        {
            drag.at = p;
            let (moved, target, offset) = (drag.moved, drag.target, drag.offset);
            let start_moving = !moved && {
                let center = self.shown_center(target);
                let from = [center[0] + offset[0], center[1] + offset[1]];
                let a = self.dp(e, from);
                (at[0] - a[0]).hypot(at[1] - a[1]) > 6.0
            };
            if start_moving {
                Self::snapshot(&self.settings, e);
                e.drag.as_mut().unwrap().moved = true;
            }
            if e.drag.as_ref().unwrap().moved && e.pinch.is_none() {
                self.drag_to(e, p);
            } else if e.pinch.is_some() {
                self.pinch(e);
            }
            if ended {
                let drag = e.drag.take().unwrap();
                e.pinch = None;
                e.guides = (Vec::new(), Vec::new());
                if !drag.moved && e.panel != Panel::List {
                    // A tap opens the control's properties.
                    e.panel = Panel::Props;
                    e.scroll = 0.0;
                }
            }
        }
        false
    }

    /// The topmost control or custom button under `p` in the editor's layer.
    fn target_at(&self, p: [f32; 2]) -> Option<Target> {
        let menus = self.editor_layer() == Layer::Menus;
        if let Some(u) = Utility::ALL.into_iter().rev().find(|u| {
            self.settings.utility.shown(*u, menus) && layout::contains(self.utility_rect(*u), p)
        }) {
            return Some(Target::Utility(u));
        }
        if let Some(i) = self.custom_at(p) {
            return Some(Target::Custom(i));
        }
        self.control_at(p).map(Target::Control)
    }
    /// Where a target is drawn (surface fractions).
    pub fn target_rect(&self, target: Target) -> [f32; 4] {
        match target {
            Target::Control(c) => self.placement(c).rect(self.aspect()),
            Target::Custom(i) => self.custom_rect(i),
            Target::Utility(u) => self.utility_rect(u),
        }
    }
    /// The GUI the utility buttons are laid out in, in GUI pixels.
    fn utility_gui(&self) -> [f32; 2] {
        let surface = if self.surface[0] > 0.0 {
            self.surface
        } else {
            [self.aspect() * 1000.0, 1000.0]
        };
        let b = HudBatch::new([surface[0] as u32, surface[1] as u32]);
        [b.width.max(1.0), b.height.max(1.0)]
    }
    /// A utility button's rectangle, in surface fractions.
    pub fn utility_rect(&self, u: Utility) -> [f32; 4] {
        let gui = self.utility_gui();
        let [x, y, w, h] = self.settings.utility.rect(u, gui);
        [x / gui[0], y / gui[1], w / gui[0], h / gui[1]]
    }
    fn shown_center(&self, target: Target) -> [f32; 2] {
        let [x, y, w, h] = self.target_rect(target);
        [x + w / 2.0, y + h / 2.0]
    }
    /// The saved placement of a target in the current scheme.
    pub fn stored(&self, target: Target) -> Placement {
        let mode = self.settings.mode.index();
        match target {
            Target::Control(c) => self.settings.layouts[mode][self.settings.slot(c)],
            Target::Custom(i) => self.settings.custom_buttons[i].layouts[mode],
            Target::Utility(u) => self.settings.utility.place(u, self.utility_gui()),
        }
    }
    fn stored_mut(&mut self, target: Target) -> Option<&mut Placement> {
        let mode = self.settings.mode.index();
        match target {
            Target::Control(c) => {
                let slot = self.settings.slot(c);
                Some(&mut self.settings.layouts[mode][slot])
            }
            Target::Custom(i) => self
                .settings
                .custom_buttons
                .get_mut(i)
                .map(|b| &mut b.layouts[mode]),
            Target::Utility(u) => {
                // A fixed button becomes a moved one the first time it changes.
                let place = self.settings.utility.place(u, self.utility_gui());
                Some(self.settings.utility.places[u as usize].get_or_insert(place))
            }
        }
    }
    /// Every target shown in the editor's layer.
    pub fn editor_targets(&self) -> Vec<Target> {
        let mut out: Vec<Target> = Control::ALL
            .into_iter()
            .filter(|c| self.visible(*c))
            .map(Target::Control)
            .collect();
        out.extend(
            (0..self.settings.custom_buttons.len())
                .filter(|&i| self.custom_visible(i))
                .map(Target::Custom),
        );
        let menus = self.editor_layer() == Layer::Menus;
        out.extend(
            Utility::ALL
                .into_iter()
                .filter(|u| self.settings.utility.shown(*u, menus))
                .map(Target::Utility),
        );
        out
    }

    /// Moves the dragged control so its centre follows the finger, snapping its
    /// edges and centre to other controls' and to the screen's centre lines, or to
    /// the grid.
    fn drag_to(&mut self, e: &mut Editor, p: [f32; 2]) {
        let Some(drag) = &e.drag else {
            return;
        };
        let target = drag.target;
        let offset = drag.offset;
        let [sw, sh] = self.surface_dp(e);
        let [_, _, fw, fh] = self.target_rect(target);
        let (hw, hh) = (fw * sw / 2.0, fh * sh / 2.0);
        let mut c = [(p[0] - offset[0]) * sw, (p[1] - offset[1]) * sh];
        let mut lines: (Vec<f32>, Vec<f32>) = (vec![sw / 2.0], vec![sh / 2.0]);
        for other in self.editor_targets() {
            if other == target {
                continue;
            }
            let [x, y, w, h] = self.target_rect(other);
            let (x, y, w, h) = (x * sw, y * sh, w * sw, h * sh);
            lines.0.extend([x, x + w / 2.0, x + w]);
            lines.1.extend([y, y + h / 2.0, y + h]);
        }
        let mut guides = (Vec::new(), Vec::new());
        for axis in 0..2 {
            let half = if axis == 0 { hw } else { hh };
            let candidates = if axis == 0 { &lines.0 } else { &lines.1 };
            let mut best: Option<(f32, f32)> = None;
            for anchor in [-half, 0.0, half] {
                for &line in candidates {
                    let d = line - (c[axis] + anchor);
                    if d.abs() < SNAP && best.is_none_or(|(b, _)| d.abs() < b.abs()) {
                        best = Some((d, line));
                    }
                }
            }
            if let Some((d, line)) = best {
                c[axis] += d;
                if axis == 0 {
                    guides.0.push(line / sw);
                } else {
                    guides.1.push(line / sh);
                }
            } else if self.settings.editor_snap {
                c[axis] = ((c[axis] - half) / GRID).round() * GRID + half;
            }
            let limit = if axis == 0 { sw } else { sh };
            c[axis] = c[axis].clamp(half.min(limit / 2.0), (limit - half).max(limit / 2.0));
        }
        e.guides = guides;
        let mut center = [c[0] / sw, c[1] / sh];
        if self.settings.left_handed && !matches!(target, Target::Utility(_)) {
            center[0] = 1.0 - center[0];
        }
        if let Some(place) = self.stored_mut(target) {
            place.center = center;
        }
    }
    fn pinch(&mut self, e: &mut Editor) {
        let (Some(drag), Some(pinch)) = (&e.drag, &e.pinch) else {
            return;
        };
        let (target, moved, from, to) = (drag.target, drag.moved, drag.at, pinch.at);
        let (distance, start) = (pinch.distance, pinch.size);
        let (a, b) = (self.dp(e, from), self.dp(e, to));
        let ratio = (a[0] - b[0]).hypot(a[1] - b[1]) / distance;
        let size = (start * ratio).clamp(MIN_SIZE, max_size(target));
        if !moved {
            Self::snapshot(&self.settings, e);
            if let Some(d) = e.drag.as_mut() {
                d.moved = true;
            }
        }
        if let Some(place) = self.stored_mut(target) {
            place.size = size;
        }
    }

    /// The value a slider shows, 0..=1.
    pub fn slider_value(&self, e: &Editor, kind: Slider) -> f32 {
        let place = e.selected.map(|t| self.stored(t));
        match kind {
            Slider::Size => match (place, e.selected) {
                (Some(p), Some(t)) => (p.size - MIN_SIZE) / (max_size(t) - MIN_SIZE),
                _ => 0.0,
            },
            Slider::Opacity => place.map_or(0.0, |p| p.opacity),
            Slider::Width => match e.selected {
                Some(Target::Custom(i)) => (self.settings.custom_buttons[i].width - 1.0) / 3.0,
                _ => 0.0,
            },
            Slider::Scale => (self.settings.button_scale - 0.5) / 1.5,
        }
        .clamp(0.0, 1.0)
    }
    fn set_value(&mut self, e: &Editor, kind: Slider, t: f32) {
        let t = t.clamp(0.0, 1.0);
        match kind {
            Slider::Scale => self.settings.button_scale = 0.5 + t * 1.5,
            Slider::Width => {
                if let Some(Target::Custom(i)) = e.selected
                    && let Some(b) = self.settings.custom_buttons.get_mut(i)
                {
                    b.width = 1.0 + t * 3.0;
                }
            }
            Slider::Size | Slider::Opacity => {
                if let Some(target) = e.selected
                    && let Some(place) = self.stored_mut(target)
                {
                    if kind == Slider::Size {
                        place.size = MIN_SIZE + t * (max_size(target) - MIN_SIZE);
                    } else {
                        place.opacity = t;
                    }
                }
            }
        }
    }
    fn set_slider(&mut self, e: &Editor, kind: Slider, [x, _, w, _]: [f32; 4], at: [f32; 2]) {
        self.set_value(e, kind, (at[0] - x) / w.max(1.0));
    }

    fn snapshot(settings: &Settings, e: &mut Editor) {
        e.undo.push(settings.clone());
        if e.undo.len() > UNDO_LIMIT {
            e.undo.remove(0);
        }
        e.redo.clear();
    }
    /// Drops a selection that undo or a deletion made stale.
    fn check_selection(&self, e: &mut Editor) {
        if let Some(Target::Custom(i)) = e.selected
            && i >= self.settings.custom_buttons.len()
        {
            e.selected = None;
            if matches!(e.panel, Panel::Props | Panel::Keys { .. }) {
                e.panel = Panel::None;
            }
        }
    }

    /// Runs a panel's action. Returns true when it closed the editor.
    fn apply(&mut self, e: &mut Editor, act: Act) -> bool {
        let selected_custom = match e.selected {
            Some(Target::Custom(i)) if i < self.settings.custom_buttons.len() => Some(i),
            _ => None,
        };
        match act {
            Act::Done => match self.settings.save() {
                Ok(()) => return true,
                Err(error) => {
                    log::warn!("Touch layout: saving failed: {error}");
                    e.save_failed = true;
                }
            },
            Act::Cancel => {
                self.settings = e.original.clone();
                return true;
            }
            Act::Undo => {
                if let Some(previous) = e.undo.pop() {
                    e.redo.push(std::mem::replace(&mut self.settings, previous));
                }
                self.check_selection(e);
            }
            Act::Redo => {
                if let Some(next) = e.redo.pop() {
                    e.undo.push(std::mem::replace(&mut self.settings, next));
                }
                self.check_selection(e);
            }
            Act::Open(panel) => {
                e.panel = if e.panel == panel { Panel::None } else { panel };
                e.scroll = 0.0;
                if matches!(e.panel, Panel::List | Panel::More | Panel::Reset) {
                    e.typing = false;
                }
            }
            Act::Close => {
                e.panel = match e.panel {
                    Panel::Keys { .. } if e.selected.is_some() => Panel::Props,
                    _ => {
                        e.selected = None;
                        Panel::None
                    }
                };
                e.scroll = 0.0;
            }
            Act::Layer(layer) => {
                self.editing = Some(layer);
                if e.selected
                    .is_some_and(|t| !self.editor_targets().contains(&t))
                {
                    e.selected = None;
                    if e.panel == Panel::Props {
                        e.panel = Panel::None;
                    }
                }
            }
            Act::Snap => self.settings.editor_snap ^= true,
            Act::Select(target) => {
                e.selected = Some(target);
                e.panel = Panel::Props;
                e.scroll = 0.0;
            }
            Act::Show(show) => {
                Self::snapshot(&self.settings, e);
                match e.selected {
                    Some(Target::Control(c)) => self.settings.set_show(c, show),
                    Some(Target::Custom(i)) => {
                        if let Some(b) = self.settings.custom_buttons.get_mut(i) {
                            b.show = show.to_u8();
                        }
                    }
                    Some(Target::Utility(u)) => self.settings.utility.set_show(u, show),
                    None => {}
                }
            }
            Act::Eye(target) => {
                Self::snapshot(&self.settings, e);
                self.toggle_shown(self.editor_layer(), target);
            }
            Act::Slider(_) | Act::Sheet => {}
            Act::Step(kind, direction) => {
                Self::snapshot(&self.settings, e);
                let step = match kind {
                    Slider::Size => 0.005 / (e.selected.map_or(MAX_SIZE, max_size) - MIN_SIZE),
                    Slider::Opacity => 0.05,
                    Slider::Width => 0.25 / 3.0,
                    Slider::Scale => 0.05 / 1.5,
                };
                let value = self.slider_value(e, kind) + step * f32::from(direction);
                self.set_value(e, kind, value);
            }
            Act::Nudge(dx, dy) => {
                if let Some(target) = e.selected {
                    Self::snapshot(&self.settings, e);
                    let [sw, sh] = self.surface_dp(e);
                    let mirror =
                        if self.settings.left_handed && !matches!(target, Target::Utility(_)) {
                            -1.0
                        } else {
                            1.0
                        };
                    if let Some(place) = self.stored_mut(target) {
                        place.center[0] =
                            (place.center[0] + mirror * f32::from(dx) * 2.0 / sw).clamp(0.0, 1.0);
                        place.center[1] =
                            (place.center[1] + f32::from(dy) * 2.0 / sh).clamp(0.0, 1.0);
                    }
                }
            }
            Act::OpacityToAll => {
                if let Some(target) = e.selected {
                    Self::snapshot(&self.settings, e);
                    let opacity = self.stored(target).opacity;
                    let mode = self.settings.mode.index();
                    for place in &mut self.settings.layouts[mode] {
                        place.opacity = opacity;
                    }
                    for button in &mut self.settings.custom_buttons {
                        button.layouts[mode].opacity = opacity;
                    }
                }
            }
            Act::Toggle(on) => {
                if let Some(i) = selected_custom {
                    Self::snapshot(&self.settings, e);
                    self.settings.custom_buttons[i].toggle = on;
                }
            }
            Act::Swipe => {
                if let Some(i) = selected_custom {
                    Self::snapshot(&self.settings, e);
                    self.settings.custom_buttons[i].swipe ^= true;
                }
            }
            Act::Shape(shape) => {
                if let Some(i) = selected_custom {
                    Self::snapshot(&self.settings, e);
                    self.settings.custom_buttons[i].shape = shape;
                }
            }
            Act::Color(color) => {
                if let Some(i) = selected_custom {
                    Self::snapshot(&self.settings, e);
                    self.settings.custom_buttons[i].color = color.min(COLORS.len() as u8 - 1);
                }
            }
            Act::Label => {
                if let Some(i) = selected_custom {
                    if e.typing {
                        e.typing = false;
                        e.hide_keyboard = true;
                    } else {
                        Self::snapshot(&self.settings, e);
                        e.typing = true;
                        e.text_request = Some(self.settings.custom_buttons[i].label.clone());
                    }
                }
            }
            Act::Slot(slot) => {
                if let Some(i) = selected_custom {
                    let slot = slot.min(self.settings.custom_buttons[i].slots().len());
                    if slot < MAX_KEYS {
                        e.panel = Panel::Keys { slot, raw: false };
                        e.scroll = 0.0;
                    }
                }
            }
            Act::RemoveSlot(slot) => {
                if let Some(i) = selected_custom {
                    Self::snapshot(&self.settings, e);
                    self.settings.custom_buttons[i].remove_slot(slot);
                }
            }
            Act::Pick(index) => {
                if let (Panel::Keys { slot, raw }, Some(i)) = (e.panel, selected_custom)
                    && let Some((token, _)) = custom::choices(raw).into_iter().nth(index)
                {
                    Self::snapshot(&self.settings, e);
                    self.settings.custom_buttons[i].set_slot(slot, token);
                    e.panel = Panel::Props;
                    e.scroll = 0.0;
                }
            }
            Act::Add => {
                if self.settings.custom_buttons.len() < 32 {
                    Self::snapshot(&self.settings, e);
                    let index = self.settings.custom_buttons.len();
                    let mut button = self.new_custom_button("key.keyboard.unknown".into());
                    if self.editor_layer() == Layer::Menus {
                        button.show = Show::Menus.to_u8();
                    }
                    self.settings.custom_buttons.push(button);
                    e.selected = Some(Target::Custom(index));
                    e.panel = Panel::Keys {
                        slot: 0,
                        raw: false,
                    };
                    e.scroll = 0.0;
                }
            }
            Act::Duplicate => {
                if let Some(i) = selected_custom
                    && self.settings.custom_buttons.len() < 32
                {
                    Self::snapshot(&self.settings, e);
                    let mut copy = self.settings.custom_buttons[i].clone();
                    let [sw, sh] = self.surface_dp(e);
                    let mirror = if self.settings.left_handed { -1.0 } else { 1.0 };
                    for place in &mut copy.layouts {
                        let shift = [mirror * 16.0 / sw, 16.0 / sh];
                        place.center[0] = (place.center[0] + shift[0]).clamp(0.0, 1.0);
                        place.center[1] = (place.center[1] + shift[1]).clamp(0.0, 1.0);
                    }
                    self.settings.custom_buttons.push(copy);
                    e.selected = Some(Target::Custom(self.settings.custom_buttons.len() - 1));
                }
            }
            Act::Delete => {
                Self::snapshot(&self.settings, e);
                match e.selected {
                    Some(Target::Custom(i)) if i < self.settings.custom_buttons.len() => {
                        self.settings.custom_buttons.remove(i);
                    }
                    Some(Target::Control(c)) => self.settings.set_show(c, Show::Hidden),
                    Some(Target::Utility(u)) => self.settings.utility.set_show(u, Show::Hidden),
                    _ => {}
                }
                e.selected = None;
                e.panel = Panel::None;
                e.typing = false;
            }
            Act::Restore => {
                Self::snapshot(&self.settings, e);
                let mode = self.settings.mode.index();
                match e.selected {
                    Some(Target::Control(c)) => {
                        let slot = self.settings.slot(c);
                        self.settings.layouts[mode][slot] = Settings::default().layouts[mode][slot];
                    }
                    Some(Target::Utility(u)) => self.settings.utility.places[u as usize] = None,
                    _ => {}
                }
            }
            Act::ResetLayout => {
                Self::snapshot(&self.settings, e);
                let mode = self.settings.mode.index();
                self.settings.layouts[mode] = Settings::default().layouts[mode];
                self.settings.button_visibility = [0; 14];
                self.settings.utility = Default::default();
                // Custom buttons go back to free space, one after another, instead of
                // onto a fixed grid over the action buttons.
                for i in 0..self.settings.custom_buttons.len() {
                    let [x, y] = self.free_custom_spot(mode, i);
                    self.settings.custom_buttons[i].layouts[mode] =
                        Placement::new(x, y, Self::NEW_CUSTOM_SIZE);
                }
                e.selected = None;
                e.panel = Panel::None;
            }
            Act::CopyToSchemes => {
                Self::snapshot(&self.settings, e);
                let mode = self.settings.mode.index();
                let layout = self.settings.layouts[mode];
                for (i, row) in self.settings.layouts.iter_mut().enumerate() {
                    if i != mode {
                        *row = layout;
                    }
                }
                for button in &mut self.settings.custom_buttons {
                    let place = button.layouts[mode];
                    button.layouts = [place; 3];
                }
                e.panel = Panel::None;
            }
        }
        false
    }
    /// The list's eye: hides a shown control, or shows a hidden one, in `layer`
    /// only: the other layer keeps what it had.
    fn toggle_shown(&mut self, layer: Layer, target: Target) {
        let in_menus = layer == Layer::Menus;
        let (mut game, mut menus) = match target {
            Target::Control(c) => {
                if in_menus && !c.menu_capable() {
                    return;
                }
                (
                    self.settings.visible(c),
                    self.settings.visible_in_menu(c, true),
                )
            }
            Target::Custom(i) => match self.settings.custom_buttons.get(i) {
                Some(b) => (b.visible(false), b.visible(true)),
                None => return,
            },
            Target::Utility(u) => {
                if !in_menus && !u.in_game() {
                    return;
                }
                (
                    self.settings.utility.shown(u, false),
                    self.settings.utility.shown(u, true),
                )
            }
        };
        if in_menus {
            menus = !menus;
        } else {
            game = !game;
        }
        let show = match (game, menus) {
            (true, true) => Show::Always,
            (true, false) => Show::Game,
            (false, true) => Show::Menus,
            (false, false) => Show::Hidden,
        };
        match target {
            Target::Control(c) => self.settings.set_show(c, show),
            Target::Custom(i) => self.settings.custom_buttons[i].show = show.to_u8(),
            Target::Utility(u) => self.settings.utility.set_show(u, show),
        }
    }
}
