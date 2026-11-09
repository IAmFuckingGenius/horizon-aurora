//! The design system's widgets drawn in the game (`crates/horizon-theme`:
//! palette, type scale, radii, control sizes, motion and the bundled fonts;
//! `docs/design-system.md`). Every menu is vanilla's (`ui`); what is left
//! here draws Horizon's own overlays that vanilla has no screen for: the
//! on-screen keyboard a controller types with (`osk`), plus the
//! `horizon.txt` preferences.
//!
//! Build a [`Ui`] over the frame's `HudBatch` with [`Ui::new`], lay out in dp
//! (the launcher's logical pixels) and give every interactive widget a stable
//! `id`. Clicks run the same layout with [`Ui::dry`] and read [`Ui::hit`]: one
//! function draws and hit-tests, so they never disagree.
use crate::hud::{HudAtlas, HudBatch, Rect};
use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};
use std::{cell::RefCell, collections::HashMap, path::Path};

/// A Horizon-only preference from `horizon.txt` beside `options.txt`
/// (`key:value` lines), so vanilla's options file stays exactly as vanilla
/// writes it.
pub fn preference(dir: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("horizon.txt")).ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == key).then(|| v.trim().to_owned())
    })
}
/// Set (or with None remove) one preference, keeping the others.
pub(crate) fn set_preference(dir: &Path, key: &str, value: Option<&str>) {
    let text = std::fs::read_to_string(dir.join("horizon.txt")).unwrap_or_default();
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| l.split_once(':').is_none_or(|(k, _)| k.trim() != key))
        .map(str::to_owned)
        .collect();
    if let Some(v) = value {
        lines.push(format!("{key}:{v}"));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    let _ = std::fs::write(dir.join("horizon.txt"), out);
}

/// The dark palette (`horizon_theme::DARK`) in the HUD's colour encoding, plus the
/// translucent plates the overlays draw over the game.
pub mod color {
    use horizon_theme::DARK as P;
    pub const fn hex(c: u32, a: u8) -> u32 {
        (a as u32) << 24 | (c & 0xff) << 16 | (c >> 8 & 0xff) << 8 | (c >> 16 & 0xff)
    }
    /// The page, under every veil.
    pub const BG: u32 = hex(P.page, 255);
    /// Hover plates, secondary buttons, fields, the segmented track: the
    /// dark palette's soft plate, as ink over the veil so the plates read the
    /// same over any backdrop (over the page colour they are `plate_soft`).
    pub const RAISED: u32 = hex(P.ink, 20);
    /// Pressed plates and a switch that is off (`plate`, the same way).
    pub const HOVER: u32 = hex(P.ink, 36);
    /// One-pixel rules between rows, slider tracks.
    pub const BORDER: u32 = hex(P.rule, 255);
    /// Text and selection (`ink`).
    pub const TEXT: u32 = hex(P.ink, 255);
    /// Values and secondary text (`ink_soft`).
    pub const MUTED: u32 = hex(P.ink_soft, 255);
    pub const ACCENT: u32 = hex(P.accent, 255);
    pub const ON_ACCENT: u32 = hex(P.on_accent, 255);
    pub(crate) fn with_alpha(c: u32, a: f32) -> u32 {
        let base = (c >> 24) as f32;
        (c & 0x00ff_ffff) | ((base * a.clamp(0.0, 1.0)).round() as u32) << 24
    }
}

/// The launcher's type scale and shapes (`horizon_theme`), in dp.
pub(crate) const SMALL: f32 = horizon_theme::CAPTION.px;
pub(crate) const BODY: f32 = horizon_theme::LABEL.px;
/// The boot screen's wordmark (`DISPLAY`) and console lines (`MICRO`).
pub(crate) const DISPLAY: f32 = horizon_theme::DISPLAY.px;
pub(crate) const MICRO: f32 = horizon_theme::MICRO.px;
/// Fields; plates (hover rows, selection); pills (buttons, segments).
pub(crate) const RADIUS: f32 = horizon_theme::RADIUS;
pub(crate) const RADIUS_PLATE: f32 = horizon_theme::RADIUS_PLATE;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Regular,
    Medium,
    /// The wordmark and page titles (the design system's 500).
    SemiBold,
    /// Counts, timestamps and the boot screen's console lines.
    Mono,
    /// The Android launcher's pixel text (GNU Unifont): Latin and Cyrillic, drawn at
    /// whole multiples of its 16 px grid so every texel stays square.
    Pixel,
    /// The Android launcher's Latin wordmark (Pixelify Sans Bold).
    PixelDisplay,
    /// The Android launcher's body text (Bricolage Grotesque).
    Body,
    BodyMedium,
    BodySemiBold,
    BodyBold,
}
struct Fonts {
    faces: [FontRef<'static>; 10],
}
impl Fonts {
    fn load() -> Self {
        let f = |b: &'static [u8]| FontRef::try_from_slice(b).expect("bundled font");
        Self {
            faces: [
                f(horizon_theme::fonts::GEIST_REGULAR),
                f(horizon_theme::fonts::GEIST_MEDIUM),
                f(horizon_theme::fonts::GEIST_SEMIBOLD),
                f(horizon_theme::fonts::GEIST_MONO),
                f(horizon_theme::fonts::UNIFONT),
                f(horizon_theme::fonts::PIXEL_BOLD),
                f(horizon_theme::fonts::BRICOLAGE_REGULAR),
                f(horizon_theme::fonts::BRICOLAGE_MEDIUM),
                f(horizon_theme::fonts::BRICOLAGE_SEMIBOLD),
                f(horizon_theme::fonts::BRICOLAGE_BOLD),
            ],
        }
    }
    fn face(&self, f: Face) -> &FontRef<'static> {
        &self.faces[f as usize]
    }
    /// The ab_glyph scale for an em size in pixels.
    fn scale(&self, f: Face, px: f32) -> PxScale {
        let font = self.face(f);
        let upem = font.units_per_em().unwrap_or(1000.0);
        PxScale::from(px * font.height_unscaled() / upem)
    }
}
/// `text` without its `§` formatting codes (`StringDecomposer.iterateFormatted`
/// skips the `§` and the character after it, a lone trailing `§` too).
pub(crate) fn unformatted(text: &str) -> impl Iterator<Item = char> + '_ {
    let mut chars = text.chars();
    std::iter::from_fn(move || {
        loop {
            match chars.next()? {
                '§' => {
                    chars.next();
                }
                ch => return Some(ch),
            }
        }
    })
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Glyph(Face, u16, u16),
    /// A quarter disc (`false`) or a 1px quarter ring of this radius in pixels.
    Corner(u16, bool),
    /// A Pixelarticons icon (index in `horizon_theme::icons::ALL`) at this many
    /// pixels per grid unit.
    Icon(u16, u16),
}

/// A 24-unit pixel icon's filled cells, from its SVG paths. Pixelarticons only use
/// axis-aligned `M m H h V v Z z` paths; nonzero winding at each cell's centre.
pub fn icon_mask(svg: &[u8]) -> Option<[[bool; 24]; 24]> {
    let svg = std::str::from_utf8(svg).ok()?;
    let mut edges: Vec<([f32; 2], [f32; 2])> = Vec::new();
    for part in svg.split(" d=\"").skip(1) {
        let d = part.split('"').next()?;
        let (mut pen, mut start) = ([0.0f32; 2], [0.0f32; 2]);
        let mut command = 'M';
        let mut numbers = Vec::new();
        let mut flush =
            |command: char, numbers: &mut Vec<f32>, pen: &mut [f32; 2], start: &mut [f32; 2]| {
                let relative = command.is_ascii_lowercase();
                let mut i = 0;
                while i < numbers.len() || (numbers.is_empty() && i == 0) {
                    match command.to_ascii_uppercase() {
                        'M' if i + 1 < numbers.len() => {
                            let to = if relative {
                                [pen[0] + numbers[i], pen[1] + numbers[i + 1]]
                            } else {
                                [numbers[i], numbers[i + 1]]
                            };
                            // Further pairs after a move are line-tos.
                            if i > 0 {
                                edges.push((*pen, to));
                            } else {
                                *start = to;
                            }
                            *pen = to;
                            i += 2;
                        }
                        'L' if i + 1 < numbers.len() => {
                            let to = if relative {
                                [pen[0] + numbers[i], pen[1] + numbers[i + 1]]
                            } else {
                                [numbers[i], numbers[i + 1]]
                            };
                            edges.push((*pen, to));
                            *pen = to;
                            i += 2;
                        }
                        'H' if i < numbers.len() => {
                            let to = [
                                if relative {
                                    pen[0] + numbers[i]
                                } else {
                                    numbers[i]
                                },
                                pen[1],
                            ];
                            edges.push((*pen, to));
                            *pen = to;
                            i += 1;
                        }
                        'V' if i < numbers.len() => {
                            let to = [
                                pen[0],
                                if relative {
                                    pen[1] + numbers[i]
                                } else {
                                    numbers[i]
                                },
                            ];
                            edges.push((*pen, to));
                            *pen = to;
                            i += 1;
                        }
                        'Z' => {
                            edges.push((*pen, *start));
                            *pen = *start;
                            break;
                        }
                        _ => break,
                    }
                }
                numbers.clear();
            };
        let mut token = String::new();
        for ch in d.chars().chain(std::iter::once(' ')) {
            if ch.is_ascii_digit() || ch == '.' || (ch == '-' && token.is_empty()) {
                token.push(ch);
                continue;
            }
            if !token.is_empty() {
                numbers.push(token.parse().ok()?);
                token.clear();
            }
            if ch == '-' {
                token.push(ch);
            } else if ch.is_ascii_alphabetic() {
                flush(command, &mut numbers, &mut pen, &mut start);
                command = ch;
            }
        }
        flush(command, &mut numbers, &mut pen, &mut start);
    }
    let mut mask = [[false; 24]; 24];
    let mut any = false;
    for (y, row) in mask.iter_mut().enumerate() {
        for (x, cell) in row.iter_mut().enumerate() {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Winding of a ray to +x: vertical edges to the right that span cy.
            let winding: i32 = edges
                .iter()
                .filter(|(a, b)| a[0] == b[0] && a[0] > cx)
                .map(|(a, b)| {
                    if a[1] <= cy && b[1] > cy {
                        1
                    } else if b[1] <= cy && a[1] > cy {
                        -1
                    } else {
                        0
                    }
                })
                .sum();
            *cell = winding != 0;
            any |= *cell;
        }
    }
    any.then_some(mask)
}
#[derive(Clone, Copy)]
struct Cached {
    rect: Option<Rect>,
    /// Offset of the bitmap from the pen (glyphs) in pixels.
    left: f32,
    top: f32,
}
/// Glyphs and corner masks rasterized into the atlas region `horizon/ui-text`.
struct Cache {
    fonts: Fonts,
    entries: HashMap<Key, Cached>,
    shelf: (u32, u32, u32),
    pending: Vec<crate::font::Patch>,
    /// The atlas the entries were uploaded to (a new atlas starts over).
    atlas: usize,
}
thread_local! {
    static CACHE: RefCell<Option<Cache>> = const { RefCell::new(None) };
}
/// Reloaded atlases can occupy the same address with empty run-time slots.
pub fn reset_atlas() {
    CACHE.with(|cache| *cache.borrow_mut() = None);
}
fn with_cache<R>(atlas: &HudAtlas, f: impl FnOnce(&mut Cache) -> R) -> R {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let id = atlas as *const HudAtlas as usize;
        let cache = c.get_or_insert_with(|| Cache {
            fonts: Fonts::load(),
            entries: HashMap::new(),
            shelf: (0, 0, 0),
            pending: Vec::new(),
            atlas: id,
        });
        if cache.atlas != id {
            cache.atlas = id;
            cache.entries.clear();
            cache.shelf = (0, 0, 0);
            cache.pending.clear();
        }
        f(cache)
    })
}
impl Cache {
    fn region(atlas: &HudAtlas) -> Option<Rect> {
        atlas.sprites.get("horizon/ui-text").copied()
    }
    /// Reserve `w x h` pixels; on overflow forget everything and start over.
    fn place(&mut self, atlas: &HudAtlas, w: u32, h: u32) -> Option<(u32, u32, Rect)> {
        let region = Self::region(atlas)?;
        let (mut x, mut y, mut row) = self.shelf;
        if x + w + 1 > region.w {
            x = 0;
            y += row + 1;
            row = 0;
        }
        if y + h > region.h || w > region.w {
            self.entries.clear();
            self.shelf = (0, 0, 0);
            return None;
        }
        self.shelf = (x + w + 1, y, row.max(h));
        Some((x, y, region))
    }
    fn upload(&mut self, atlas: &HudAtlas, w: u32, h: u32, alpha: &[f32]) -> Option<Rect> {
        let (x, y, region) = self.place(atlas, w, h)?;
        let (ax, ay) = (
            (region.u0 * atlas.width as f32).round() as u32 + x,
            (region.v0 * atlas.height as f32).round() as u32 + y,
        );
        let rgba = alpha
            .iter()
            .flat_map(|a| [255, 255, 255, (a.clamp(0.0, 1.0) * 255.0).round() as u8])
            .collect();
        self.pending.push(crate::font::Patch {
            x: ax,
            y: ay,
            width: w,
            height: h,
            rgba,
        });
        let (aw, ah) = (atlas.width as f32, atlas.height as f32);
        Some(Rect {
            u0: ax as f32 / aw,
            v0: ay as f32 / ah,
            u1: (ax + w) as f32 / aw,
            v1: (ay + h) as f32 / ah,
            w,
            h,
        })
    }
    fn glyph(&mut self, atlas: &HudAtlas, face: Face, id: ab_glyph::GlyphId, px: f32) -> Cached {
        let size = (px * 4.0).round() as u16;
        let key = Key::Glyph(face, id.0, size);
        if let Some(c) = self.entries.get(&key) {
            return *c;
        }
        let scale = self.fonts.scale(face, f32::from(size) / 4.0);
        let glyph = id.with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));
        let made = match self.fonts.face(face).outline_glyph(glyph) {
            Some(outline) => {
                let b = outline.px_bounds();
                let (w, h) = (b.width() as u32, b.height() as u32);
                let mut alpha = vec![0.0; (w * h) as usize];
                outline.draw(|x, y, c| {
                    if x < w && y < h {
                        alpha[(y * w + x) as usize] = c;
                    }
                });
                // Light text on dark backgrounds reads thin: a gentle boost.
                for a in &mut alpha {
                    *a = a.powf(0.85);
                }
                Cached {
                    rect: self.upload(atlas, w, h, &alpha),
                    left: b.min.x,
                    top: b.min.y,
                }
            }
            None => Cached {
                rect: None,
                left: 0.0,
                top: 0.0,
            },
        };
        if made.rect.is_some() || made.left == 0.0 {
            self.entries.insert(key, made);
        }
        made
    }
    fn icon(&mut self, atlas: &HudAtlas, index: usize, unit: u16) -> Option<Rect> {
        let key = Key::Icon(index as u16, unit);
        if let Some(c) = self.entries.get(&key) {
            return c.rect;
        }
        let mask = icon_mask(horizon_theme::icons::ALL.get(index)?.1)?;
        let u = u32::from(unit.max(1));
        let n = 24 * u;
        let mut alpha = vec![0.0; (n * n) as usize];
        for y in 0..n {
            for x in 0..n {
                if mask[(y / u) as usize][(x / u) as usize] {
                    alpha[(y * n + x) as usize] = 1.0;
                }
            }
        }
        let rect = self.upload(atlas, n, n, &alpha);
        if rect.is_some() {
            self.entries.insert(
                key,
                Cached {
                    rect,
                    left: 0.0,
                    top: 0.0,
                },
            );
        }
        rect
    }
    fn corner(&mut self, atlas: &HudAtlas, r: u16, ring: bool) -> Option<Rect> {
        let key = Key::Corner(r, ring);
        if let Some(c) = self.entries.get(&key) {
            return c.rect;
        }
        // The top-left quadrant of a circle centred at (r, r), 4x4 supersampled.
        let n = u32::from(r);
        let rf = f32::from(r);
        let mut alpha = vec![0.0; (n * n) as usize];
        for y in 0..n {
            for x in 0..n {
                let mut hits = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let px = x as f32 + (sx as f32 + 0.5) / 4.0;
                        let py = y as f32 + (sy as f32 + 0.5) / 4.0;
                        let d = ((px - rf).powi(2) + (py - rf).powi(2)).sqrt();
                        if d <= rf && (!ring || d >= rf - 1.0) {
                            hits += 1;
                        }
                    }
                }
                alpha[(y * n + x) as usize] = hits as f32 / 16.0;
            }
        }
        let rect = self.upload(atlas, n, n, &alpha);
        if rect.is_some() {
            self.entries.insert(
                key,
                Cached {
                    rect,
                    left: 0.0,
                    top: 0.0,
                },
            );
        }
        rect
    }
}

/// One frame of design-system drawing, in dp: the
/// launcher's logical pixels, scaled so the screen reads like the launcher's
/// 1040x700 window at any size.
pub struct Ui<'a> {
    pub atlas: &'a HudAtlas,
    pub batch: &'a mut HudBatch,
    /// The screen in dp.
    pub w: f32,
    pub h: f32,
    /// The cursor in dp.
    pub cursor: (f32, f32),
    /// Physical pixels per dp.
    pub density: f32,
    scale: f32,
}
impl<'a> Ui<'a> {
    /// `cursor` is in GUI pixels, like every screen's input.
    pub fn new(atlas: &'a HudAtlas, batch: &'a mut HudBatch, cursor: (f32, f32)) -> Self {
        let scale = batch.scale;
        let (pw, ph) = (batch.width * scale, batch.height * scale);
        let density = (pw / 1040.0).min(ph / 700.0).clamp(1.0, 4.0);
        Self {
            atlas,
            w: pw / density,
            h: ph / density,
            cursor: (cursor.0 * scale / density, cursor.1 * scale / density),
            density,
            scale,
            batch,
        }
    }
    /// Native launcher density on tall phone screens; game HUD scale is untouched.
    pub fn mobile(atlas: &'a HudAtlas, batch: &'a mut HudBatch, cursor: (f32, f32)) -> Self {
        let mut ui = Self::new(atlas, batch, cursor);
        let pw = ui.batch.width * ui.scale;
        let ph = ui.batch.height * ui.scale;
        if ph > pw {
            ui.density = (pw / 480.0).clamp(1.0, 4.0);
            ui.w = pw / ui.density;
            ui.h = ph / ui.density;
            ui.cursor = (
                cursor.0 * ui.scale / ui.density,
                cursor.1 * ui.scale / ui.density,
            );
        } else if ph < 700.0 {
            ui.density = (pw / 1040.0).min(ph / 700.0).clamp(0.5, 4.0);
            ui.w = pw / ui.density;
            ui.h = ph / ui.density;
            ui.cursor = (
                cursor.0 * ui.scale / ui.density,
                cursor.1 * ui.scale / ui.density,
            );
        }
        ui
    }

    /// The em size in physical pixels for `size` dp: Unifont snaps to its 16 px grid
    /// (never below one grid), everything else to quarter pixels.
    pub fn font_px(&self, face: Face, size: f32) -> f32 {
        let px = size * self.density;
        if face == Face::Pixel {
            (px / 16.0).round().max(1.0) * 16.0
        } else {
            (px * 4.0).round() / 4.0
        }
    }
    /// The Android launcher at the display's own density (`DisplayMetrics.density`,
    /// winit's scale factor), so a dp is the platform's dp: 48 dp touch targets
    /// are the size Android recommends. Very small screens shrink to keep a usable
    /// 320 × 240 dp canvas.
    pub fn launcher(
        atlas: &'a HudAtlas,
        batch: &'a mut HudBatch,
        cursor: (f32, f32),
        density: f32,
    ) -> Self {
        let mut ui = Self::new(atlas, batch, cursor);
        let pw = ui.batch.width * ui.scale;
        let ph = ui.batch.height * ui.scale;
        let fit = (pw.min(ph) / 240.0).min(pw.max(ph) / 320.0);
        ui.density = density.clamp(0.75, 6.0).min(fit.max(0.5));
        ui.w = pw / ui.density;
        ui.h = ph / ui.density;
        ui.cursor = (
            cursor.0 * ui.scale / ui.density,
            cursor.1 * ui.scale / ui.density,
        );
        ui
    }
    pub fn fit_text(&self, face: Face, size: f32, text: &str, width: f32) -> String {
        if self.measure(face, size, text) <= width {
            return text.to_owned();
        }
        let px = self.font_px(face, size);
        with_cache(self.atlas, |cache| {
            let font = cache.fonts.face(face);
            let scaled = font.as_scaled(cache.fonts.scale(face, px));
            crate::ui_text::fit(text, width * self.density, |previous, ch| {
                let id = font.glyph_id(ch);
                previous.map_or(0.0, |p| scaled.kern(font.glyph_id(p), id)) + scaled.h_advance(id)
            })
        })
    }
    /// Move rasterized glyphs into the batch's atlas uploads (real frames only).
    pub fn finish(self) {
        let pending = with_cache(self.atlas, |c| std::mem::take(&mut c.pending));
        self.batch.patches.extend(pending);
        // Pack glyphs `layout` borrowed from the resource pack's font.
        self.batch.patches.extend(self.atlas.font.take_patches());
    }

    // ---- geometry, snapped to physical pixels ----
    fn px_x(&self, v: f32) -> f32 {
        (v * self.density).round()
    }
    fn px_y(&self, v: f32) -> f32 {
        (v * self.density).round()
    }
    fn raw_fill(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: u32) {
        if x1 > x0 && y1 > y0 {
            let s = self.scale;
            self.batch.fill(
                self.atlas,
                x0 / s,
                y0 / s,
                (x1 - x0) / s,
                (y1 - y0) / s,
                color,
            );
        }
    }
    fn corner_quad(&mut self, rect: Rect, x: f32, y: f32, r: f32, flip: (bool, bool), color: u32) {
        let (u0, u1) = if flip.0 {
            (rect.u1, rect.u0)
        } else {
            (rect.u0, rect.u1)
        };
        let (v0, v1) = if flip.1 {
            (rect.v1, rect.v0)
        } else {
            (rect.v0, rect.v1)
        };
        let s = self.scale;
        let (gx, gy, gr) = (x / s, y / s, r / s);
        self.batch.quad(
            [[gx, gy], [gx + gr, gy], [gx + gr, gy + gr], [gx, gy + gr]],
            [[u0, v0], [u1, v0], [u1, v1], [u0, v1]],
            color,
        );
    }
    fn corners(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, ring: bool, color: u32) {
        if r < 1.0 {
            return;
        }
        let atlas = self.atlas;
        if let Some(rect) = with_cache(atlas, |c| c.corner(atlas, r as u16, ring)) {
            self.corner_quad(rect, x0, y0, r, (false, false), color);
            self.corner_quad(rect, x1 - r, y0, r, (true, false), color);
            self.corner_quad(rect, x1 - r, y1 - r, r, (true, true), color);
            self.corner_quad(rect, x0, y1 - r, r, (false, true), color);
        }
    }
    /// A filled rectangle with rounded corners.
    pub fn rounded(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: u32) {
        if color >> 24 == 0 {
            return;
        }
        let (x0, y0, x1, y1) = (
            self.px_x(x),
            self.px_y(y),
            self.px_x(x + w),
            self.px_y(y + h),
        );
        let r = (radius * self.density)
            .round()
            .min(((x1 - x0) / 2.0).floor())
            .min(((y1 - y0) / 2.0).floor())
            .max(0.0);
        if r < 1.0 {
            self.raw_fill(x0, y0, x1, y1, color);
            return;
        }
        self.raw_fill(x0 + r, y0, x1 - r, y1, color);
        self.raw_fill(x0, y0 + r, x0 + r, y1 - r, color);
        self.raw_fill(x1 - r, y0 + r, x1, y1 - r, color);
        self.corners(x0, y0, x1, y1, r, false, color);
    }
    /// A 1px rounded border.
    pub fn border(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: u32) {
        if color >> 24 == 0 {
            return;
        }
        let (x0, y0, x1, y1) = (
            self.px_x(x),
            self.px_y(y),
            self.px_x(x + w),
            self.px_y(y + h),
        );
        let r = (radius * self.density)
            .round()
            .min(((x1 - x0) / 2.0).floor())
            .min(((y1 - y0) / 2.0).floor())
            .max(0.0);
        self.raw_fill(x0 + r, y0, x1 - r, y0 + 1.0, color);
        self.raw_fill(x0 + r, y1 - 1.0, x1 - r, y1, color);
        self.raw_fill(x0, y0 + r, x0 + 1.0, y1 - r, color);
        self.raw_fill(x1 - 1.0, y0 + r, x1, y1 - r, color);
        self.corners(x0, y0, x1, y1, r, true, color);
    }

    /// A Pixelarticons icon (`horizon_theme::icons`) `size` dp square at (x, y),
    /// tinted `color`. Drawn at a whole number of pixels per grid unit, so its
    /// cells stay square; centred in the requested box.
    pub fn icon(&mut self, name: &str, x: f32, y: f32, size: f32, color: u32) {
        let Some(index) = horizon_theme::icons::ALL
            .iter()
            .position(|(n, _)| *n == name)
        else {
            return;
        };
        let unit = ((size * self.density) / 24.0).round().max(1.0) as u16;
        let atlas = self.atlas;
        let Some(rect) = with_cache(atlas, |c| c.icon(atlas, index, unit)) else {
            return;
        };
        let side = 24.0 * f32::from(unit);
        let (x0, y0) = (
            (self.px_x(x) + (size * self.density - side) / 2.0).round(),
            (self.px_y(y) + (size * self.density - side) / 2.0).round(),
        );
        let s = self.scale;
        self.batch
            .rect_uv(x0 / s, y0 / s, side / s, side / s, rect, color);
    }

    // ---- text ----
    /// `text` laid out at `px` pixels to the em: its advance, the face's ascent
    /// and descent, and (`draw`) its glyphs as `(x, y, w, h, rect)` from the pen
    /// on the baseline, all in physical pixels. `§` codes are dropped (the
    /// design system keeps its own colours). A character Geist lacks, such as a
    /// pack's private-use icon, is the resource pack's default-font glyph:
    /// vanilla's 9px line scaled onto Geist's, or at GUI scale as the pack
    /// draws it when every character is such a glyph (a picture title).
    fn layout(
        &self,
        face: Face,
        px: f32,
        text: &str,
        draw: bool,
    ) -> (f32, f32, f32, Vec<(f32, f32, f32, f32, Rect)>) {
        let atlas = self.atlas;
        let gui = self.scale;
        with_cache(atlas, |c| {
            let scale = c.fonts.scale(face, px);
            let (ascent, descent) = {
                let f = c.fonts.face(face).as_scaled(scale);
                (f.ascent(), f.descent())
            };
            let chars: Vec<(char, Option<crate::font::Glyph>)> = unformatted(text)
                .map(|ch| {
                    let pack = (!ch.is_whitespace() && c.fonts.face(face).glyph_id(ch).0 == 0)
                        .then(|| atlas.font.glyph(ch))
                        // The pack lacks it too (its missing glyph): Geist's box, like vanilla's.
                        .filter(|g| g.rect.is_some() || g.right <= 0.0);
                    (ch, pack)
                })
                .collect();
            let k = if chars.iter().all(|(_, g)| g.is_some()) {
                gui
            } else {
                (ascent - descent) / 9.0
            };
            let (mut pen, mut prev, mut quads) = (0.0, None, Vec::new());
            for (ch, pack) in chars {
                if let Some(g) = pack {
                    if let (true, Some(rect)) = (draw, g.rect) {
                        // Vanilla's baseline is 7 below the line's top.
                        let (x, y) = ((pen + g.left * k).round(), ((g.top - 7.0) * k).round());
                        quads.push((x, y, (g.right - g.left) * k, (g.bottom - g.top) * k, rect));
                    }
                    pen += g.advance * k;
                    prev = None;
                    continue;
                }
                let (id, advance, kern) = {
                    let font = c.fonts.face(face);
                    let id = font.glyph_id(ch);
                    let f = font.as_scaled(scale);
                    (id, f.h_advance(id), prev.map_or(0.0, |p| f.kern(p, id)))
                };
                pen += kern;
                if draw && !ch.is_whitespace() {
                    let g = c.glyph(atlas, face, id, px);
                    if let Some(rect) = g.rect {
                        quads.push((
                            (pen + g.left).round(),
                            g.top,
                            rect.w as f32,
                            rect.h as f32,
                            rect,
                        ));
                    }
                }
                pen += advance;
                prev = Some(id);
            }
            (pen, ascent, descent, quads)
        })
    }
    /// The advance of `text` in dp.
    pub fn measure(&self, face: Face, size: f32, text: &str) -> f32 {
        let px = self.font_px(face, size);
        self.layout(face, px, text, false).0 / self.density
    }
    /// Draw a line of text in a `line`-tall box whose top is `y` (the
    /// launcher's line heights: about 1.45 × size for the interface);
    /// returns its width in dp.
    pub fn text_line(
        &mut self,
        face: Face,
        size: f32,
        line: f32,
        x: f32,
        y: f32,
        color: u32,
        text: &str,
    ) -> f32 {
        let px = self.font_px(face, size);
        let (x0, y0) = (self.px_x(x), self.px_y(y));
        let (width, ascent, descent, quads) = self.layout(face, px, text, color >> 24 != 0);
        let baseline = y0 + ((line * self.density - (ascent - descent)) / 2.0 + ascent).round();
        let s = self.scale;
        for (gx, gy, w, h, rect) in quads {
            self.batch.rect_uv(
                (x0 + gx) / s,
                (baseline + gy) / s,
                w / s,
                h / s,
                rect,
                color,
            );
        }
        width / self.density
    }
    /// Text in a `1.45 × size` line box whose top is `y`.
    pub fn text(&mut self, face: Face, size: f32, x: f32, y: f32, color: u32, text: &str) -> f32 {
        self.text_line(face, size, (size * 1.45).round(), x, y, color, text)
    }
    pub fn text_centered(
        &mut self,
        face: Face,
        size: f32,
        cx: f32,
        y: f32,
        color: u32,
        text: &str,
    ) {
        let w = self.measure(face, size, text);
        self.text(face, size, cx - w / 2.0, y, color, text);
    }

    // ---- the launcher's components ----
}

// ---- the boot screen ----

/// The boot screen's palette on the pure black: a white wordmark and dimmed
/// console lines (the theme's ink is for light pages).
const BOOT_TEXT: u32 = color::hex(0xFFFFFF, 255);
const BOOT_LOG: u32 = color::hex(0x9BA1A6, 255);

/// The boot screen while the menu is not ready yet: the wordmark centred on
/// pure black and the console's last lines at the bottom right, as the
/// terminal prints them.
pub fn draw_boot(atlas: &HudAtlas, batch: &mut HudBatch, logs: &[String]) {
    // Pure black under everything: the frame's panorama (the title artwork
    // once the bundle is in) must not leak through.
    batch.fill(
        atlas,
        0.0,
        0.0,
        batch.width,
        batch.height,
        color::hex(0x000000, 255),
    );
    wordmark(atlas, batch);
    draw_boot_log(atlas, batch, logs);
}

/// The wordmark with its tagline, centred.
fn wordmark(atlas: &HudAtlas, batch: &mut HudBatch) {
    let mut ui = Ui::new(atlas, batch, (0.0, 0.0));
    let size = (ui.w / 16.0).clamp(DISPLAY, 96.0);
    let small = (size * 0.25).clamp(MICRO, 18.0);
    let block = size * 1.45 + small * 0.6 + small * 1.5;
    let top = (ui.h - block) / 2.0;
    ui.text_centered(Face::SemiBold, size, ui.w / 2.0, top, BOOT_TEXT, "Horizon");
    ui.text_centered(
        Face::Mono,
        small,
        ui.w / 2.0,
        top + size * 1.45 + small * 0.6,
        BOOT_LOG,
        "Rust based minecraft client",
    );
    ui.finish();
}

/// The console's last lines at the bottom left, over any loading screen: long
/// lines wrap word-wise, the block grows upwards.
pub fn draw_boot_log(atlas: &HudAtlas, batch: &mut HudBatch, logs: &[String]) {
    if logs.is_empty() {
        return;
    }
    let mut ui = Ui::new(atlas, batch, (0.0, 0.0));
    let (size, line, pad) = (MICRO, MICRO * 1.5, 16.0);
    let max_w = ui.w / 2.0;
    let max_rows = (((ui.h - pad) / line) as usize).clamp(1, 24);
    let mut y = ui.h - pad - line;
    let mut drawn = 0;
    'lines: for text in logs.iter().rev() {
        let chunks = wrapping_lines(&ui, Face::Mono, size, text, max_w);
        for chunk in chunks.iter().rev() {
            if drawn == max_rows {
                break 'lines;
            }
            ui.text(Face::Mono, size, pad, y, BOOT_LOG, chunk);
            y -= line;
            drawn += 1;
        }
    }
    ui.finish();
}

/// The lines `text` wraps into at `max_w`, word-wise; a word wider than
/// `max_w` on its own breaks at the edge.
fn wrapping_lines(ui: &Ui, face: Face, size: f32, text: &str, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let mut word = word;
        loop {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if ui.measure(face, size, &candidate) <= max_w {
                line = candidate;
                break;
            }
            if line.is_empty() {
                // The word alone does not fit: hard-break it at the edge.
                let mut end = 0;
                for (i, ch) in word.char_indices() {
                    if i > 0 && ui.measure(face, size, &word[..i]) > max_w {
                        break;
                    }
                    end = i + ch.len_utf8();
                }
                lines.push(word[..end].to_owned());
                word = &word[end..];
            } else {
                lines.push(std::mem::take(&mut line));
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod icon_tests {
    #[test]
    fn every_pixel_icon_parses_to_cells() {
        for (name, svg) in horizon_theme::icons::ALL {
            assert!(super::icon_mask(svg).is_some(), "{name}");
        }
        // home.svg's roof apex: the 4-unit block at x 10..14, y 2..4.
        let home = super::icon_mask(horizon_theme::icons::get("home").unwrap()).unwrap();
        assert!(home[2][10] && home[3][13] && !home[2][9] && !home[0][0]);
        // Its floor line spans x 4..20 at y 20..22.
        assert!(home[21][4] && home[21][19] && !home[21][3] && !home[21][20]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_reload_regenerates_text_and_corners_at_the_same_address() {
        let atlas = HudAtlas::build(Path::new("/nonexistent/horizon-test-assets")).unwrap();
        reset_atlas();
        let draw = |cache: &mut Cache| {
            let glyph = cache.fonts.face(Face::Regular).glyph_id('A');
            assert!(
                cache
                    .glyph(&atlas, Face::Regular, glyph, 16.0)
                    .rect
                    .is_some()
            );
            assert!(cache.corner(&atlas, 8, false).is_some());
            std::mem::take(&mut cache.pending)
        };
        let before = with_cache(&atlas, draw);
        assert_eq!(before.len(), 2);
        assert!(with_cache(&atlas, draw).is_empty());
        reset_atlas();
        let after = with_cache(&atlas, draw);
        assert_eq!(after.len(), 2);
        for (a, b) in before.iter().zip(&after) {
            assert_eq!((a.x, a.y, a.width, a.height), (b.x, b.y, b.width, b.height));
            assert_eq!(a.rgba, b.rgba);
        }
        reset_atlas();
    }

    #[test]
    fn the_boot_atlas_holds_the_run_time_glyph_region_without_a_pack() {
        let atlas = HudAtlas::boot();
        assert_eq!((atlas.width, atlas.height), (1024, 1024));
        assert_eq!(
            atlas.sprites["horizon/ui-text"].texels((atlas.width, atlas.height)),
            (1024, 512)
        );
        assert_eq!(
            atlas.sprites["horizon/white"].texels((atlas.width, atlas.height)),
            (4, 4)
        );
    }

    #[test]
    fn the_boot_screen_centres_the_wordmark_and_left_aligns_the_console() {
        reset_atlas();
        let atlas = HudAtlas::boot();
        let mut batch = HudBatch::new([1280, 720]);
        draw_boot(&atlas, &mut batch, &[]);
        // The first quad is the pure black backdrop over the frame's panorama.
        let (w, h) = (batch.width, batch.height);
        let bg = &batch.vertices[..6];
        assert!(bg.iter().all(|v| v.color == 0xFF00_0000));
        assert!(bg.iter().all(|v| v.pos[0] == 0.0 || v.pos[0] == w));
        assert!(bg.iter().all(|v| v.pos[1] == 0.0 || v.pos[1] == h));
        assert!(
            !batch.patches.is_empty(),
            "the glyphs are queued for the atlas"
        );

        let mut batch = HudBatch::new([1280, 720]);
        wordmark(&atlas, &mut batch);
        let (min, max) = batch
            .vertices
            .iter()
            .map(|v| v.pos[0])
            .fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
        assert!(
            min < batch.width / 2.0 && max > batch.width / 2.0,
            "the wordmark straddles the centre"
        );
        assert!(((min + max) / 2.0 - batch.width / 2.0).abs() < 1.0);

        let long = "[12:34:56] [Render thread/INFO]: Horizon: resources loaded in 1.0s; 1240 block states, 1500 items, 62 biomes; 8 worker threads".to_owned();
        let mut batch = HudBatch::new([1280, 720]);
        draw_boot_log(&atlas, &mut batch, &[long]);
        let (min, max) = batch
            .vertices
            .iter()
            .map(|v| v.pos[0])
            .fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
        assert!(min >= 4.0, "the line stays inside the left inset");
        assert!(
            max - min <= batch.width / 2.0 + 1.0,
            "a too wide line is cut to half the screen"
        );

        let mut batch = HudBatch::new([1280, 720]);
        let ui = Ui::new(&atlas, &mut batch, (0.0, 0.0));
        let lines = wrapping_lines(
            &ui,
            Face::Mono,
            MICRO,
            "the quick brown fox jumps over the lazy dog",
            90.0,
        );
        assert!(lines.len() > 1, "a long line wraps");
        assert!(
            lines
                .iter()
                .all(|l| ui.measure(Face::Mono, MICRO, l) <= 90.0),
            "every wrapped line fits"
        );
        assert_eq!(
            lines
                .iter()
                .map(|l| l.split_whitespace().count())
                .sum::<usize>(),
            9,
            "the wrap keeps every word"
        );

        let mut batch = HudBatch::new([1280, 720]);
        draw_boot_log(&atlas, &mut batch, &[]);
        assert!(batch.vertices.is_empty() && batch.patches.is_empty());
        reset_atlas();
    }

    /// The quads' top-left corners and UVs `text_line` draws for `text`.
    fn drawn(atlas: &HudAtlas, text: &str) -> (f32, Vec<([f32; 2], [f32; 2])>) {
        let mut batch = HudBatch::new([1280, 720]);
        let mut ui = Ui::new(atlas, &mut batch, (0.0, 0.0));
        let w = ui.text_line(Face::Medium, BODY, 20.0, 10.0, 10.0, color::TEXT, text);
        let quads = batch
            .vertices
            .iter()
            .step_by(6)
            .map(|v| (v.pos, v.uv))
            .collect();
        (w, quads)
    }

    #[test]
    fn section_sign_codes_are_formatting_not_text() {
        reset_atlas();
        let atlas = HudAtlas::boot();
        // A pack's lang gives `menu.options` as "§6Options..." (vanilla draws it gold).
        assert_eq!(drawn(&atlas, "§6Options..."), drawn(&atlas, "Options..."));
        assert_eq!(drawn(&atlas, "§lA§r§"), drawn(&atlas, "A"));
        reset_atlas();
    }

    #[test]
    fn a_private_use_glyph_geist_lacks_is_the_packs_bitmap() {
        let root = std::env::temp_dir().join(format!("hz-ui-pua-{}", std::process::id()));
        let font = root.join("assets/minecraft/font");
        let tex = root.join("assets/minecraft/textures/font");
        std::fs::create_dir_all(&font).unwrap();
        std::fs::create_dir_all(&tex).unwrap();
        std::fs::write(
            font.join("default.json"),
            r#"{"providers":[{"type":"bitmap","file":"minecraft:font/icon.png","ascent":7,"chars":[""]}]}"#,
        )
        .unwrap();
        let mut png = Vec::new();
        let mut enc = png::Encoder::new(&mut png, 8, 8);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&[255; 256])
            .unwrap();
        std::fs::write(tex.join("icon.png"), png).unwrap();
        reset_atlas();
        let atlas = HudAtlas::build(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);
        let icon = atlas.font.glyph('\u{e238}');
        let uv = icon
            .rect
            .map(|r| [r.u0, r.v0])
            .expect("the pack draws U+E238");
        // Alone (a picture label) and inside a Geist line, the quad is the pack's bitmap.
        for text in ["\u{e238}", "Quit \u{e238}"] {
            let (w, quads) = drawn(&atlas, text);
            assert_eq!(quads.last().unwrap().1, uv, "{text:?}");
            assert!(w > 0.0);
        }
        // Alone it is drawn as the pack draws it: 8 GUI pixels wide, advance 9.
        let (w, quads) = drawn(&atlas, "\u{e238}");
        assert_eq!(quads.len(), 1);
        let mut batch = HudBatch::new([1280, 720]);
        let ui = Ui::new(&atlas, &mut batch, (0.0, 0.0));
        assert!(
            (w * ui.density / ui.scale - icon.advance).abs() < 0.01,
            "{w}"
        );
        reset_atlas();
    }
}
