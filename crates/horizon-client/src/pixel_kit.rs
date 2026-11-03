//! The «Горизонт» look shared by the Android launcher and the in-game touch layout
//! editor: a night-sky palette and pixel-stepped plates, drawn only from solid
//! quads, atlas icons and text so it looks the same on Vulkan and on the GLES
//! fallback. Every size is in dp.
use crate::ui_theme::{Face, Ui, color::hex};

/// The palette (HUD colour encoding).
pub mod col {
    use super::hex;
    pub const PAGE: u32 = hex(0x0e0b26, 255);
    pub const SIDE: u32 = hex(0x0a0820, 255);
    pub const RAIL_EDGE: u32 = hex(0x1d1840, 255);
    pub const PLATE: u32 = hex(0x15112e, 255);
    /// Panels over Home's sky let it show through a little.
    pub const PLATE_HOME: u32 = hex(0x15112e, 224);
    pub const PLATE_RING: u32 = hex(0x2c2652, 255);
    pub const CONTROL: u32 = hex(0x231c4a, 255);
    pub const CONTROL_RING: u32 = hex(0x3a3070, 255);
    pub const CONTROL_PRESSED: u32 = hex(0x2e2560, 255);
    pub const RULE: u32 = hex(0x221d42, 255);
    pub const SWITCH_OFF: u32 = hex(0x2b2641, 255);
    pub const INK: u32 = hex(0xf4f2ff, 255);
    pub const INK_SOFT: u32 = hex(0xcfc9ea, 255);
    pub const MUTED: u32 = hex(0xa59fc6, 255);
    pub const PLACEHOLDER: u32 = hex(0x8a84ad, 255);
    pub const WHITE: u32 = hex(0xffffff, 255);
    pub const ACCENT: u32 = hex(0x7656fa, 255);
    pub const ACCENT_HI: u32 = hex(0xa08cff, 255);
    pub const ACCENT_TOP: u32 = hex(0x9a86ff, 255);
    pub const LIP: u32 = hex(0x3a2694, 255);
    pub const ACCENT_PRESSED: u32 = hex(0x6a4cf0, 255);
    pub const SUCCESS: u32 = hex(0x5fd39a, 255);
    pub const DANGER: u32 = hex(0xff8a78, 255);
    pub const DANGER_RING: u32 = hex(0x6b3050, 255);
    pub const SCRIM: u32 = hex(0x05040f, 184);
}

/// A solid rectangle.
pub fn fill(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, color: u32) {
    if w > 0.0 && h > 0.0 {
        ui.rounded(x, y, w, h, 0.0, color);
    }
}

/// A rectangle with pixel-stepped corners: `steps` stairs of `s` dp (C1-4, C2-4, C1-2).
pub fn stepped(ui: &mut Ui, [x, y, w, h]: [f32; 4], steps: u8, s: f32, color: u32) {
    if steps >= 2 {
        fill(ui, x + 2.0 * s, y, w - 4.0 * s, h, color);
        fill(ui, x + s, y + s, s, h - 2.0 * s, color);
        fill(ui, x + w - 2.0 * s, y + s, s, h - 2.0 * s, color);
        fill(ui, x, y + 2.0 * s, s, h - 4.0 * s, color);
        fill(ui, x + w - s, y + 2.0 * s, s, h - 4.0 * s, color);
    } else {
        fill(ui, x + s, y, w - 2.0 * s, h, color);
        fill(ui, x, y + s, s, h - 2.0 * s, color);
        fill(ui, x + w - s, y + s, s, h - 2.0 * s, color);
    }
}

/// The 2 dp ring that follows one (R1-4-2) or two (R2-4-2) 4 dp steps.
pub fn ring(ui: &mut Ui, [x, y, w, h]: [f32; 4], steps: u8, color: u32) {
    let mut q = |dx: f32, dy: f32, qw: f32, qh: f32| fill(ui, x + dx, y + dy, qw, qh, color);
    if steps >= 2 {
        q(8.0, 0.0, w - 16.0, 2.0);
        q(8.0, h - 2.0, w - 16.0, 2.0);
        q(0.0, 8.0, 2.0, h - 16.0);
        q(w - 2.0, 8.0, 2.0, h - 16.0);
        for (dx, dy, qw, qh) in [
            (8.0, 2.0, 2.0, 2.0),
            (4.0, 4.0, 6.0, 2.0),
            (4.0, 6.0, 2.0, 2.0),
            (2.0, 8.0, 4.0, 2.0),
        ] {
            q(dx, dy, qw, qh);
            q(w - dx - qw, dy, qw, qh);
            q(dx, h - dy - qh, qw, qh);
            q(w - dx - qw, h - dy - qh, qw, qh);
        }
    } else {
        q(4.0, 0.0, w - 8.0, 2.0);
        q(4.0, h - 2.0, w - 8.0, 2.0);
        q(0.0, 4.0, 2.0, h - 8.0);
        q(w - 2.0, 4.0, 2.0, h - 8.0);
        for (dx, dy, qw, qh) in [(4.0, 2.0, 2.0, 2.0), (2.0, 4.0, 4.0, 2.0)] {
            q(dx, dy, qw, qh);
            q(w - dx - qw, dy, qw, qh);
            q(dx, h - dy - qh, qw, qh);
            q(w - dx - qw, h - dy - qh, qw, qh);
        }
    }
}

/// A square 2 dp frame (code boxes, radio boxes).
pub fn frame(ui: &mut Ui, [x, y, w, h]: [f32; 4], color: u32) {
    fill(ui, x, y, w, 2.0, color);
    fill(ui, x, y + h - 2.0, w, 2.0, color);
    fill(ui, x, y + 2.0, 2.0, h - 4.0, color);
    fill(ui, x + w - 2.0, y + 2.0, 2.0, h - 4.0, color);
}

/// A stepped plate with its ring.
pub fn plate(ui: &mut Ui, r: [f32; 4], steps: u8, body: u32, edge: u32) {
    stepped(ui, r, steps, 4.0, body);
    ring(ui, r, steps, edge);
}

/// The D-pad focus ring, 2 dp outside the control.
pub fn focus(ui: &mut Ui, [x, y, w, h]: [f32; 4], steps: u8) {
    ring(
        ui,
        [x - 4.0, y - 4.0, w + 8.0, h + 8.0],
        steps,
        col::ACCENT_HI,
    );
}

/// A pixel sprite: rows of characters, `cell` dp each, coloured by `paint`
/// (`None` = transparent). Runs of one colour are merged into a single quad.
pub fn sprite(
    ui: &mut Ui,
    x: f32,
    y: f32,
    cell: f32,
    rows: &[&str],
    paint: impl Fn(char) -> Option<u32>,
) {
    for (row, line) in rows.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut start = 0;
        while start < chars.len() {
            let mut end = start + 1;
            while end < chars.len() && chars[end] == chars[start] {
                end += 1;
            }
            if let Some(color) = paint(chars[start]) {
                fill(
                    ui,
                    x + start as f32 * cell,
                    y + row as f32 * cell,
                    (end - start) as f32 * cell,
                    cell,
                    color,
                );
            }
            start = end;
        }
    }
}

/// Icons the Pixelarticons set lacks, on a 12-cell grid.
pub const ALERT: &[&str] = &[
    ".....##.....",
    ".....##.....",
    "....#..#....",
    "....#..#....",
    "...#.##.#...",
    "...#.##.#...",
    "..#..##..#..",
    "..#......#..",
    ".#...##...#.",
    ".#........#.",
    "############",
    "............",
];
pub const COPY: &[&str] = &[
    "............",
    ".######.....",
    ".#....#.....",
    ".#..######..",
    ".#..#....#..",
    ".#..#....#..",
    ".####....#..",
    "....#....#..",
    "....#....#..",
    "....######..",
    "............",
    "............",
];
pub const CHEVRON_LEFT: &[&str] = &[
    "............",
    "............",
    "......##....",
    ".....##.....",
    "....##......",
    "...##.......",
    "...##.......",
    "....##......",
    ".....##.....",
    "......##....",
    "............",
    "............",
];
/// The rail's brand mark, 16 cells of 2 dp.
pub const BRAND: &[&str] = &[
    "................",
    "................",
    "................",
    "....FF....FF....",
    "....FF....FF....",
    "....FF....FF....",
    "....FF....FF....",
    ".AAAAAAAAAAAAAA.",
    "....ff....ff....",
    "....ff....ff....",
    "....ff....ff....",
    "....ff....ff....",
];
/// A stand-in player head, 8 cells.
pub const HEAD: &[&str] = &[
    "PPPPPPPP", "PPPPPPPP", "pppppppp", "SSSSSSSS", "SKKSSKKS", "SSSSSSSS", "SSSmmSSS", "sSSSSSSs",
];

pub fn brand(ui: &mut Ui, x: f32, y: f32) {
    sprite(ui, x, y, 2.0, BRAND, |c| match c {
        'F' => Some(col::INK),
        'f' => Some(hex(0xf4f2ff, 92)),
        'A' => Some(col::ACCENT_HI),
        _ => None,
    });
}
pub fn head(ui: &mut Ui, x: f32, y: f32, size: f32) {
    sprite(ui, x, y, size / 8.0, HEAD, |c| {
        Some(match c {
            'P' => hex(0x5b3fd6, 255),
            'p' => hex(0x8a74ff, 255),
            'S' => hex(0xe3b48c, 255),
            's' => hex(0xc99a74, 255),
            'K' => hex(0x1b1530, 255),
            'm' => hex(0x9c5a4a, 255),
            _ => return None,
        })
    });
}

/// An icon by Pixelarticons name, or one of the custom sprites above.
#[derive(Clone, Copy)]
pub enum Icon {
    Pixel(&'static str),
    Sprite(&'static [&'static str]),
}
pub fn icon(ui: &mut Ui, icon: Icon, x: f32, y: f32, size: f32, color: u32) {
    match icon {
        Icon::Pixel(name) => ui.icon(name, x, y, size, color),
        Icon::Sprite(rows) => sprite(ui, x, y, size / rows.len() as f32, rows, |c| {
            (c == '#').then_some(color)
        }),
    }
}

/// How a button looks.
#[derive(Clone, Copy, PartialEq)]
pub enum Style {
    Primary,
    /// The big «Играть»: 56 tall, a pixel-font label.
    PrimaryXl,
    Secondary,
    Danger,
    Ghost,
    Disabled,
}

/// One frame of launcher drawing: the `Ui`, the frame's tap targets, and the
/// finger that is down (pressed controls draw their pressed state).
pub struct Kit<'k, 'a, A> {
    pub ui: &'k mut Ui<'a>,
    pub targets: &'k mut Vec<([f32; 4], A)>,
    pub press: Option<[f32; 2]>,
    /// A control just released: it keeps its pressed look for a moment.
    pub flash: Option<A>,
}

impl<A: Copy + PartialEq> Kit<'_, '_, A> {
    pub fn pressed(&self, [x, y, w, h]: [f32; 4], action: Option<A>) -> bool {
        self.press
            .is_some_and(|[px, py]| px >= x && px <= x + w && py >= y && py <= y + h)
            || (action.is_some() && self.flash == action)
    }
    pub fn hit(&mut self, r: [f32; 4], action: A) {
        self.targets.push((r, action));
    }
    /// Where the vertices and tap targets drawn from now on begin, for [`Self::clip`].
    pub fn mark(&self) -> (usize, usize) {
        (self.ui.batch.vertices.len(), self.targets.len())
    }
    /// Cut everything drawn since `mark` to `r` (a scrolling list's viewport): rows
    /// half out of view show their visible half instead of vanishing, and their
    /// tap targets shrink to it.
    pub fn clip(&mut self, (vertices, targets): (usize, usize), [x, y, w, h]: [f32; 4]) {
        let (d, s) = (self.ui.density, self.ui.batch.scale);
        let edge = |v: f32| (v * d).round() / s;
        self.ui
            .batch
            .clip_since(vertices, edge(x), edge(y), edge(x + w), edge(y + h));
        let mut kept = Vec::new();
        for ([tx, ty, tw, th], action) in self.targets.drain(targets..) {
            let (x0, y0) = (tx.max(x), ty.max(y));
            let (x1, y1) = ((tx + tw).min(x + w), (ty + th).min(y + h));
            if x1 > x0 && y1 > y0 {
                kept.push(([x0, y0, x1 - x0, y1 - y0], action));
            }
        }
        self.targets.extend(kept);
    }
    /// Text in a `line`-tall box whose top is `y`; returns its width.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        face: Face,
        size: f32,
        line: f32,
        x: f32,
        y: f32,
        color: u32,
        text: &str,
    ) -> f32 {
        self.ui.text_line(face, size, line, x, y, color, text)
    }
    /// Text cut with an ellipsis to `width`.
    #[allow(clippy::too_many_arguments)]
    pub fn text_fit(
        &mut self,
        face: Face,
        size: f32,
        line: f32,
        x: f32,
        y: f32,
        width: f32,
        color: u32,
        text: &str,
    ) -> f32 {
        let text = self.ui.fit_text(face, size, text, width.max(8.0));
        self.ui.text_line(face, size, line, x, y, color, &text)
    }
    /// UPPERCASE text with extra letter spacing (section labels, the status line).
    #[allow(clippy::too_many_arguments)]
    pub fn spaced(
        &mut self,
        face: Face,
        size: f32,
        line: f32,
        mut x: f32,
        y: f32,
        spacing: f32,
        color: u32,
        text: &str,
    ) -> f32 {
        let start = x;
        let mut buffer = [0u8; 4];
        for ch in text.chars().flat_map(char::to_uppercase) {
            let s = ch.encode_utf8(&mut buffer);
            x += self.ui.text_line(face, size, line, x, y, color, s) + spacing;
        }
        x - start - spacing
    }
    pub fn measure(&self, face: Face, size: f32, text: &str) -> f32 {
        self.ui.measure(face, size, text)
    }
    /// Greedy word wrap to `width`.
    pub fn wrap(&self, face: Face, size: f32, text: &str, width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        for paragraph in text.split('\n') {
            let mut line = String::new();
            for word in paragraph.split_whitespace() {
                let candidate = if line.is_empty() {
                    word.to_owned()
                } else {
                    format!("{line} {word}")
                };
                if !line.is_empty() && self.ui.measure(face, size, &candidate) > width {
                    lines.push(std::mem::replace(&mut line, word.to_owned()));
                } else {
                    line = candidate;
                }
            }
            lines.push(line);
        }
        lines
    }
    /// Word-wrapped lines, at most `max_lines`; a longer text ends in an ellipsis
    /// on the last line instead of stopping mid-sentence.
    pub fn wrap_max(
        &self,
        face: Face,
        size: f32,
        text: &str,
        width: f32,
        max_lines: usize,
    ) -> Vec<String> {
        let mut lines: Vec<String> = self
            .wrap(face, size, text, width)
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect();
        if lines.len() > max_lines {
            lines.truncate(max_lines);
            if let Some(last) = lines.last_mut() {
                *last = self.ui.fit_text(face, size, &format!("{last}…"), width);
            }
        }
        lines
    }
    /// Wrapped text, at most `max_lines`; returns the height used.
    #[allow(clippy::too_many_arguments)]
    pub fn paragraph(
        &mut self,
        face: Face,
        size: f32,
        line: f32,
        x: f32,
        y: f32,
        width: f32,
        max_lines: usize,
        color: u32,
        text: &str,
    ) -> f32 {
        let mut lines = self.wrap(face, size, text, width);
        if lines.len() > max_lines {
            lines.truncate(max_lines);
            if let Some(last) = lines.last_mut() {
                *last = self.ui.fit_text(face, size, &format!("{last}…"), width);
            }
        }
        for (i, text) in lines.iter().enumerate() {
            self.ui
                .text_line(face, size, line, x, y + i as f32 * line, color, text);
        }
        lines.len() as f32 * line
    }

    /// The width a button needs for `label` (and an icon).
    pub fn button_width(&self, style: Style, icon: Option<Icon>, label: &str) -> f32 {
        let (face, size, pad) = if style == Style::PrimaryXl {
            (Face::Pixel, 32.0, 28.0)
        } else {
            (Face::BodySemiBold, 15.0, 16.0)
        };
        if label.is_empty() {
            return 48.0;
        }
        let text = self.ui.measure(face, size, label);
        let icon = if icon.is_some() { 32.0 } else { 0.0 };
        let min = if style == Style::PrimaryXl {
            176.0
        } else {
            48.0
        };
        (2.0 * pad + icon + text).max(min)
    }
    /// A button; `action` None draws it without a target (a disabled look).
    pub fn button(
        &mut self,
        r: [f32; 4],
        style: Style,
        icon: Option<Icon>,
        label: &str,
        action: Option<A>,
    ) {
        let [x, y, w, h] = r;
        let pressed = action.is_some() && self.pressed(r, action);
        let mut offset = 0.0;
        let (text_color, icon_color) = match style {
            Style::Primary | Style::PrimaryXl => {
                if pressed {
                    let b = [x, y + 2.0, w, h - 2.0];
                    stepped(self.ui, b, 1, 4.0, col::ACCENT_PRESSED);
                    fill(self.ui, x + 4.0, y + 2.0, w - 8.0, 2.0, col::ACCENT_TOP);
                    offset = 2.0;
                } else {
                    stepped(self.ui, r, 1, 4.0, col::ACCENT);
                    fill(self.ui, x + 4.0, y, w - 8.0, 2.0, col::ACCENT_TOP);
                    fill(self.ui, x + 4.0, y + h - 4.0, w - 8.0, 4.0, col::LIP);
                    offset = -1.0;
                }
                (col::WHITE, col::WHITE)
            }
            Style::Secondary | Style::Danger => {
                stepped(
                    self.ui,
                    r,
                    1,
                    4.0,
                    if pressed {
                        col::CONTROL_PRESSED
                    } else {
                        col::CONTROL
                    },
                );
                if style == Style::Danger {
                    ring(self.ui, r, 1, col::DANGER_RING);
                    (col::DANGER, col::DANGER)
                } else {
                    ring(self.ui, r, 1, col::CONTROL_RING);
                    (col::INK, col::INK)
                }
            }
            Style::Ghost => {
                if pressed {
                    stepped(self.ui, r, 1, 4.0, col::CONTROL);
                }
                (col::ACCENT_HI, col::ACCENT_HI)
            }
            Style::Disabled => {
                plate(self.ui, r, 1, col::PLATE, col::CONTROL_RING);
                (col::MUTED, col::MUTED)
            }
        };
        let (face, size, line) = if style == Style::PrimaryXl {
            (Face::Pixel, 32.0, 32.0)
        } else {
            (Face::BodySemiBold, 15.0, 18.0)
        };
        let text_w = if label.is_empty() {
            0.0
        } else {
            self.ui.measure(face, size, label)
        };
        let pad = if style == Style::PrimaryXl {
            28.0
        } else {
            16.0
        };
        let icon_w = if icon.is_some() {
            if label.is_empty() { 24.0 } else { 32.0 }
        } else {
            0.0
        };
        let label = if text_w + icon_w > w - 2.0 * pad + 0.5 && !label.is_empty() {
            self.ui
                .fit_text(face, size, label, (w - 2.0 * pad - icon_w).max(8.0))
        } else {
            label.to_owned()
        };
        let content = icon_w
            + if label.is_empty() {
                0.0
            } else {
                self.ui.measure(face, size, &label)
            };
        let mut cx = x + (w - content) / 2.0;
        let cy = y + h / 2.0 + offset;
        if let Some(i) = icon {
            icon_draw(self.ui, i, cx, cy - 12.0, icon_color);
            cx += icon_w;
        }
        if !label.is_empty() {
            self.ui
                .text_line(face, size, line, cx, cy - line / 2.0, text_color, &label);
        }
        if let Some(action) = action {
            self.hit(r, action);
        }
    }

    /// A panel: the plate cards and dialogs use.
    pub fn panel(&mut self, r: [f32; 4], over_sky: bool) {
        plate(
            self.ui,
            r,
            2,
            if over_sky {
                col::PLATE_HOME
            } else {
                col::PLATE
            },
            col::PLATE_RING,
        );
    }

    /// A navigation item (rail or bottom bar).
    pub fn nav(
        &mut self,
        r: [f32; 4],
        icon_name: &'static str,
        label: &str,
        selected: bool,
        action: A,
    ) {
        let [x, y, w, h] = r;
        if selected {
            plate(self.ui, r, 2, col::CONTROL, col::CONTROL_RING);
        } else if self.pressed(r, Some(action)) {
            stepped(self.ui, r, 2, 4.0, col::CONTROL);
        }
        let content = 24.0 + 3.0 + 12.1;
        let top = y + (h - content) / 2.0;
        self.ui.icon(
            icon_name,
            x + (w - 24.0) / 2.0,
            top,
            24.0,
            if selected { col::ACCENT_HI } else { col::MUTED },
        );
        // A long label («Дополнения») takes a smaller size before it is cut.
        let size = [11.0, 10.0, 9.0]
            .into_iter()
            .find(|&s| self.ui.measure(Face::BodySemiBold, s, label) <= w - 2.0)
            .unwrap_or(9.0);
        let label = self.ui.fit_text(Face::BodySemiBold, size, label, w - 2.0);
        let lw = self.ui.measure(Face::BodySemiBold, size, &label);
        self.ui.text_line(
            Face::BodySemiBold,
            size,
            12.1,
            x + (w - lw) / 2.0,
            top + 27.0,
            if selected { col::INK } else { col::MUTED },
            &label,
        );
        self.hit(r, action);
    }

    /// A dropdown row: an optional key above the value, and a chevron.
    pub fn dropdown(&mut self, r: [f32; 4], key: Option<&str>, value: &str, action: A) {
        let [x, y, w, h] = r;
        let pressed = self.pressed(r, Some(action));
        plate(
            self.ui,
            r,
            1,
            if pressed {
                col::CONTROL_PRESSED
            } else {
                col::CONTROL
            },
            col::CONTROL_RING,
        );
        let text_w = w - 14.0 - 12.0 - 24.0 - 8.0;
        // A long value takes a second line, in a smaller size if it must, rather
        // than ending in an ellipsis.
        let lines_at = |size: f32| -> Vec<String> {
            self.wrap(Face::BodySemiBold, size, value, text_w)
                .into_iter()
                .filter(|l| !l.is_empty())
                .collect()
        };
        let mut size = 15.0;
        let mut lines = lines_at(size);
        if lines.len() > 1 {
            size = 13.0;
            lines = self.wrap_max(Face::BodySemiBold, size, value, text_w, 2);
        }
        let line = (size * 1.15_f32).round();
        let value_h = lines.len().max(1) as f32 * line;
        let key_h = if key.is_some() { 14.0 + 4.0 } else { 0.0 };
        let mut ty = y + (h - key_h - value_h) / 2.0;
        if let Some(key) = key {
            self.text_fit(
                Face::BodyMedium,
                12.0,
                14.0,
                x + 14.0,
                ty,
                text_w,
                col::MUTED,
                key,
            );
            ty += key_h;
        }
        for text in &lines {
            self.ui
                .text_line(Face::BodySemiBold, size, line, x + 14.0, ty, col::INK, text);
            ty += line;
        }
        self.ui.icon(
            "chevron-down",
            x + w - 12.0 - 24.0,
            y + (h - 24.0) / 2.0,
            24.0,
            col::ACCENT_HI,
        );
        self.hit(r, action);
    }

    /// A 60 × 36 switch at (x, y).
    pub fn switch(&mut self, x: f32, y: f32, on: bool, action: A) {
        let r = [x, y, 60.0, 36.0];
        stepped(
            self.ui,
            r,
            1,
            4.0,
            if on { col::ACCENT } else { col::SWITCH_OFF },
        );
        stepped(
            self.ui,
            [x + if on { 28.0 } else { 4.0 }, y + 4.0, 28.0, 28.0],
            1,
            2.0,
            if on { col::WHITE } else { col::MUTED },
        );
        self.hit(r, action);
    }

    /// A text field (a sunk well) showing `value`, or `placeholder` when empty.
    #[allow(clippy::too_many_arguments)]
    pub fn field(
        &mut self,
        r: [f32; 4],
        icon_name: Option<&'static str>,
        value: &str,
        placeholder: &str,
        action: A,
        editing: bool,
    ) -> f32 {
        let [x, y, w, h] = r;
        plate(self.ui, r, 1, col::SIDE, col::PLATE_RING);
        let mut tx = x + 12.0;
        if let Some(name) = icon_name {
            self.ui
                .icon(name, tx, y + (h - 24.0) / 2.0, 24.0, col::MUTED);
            tx += 32.0;
        }
        let width = x + w - 12.0 - tx;
        let (text, color) = if value.is_empty() {
            (placeholder, col::PLACEHOLDER)
        } else {
            (value, col::INK)
        };
        // While typing, show the end of a long value: the caret is there.
        let shown = if editing && self.ui.measure(Face::BodyMedium, 14.0, text) > width {
            let chars: Vec<char> = text.chars().collect();
            let mut start = 0;
            while start < chars.len()
                && self.ui.measure(
                    Face::BodyMedium,
                    14.0,
                    &chars[start..].iter().collect::<String>(),
                ) > width
            {
                start += 1;
            }
            chars[start..].iter().collect()
        } else {
            self.ui.fit_text(Face::BodyMedium, 14.0, text, width)
        };
        let advance = self.ui.text_line(
            Face::BodyMedium,
            14.0,
            20.3,
            tx,
            y + (h - 20.3) / 2.0,
            color,
            &shown,
        );
        if editing {
            focus(self.ui, r, 1);
        }
        self.hit(r, action);
        if value.is_empty() { tx } else { tx + advance }
    }

    /// A segmented control. `items`: (icon, label, action, selected). Items share
    /// `r`'s width when `stretch`, else take their natural width from the left.
    pub fn segmented(
        &mut self,
        r: [f32; 4],
        items: &[(Option<&'static str>, &str, A, bool)],
        stretch: bool,
    ) {
        let [x, y, w, h] = r;
        plate(self.ui, r, 1, col::SIDE, col::PLATE_RING);
        let natural: Vec<f32> = items
            .iter()
            .map(|(icon, label, _, _)| {
                28.0 + if icon.is_some() { 30.0 } else { 0.0 }
                    + self.ui.measure(Face::BodySemiBold, 14.0, label)
            })
            .collect();
        let total: f32 = natural.iter().sum();
        let mut ix = x;
        for (i, (icon_name, label, action, selected)) in items.iter().enumerate() {
            let iw = if stretch || total > w {
                w / items.len() as f32
            } else {
                natural[i]
            };
            let ir = [ix, y, iw, h];
            let color = if *selected { col::PLATE } else { col::INK_SOFT };
            if *selected {
                stepped(self.ui, ir, 1, 4.0, col::INK);
            } else if self.pressed(ir, Some(*action)) {
                stepped(self.ui, ir, 1, 4.0, col::CONTROL);
            }
            let show_icon = icon_name.is_some() && !(stretch || total > w);
            let label = self.ui.fit_text(
                Face::BodySemiBold,
                14.0,
                label,
                iw - 12.0 - if show_icon { 30.0 } else { 0.0 },
            );
            let content = self.ui.measure(Face::BodySemiBold, 14.0, &label)
                + if show_icon { 30.0 } else { 0.0 };
            let mut cx = ix + (iw - content) / 2.0;
            if show_icon && let Some(name) = icon_name {
                self.ui.icon(name, cx, y + (h - 24.0) / 2.0, 24.0, color);
                cx += 30.0;
            }
            self.ui.text_line(
                Face::BodySemiBold,
                14.0,
                17.0,
                cx,
                y + (h - 17.0) / 2.0,
                color,
                &label,
            );
            self.hit(ir, *action);
            ix += iw;
        }
    }

    /// «ПОСЛЕДНИЙ МИР»-style section label.
    pub fn section_label(&mut self, x: f32, y: f32, text: &str) {
        self.spaced(Face::BodyBold, 12.0, 14.0, x, y, 0.72, col::MUTED, text);
    }
    /// A square dot and an uppercase mono line.
    pub fn status_line(&mut self, x: f32, y: f32, dot: u32, text: &str) {
        fill(self.ui, x, y + 3.0, 8.0, 8.0, dot);
        self.spaced(
            Face::Mono,
            11.0,
            14.0,
            x + 16.0,
            y,
            0.66,
            col::INK_SOFT,
            text,
        );
    }
    /// The account chip, right-aligned at `right`; returns its width.
    #[allow(clippy::too_many_arguments)]
    pub fn chip(
        &mut self,
        right: f32,
        y: f32,
        name: &str,
        status: &str,
        sign_in: &str,
        action: A,
    ) -> f32 {
        let name_w = self.ui.measure(Face::BodyBold, 15.0, name);
        let status_w = self.ui.measure(Face::Body, 11.0, status);
        let sign_w = if sign_in.is_empty() {
            0.0
        } else {
            self.ui.measure(Face::BodySemiBold, 15.0, sign_in) + 14.0
        };
        let w = 8.0 + 32.0 + 10.0 + name_w.max(status_w) + 12.0 + sign_w;
        let x = right - w;
        let r = [x, y, w, 48.0];
        let pressed = self.pressed(r, Some(action));
        plate(
            self.ui,
            r,
            1,
            if pressed {
                col::CONTROL_PRESSED
            } else {
                col::CONTROL
            },
            col::CONTROL_RING,
        );
        head(self.ui, x + 8.0, y + 8.0, 32.0);
        self.ui.text_line(
            Face::BodyBold,
            15.0,
            16.5,
            x + 50.0,
            y + 9.3,
            col::INK,
            name,
        );
        self.ui.text_line(
            Face::Body,
            11.0,
            13.0,
            x + 50.0,
            y + 25.8,
            col::MUTED,
            status,
        );
        if !sign_in.is_empty() {
            self.ui.text_line(
                Face::BodySemiBold,
                15.0,
                18.0,
                x + w - 12.0 - sign_w + 14.0,
                y + 15.0,
                col::ACCENT_HI,
                sign_in,
            );
        }
        self.hit(r, action);
        w
    }
}

fn icon_draw(ui: &mut Ui, i: Icon, x: f32, y: f32, color: u32) {
    icon(ui, i, x, y, 24.0, color);
}
