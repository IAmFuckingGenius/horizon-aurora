//! The touch layout editor's look: Horizon's «Горизонт» panels over the game, in
//! dp at the display's density. The editor's model and input live in
//! `touch::editor`; this draws one frame and records its tappable rectangles
//! there, so the next touch is matched against what the player sees.
use crate::hud::{HudAtlas, HudBatch};
use crate::pixel_kit::{self as kit, Icon, Kit, Style, col};
use crate::touch::{
    Act, COLORS, Control, Layer, MAX_KEYS, Panel, Shape, Show, Slider, Target, TouchControls,
    Utility, key_choices,
};
use crate::ui_theme::{Face, Ui, color::hex};

/// An editor string in the game's language.
fn t(key: &str) -> String {
    crate::lang::get(&format!("horizon.touch.editor.{key}"))
}

/// What one frame needs from the editor, copied out so the kit can hold its
/// tap targets.
struct Frame {
    panel: Panel,
    selected: Option<Target>,
    scroll: f32,
    layer: Layer,
    dragging: bool,
    typing: bool,
    hint: bool,
    save_failed: bool,
    can_undo: bool,
    can_redo: bool,
    guides: (Vec<f32>, Vec<f32>),
    /// Slider values: size, opacity, width, scale.
    values: [f32; 4],
    /// The saved layouts' names and custom-button counts.
    profiles: Vec<(String, usize)>,
    /// The new layout's name being typed.
    naming: Option<String>,
    /// A short message to show now.
    toast: Option<String>,
}
/// What drawing a panel reports back: its scrolling viewport and how far it scrolls.
#[derive(Default)]
struct Sheet {
    viewport: Option<[f32; 4]>,
    scroll_max: f32,
}

/// The dim layer, grid and hotbar zone, drawn under the controls.
pub fn backdrop(touch: &TouchControls, b: &mut HudBatch, a: &HudAtlas, density: f32) {
    if touch.editor.is_none() {
        return;
    }
    let mut ui = Ui::launcher(a, b, (0.0, 0.0), density);
    let (w, h) = (ui.w, ui.h);
    kit::fill(&mut ui, 0.0, 0.0, w, h, hex(0x0a0820, 110));
    if touch.settings.editor_snap {
        let line = hex(0xffffff, 16);
        let mut x = 32.0;
        while x < w {
            kit::fill(&mut ui, x, 0.0, 1.0, h, line);
            x += 32.0;
        }
        let mut y = 32.0;
        while y < h {
            kit::fill(&mut ui, 0.0, y, w, 1.0, line);
            y += 32.0;
        }
    }
    // The hotbar takes its own touches: controls over it cover slots.
    let gui = ui.batch.scale / ui.density;
    let (gw, gh) = (ui.batch.width, ui.batch.height);
    let zone = [
        (gw / 2.0 - 91.0) * gui,
        (gh - 22.0) * gui,
        182.0 * gui,
        22.0 * gui,
    ];
    dashed(&mut ui, zone, hex(0xf4f2ff, 110));
    let label = t("hotbar");
    let lw = ui.measure(Face::Mono, 10.0, &label);
    ui.text_line(
        Face::Mono,
        10.0,
        12.0,
        zone[0] + (zone[2] - lw) / 2.0,
        zone[1] - 14.0,
        hex(0xf4f2ff, 150),
        &label,
    );
    ui.finish();
}

/// The selection, guides, toolbar and panels, drawn over the controls.
pub fn draw(touch: &mut TouchControls, b: &mut HudBatch, a: &HudAtlas, density: f32) {
    let Some(mut e) = touch.editor.take() else {
        return;
    };
    let frame = Frame {
        panel: e.panel,
        selected: e.selected,
        scroll: e.scroll,
        layer: touch.editor_layer(),
        dragging: e.dragging(),
        typing: e.typing,
        hint: e.hint,
        save_failed: e.save_failed,
        can_undo: e.can_undo(),
        can_redo: e.can_redo(),
        guides: e.guides.clone(),
        profiles: e
            .profiles
            .iter()
            .map(|p| (p.name.clone(), p.custom_buttons.len()))
            .collect(),
        naming: e.naming.clone(),
        toast: e
            .toast
            .as_ref()
            .filter(|(_, at)| at.elapsed() < std::time::Duration::from_millis(2600))
            .map(|(text, _)| text.clone()),
        values: [
            touch.slider_value(&e, Slider::Size),
            touch.slider_value(&e, Slider::Opacity),
            touch.slider_value(&e, Slider::Width),
            touch.slider_value(&e, Slider::Scale),
        ],
    };
    let press = e.pressed_at();
    let mut ui = Ui::launcher(a, b, (0.0, 0.0), density);
    e.density = ui.density;
    e.targets.clear();
    let sheet = {
        let mut k = Kit {
            ui: &mut ui,
            targets: &mut e.targets,
            press,
            flash: None,
        };
        draw_frame(&mut k, touch, &frame)
    };
    e.sheet = sheet.viewport;
    e.scroll_max = sheet.scroll_max;
    e.scroll = e.scroll.min(e.scroll_max);
    ui.finish();
    touch.editor = Some(e);
}

fn draw_frame(k: &mut Kit<Act>, touch: &TouchControls, f: &Frame) -> Sheet {
    let (w, h) = (k.ui.w, k.ui.h);
    let surface = |r: [f32; 4]| [r[0] * w, r[1] * h, r[2] * w, r[3] * h];
    // Every control's outline, so the player sees what can be moved.
    for target in touch.editor_targets() {
        if Some(target) != f.selected {
            outline(k.ui, surface(touch.target_rect(target)), hex(0xf4f2ff, 70));
        }
    }
    for x in &f.guides.0 {
        kit::fill(k.ui, x * w - 0.5, 0.0, 1.0, h, col::ACCENT_HI);
    }
    for y in &f.guides.1 {
        kit::fill(k.ui, 0.0, y * h - 0.5, w, 1.0, col::ACCENT_HI);
    }
    if let Some(target) = f.selected
        && touch.editor_targets().contains(&target)
    {
        let r = surface(touch.target_rect(target));
        kit::ring(
            k.ui,
            [r[0] - 4.0, r[1] - 4.0, r[2] + 8.0, r[3] + 8.0],
            1,
            col::ACCENT_HI,
        );
        let label = format!("{} · {} dp", name(touch, target), r[3].round());
        let lw = k.ui.measure(Face::BodySemiBold, 12.0, &label) + 16.0;
        let ly = if r[1] - 34.0 >= 4.0 {
            r[1] - 34.0
        } else {
            r[1] + r[3] + 8.0
        };
        let lx = (r[0] + r[2] / 2.0 - lw / 2.0).clamp(4.0, (w - lw - 4.0).max(4.0));
        kit::stepped(k.ui, [lx, ly, lw, 24.0], 1, 2.0, col::ACCENT);
        k.ui.text_line(
            Face::BodySemiBold,
            12.0,
            14.0,
            lx + 8.0,
            ly + 5.0,
            col::WHITE,
            &label,
        );
    }
    if f.dragging {
        return Sheet::default();
    }
    // The panel docks on the side away from the selected control.
    let portrait = h > w;
    let left = f.selected.is_some_and(|t| {
        let [x, _, rw, _] = touch.target_rect(t);
        x + rw / 2.0 > 0.5
    });
    let sheet_rect = if portrait {
        let sh = (h * 0.5).max(260.0).min(h - 16.0);
        [8.0, h - sh - 8.0, w - 16.0, sh]
    } else {
        let sw = (w * 0.4).clamp(280.0, 360.0).min(w - 16.0);
        [if left { 8.0 } else { w - sw - 8.0 }, 8.0, sw, h - 16.0]
    };
    let has_sheet = matches!(
        f.panel,
        Panel::Props | Panel::List | Panel::Keys { .. } | Panel::More
    ) && !(f.panel == Panel::Props && f.selected.is_none());
    // The toolbar sits in the free part of the screen, clear of the panel.
    let free = if has_sheet && !portrait {
        if left {
            [sheet_rect[0] + sheet_rect[2], w]
        } else {
            [0.0, sheet_rect[0]]
        }
    } else {
        [0.0, w]
    };
    let bar_y = if portrait && has_sheet {
        (sheet_rect[1] - 72.0).max(8.0)
    } else {
        (h * 0.3 - 28.0).max(8.0)
    };
    toolbar(k, f, free, bar_y);
    if let Some(text) = &f.toast {
        toast(k, w, h, text, false);
    } else if f.hint && !has_sheet {
        toast(k, w, h, &t("hint"), false);
    } else if f.save_failed {
        toast(k, w, h, &t("save_failed"), true);
    }
    let mut out = Sheet::default();
    if has_sheet {
        out = sheet(k, touch, f, sheet_rect);
    }
    if f.panel == Panel::Reset {
        confirm_reset(k, w, h);
    }
    if let Panel::DeleteProfile(i) = f.panel {
        let name = f.profiles.get(i).map_or("", |p| p.0.as_str());
        confirm(
            k,
            w,
            h,
            &t("delete_layout_title"),
            &t("delete_layout_text").replace("%s", name),
            &t("delete"),
            Act::ConfirmDelete(i),
        );
    }
    out
}

/// A control's name in the editor: built-in names, or a custom button's label.
fn name(touch: &TouchControls, target: Target) -> String {
    match target {
        Target::Control(c) => c.name(),
        Target::Custom(i) => touch
            .settings
            .custom_buttons
            .get(i)
            .map_or_else(String::new, |b| b.label()),
        Target::Utility(u) => u.name(),
    }
}

fn outline(ui: &mut Ui, [x, y, w, h]: [f32; 4], color: u32) {
    kit::fill(ui, x, y, w, 1.0, color);
    kit::fill(ui, x, y + h - 1.0, w, 1.0, color);
    kit::fill(ui, x, y + 1.0, 1.0, h - 2.0, color);
    kit::fill(ui, x + w - 1.0, y + 1.0, 1.0, h - 2.0, color);
}
fn dashed(ui: &mut Ui, [x, y, w, h]: [f32; 4], color: u32) {
    let mut i = 0.0;
    while i < w {
        let len = (w - i).min(6.0);
        kit::fill(ui, x + i, y, len, 1.0, color);
        kit::fill(ui, x + i, y + h - 1.0, len, 1.0, color);
        i += 10.0;
    }
    let mut j = 0.0;
    while j < h {
        let len = (h - j).min(6.0);
        kit::fill(ui, x, y + j, 1.0, len, color);
        kit::fill(ui, x + w - 1.0, y + j, 1.0, len, color);
        j += 10.0;
    }
}

/// A square icon button.
fn icon_button(k: &mut Kit<Act>, r: [f32; 4], icon: Icon, act: Act, enabled: bool, on: bool) {
    let pressed = enabled && k.pressed(r, Some(act));
    let (body, edge, ink) = if !enabled {
        (col::PLATE, col::RULE, hex(0xa59fc6, 110))
    } else if on {
        (col::ACCENT, col::ACCENT_HI, col::WHITE)
    } else if pressed {
        (col::CONTROL_PRESSED, col::CONTROL_RING, col::INK)
    } else {
        (col::CONTROL, col::CONTROL_RING, col::INK)
    };
    kit::plate(k.ui, r, 1, body, edge);
    kit::icon(
        k.ui,
        icon,
        r[0] + (r[2] - 24.0) / 2.0,
        r[1] + (r[3] - 24.0) / 2.0,
        24.0,
        ink,
    );
    if enabled {
        k.hit(r, act);
    }
}

fn toolbar(k: &mut Kit<Act>, f: &Frame, [from, to]: [f32; 2], y: f32) {
    let done = t("done");
    let done_w = k.button_width(Style::Primary, Some(Icon::Pixel("check")), &done);
    let game = t("layer_game");
    let menus = t("layer_menus");
    let seg_w = k.ui.measure(Face::BodySemiBold, 14.0, &game)
        + k.ui.measure(Face::BodySemiBold, 14.0, &menus)
        + 56.0;
    let icons = 6.0 * 40.0 + 5.0 * 6.0;
    let gaps = 3.0 * 12.0;
    let width = 16.0 + done_w + icons + seg_w + gaps;
    let avail = to - from - 16.0;
    // Narrow screens drop the layer switch's labels into two icon-free halves
    // and, if still too wide, the button's label.
    let compact = width > avail;
    let done_w = if compact { 48.0 } else { done_w };
    let seg_w = if compact { 112.0 } else { seg_w };
    let width = 16.0 + done_w + icons + seg_w + gaps;
    let x0 = (from + (to - from - width) / 2.0).max(from + 4.0);
    k.panel([x0, y, width, 56.0], false);
    let mut x = x0 + 8.0;
    let by = y + 8.0;
    k.button(
        [x, by, done_w, 40.0],
        Style::Primary,
        Some(Icon::Pixel("check")),
        if compact { "" } else { &done },
        Some(Act::Done),
    );
    x += done_w + 6.0;
    icon_button(
        k,
        [x, by, 40.0, 40.0],
        Icon::Pixel("close"),
        Act::Cancel,
        true,
        false,
    );
    x += 40.0 + 12.0;
    icon_button(
        k,
        [x, by, 40.0, 40.0],
        Icon::Sprite(UNDO),
        Act::Undo,
        f.can_undo,
        false,
    );
    x += 46.0;
    icon_button(
        k,
        [x, by, 40.0, 40.0],
        Icon::Sprite(REDO),
        Act::Redo,
        f.can_redo,
        false,
    );
    x += 40.0 + 12.0;
    let (game, menus) = if compact {
        (t("layer_game_short"), t("layer_menus_short"))
    } else {
        (game, menus)
    };
    k.segmented(
        [x, by, seg_w, 40.0],
        &[
            (None, &game, Act::Layer(Layer::Game), f.layer == Layer::Game),
            (
                None,
                &menus,
                Act::Layer(Layer::Menus),
                f.layer == Layer::Menus,
            ),
        ],
        true,
    );
    x += seg_w + 12.0;
    icon_button(
        k,
        [x, by, 40.0, 40.0],
        Icon::Pixel("plus"),
        Act::Add,
        true,
        false,
    );
    x += 46.0;
    icon_button(
        k,
        [x, by, 40.0, 40.0],
        Icon::Sprite(LIST),
        Act::Open(Panel::List),
        true,
        f.panel == Panel::List,
    );
    x += 46.0;
    icon_button(
        k,
        [x, by, 40.0, 40.0],
        Icon::Pixel("sliders"),
        Act::Open(Panel::More),
        true,
        f.panel == Panel::More,
    );
}

fn toast(k: &mut Kit<Act>, w: f32, h: f32, text: &str, danger: bool) {
    let width = (w - 32.0).min(560.0);
    let lines = k.wrap_max(Face::BodyMedium, 13.0, text, width - 32.0, 3);
    let th = lines.len() as f32 * 17.0 + 20.0;
    let x = (w - width) / 2.0;
    let y = h * 0.62 - th / 2.0;
    kit::plate(
        k.ui,
        [x, y, width, th],
        1,
        hex(0x15112e, 236),
        if danger {
            col::DANGER_RING
        } else {
            col::PLATE_RING
        },
    );
    for (i, line) in lines.iter().enumerate() {
        let lw = k.ui.measure(Face::BodyMedium, 13.0, line);
        k.ui.text_line(
            Face::BodyMedium,
            13.0,
            17.0,
            x + (width - lw) / 2.0,
            y + 10.0 + i as f32 * 17.0,
            if danger { col::DANGER } else { col::INK_SOFT },
            line,
        );
    }
}

/// The side (or, in portrait, bottom) panel: a header, then a scrolling body.
fn sheet(k: &mut Kit<Act>, touch: &TouchControls, f: &Frame, r: [f32; 4]) -> Sheet {
    let [x, y, w, h] = r;
    // The whole panel takes its taps; drags on it scroll.
    k.hit(r, Act::Sheet);
    kit::plate(k.ui, r, 2, hex(0x15112e, 244), col::PLATE_RING);
    let (title, back) = match f.panel {
        Panel::Props => (
            f.selected.map_or_else(String::new, |s| name(touch, s)),
            false,
        ),
        Panel::List => (t("list_title"), false),
        Panel::Keys { slot, .. } => (
            crate::lang::get("horizon.touch.editor.key_title")
                .replace("%s", &(slot + 1).to_string()),
            true,
        ),
        Panel::More => (t("more_title"), false),
        _ => (String::new(), false),
    };
    let mut tx = x + 16.0;
    if back {
        icon_button(
            k,
            [x + 10.0, y + 10.0, 36.0, 36.0],
            Icon::Sprite(kit::CHEVRON_LEFT),
            Act::Close,
            true,
            false,
        );
        tx += 40.0;
    }
    let close_x = x + w - 46.0;
    k.text_fit(
        Face::BodyBold,
        17.0,
        20.0,
        tx,
        y + 18.0,
        close_x - tx - 8.0,
        col::INK,
        &title,
    );
    if !back {
        icon_button(
            k,
            [close_x, y + 10.0, 36.0, 36.0],
            Icon::Pixel("close"),
            Act::Close,
            true,
            false,
        );
    }
    kit::fill(k.ui, x + 2.0, y + 55.0, w - 4.0, 1.0, col::RULE);
    let body = [x + 2.0, y + 56.0, w - 4.0, h - 58.0];
    let mark = k.mark();
    let top = body[1] + 12.0 - f.scroll;
    let cx = x + 16.0;
    let cw = w - 32.0;
    let end = match f.panel {
        Panel::Props => match f.selected {
            Some(Target::Custom(i)) if i < touch.settings.custom_buttons.len() => {
                custom_props(k, touch, f, i, cx, top, cw)
            }
            Some(Target::Control(c)) => control_props(k, touch, f, c, cx, top, cw),
            Some(Target::Utility(u)) => utility_props(k, touch, f, u, cx, top, cw),
            _ => top,
        },
        Panel::List => list(k, touch, f, cx, top, cw),
        Panel::Keys { slot, raw } => keys(k, slot, raw, cx, top, cw),
        Panel::More => more(k, touch, f, cx, top, cw),
        _ => top,
    };
    k.clip(mark, body);
    let content = end + f.scroll - body[1] + 12.0;
    Sheet {
        viewport: Some(body),
        scroll_max: (content - body[3]).max(0.0),
    }
}

/// A section label; returns the y below it.
fn section(k: &mut Kit<Act>, x: f32, y: f32, text: &str) -> f32 {
    k.section_label(x, y, text);
    y + 22.0
}

/// A slider with its value and − / + steps; returns the y below it.
#[allow(clippy::too_many_arguments)]
fn slider(
    k: &mut Kit<Act>,
    x: f32,
    y: f32,
    w: f32,
    label: &str,
    value_text: &str,
    value: f32,
    kind: Slider,
) -> f32 {
    k.ui.text_line(Face::BodySemiBold, 14.0, 17.0, x, y, col::INK, label);
    let vw = k.ui.measure(Face::Mono, 12.0, value_text);
    k.ui.text_line(
        Face::Mono,
        12.0,
        17.0,
        x + w - vw,
        y + 1.0,
        col::ACCENT_HI,
        value_text,
    );
    let row = y + 24.0;
    icon_button(
        k,
        [x, row, 36.0, 36.0],
        Icon::Sprite(MINUS),
        Act::Step(kind, -1),
        true,
        false,
    );
    icon_button(
        k,
        [x + w - 36.0, row, 36.0, 36.0],
        Icon::Pixel("plus"),
        Act::Step(kind, 1),
        true,
        false,
    );
    let track = [x + 48.0, row, w - 96.0, 36.0];
    let ty = row + 15.0;
    kit::stepped(k.ui, [track[0], ty, track[2], 6.0], 1, 2.0, col::SWITCH_OFF);
    let filled = track[2] * value.clamp(0.0, 1.0);
    if filled > 4.0 {
        kit::stepped(k.ui, [track[0], ty, filled, 6.0], 1, 2.0, col::ACCENT);
    }
    let thumb = [track[0] + filled - 8.0, row + 4.0, 16.0, 28.0];
    kit::stepped(k.ui, thumb, 1, 2.0, col::INK);
    k.hit(track, Act::Slider(kind));
    row + 44.0
}

/// Where-to-show choices; returns the y below them.
fn show_choice(k: &mut Kit<Act>, x: f32, y: f32, w: f32, current: Show, options: &[Show]) -> f32 {
    let labels: Vec<(Show, String)> = options
        .iter()
        .map(|s| {
            let key = match s {
                Show::Auto => "show_auto",
                Show::Game => "show_game",
                Show::Menus => "show_menus",
                Show::Always => "show_always",
                Show::Hidden => "show_hidden",
            };
            (*s, t(key))
        })
        .collect();
    // Up to three per row.
    let mut y = y;
    for chunk in labels.chunks(3) {
        let items: Vec<(Option<&'static str>, &str, Act, bool)> = chunk
            .iter()
            .map(|(s, label)| (None, label.as_str(), Act::Show(*s), *s == current))
            .collect();
        k.segmented([x, y, w, 40.0], &items, true);
        y += 46.0;
    }
    y
}

fn nudge(k: &mut Kit<Act>, x: f32, y: f32, w: f32) -> f32 {
    let y = section(k, x, y, &t("position"));
    let size = 44.0;
    let gap = ((w - 4.0 * size) / 3.0).min(12.0);
    for (i, (icon, dx, dy)) in [
        (kit::CHEVRON_LEFT, -1, 0),
        (UP, 0, -1),
        (DOWN, 0, 1),
        (RIGHT, 1, 0),
    ]
    .into_iter()
    .enumerate()
    {
        icon_button(
            k,
            [x + i as f32 * (size + gap), y, size, size],
            Icon::Sprite(icon),
            Act::Nudge(dx, dy),
            true,
            false,
        );
    }
    let hint = t("position_hint");
    let hx = x + 4.0 * (size + gap);
    let used = k.paragraph(
        Face::Body,
        12.0,
        14.0,
        hx,
        y + 8.0,
        (x + w - hx).max(0.0),
        usize::MAX,
        col::MUTED,
        &hint,
    );
    y + size.max(8.0 + used) + 16.0
}

fn size_text(touch: &TouchControls, target: Target, h: f32) -> String {
    format!("{} dp", (touch.target_rect(target)[3] * h).round())
}

fn control_props(
    k: &mut Kit<Act>,
    touch: &TouchControls,
    f: &Frame,
    c: Control,
    x: f32,
    y: f32,
    w: f32,
) -> f32 {
    let h = k.ui.h;
    let mut y = section(k, x, y, &t("where"));
    let options: &[Show] = if c.menu_capable() {
        &[
            Show::Game,
            Show::Menus,
            Show::Always,
            Show::Auto,
            Show::Hidden,
        ]
    } else {
        &[Show::Auto, Show::Game, Show::Hidden]
    };
    y = show_choice(k, x, y, w, touch.settings.show(c), options);
    y += k.paragraph(
        Face::Body,
        12.0,
        15.0,
        x,
        y - 2.0,
        w,
        usize::MAX,
        col::MUTED,
        &t(if c.menu_capable() {
            "where_hint_menu"
        } else {
            "where_hint"
        }),
    ) + 10.0;
    let target = Target::Control(c);
    y = slider(
        k,
        x,
        y,
        w,
        &t("size"),
        &size_text(touch, target, h),
        f.values[0],
        Slider::Size,
    );
    y = slider(
        k,
        x,
        y,
        w,
        &t("opacity"),
        &format!("{:.0} %", f.values[1] * 100.0),
        f.values[1],
        Slider::Opacity,
    );
    k.button(
        [x, y, w, 40.0],
        Style::Ghost,
        None,
        &t("opacity_all"),
        Some(Act::OpacityToAll),
    );
    y += 52.0;
    y = nudge(k, x, y, w);
    footer_restore_hide(k, x, y, w)
}

/// "Back to its place" and "Hide", side by side; returns the y below them.
fn footer_restore_hide(k: &mut Kit<Act>, x: f32, y: f32, w: f32) -> f32 {
    let half = (w - 8.0) / 2.0;
    k.button(
        [x, y, half, 44.0],
        Style::Secondary,
        Some(Icon::Pixel("reload")),
        &t("restore"),
        Some(Act::Restore),
    );
    k.button(
        [x + half + 8.0, y, half, 44.0],
        Style::Danger,
        Some(Icon::Sprite(EYE_OFF)),
        &t("hide"),
        Some(Act::Delete),
    );
    y + 56.0
}

fn utility_props(
    k: &mut Kit<Act>,
    touch: &TouchControls,
    f: &Frame,
    u: Utility,
    x: f32,
    y: f32,
    w: f32,
) -> f32 {
    let h = k.ui.h;
    let mut y = section(k, x, y, &t("where"));
    let options: &[Show] = if u.in_game() {
        &[Show::Game, Show::Menus, Show::Always, Show::Hidden]
    } else {
        &[Show::Menus, Show::Hidden]
    };
    y = show_choice(k, x, y, w, touch.settings.utility.show(u), options);
    y += k.paragraph(
        Face::Body,
        12.0,
        15.0,
        x,
        y - 2.0,
        w,
        usize::MAX,
        col::MUTED,
        &t(&format!("utility_hint.{}", u.id())),
    ) + 10.0;
    let target = Target::Utility(u);
    y = slider(
        k,
        x,
        y,
        w,
        &t("size"),
        &size_text(touch, target, h),
        f.values[0],
        Slider::Size,
    );
    y = slider(
        k,
        x,
        y,
        w,
        &t("opacity"),
        &format!("{:.0} %", f.values[1] * 100.0),
        f.values[1],
        Slider::Opacity,
    );
    y = nudge(k, x, y, w);
    footer_restore_hide(k, x, y, w)
}

#[allow(clippy::too_many_arguments)]
fn custom_props(
    k: &mut Kit<Act>,
    touch: &TouchControls,
    f: &Frame,
    i: usize,
    x: f32,
    y: f32,
    w: f32,
) -> f32 {
    let h = k.ui.h;
    let button = &touch.settings.custom_buttons[i];
    let mut y = section(k, x, y, &t("label"));
    let auto = {
        let mut auto = button.clone();
        auto.label.clear();
        auto.label()
    };
    k.field(
        [x, y, w, 44.0],
        Some("notes"),
        &button.label,
        &auto,
        Act::Label,
        f.typing,
    );
    y += 56.0;
    y = section(k, x, y, &t("keys"));
    let slots = button.slots();
    for (slot, token) in slots.iter().enumerate() {
        let r = [x, y, w - 46.0, 40.0];
        let label = crate::touch::CustomButton::token_label(token);
        k.dropdown(r, None, &label, Act::Slot(slot));
        icon_button(
            k,
            [x + w - 40.0, y, 40.0, 40.0],
            Icon::Pixel("trash"),
            Act::RemoveSlot(slot),
            slots.len() > 1 || *token != "key.keyboard.unknown",
            false,
        );
        y += 46.0;
    }
    if slots.len() < MAX_KEYS {
        k.button(
            [x, y, w, 40.0],
            Style::Ghost,
            Some(Icon::Pixel("plus")),
            &t("add_key"),
            Some(Act::Slot(MAX_KEYS)),
        );
        y += 44.0;
    }
    y += k.paragraph(
        Face::Body,
        12.0,
        15.0,
        x,
        y,
        w,
        usize::MAX,
        col::MUTED,
        &t("keys_hint"),
    ) + 12.0;
    y = section(k, x, y, &t("mode"));
    let hold = t("mode_hold");
    let toggle = t("mode_toggle");
    k.segmented(
        [x, y, w, 40.0],
        &[
            (None, &hold, Act::Toggle(false), !button.toggle),
            (None, &toggle, Act::Toggle(true), button.toggle),
        ],
        true,
    );
    y += 50.0;
    // The sliding switch, with its explanation.
    let text_w = w - 72.0;
    k.ui.text_line(Face::BodySemiBold, 14.0, 17.0, x, y, col::INK, &t("swipe"));
    let used = k.paragraph(
        Face::Body,
        12.0,
        15.0,
        x,
        y + 20.0,
        text_w,
        usize::MAX,
        col::MUTED,
        &t("swipe_hint"),
    );
    k.switch(x + w - 60.0, y + 2.0, button.swipe, Act::Swipe);
    y += (20.0 + used).max(40.0) + 14.0;
    y = section(k, x, y, &t("where"));
    y = show_choice(
        k,
        x,
        y,
        w,
        match button.show() {
            Show::Auto => Show::Game,
            s => s,
        },
        &[Show::Game, Show::Menus, Show::Always],
    );
    y += 6.0;
    y = section(k, x, y, &t("look"));
    let classic = t("shape_classic");
    let flat = t("shape_flat");
    let round = t("shape_round");
    k.segmented(
        [x, y, w, 40.0],
        &[
            (
                None,
                &classic,
                Act::Shape(Shape::Classic),
                button.shape == Shape::Classic,
            ),
            (
                None,
                &flat,
                Act::Shape(Shape::Flat),
                button.shape == Shape::Flat,
            ),
            (
                None,
                &round,
                Act::Shape(Shape::Round),
                button.shape == Shape::Round,
            ),
        ],
        true,
    );
    y += 50.0;
    let swatch = ((w - 7.0 * 6.0) / 8.0).min(36.0);
    for (ci, rgb) in COLORS.iter().enumerate() {
        let r = [x + ci as f32 * (swatch + 6.0), y, swatch, swatch];
        if button.color as usize == ci {
            kit::ring(
                k.ui,
                [r[0] - 4.0, r[1] - 4.0, r[2] + 8.0, r[3] + 8.0],
                1,
                col::ACCENT_HI,
            );
        }
        kit::plate(k.ui, r, 1, hex(*rgb, 255), hex(0xffffff, 60));
        k.hit(r, Act::Color(ci as u8));
    }
    y += swatch + 16.0;
    let target = Target::Custom(i);
    y = slider(
        k,
        x,
        y,
        w,
        &t("size"),
        &size_text(touch, target, h),
        f.values[0],
        Slider::Size,
    );
    y = slider(
        k,
        x,
        y,
        w,
        &t("width"),
        &format!("×{:.2}", button.width),
        f.values[2],
        Slider::Width,
    );
    y = slider(
        k,
        x,
        y,
        w,
        &t("opacity"),
        &format!("{:.0} %", f.values[1] * 100.0),
        f.values[1],
        Slider::Opacity,
    );
    y = nudge(k, x, y, w);
    let half = (w - 8.0) / 2.0;
    k.button(
        [x, y, half, 44.0],
        Style::Secondary,
        Some(Icon::Sprite(kit::COPY)),
        &t("duplicate"),
        Some(Act::Duplicate),
    );
    k.button(
        [x + half + 8.0, y, half, 44.0],
        Style::Danger,
        Some(Icon::Pixel("trash")),
        &t("delete"),
        Some(Act::Delete),
    );
    y + 56.0
}

/// Where a control shows, as the list says it.
fn status(touch: &TouchControls, target: Target) -> String {
    let show = match target {
        Target::Control(c) => {
            let show = touch.settings.show(c);
            if show == Show::Auto {
                let game = touch.settings.visible(c);
                let menus = touch.settings.visible_in_menu(c, true);
                return match (game, menus) {
                    (true, true) => t("status_auto_always"),
                    (true, false) => t("status_auto_game"),
                    (false, true) => t("status_auto_menus"),
                    (false, false) => t("status_auto_hidden"),
                };
            }
            show
        }
        Target::Custom(i) => match touch.settings.custom_buttons[i].show() {
            Show::Auto => Show::Game,
            s => s,
        },
        Target::Utility(u) => touch.settings.utility.show(u),
    };
    t(match show {
        Show::Game => "show_game",
        Show::Menus => "show_menus",
        Show::Always => "show_always",
        Show::Hidden => "show_hidden",
        Show::Auto => "show_auto",
    })
}

fn control_icon(c: Control) -> Icon {
    match c {
        Control::Move => Icon::Pixel("gamepad"),
        Control::Sprint => Icon::Pixel("zap"),
        Control::Jump => Icon::Sprite(UP),
        Control::Sneak => Icon::Sprite(DOWN),
        Control::Attack => Icon::Sprite(SWORD),
        Control::Use => Icon::Pixel("box"),
        Control::Chat => Icon::Pixel("message"),
        Control::Pause => Icon::Sprite(PAUSE),
        Control::Settings => Icon::Pixel("sliders"),
        Control::Inventory => Icon::Pixel("archive"),
        Control::Debug => Icon::Pixel("bug"),
        Control::Perspective => Icon::Pixel("monitor"),
        Control::Offhand => Icon::Sprite(SWAP),
        Control::Shield => Icon::Sprite(SHIELD),
    }
}

fn list(k: &mut Kit<Act>, touch: &TouchControls, f: &Frame, x: f32, y: f32, w: f32) -> f32 {
    let mut y = y;
    k.button(
        [x, y, w, 44.0],
        Style::Primary,
        Some(Icon::Pixel("plus")),
        &t("new_button"),
        Some(Act::Add),
    );
    y += 56.0;
    let layer = t(if f.layer == Layer::Game {
        "list_layer_game"
    } else {
        "list_layer_menus"
    });
    y += k.paragraph(
        Face::Body,
        12.0,
        15.0,
        x,
        y,
        w,
        usize::MAX,
        col::MUTED,
        &layer,
    ) + 10.0;
    let mut rows: Vec<(Target, Icon)> = Control::ALL
        .into_iter()
        .map(|c| (Target::Control(c), control_icon(c)))
        .collect();
    rows.extend(
        (0..touch.settings.custom_buttons.len())
            .map(|i| (Target::Custom(i), Icon::Sprite(KEYBOARD))),
    );
    rows.extend(
        Utility::ALL
            .into_iter()
            .map(|u| (Target::Utility(u), Icon::Pixel("terminal"))),
    );
    let shown = touch.editor_targets();
    for (target, icon) in rows {
        let r = [x, y, w - 52.0, 48.0];
        let selected = f.selected == Some(target);
        let visible = shown.contains(&target);
        if selected {
            kit::plate(k.ui, r, 1, col::CONTROL, col::CONTROL_RING);
        } else if k.pressed(r, Some(Act::Select(target))) {
            kit::stepped(k.ui, r, 1, 4.0, col::CONTROL);
        }
        let ink = if visible { col::INK } else { col::MUTED };
        kit::icon(
            k.ui,
            icon,
            x + 10.0,
            y + 12.0,
            24.0,
            if visible { col::ACCENT_HI } else { col::MUTED },
        );
        k.text_fit(
            Face::BodySemiBold,
            14.0,
            17.0,
            x + 44.0,
            y + 7.0,
            w - 52.0 - 52.0,
            ink,
            &name(touch, target),
        );
        k.text_fit(
            Face::Body,
            11.0,
            14.0,
            x + 44.0,
            y + 26.0,
            w - 52.0 - 52.0,
            col::MUTED,
            &status(touch, target),
        );
        k.hit(r, Act::Select(target));
        let can_toggle = match target {
            Target::Control(c) => f.layer == Layer::Game || c.menu_capable(),
            Target::Custom(_) => true,
            Target::Utility(u) => f.layer == Layer::Menus || u.in_game(),
        };
        icon_button(
            k,
            [x + w - 44.0, y + 4.0, 40.0, 40.0],
            Icon::Sprite(if visible { EYE } else { EYE_OFF }),
            Act::Eye(target),
            can_toggle,
            false,
        );
        y += 52.0;
    }
    y
}

fn keys(k: &mut Kit<Act>, slot: usize, raw: bool, x: f32, y: f32, w: f32) -> f32 {
    let mut y = y;
    let actions = t("keys_actions");
    let keyboard = t("keys_keyboard");
    k.segmented(
        [x, y, w, 40.0],
        &[
            (
                None,
                &actions,
                Act::Open(Panel::Keys { slot, raw: false }),
                !raw,
            ),
            (
                None,
                &keyboard,
                Act::Open(Panel::Keys { slot, raw: true }),
                raw,
            ),
        ],
        true,
    );
    y += 50.0;
    y += k.paragraph(
        Face::Body,
        12.0,
        15.0,
        x,
        y,
        w,
        usize::MAX,
        col::MUTED,
        &t("keys_physical"),
    ) + 10.0;
    let choices = key_choices(raw);
    if raw {
        // Keyboard keys: a grid of short chips.
        let cell = 64.0;
        let per_row = ((w + 6.0) / (cell + 6.0)).floor().max(1.0) as usize;
        let cw = (w - (per_row - 1) as f32 * 6.0) / per_row as f32;
        for (i, (_, label)) in choices.iter().enumerate() {
            let r = [
                x + (i % per_row) as f32 * (cw + 6.0),
                y + (i / per_row) as f32 * 46.0,
                cw,
                40.0,
            ];
            k.button(r, Style::Secondary, None, label, Some(Act::Pick(i)));
        }
        y + choices.len().div_ceil(per_row) as f32 * 46.0
    } else {
        for (i, (_, label)) in choices.iter().enumerate() {
            let r = [x, y, w, 40.0];
            let pressed = k.pressed(r, Some(Act::Pick(i)));
            kit::plate(
                k.ui,
                r,
                1,
                if pressed {
                    col::CONTROL_PRESSED
                } else {
                    col::CONTROL
                },
                col::CONTROL_RING,
            );
            k.text_fit(
                Face::BodySemiBold,
                14.0,
                17.0,
                x + 14.0,
                y + 11.5,
                w - 28.0,
                col::INK,
                label,
            );
            k.hit(r, Act::Pick(i));
            y += 46.0;
        }
        y
    }
}

/// Saved layouts: save the current one under a name, put one on the screen,
/// overwrite or delete it, and share one as a code through the clipboard.
fn layouts(k: &mut Kit<Act>, f: &Frame, x: f32, y: f32, w: f32) -> f32 {
    let mut y = section(k, x, y, &t("layouts"));
    if let Some(name) = &f.naming {
        k.field(
            [x, y, w, 44.0],
            Some("notes"),
            name,
            &t("layout_name"),
            Act::NameField,
            f.typing,
        );
        y += 52.0;
        let half = (w - 8.0) / 2.0;
        k.button(
            [x, y, half, 44.0],
            Style::Primary,
            Some(Icon::Pixel("check")),
            &t("save"),
            Some(Act::NameOk),
        );
        k.button(
            [x + half + 8.0, y, half, 44.0],
            Style::Secondary,
            None,
            &t("cancel"),
            Some(Act::NameCancel),
        );
        y += 56.0;
    } else {
        k.button(
            [x, y, w, 44.0],
            Style::Secondary,
            Some(Icon::Pixel("plus")),
            &t("save_layout"),
            Some(Act::SaveNew),
        );
        y += 52.0;
    }
    if f.profiles.is_empty() {
        y += k.paragraph(
            Face::Body,
            12.0,
            15.0,
            x,
            y,
            w,
            usize::MAX,
            col::MUTED,
            &t("layouts_empty"),
        ) + 10.0;
    }
    for (i, (name, customs)) in f.profiles.iter().enumerate() {
        let r = [x, y, w, 52.0];
        kit::plate(k.ui, r, 1, col::CONTROL, col::CONTROL_RING);
        let buttons = 3.0 * 40.0 + 2.0 * 6.0 + 6.0;
        let text_w = w - 14.0 - buttons - 8.0;
        k.text_fit(
            Face::BodySemiBold,
            14.0,
            17.0,
            x + 14.0,
            y + 8.0,
            text_w,
            col::INK,
            name,
        );
        let detail = t("layout_buttons").replace("%s", &customs.to_string());
        k.text_fit(
            Face::Body,
            11.0,
            14.0,
            x + 14.0,
            y + 28.0,
            text_w,
            col::MUTED,
            &detail,
        );
        let mut bx = x + w - 6.0 - 40.0;
        icon_button(
            k,
            [bx, y + 6.0, 40.0, 40.0],
            Icon::Pixel("trash"),
            Act::Open(Panel::DeleteProfile(i)),
            true,
            false,
        );
        bx -= 46.0;
        icon_button(
            k,
            [bx, y + 6.0, 40.0, 40.0],
            Icon::Pixel("download"),
            Act::UpdateProfile(i),
            true,
            false,
        );
        bx -= 46.0;
        icon_button(
            k,
            [bx, y + 6.0, 40.0, 40.0],
            Icon::Pixel("check"),
            Act::LoadProfile(i),
            true,
            true,
        );
        y += 58.0;
    }
    y += k.paragraph(
        Face::Body,
        12.0,
        15.0,
        x,
        y,
        w,
        usize::MAX,
        col::MUTED,
        &t("layouts_hint"),
    ) + 10.0;
    let half = (w - 8.0) / 2.0;
    k.button(
        [x, y, half, 44.0],
        Style::Secondary,
        Some(Icon::Sprite(kit::COPY)),
        &t("copy_code"),
        Some(Act::CopyCode),
    );
    k.button(
        [x + half + 8.0, y, half, 44.0],
        Style::Secondary,
        Some(Icon::Pixel("download")),
        &t("paste_code"),
        Some(Act::PasteCode),
    );
    y += 52.0;
    y += k.paragraph(
        Face::Body,
        12.0,
        15.0,
        x,
        y,
        w,
        usize::MAX,
        col::MUTED,
        &t("code_hint"),
    ) + 16.0;
    y
}

fn more(k: &mut Kit<Act>, touch: &TouchControls, f: &Frame, x: f32, y: f32, w: f32) -> f32 {
    let mut y = layouts(k, f, x, y, w);
    y = section(k, x, y, &t("editor_options"));
    let scheme = crate::lang::get("horizon.touch.editor.scheme").replace(
        "%s",
        &crate::lang::get(&format!(
            "horizon.touch.editor.scheme.{}",
            match touch.settings.mode {
                crate::touch::Mode::Crosshair => "crosshair",
                crate::touch::Mode::Tap => "tap",
                crate::touch::Mode::Dpad => "dpad",
            }
        )),
    );
    y += k.paragraph(
        Face::Body,
        12.0,
        15.0,
        x,
        y,
        w,
        usize::MAX,
        col::MUTED,
        &scheme,
    ) + 12.0;
    k.ui.text_line(Face::BodySemiBold, 14.0, 17.0, x, y, col::INK, &t("snap"));
    let used = k.paragraph(
        Face::Body,
        12.0,
        15.0,
        x,
        y + 20.0,
        w - 72.0,
        usize::MAX,
        col::MUTED,
        &t("snap_hint"),
    );
    k.switch(x + w - 60.0, y + 2.0, touch.settings.editor_snap, Act::Snap);
    y += (20.0 + used).max(40.0) + 16.0;
    y = slider(
        k,
        x,
        y,
        w,
        &t("scale"),
        &format!("{:.0} %", touch.settings.button_scale * 100.0),
        f.values[3],
        Slider::Scale,
    );
    y += 4.0;
    k.button(
        [x, y, w, 44.0],
        Style::Secondary,
        Some(Icon::Sprite(kit::COPY)),
        &t("copy_schemes"),
        Some(Act::CopyToSchemes),
    );
    y += 50.0;
    y += k.paragraph(
        Face::Body,
        12.0,
        15.0,
        x,
        y,
        w,
        usize::MAX,
        col::MUTED,
        &t("copy_schemes_hint"),
    ) + 14.0;
    k.button(
        [x, y, w, 44.0],
        Style::Danger,
        Some(Icon::Pixel("reload")),
        &t("reset"),
        Some(Act::Open(Panel::Reset)),
    );
    y + 56.0
}

fn confirm_reset(k: &mut Kit<Act>, w: f32, h: f32) {
    confirm(
        k,
        w,
        h,
        &t("reset_title"),
        &t("reset_text"),
        &t("reset_confirm"),
        Act::ResetLayout,
    );
}

/// A question over a scrim, with Cancel and a red confirming button.
#[allow(clippy::too_many_arguments)]
fn confirm(k: &mut Kit<Act>, w: f32, h: f32, title: &str, text: &str, yes: &str, act: Act) {
    kit::fill(k.ui, 0.0, 0.0, w, h, col::SCRIM);
    k.hit([0.0, 0.0, w, h], Act::Sheet);
    let dw = (w - 32.0).min(400.0);
    let title_lines = k.wrap_max(Face::BodyBold, 18.0, title, dw - 40.0, usize::MAX);
    let lines = k.wrap_max(Face::Body, 14.0, text, dw - 40.0, usize::MAX);
    let title_h = title_lines.len().max(1) as f32 * 24.0;
    let dh = 20.0 + title_h + 12.0 + lines.len() as f32 * 19.0 + 20.0 + 44.0 + 20.0;
    let (x, y) = ((w - dw) / 2.0, ((h - dh) / 2.0).max(8.0));
    k.panel([x, y, dw, dh], false);
    for (i, line) in title_lines.iter().enumerate() {
        k.ui.text_line(
            Face::BodyBold,
            18.0,
            24.0,
            x + 20.0,
            y + 20.0 + i as f32 * 24.0,
            col::INK,
            line,
        );
    }
    for (i, line) in lines.iter().enumerate() {
        k.ui.text_line(
            Face::Body,
            14.0,
            19.0,
            x + 20.0,
            y + 32.0 + title_h + i as f32 * 19.0,
            col::INK_SOFT,
            line,
        );
    }
    let by = y + dh - 64.0;
    let half = (dw - 48.0) / 2.0;
    k.button(
        [x + 20.0, by, half, 44.0],
        Style::Secondary,
        None,
        &t("cancel"),
        Some(Act::Close),
    );
    k.button(
        [x + 28.0 + half, by, half, 44.0],
        Style::Danger,
        None,
        yes,
        Some(act),
    );
}

// Icons the Pixelarticons subset lacks, on the same 12-cell grid as the kit's.
const UNDO: &[&str] = &[
    "............",
    "....#.......",
    "...##.......",
    "..#######...",
    "...##....#..",
    "....#.....#.",
    "..........#.",
    "..........#.",
    ".........#..",
    "...######...",
    "............",
    "............",
];
const REDO: &[&str] = &[
    "............",
    ".......#....",
    ".......##...",
    "...#######..",
    "..#....##...",
    ".#.....#....",
    ".#..........",
    ".#..........",
    "..#.........",
    "...######...",
    "............",
    "............",
];
const LIST: &[&str] = &[
    "............",
    "............",
    ".##.#######.",
    ".##.#######.",
    "............",
    ".##.#######.",
    ".##.#######.",
    "............",
    ".##.#######.",
    ".##.#######.",
    "............",
    "............",
];
const EYE: &[&str] = &[
    "............",
    "............",
    "....####....",
    "..##....##..",
    ".#...##...#.",
    "#...####...#",
    ".#...##...#.",
    "..##....##..",
    "....####....",
    "............",
    "............",
    "............",
];
const EYE_OFF: &[&str] = &[
    "..........#.",
    ".........#..",
    "....####.#..",
    "..##....#...",
    ".#...##.#.#.",
    "#...###....#",
    ".#.#.##...#.",
    "..#.....##..",
    ".#..####....",
    "#...........",
    "............",
    "............",
];
const UP: &[&str] = &[
    "............",
    "............",
    "............",
    ".....##.....",
    "....####....",
    "...##..##...",
    "..##....##..",
    ".##......##.",
    "............",
    "............",
    "............",
    "............",
];
const DOWN: &[&str] = &[
    "............",
    "............",
    "............",
    ".##......##.",
    "..##....##..",
    "...##..##...",
    "....####....",
    ".....##.....",
    "............",
    "............",
    "............",
    "............",
];
const RIGHT: &[&str] = &[
    "............",
    "............",
    "....##......",
    ".....##.....",
    "......##....",
    ".......##...",
    ".......##...",
    "......##....",
    ".....##.....",
    "....##......",
    "............",
    "............",
];
const MINUS: &[&str] = &[
    "............",
    "............",
    "............",
    "............",
    "............",
    "..########..",
    "..########..",
    "............",
    "............",
    "............",
    "............",
    "............",
];
const PAUSE: &[&str] = &[
    "............",
    "............",
    "...##..##...",
    "...##..##...",
    "...##..##...",
    "...##..##...",
    "...##..##...",
    "...##..##...",
    "...##..##...",
    "...##..##...",
    "............",
    "............",
];
const SWAP: &[&str] = &[
    "............",
    "...#........",
    "..##........",
    ".#########..",
    "..##........",
    "...#....#...",
    "........##..",
    "..#########.",
    "........##..",
    "........#...",
    "............",
    "............",
];
const SHIELD: &[&str] = &[
    "............",
    ".##########.",
    ".#........#.",
    ".#........#.",
    ".#........#.",
    ".#........#.",
    "..#......#..",
    "..#......#..",
    "...#....#...",
    "....#..#....",
    ".....##.....",
    "............",
];
const SWORD: &[&str] = &[
    ".........###",
    "........#..#",
    ".......#..#.",
    "......#..#..",
    ".....#..#...",
    "....#..#....",
    ".#.#..#.....",
    "..#..#......",
    "..##........",
    ".#..#.......",
    "#...........",
    "............",
];
const KEYBOARD: &[&str] = &[
    "............",
    "............",
    "############",
    "#..........#",
    "#.#.#.#.#..#",
    "#..........#",
    "#.#.#.#.#..#",
    "#..........#",
    "#..######..#",
    "#..........#",
    "############",
    "............",
];
