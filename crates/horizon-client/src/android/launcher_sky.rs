//! The launcher's pixel night sky: the desktop launcher's sky (`launcher/src/sky.rs`)
//! redrawn as quads, with 4 dp texels. Static apart from twinkling stars and slow
//! clouds, which step on a half-second beat.
use super::kit::fill;
use horizon_client::ui_theme::{Ui, color::hex};

const TEXEL: f32 = 4.0;
const STOPS: [u32; 6] = [0x07061a, 0x140f38, 0x2a1656, 0x5b2a78, 0xb0477d, 0xf08a5d];
const SHARES: [f32; 6] = [0.2, 0.18, 0.17, 0.15, 0.15, 0.15];
const STAR_LEVELS: [f32; 4] = [0.95, 0.6, 0.3, 0.6];
const STAR_PERIODS: [f32; 4] = [4.0, 6.0, 8.0, 10.0];

const PUFF: &[&str] = &[
    "..............############..................",
    "......##########################............",
    "..##########################################",
    "============================================",
    ".....----------------------------------.....",
];
const LONG: &[&str] = &[
    "......................##############............................",
    "..............##################################................",
    "....############################################################",
    "==============================================================..",
    "..........------------------------------------------............",
    "..............................#################.................",
    "..............................=================.................",
];
const WISP: &[&str] = &[
    "......##################......",
    "==============================",
];
const BANK: &[&str] = &[
    "............##########........................................",
    "......##########################........##############........",
    "..##################################..########################",
    "====================================..========================",
    "......-------------------------.........---------------.......",
];
type Placement = (f32, f32, &'static [&'static str]);
/// (period in texels, top share, speed in texels/s, clouds).
const CLOUDS: [(f32, f32, f32, &[Placement]); 3] = [
    (
        300.0,
        0.36,
        0.5,
        &[(20.0, 0.0, WISP), (140.0, 3.0, PUFF), (230.0, 1.0, WISP)],
    ),
    (
        380.0,
        0.52,
        1.0,
        &[(30.0, 1.0, BANK), (200.0, 0.0, PUFF), (300.0, 4.0, WISP)],
    ),
    (460.0, 0.64, 2.0, &[(60.0, 0.0, LONG), (290.0, 2.0, PUFF)]),
];
const CLOUD_BODY: [u32; 3] = [0x3b2869, 0x6e3a7e, 0xc96a86];
const CLOUD_LIT: [u32; 3] = [0x5a3f8a, 0x9c5a92, 0xf7a98a];
const CLOUD_SHADE: u32 = 0x2a1a4c;

const TREE: &[&str] = &[".###.", "#####", "#####", ".###.", "..#..", "..#.."];
const PINE: &[&str] = &["..#..", ".###.", "..#..", ".###.", "#####", "..#.."];
const POST: &[&str] = &[".#.", "###", ".#.", ".#.", ".#."];
const TOWER: &[&str] = &["#.#", "###", "###", "###", "###", "###", "###"];
struct Skyline {
    rows: usize,
    lift: usize,
    runs: &'static [(usize, usize)],
    props: &'static [(usize, &'static [&'static str])],
}
const FAR: Skyline = Skyline {
    rows: 20,
    lift: 2,
    runs: &[
        (18, 8),
        (10, 10),
        (6, 13),
        (14, 11),
        (22, 9),
        (8, 12),
        (12, 14),
        (16, 11),
        (20, 8),
        (10, 10),
        (14, 12),
        (24, 9),
        (12, 11),
        (18, 8),
    ],
    props: &[(32, TOWER), (118, PINE), (176, TOWER)],
};
const NEAR: Skyline = Skyline {
    rows: 18,
    lift: 0,
    runs: &[
        (12, 7),
        (6, 9),
        (10, 8),
        (4, 11),
        (14, 10),
        (8, 8),
        (6, 6),
        (16, 7),
        (5, 10),
        (9, 12),
        (7, 9),
        (12, 7),
        (10, 8),
        (6, 11),
        (11, 9),
        (9, 7),
        (15, 7),
    ],
    props: &[
        (5, TREE),
        (36, POST),
        (60, PINE),
        (95, TREE),
        (124, POST),
        (139, TREE),
    ],
};

fn over(top: u32, a: f32, under: u32) -> u32 {
    let ch = |c: u32, s: u32| (c >> s) & 0xff;
    let mix = |s: u32| {
        ((ch(top, s) as f32 * a + ch(under, s) as f32 * (1.0 - a)).round() as u32).min(255) << s
    };
    mix(16) | mix(8) | mix(0)
}

/// The colour bands: (top, height, rgb), hard-edged, `stops` from the top.
fn bands(stops: &[u32], h: f32) -> Vec<(f32, f32, u32)> {
    let rows = (h / TEXEL).ceil();
    let mut out = Vec::new();
    let mut at = 0.0f32;
    for (i, stop) in stops.iter().enumerate() {
        let share = if stops.len() == SHARES.len() {
            SHARES[i]
        } else {
            1.0 / stops.len() as f32
        };
        let span = (share * rows).round().max(4.0);
        if i + 1 == stops.len() {
            let height = (h / TEXEL - at).max(span) * TEXEL;
            out.push((at * TEXEL, height, *stop));
            break;
        }
        let mut from = at;
        for k in 0..4 {
            let to = (at + span * (k + 1) as f32 / 4.0).round();
            out.push((
                from * TEXEL,
                (to - from) * TEXEL,
                over(stops[i + 1], k as f32 / 4.0, *stop),
            ));
            from = to;
        }
        at += span;
    }
    out
}

struct Star {
    x: f32,
    y: f32,
    group: usize,
    big: bool,
}
fn stars() -> &'static [Star] {
    static STARS: std::sync::OnceLock<Vec<Star>> = std::sync::OnceLock::new();
    STARS.get_or_init(|| {
        let mut s: u32 = 0x2545_f491;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s >> 8) as f32 / 16_777_216.0
        };
        let mut out = Vec::new();
        for i in 0..240 {
            let x = (next() * 960.0).floor();
            let y = next();
            let keep = next() > y * y;
            let big = next() < 0.08;
            if keep {
                out.push(Star {
                    x,
                    y,
                    group: i % 4,
                    big,
                });
            }
        }
        out
    })
}

fn stamp(ui: &mut Ui, ox: f32, oy: f32, rows: &[&str], paint: impl Fn(char) -> Option<u32>) {
    super::kit::sprite(ui, ox, oy, TEXEL, rows, |c| {
        paint(c).map(|rgb| hex(rgb, 255))
    });
}

fn skyline(ui: &mut Ui, [x, y, w, h]: [f32; 4], line: &Skyline, rgb: u32, start: f32) {
    let width: usize = line.runs.iter().map(|r| r.0).sum();
    let mut tops = Vec::with_capacity(width);
    for (run, height) in line.runs {
        tops.extend(std::iter::repeat_n(line.rows - height, *run));
    }
    let top0 = y + h - (line.rows + line.lift) as f32 * TEXEL;
    let color = hex(rgb, 255);
    let mut x0 = start;
    while x0 < w {
        let mut column = 0;
        for (run, height) in line.runs {
            let left = x0 + column as f32 * TEXEL;
            let right = (left + *run as f32 * TEXEL).min(w);
            let left = left.max(0.0);
            if right > left {
                fill(
                    ui,
                    x + left,
                    top0 + (line.rows - height) as f32 * TEXEL,
                    right - left,
                    *height as f32 * TEXEL,
                    color,
                );
            }
            column += run;
        }
        for (px, art) in line.props {
            let span = art[0].len();
            let ground = tops[*px..(*px + span).min(width)]
                .iter()
                .copied()
                .min()
                .unwrap_or(0);
            let ox = x0 + *px as f32 * TEXEL;
            if ox + span as f32 * TEXEL <= 0.0 || ox >= w {
                continue;
            }
            stamp(
                ui,
                x + ox,
                top0 + (ground as f32 - art.len() as f32) * TEXEL,
                art,
                |c| (c == '#').then_some(rgb),
            );
        }
        x0 += width as f32 * TEXEL;
    }
    if line.lift > 0 {
        fill(
            ui,
            x,
            y + h - line.lift as f32 * TEXEL,
            w,
            line.lift as f32 * TEXEL,
            color,
        );
    }
}

/// Home's full sky in `r`, at time `t` seconds.
pub(super) fn full(ui: &mut Ui, r: [f32; 4], t: f32) {
    let [x, y, w, h] = r;
    for (top, height, rgb) in bands(&STOPS, h) {
        let height = height.min(h - top);
        fill(ui, x, y + top, w, height, hex(rgb, 255));
    }
    draw_stars(ui, r, t, 0.5);
    for (depth, (period, share, speed, clouds)) in CLOUDS.iter().enumerate() {
        let strip = period * TEXEL;
        let shift = ((t * speed).floor() * TEXEL) % strip;
        let top = (h * share / TEXEL).round() * TEXEL;
        let mut x0 = -shift;
        while x0 < w {
            for (cx, cy, art) in clouds.iter() {
                let ox = x0 + cx * TEXEL;
                if ox >= w || ox + art[0].len() as f32 * TEXEL <= 0.0 {
                    continue;
                }
                // Clip to the canvas column by column: clouds slide off the left.
                clipped_stamp(ui, [x, y], ox, top + cy * TEXEL, w, art, |c| match c {
                    '#' => Some(CLOUD_BODY[depth]),
                    '=' => Some(CLOUD_LIT[depth]),
                    '-' => Some(CLOUD_SHADE),
                    _ => None,
                });
            }
            x0 += strip;
        }
    }
    // The sun: an upper half-disc, sliced, behind the skylines.
    let radius = 15.0f32;
    let sx = ((w * 0.72 - 60.0) / TEXEL).round() * TEXEL;
    let sy = h - (8.0 + radius) * TEXEL;
    for row in 0..15 {
        let ry = row as f32;
        let dy = radius - ry - 0.5;
        let half = (radius * radius - dy * dy).sqrt().round();
        if ry / radius > 0.5 && (15 - row) % 3 == 0 {
            continue;
        }
        let rgb = if ry / radius < 0.4 {
            0xffe28c
        } else if ry / radius < 0.7 {
            0xffbf66
        } else {
            0xff944f
        };
        fill(
            ui,
            x + sx + (radius - half) * TEXEL,
            y + sy + TEXEL * ry,
            half * 2.0 * TEXEL,
            TEXEL,
            hex(rgb, 255),
        );
    }
    let far_width: usize = FAR.runs.iter().map(|r| r.0).sum();
    skyline(ui, r, &FAR, 0x3a1f5e, -(far_width as f32 * TEXEL) / 2.0);
    skyline(ui, r, &NEAR, 0x150f2e, 0.0);
}

/// The short band at the top of the other pages: the night stops only, stars, and
/// the near skyline in the page colour so it melts into the page.
pub(super) fn band(ui: &mut Ui, r: [f32; 4], t: f32) {
    let [x, y, w, h] = r;
    for (top, height, rgb) in bands(&STOPS[..4], h) {
        let height = height.min(h - top);
        fill(ui, x, y + top, w, height, hex(rgb, 255));
    }
    draw_stars(ui, r, t, 0.75);
    skyline(ui, r, &NEAR, 0x0e0b26, 0.0);
}

fn draw_stars(ui: &mut Ui, [x, y, w, h]: [f32; 4], t: f32, depth: f32) {
    for star in stars() {
        let px = star.x * TEXEL;
        if px >= w {
            continue;
        }
        let py = (star.y * h * depth).round();
        let period = STAR_PERIODS[star.group];
        let phase = (t / period + star.group as f32 * 0.25).fract();
        let level = STAR_LEVELS[((phase * 4.0) as usize).min(3)];
        let size = if star.big { 2.0 } else { 1.0 };
        fill(
            ui,
            x + px,
            y + py,
            size,
            size,
            hex(0xf4f2ff, (level * 255.0) as u8),
        );
    }
}

fn clipped_stamp(
    ui: &mut Ui,
    [x, y]: [f32; 2],
    ox: f32,
    oy: f32,
    w: f32,
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
            if let Some(rgb) = paint(chars[start]) {
                let left = (ox + start as f32 * TEXEL).max(0.0);
                let right = (ox + end as f32 * TEXEL).min(w);
                if right > left {
                    fill(
                        ui,
                        x + left,
                        y + oy + row as f32 * TEXEL,
                        right - left,
                        TEXEL,
                        hex(rgb, 255),
                    );
                }
            }
            start = end;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn landscape_bands_match_the_mockup() {
        let bands = super::bands(&super::STOPS, 380.0);
        assert_eq!(bands[0], (0.0, 20.0, 0x07061a));
        assert_eq!(bands[1], (20.0, 20.0, 0x0a0822));
        assert_eq!(bands.last().unwrap(), &(320.0, 60.0, 0xf08a5d));
    }
    #[test]
    fn the_star_field_keeps_155_stars() {
        assert_eq!(super::stars().len(), 155);
        assert_eq!(super::stars().iter().filter(|s| s.big).count(), 8);
    }
}
