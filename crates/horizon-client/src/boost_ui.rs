//! Horizon Boost's screen: presets, quality, animations, details and performance in one
//! touch-friendly sheet, drawn with the «Горизонт» kit over the game's options. It edits
//! the settings the game already has (vanilla's options, Horizon's graphics settings and
//! `boost::Boost`); the game saves and applies them after each change.
use crate::graphics::Preset;
use crate::hud::{HudAtlas, HudBatch};
use crate::pixel_kit::{self as kit, Kit, Style, col};
use crate::settings::Settings;
use crate::ui_theme::{Face, Ui};

/// A Boost string in the game's language.
fn t(key: &str) -> String {
    crate::lang::get(&format!("horizon.boost.{key}"))
}
fn tr(key: &str, args: &[&dyn std::fmt::Display]) -> String {
    crate::lang::tr(&format!("horizon.boost.{key}"), args)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Presets,
    Quality,
    Animations,
    Details,
    Performance,
    Heat,
}
impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Presets,
        Tab::Quality,
        Tab::Animations,
        Tab::Details,
        Tab::Performance,
        Tab::Heat,
    ];
    fn key(self) -> &'static str {
        match self {
            Tab::Presets => "presets",
            Tab::Quality => "quality",
            Tab::Animations => "animations",
            Tab::Details => "details",
            Tab::Performance => "performance",
            Tab::Heat => "heat",
        }
    }
    fn knobs(self) -> &'static [Knob] {
        use Knob::*;
        match self {
            Tab::Presets => &[],
            Tab::Quality => &[
                RenderDistance,
                EntityDistance,
                Particles,
                Clouds,
                Weather,
                WeatherRadius,
                WeatherDensity,
                RainSplashes,
                Leaves,
                BiomeBlend,
                Mipmaps,
            ],
            Tab::Animations => &[
                AnimateAll,
                AnimateWater,
                AnimateLava,
                AnimateFire,
                AnimatePortal,
                AnimateOther,
            ],
            Tab::Details => &[Sky, Stars, SunMoon, Fog, FogStart, Vignette, EntityShadows],
            Tab::Performance => &[
                MaxFps,
                ResolutionScale,
                ChunkUpdates,
                Workers,
                Overlay,
                BatteryMode,
            ],
            Tab::Heat => &[Thermal, ThermalMinFps, ThermalResolution, ThermalReadout],
        }
    }
}

/// One setting on the sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Knob {
    RenderDistance,
    EntityDistance,
    Particles,
    Clouds,
    Weather,
    WeatherRadius,
    WeatherDensity,
    RainSplashes,
    Leaves,
    BiomeBlend,
    Mipmaps,
    AnimateAll,
    AnimateWater,
    AnimateLava,
    AnimateFire,
    AnimatePortal,
    AnimateOther,
    Sky,
    Stars,
    SunMoon,
    Fog,
    FogStart,
    Vignette,
    EntityShadows,
    MaxFps,
    ResolutionScale,
    ChunkUpdates,
    Workers,
    Thermal,
    ThermalMinFps,
    ThermalResolution,
    ThermalReadout,
    Overlay,
    BatteryMode,
}

/// How a knob is changed: a switch, minus and plus, or a tap that cycles.
enum Control {
    Switch(bool),
    Stepper,
    Cycle,
}

const MAX_FPS_STEPS: [u32; 12] = [20, 30, 40, 45, 60, 72, 90, 120, 144, 165, 240, 260];
const SCALE_STEPS: [u32; 12] = [50, 60, 67, 70, 75, 80, 85, 90, 100, 125, 150, 200];
const CHUNK_UPDATE_STEPS: [u32; 7] = [0, 1, 2, 4, 8, 16, 32];
const ENTITY_STEPS: [f32; 8] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 5.0];

fn step_in<T: Copy + PartialOrd>(steps: &[T], now: T, up: bool) -> T {
    let i = steps
        .iter()
        .position(|&s| s >= now)
        .unwrap_or(steps.len() - 1);
    // A value between two steps goes to the step on its side.
    let at = steps[i] == now;
    let next = if up {
        if at { i + 1 } else { i }
    } else {
        i.saturating_sub(1)
    };
    steps[next.min(steps.len() - 1)]
}
fn cycle<T: Copy + PartialEq>(all: &[T], now: T) -> T {
    let i = all.iter().position(|x| *x == now).unwrap_or(0);
    all[(i + 1) % all.len()]
}
fn workers_max() -> u32 {
    std::thread::available_parallelism().map_or(8, |n| n.get() as u32)
}

impl Knob {
    fn key(self) -> &'static str {
        use Knob::*;
        match self {
            RenderDistance => "render_distance",
            EntityDistance => "entity_distance",
            Particles => "particles",
            Clouds => "clouds",
            Weather => "weather",
            WeatherRadius => "weather_radius",
            WeatherDensity => "weather_density",
            RainSplashes => "rain_splashes",
            Leaves => "leaves",
            BiomeBlend => "biome_blend",
            Mipmaps => "mipmaps",
            AnimateAll => "animate_all",
            AnimateWater => "animate_water",
            AnimateLava => "animate_lava",
            AnimateFire => "animate_fire",
            AnimatePortal => "animate_portal",
            AnimateOther => "animate_other",
            Sky => "sky",
            Stars => "stars",
            SunMoon => "sun_moon",
            Fog => "fog",
            FogStart => "fog_start",
            Vignette => "vignette",
            EntityShadows => "entity_shadows",
            MaxFps => "max_fps",
            ResolutionScale => "resolution_scale",
            ChunkUpdates => "chunk_updates",
            Workers => "workers",
            Thermal => "thermal",
            ThermalMinFps => "thermal_min_fps",
            ThermalResolution => "thermal_resolution",
            ThermalReadout => "thermal_readout",
            Overlay => "overlay",
            BatteryMode => "battery_mode",
        }
    }
    fn control(self, s: &Settings) -> Control {
        use Knob::*;
        let g = &s.horizon;
        let b = &g.boost;
        match self {
            RenderDistance | EntityDistance | WeatherRadius | BiomeBlend | Mipmaps | MaxFps
            | ResolutionScale | ChunkUpdates | Workers => Control::Stepper,
            Particles | Clouds | WeatherDensity | RainSplashes | FogStart | Overlay | Thermal
            | ThermalMinFps => Control::Cycle,
            Weather => Control::Switch(g.weather),
            Leaves => Control::Switch(s.cutout_leaves),
            AnimateAll => Control::Switch(g.animated_textures),
            AnimateWater => Control::Switch(b.animate_water),
            AnimateLava => Control::Switch(b.animate_lava),
            AnimateFire => Control::Switch(b.animate_fire),
            AnimatePortal => Control::Switch(b.animate_portal),
            AnimateOther => Control::Switch(b.animate_other),
            Sky => Control::Switch(g.sky),
            Stars => Control::Switch(b.stars),
            SunMoon => Control::Switch(b.sun_moon),
            Fog => Control::Switch(g.fog),
            Vignette => Control::Switch(s.vignette),
            EntityShadows => Control::Switch(s.entity_shadows),
            ThermalResolution => Control::Switch(b.thermal_resolution),
            ThermalReadout => Control::Switch(b.thermal_readout),
            BatteryMode => Control::Switch(g.battery),
        }
    }
    /// The value shown beside a stepper or cycling control.
    fn value(self, s: &Settings) -> String {
        use Knob::*;
        let g = &s.horizon;
        let b = &g.boost;
        let percent = |p: u32| format!("{p}%");
        match self {
            RenderDistance => tr("value.chunks", &[&s.render_distance]),
            EntityDistance => percent((s.entity_distance * 100.0).round() as u32),
            Particles => t(match s.particles {
                0 => "value.particles.all",
                1 => "value.particles.decreased",
                _ => "value.particles.minimal",
            }),
            Clouds => t(match s.clouds.as_str() {
                "true" => "value.clouds.fancy",
                "fast" => "value.clouds.fast",
                _ => "value.off",
            }),
            WeatherRadius => tr("value.blocks", &[&s.weather_radius]),
            WeatherDensity => percent(b.weather_density),
            RainSplashes => match b.rain_splashes {
                0 => t("value.off"),
                p => percent(p),
            },
            BiomeBlend => match s.biome_blend {
                0 => t("value.off"),
                r => {
                    let side = 2 * r + 1;
                    format!("{side}×{side}")
                }
            },
            Mipmaps => match s.mipmap_levels {
                0 => t("value.off"),
                n => n.to_string(),
            },
            FogStart => match b.fog_start {
                0 => t("value.vanilla"),
                p => percent(p),
            },
            MaxFps => match s.max_fps {
                260.. => t("value.unlimited"),
                n => tr("value.fps", &[&n]),
            },
            ResolutionScale => percent(g.resolution_scale),
            ChunkUpdates => match g.chunk_updates {
                0 => t("value.unlimited"),
                n => n.to_string(),
            },
            Workers => match g.workers {
                0 => tr("value.auto", &[&g.worker_threads()]),
                n => n.to_string(),
            },
            Thermal => t(&format!("value.heat.{}", b.thermal.key())),
            ThermalMinFps => tr("value.fps", &[&b.thermal_min_fps]),
            Overlay => t(match g.overlay {
                crate::graphics::Overlay::Off => "value.off",
                crate::graphics::Overlay::Fps => "value.overlay.fps",
                crate::graphics::Overlay::Graph => "value.overlay.graph",
            }),
            _ => String::new(),
        }
    }
    /// Greyed out while another setting makes it do nothing.
    fn inactive(self, s: &Settings) -> bool {
        use Knob::*;
        let g = &s.horizon;
        match self {
            AnimateWater | AnimateLava | AnimateFire | AnimatePortal | AnimateOther => {
                !g.animated_textures
            }
            Stars | SunMoon => !g.sky,
            FogStart => !g.fog,
            WeatherRadius | WeatherDensity | RainSplashes => !g.weather,
            ThermalReadout => g.overlay == crate::graphics::Overlay::Off,
            ThermalMinFps | ThermalResolution => g.boost.thermal == crate::boost::Heat::Off,
            _ => false,
        }
    }
    /// Whether changing it makes the preset Custom (the quality settings presets set).
    fn presets_it(self) -> bool {
        use Knob::*;
        !matches!(
            self,
            MaxFps
                | Workers
                | Thermal
                | ThermalMinFps
                | ThermalResolution
                | ThermalReadout
                | Overlay
                | BatteryMode
        )
    }
    /// A tap on a switch or cycling control, or minus (`up` false) / plus.
    fn change(self, s: &mut Settings, up: bool) {
        use Knob::*;
        let g = &mut s.horizon;
        let b = &mut g.boost;
        match self {
            RenderDistance => {
                s.render_distance = (s.render_distance + if up { 1 } else { -1 })
                    .clamp(2, crate::options_file::MAX_RENDER_DISTANCE.min(32))
            }
            EntityDistance => s.entity_distance = step_in(&ENTITY_STEPS, s.entity_distance, up),
            Particles => s.particles = (s.particles + 1) % 3,
            Clouds => {
                s.clouds = match s.clouds.as_str() {
                    "true" => "fast",
                    "fast" => "false",
                    _ => "true",
                }
                .into()
            }
            Weather => g.weather = !g.weather,
            WeatherRadius => {
                s.weather_radius = (s.weather_radius + if up { 1 } else { -1 }).clamp(3, 10)
            }
            WeatherDensity => {
                b.weather_density = cycle(&crate::boost::DENSITIES, b.weather_density)
            }
            RainSplashes => b.rain_splashes = cycle(&crate::boost::SPLASHES, b.rain_splashes),
            Leaves => s.cutout_leaves = !s.cutout_leaves,
            BiomeBlend => s.biome_blend = (s.biome_blend + if up { 1 } else { -1 }).clamp(0, 7),
            Mipmaps => s.mipmap_levels = (s.mipmap_levels + if up { 1 } else { -1 }).clamp(0, 4),
            AnimateAll => g.animated_textures = !g.animated_textures,
            AnimateWater => b.animate_water = !b.animate_water,
            AnimateLava => b.animate_lava = !b.animate_lava,
            AnimateFire => b.animate_fire = !b.animate_fire,
            AnimatePortal => b.animate_portal = !b.animate_portal,
            AnimateOther => b.animate_other = !b.animate_other,
            Sky => g.sky = !g.sky,
            Stars => b.stars = !b.stars,
            SunMoon => b.sun_moon = !b.sun_moon,
            Fog => g.fog = !g.fog,
            FogStart => b.fog_start = cycle(&crate::boost::FOG_STARTS, b.fog_start),
            Vignette => s.vignette = !s.vignette,
            EntityShadows => s.entity_shadows = !s.entity_shadows,
            MaxFps => s.max_fps = step_in(&MAX_FPS_STEPS, s.max_fps, up),
            ResolutionScale => g.resolution_scale = step_in(&SCALE_STEPS, g.resolution_scale, up),
            ChunkUpdates => g.chunk_updates = step_in(&CHUNK_UPDATE_STEPS, g.chunk_updates, up),
            Workers => {
                g.workers = if up {
                    (g.workers + 1).min(workers_max())
                } else {
                    g.workers.saturating_sub(1)
                }
            }
            Thermal => b.thermal = cycle(&crate::boost::Heat::ALL, b.thermal),
            ThermalMinFps => {
                b.thermal_min_fps = cycle(&crate::boost::THERMAL_MIN_FPS, b.thermal_min_fps)
            }
            ThermalResolution => b.thermal_resolution = !b.thermal_resolution,
            ThermalReadout => b.thermal_readout = !b.thermal_readout,
            Overlay => {
                g.overlay = cycle(
                    &[
                        crate::graphics::Overlay::Off,
                        crate::graphics::Overlay::Fps,
                        crate::graphics::Overlay::Graph,
                    ],
                    g.overlay,
                )
            }
            BatteryMode => g.battery = !g.battery,
        }
        if self.presets_it() {
            g.preset = Preset::Custom;
            s.graphics_preset = "custom".into();
        }
    }
}

/// What a tap does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Tab(Tab),
    Close,
    Preset(Preset),
    Change(Knob, bool),
}

/// What the game must do after input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// A setting changed: save and apply.
    Changed,
    /// A control was tapped that changed nothing (a tab): a click sound.
    Clicked,
    Close,
}

/// The live figures at the top of the sheet.
#[derive(Clone, Debug, Default)]
pub struct Readout {
    pub fps: f32,
    pub frame_ms: f32,
    pub gpu_ms: Option<f32>,
    /// The thermal line (`Game::thermal_line`), when the system reports one.
    pub thermal: Option<String>,
    /// The governor's level, 0 when it changes nothing.
    pub thermal_level: usize,
}

/// The open sheet's state.
pub struct BoostUi {
    pub tab: Tab,
    scroll: f32,
    scroll_max: f32,
    /// The list's viewport in dp, as last drawn.
    viewport: Option<[f32; 4]>,
    targets: Vec<([f32; 4], Act)>,
    /// The pointer that is down: where it went down, where it is, the scroll then.
    press: Option<([f32; 2], [f32; 2], f32)>,
    moved: bool,
    /// dp per physical pixel, as last drawn.
    density: f32,
}
impl Default for BoostUi {
    fn default() -> Self {
        Self {
            tab: Tab::Presets,
            scroll: 0.0,
            scroll_max: 0.0,
            viewport: None,
            targets: Vec::new(),
            press: None,
            moved: false,
            density: 1.0,
        }
    }
}

/// A move past this many dp is a drag (the list scrolls), not a tap.
const SLOP: f32 = 10.0;

impl BoostUi {
    fn dp(&self, px: [f32; 2]) -> [f32; 2] {
        [px[0] / self.density, px[1] / self.density]
    }
    /// A finger or the mouse went down at `px` (physical pixels).
    pub fn pointer_down(&mut self, px: [f32; 2]) {
        let p = self.dp(px);
        self.press = Some((p, p, self.scroll));
        self.moved = false;
    }
    /// It moved: past the slop, inside the list, it scrolls the list.
    pub fn pointer_move(&mut self, px: [f32; 2]) {
        let p = self.dp(px);
        let viewport = self.viewport;
        let Some((start, now, scroll)) = self.press.as_mut() else {
            return;
        };
        *now = p;
        if (p[0] - start[0]).hypot(p[1] - start[1]) > SLOP {
            self.moved = true;
        }
        let in_list = viewport.is_some_and(|[x, y, w, h]| {
            start[0] >= x && start[0] <= x + w && start[1] >= y && start[1] <= y + h
        });
        if self.moved && in_list {
            self.scroll = (*scroll - (p[1] - start[1])).clamp(0.0, self.scroll_max);
        }
    }
    /// It went up: a tap (no drag) runs what is under it.
    pub fn pointer_up(&mut self, px: [f32; 2], s: &mut Settings) -> Outcome {
        let p = self.dp(px);
        let Some(_) = self.press.take() else {
            return Outcome::None;
        };
        if self.moved {
            return Outcome::None;
        }
        let Some(act) = self
            .targets
            .iter()
            .rev()
            .find(|([x, y, w, h], _)| p[0] >= *x && p[0] <= x + w && p[1] >= *y && p[1] <= y + h)
            .map(|(_, a)| *a)
        else {
            return Outcome::None;
        };
        self.act(act, s)
    }
    /// The pointer went away (a cancelled touch): nothing is tapped.
    pub fn cancel(&mut self) {
        self.press = None;
        self.moved = false;
    }
    /// The mouse wheel: `lines` up (positive) or down.
    pub fn wheel(&mut self, lines: f32) {
        self.scroll = (self.scroll - lines * 40.0).clamp(0.0, self.scroll_max);
    }
    pub fn act(&mut self, act: Act, s: &mut Settings) -> Outcome {
        match act {
            Act::Close => Outcome::Close,
            Act::Tab(tab) => {
                if self.tab != tab {
                    self.tab = tab;
                    self.scroll = 0.0;
                }
                Outcome::Clicked
            }
            Act::Preset(preset) => {
                crate::graphics::apply_preset(preset, s);
                s.horizon.preset = preset;
                Outcome::Changed
            }
            Act::Change(knob, up) => {
                if knob.inactive(s) {
                    return Outcome::None;
                }
                knob.change(s, up);
                Outcome::Changed
            }
        }
    }
}

/// Draws the sheet over everything else in `batch`.
pub fn draw(
    boost: &mut BoostUi,
    s: &Settings,
    readout: &Readout,
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    density: f32,
) {
    let mut ui = Ui::launcher(atlas, batch, (0.0, 0.0), density);
    boost.density = ui.density;
    let mut targets = std::mem::take(&mut boost.targets);
    targets.clear();
    let press = boost.press.filter(|_| !boost.moved).map(|(_, p, _)| p);
    {
        let mut k = Kit {
            ui: &mut ui,
            targets: &mut targets,
            press,
            flash: None,
        };
        sheet(&mut k, boost, s, readout);
    }
    boost.targets = targets;
    ui.finish();
}

fn sheet(k: &mut Kit<Act>, boost: &mut BoostUi, s: &Settings, readout: &Readout) {
    let (w, h) = (k.ui.w, k.ui.h);
    kit::fill(k.ui, 0.0, 0.0, w, h, col::SCRIM);
    let margin = if w < 520.0 { 8.0 } else { 16.0 };
    let pw = (w - 2.0 * margin).min(820.0);
    let px = ((w - pw) / 2.0).floor();
    let (py, ph) = (margin, h - 2.0 * margin);
    k.panel([px, py, pw, ph], false);
    let pad = 16.0;
    let (ix, iw) = (px + pad, pw - 2.0 * pad);
    // Header: the mark, the title, Done.
    let mut y = py + 14.0;
    k.ui.icon("zap", ix, y + 2.0, 24.0, col::ACCENT_HI);
    k.text(
        Face::BodyBold,
        20.0,
        28.0,
        ix + 32.0,
        y,
        col::INK,
        &t("title"),
    );
    let done = t("done");
    let bw = k.button_width(Style::Secondary, None, &done).max(96.0);
    k.button(
        [ix + iw - bw, y - 4.0, bw, 40.0],
        Style::Secondary,
        None,
        &done,
        Some(Act::Close),
    );
    y += 32.0;
    // The live readout.
    let mut line = tr("readout.fps", &[&format!("{:.0}", readout.fps)]);
    line.push_str(" · ");
    line.push_str(&tr("readout.frame", &[&format!("{:.1}", readout.frame_ms)]));
    if let Some(gpu) = readout.gpu_ms {
        line.push_str(" · ");
        line.push_str(&tr("readout.gpu", &[&format!("{gpu:.1}")]));
    }
    line.push_str(" · ");
    line.push_str(
        &readout
            .thermal
            .clone()
            .unwrap_or_else(|| t("thermal.unavailable")),
    );
    let dot = match readout.thermal_level {
        0 => col::SUCCESS,
        1 => col::ACCENT_HI,
        _ => col::DANGER,
    };
    kit::fill(k.ui, ix, y + 5.0, 8.0, 8.0, dot);
    let lines = k.wrap(Face::Body, 12.0, &line, iw - 16.0);
    for (i, l) in lines.iter().enumerate() {
        k.text(
            Face::Body,
            12.0,
            16.0,
            ix + 16.0,
            y + i as f32 * 16.0,
            col::INK_SOFT,
            l,
        );
    }
    y += lines.len() as f32 * 16.0 + 8.0;
    tabs(k, boost.tab, [ix, y, iw, 44.0]);
    y += 52.0;
    // The list.
    let viewport = [px + 4.0, y, pw - 8.0, py + ph - 8.0 - y];
    boost.viewport = Some(viewport);
    let mark = k.mark();
    let top = y - boost.scroll;
    let content = match boost.tab {
        Tab::Presets => presets(k, s, ix, top, iw),
        tab => knobs(k, s, tab, ix, top, iw),
    };
    k.clip(mark, viewport);
    boost.scroll_max = (content - viewport[3]).max(0.0);
    boost.scroll = boost.scroll.min(boost.scroll_max);
    // A scroll bar when the list is longer than its viewport.
    if boost.scroll_max > 0.0 {
        let track = viewport[3];
        let thumb = (track * track / content).max(24.0);
        let ty = viewport[1] + (track - thumb) * boost.scroll / boost.scroll_max;
        kit::fill(k.ui, px + pw - 7.0, ty, 3.0, thumb, col::CONTROL_RING);
    }
}

/// The tab bar: equal segments; a label too long for its segment takes a smaller size
/// and a second line rather than an ellipsis.
fn tabs(k: &mut Kit<Act>, current: Tab, r: [f32; 4]) {
    let [x, y, w, h] = r;
    kit::plate(k.ui, r, 1, col::SIDE, col::PLATE_RING);
    let tw = w / Tab::ALL.len() as f32;
    for (i, &tab) in Tab::ALL.iter().enumerate() {
        let tr = [x + i as f32 * tw, y, tw, h];
        let selected = tab == current;
        let act = Act::Tab(tab);
        if selected {
            kit::stepped(k.ui, tr, 1, 4.0, col::INK);
        } else if k.pressed(tr, Some(act)) {
            kit::stepped(k.ui, tr, 1, 4.0, col::CONTROL);
        }
        let color = if selected { col::PLATE } else { col::INK_SOFT };
        let label = t(&format!("tab.{}", tab.key()));
        let room = tw - 8.0;
        let (size, line) = if k.measure(Face::BodySemiBold, 14.0, &label) <= room {
            (14.0, 17.0)
        } else {
            (12.0, 14.0)
        };
        let lines = k.wrap(Face::BodySemiBold, size, &label, room);
        let top = y + (h - lines.len() as f32 * line) / 2.0;
        for (j, l) in lines.iter().enumerate() {
            let lw = k.measure(Face::BodySemiBold, size, l);
            k.text(
                Face::BodySemiBold,
                size,
                line,
                tr[0] + (tw - lw) / 2.0,
                top + j as f32 * line,
                color,
                l,
            );
        }
        k.hit(tr, act);
    }
}

/// Text lines of a row's description: whole, never cut short.
fn description(k: &Kit<Act>, text: &str, width: f32) -> Vec<String> {
    k.wrap(Face::Body, 12.0, text, width)
        .into_iter()
        .filter(|l| !l.is_empty())
        .collect()
}

/// The preset cards; returns the content height.
fn presets(k: &mut Kit<Act>, s: &Settings, x: f32, top: f32, w: f32) -> f32 {
    let mut y = top;
    let current = s.horizon.preset;
    for (preset, key) in [
        (Preset::Performance, "performance"),
        (Preset::Balanced, "balanced"),
        (Preset::Quality, "quality"),
        (Preset::Custom, "custom"),
    ] {
        let button = t("preset.apply");
        let bw = k.button_width(Style::Primary, None, &button).max(112.0);
        let text_w = w - bw - 16.0;
        let title = t(&format!("preset.{key}"));
        let lines = description(k, &t(&format!("preset.{key}.desc")), text_w);
        let height = (12.0 + 20.0 + 4.0 + lines.len() as f32 * 16.0 + 12.0).max(64.0);
        let active = current == preset;
        kit::plate(
            k.ui,
            [x, y, w, height - 8.0],
            1,
            if active { col::CONTROL } else { col::SIDE },
            if active { col::ACCENT } else { col::PLATE_RING },
        );
        k.text(
            Face::BodySemiBold,
            15.0,
            20.0,
            x + 12.0,
            y + 10.0,
            col::INK,
            &title,
        );
        for (i, l) in lines.iter().enumerate() {
            k.text(
                Face::Body,
                12.0,
                16.0,
                x + 12.0,
                y + 34.0 + i as f32 * 16.0,
                col::MUTED,
                l,
            );
        }
        let by = y + (height - 8.0 - 40.0) / 2.0;
        if active {
            let label = t("preset.active");
            let lw = k.measure(Face::BodySemiBold, 14.0, &label);
            k.ui.icon(
                "check",
                x + w - 12.0 - lw - 28.0,
                by + 8.0,
                24.0,
                col::SUCCESS,
            );
            k.text(
                Face::BodySemiBold,
                14.0,
                40.0,
                x + w - 12.0 - lw,
                by,
                col::SUCCESS,
                &label,
            );
        } else if preset != Preset::Custom {
            k.button(
                [x + w - bw - 8.0, by, bw, 40.0],
                Style::Primary,
                None,
                &button,
                Some(Act::Preset(preset)),
            );
        }
        y += height;
    }
    // What stays automatic, so players don't look for it.
    let note = description(k, &t("note.smart"), w);
    for (i, l) in note.iter().enumerate() {
        k.text(
            Face::Body,
            12.0,
            16.0,
            x,
            y + 4.0 + i as f32 * 16.0,
            col::PLACEHOLDER,
            l,
        );
    }
    y += note.len() as f32 * 16.0 + 12.0;
    y - top
}

/// A tab's rows; returns the content height.
fn knobs(k: &mut Kit<Act>, s: &Settings, tab: Tab, x: f32, top: f32, w: f32) -> f32 {
    let control_w = 196.0f32.min(w * 0.42);
    let text_w = w - control_w - 16.0;
    let mut y = top;
    for &knob in tab.knobs() {
        let inactive = knob.inactive(s);
        let title = t(&format!("knob.{}", knob.key()));
        let lines = description(k, &t(&format!("knob.{}.desc", knob.key())), text_w);
        let title_lines = k.wrap(Face::BodySemiBold, 15.0, &title, text_w);
        let text_h = title_lines.len() as f32 * 19.0 + 4.0 + lines.len() as f32 * 16.0;
        let height = (text_h + 20.0).max(60.0);
        let ink = if inactive { col::PLACEHOLDER } else { col::INK };
        for (i, l) in title_lines.iter().enumerate() {
            k.text(
                Face::BodySemiBold,
                15.0,
                19.0,
                x,
                y + 10.0 + i as f32 * 19.0,
                ink,
                l,
            );
        }
        let dy = y + 10.0 + title_lines.len() as f32 * 19.0 + 4.0;
        for (i, l) in lines.iter().enumerate() {
            k.text(
                Face::Body,
                12.0,
                16.0,
                x,
                dy + i as f32 * 16.0,
                col::MUTED,
                l,
            );
        }
        let cx = x + w - control_w;
        let cy = y + (height - 40.0) / 2.0;
        let act = |up| (!inactive).then_some(Act::Change(knob, up));
        match knob.control(s) {
            Control::Switch(on) => {
                if inactive {
                    kit::stepped(
                        k.ui,
                        [x + w - 60.0, cy + 2.0, 60.0, 36.0],
                        1,
                        4.0,
                        col::SWITCH_OFF,
                    );
                } else {
                    k.switch(x + w - 60.0, cy + 2.0, on, Act::Change(knob, true));
                }
            }
            Control::Stepper => {
                let style = if inactive {
                    Style::Disabled
                } else {
                    Style::Secondary
                };
                k.button([cx, cy, 44.0, 40.0], style, None, "−", act(false));
                k.button(
                    [cx + control_w - 44.0, cy, 44.0, 40.0],
                    style,
                    None,
                    "+",
                    act(true),
                );
                let value = knob.value(s);
                let vw = control_w - 96.0;
                let lines = k.wrap(Face::BodySemiBold, 14.0, &value, vw);
                let vh = lines.len() as f32 * 17.0;
                for (i, l) in lines.iter().enumerate() {
                    let lw = k.measure(Face::BodySemiBold, 14.0, l);
                    k.text(
                        Face::BodySemiBold,
                        14.0,
                        17.0,
                        cx + 48.0 + (vw - lw) / 2.0,
                        cy + (40.0 - vh) / 2.0 + i as f32 * 17.0,
                        ink,
                        l,
                    );
                }
            }
            Control::Cycle => {
                let value = knob.value(s);
                if inactive {
                    kit::plate(
                        k.ui,
                        [cx, cy, control_w, 40.0],
                        1,
                        col::SIDE,
                        col::PLATE_RING,
                    );
                    k.text(
                        Face::BodySemiBold,
                        14.0,
                        40.0,
                        cx + 14.0,
                        cy,
                        col::PLACEHOLDER,
                        &value,
                    );
                } else {
                    k.dropdown(
                        [cx, cy, control_w, 40.0],
                        None,
                        &value,
                        Act::Change(knob, true),
                    );
                }
            }
        }
        y += height;
        kit::fill(k.ui, x, y - 1.0, w, 1.0, col::RULE);
    }
    y - top + 8.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steppers_move_through_their_steps_and_stop_at_the_ends() {
        let mut s = Settings::default();
        s.max_fps = 60;
        Knob::MaxFps.change(&mut s, true);
        assert_eq!(s.max_fps, 72);
        Knob::MaxFps.change(&mut s, false);
        Knob::MaxFps.change(&mut s, false);
        assert_eq!(s.max_fps, 45);
        // A value between steps goes to the next step on that side.
        s.max_fps = 100;
        Knob::MaxFps.change(&mut s, true);
        assert_eq!(s.max_fps, 120);
        s.max_fps = 100;
        Knob::MaxFps.change(&mut s, false);
        assert_eq!(s.max_fps, 90);
        s.max_fps = 260;
        Knob::MaxFps.change(&mut s, true);
        assert_eq!(s.max_fps, 260);
        s.weather_radius = 3;
        Knob::WeatherRadius.change(&mut s, false);
        assert_eq!(s.weather_radius, 3);
        s.horizon.resolution_scale = 50;
        Knob::ResolutionScale.change(&mut s, false);
        assert_eq!(s.horizon.resolution_scale, 50);
    }

    #[test]
    fn quality_changes_make_the_preset_custom_and_performance_ones_do_not() {
        let mut s = Settings::default();
        crate::graphics::apply_preset(Preset::Balanced, &mut s);
        s.horizon.preset = Preset::Balanced;
        Knob::MaxFps.change(&mut s, true);
        Knob::Thermal.change(&mut s, true);
        assert_eq!(s.horizon.preset, Preset::Balanced);
        Knob::Stars.change(&mut s, true);
        assert_eq!(s.horizon.preset, Preset::Custom);
    }

    #[test]
    fn inactive_knobs_ignore_taps() {
        let mut s = Settings::default();
        s.horizon.animated_textures = false;
        let mut ui = BoostUi::default();
        assert_eq!(
            ui.act(Act::Change(Knob::AnimateWater, true), &mut s),
            Outcome::None
        );
        assert!(s.horizon.boost.animate_water);
        assert_eq!(
            ui.act(Act::Change(Knob::AnimateAll, true), &mut s),
            Outcome::Changed
        );
        assert!(s.horizon.animated_textures);
    }

    #[test]
    fn a_drag_scrolls_and_does_not_tap() {
        let mut s = Settings::default();
        let mut ui = BoostUi {
            viewport: Some([0.0, 100.0, 400.0, 200.0]),
            scroll_max: 500.0,
            targets: vec![([0.0, 100.0, 400.0, 60.0], Act::Change(Knob::Stars, true))],
            ..BoostUi::default()
        };
        ui.pointer_down([50.0, 150.0]);
        ui.pointer_move([50.0, 90.0]);
        assert_eq!(ui.pointer_up([50.0, 90.0], &mut s), Outcome::None);
        assert_eq!(ui.scroll, 60.0);
        assert!(s.horizon.boost.stars);
        // A tap on the row changes it.
        ui.pointer_down([50.0, 120.0]);
        assert_eq!(ui.pointer_up([52.0, 121.0], &mut s), Outcome::Changed);
        assert!(!s.horizon.boost.stars);
    }

    #[test]
    fn every_string_the_sheet_shows_is_translated() {
        let en: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(include_str!("../assets/horizon/lang/en_us.json")).unwrap();
        let mut keys: Vec<String> = [
            "title",
            "done",
            "readout.fps",
            "readout.frame",
            "readout.gpu",
            "thermal.unavailable",
            "thermal.line",
            "thermal.capped",
            "preset.apply",
            "preset.active",
            "note.smart",
            "value.on",
            "value.off",
            "value.vanilla",
            "value.unlimited",
            "value.fps",
            "value.chunks",
            "value.blocks",
            "value.auto",
            "value.particles.all",
            "value.particles.decreased",
            "value.particles.minimal",
            "value.clouds.fancy",
            "value.clouds.fast",
            "value.overlay.fps",
            "value.overlay.graph",
            "value.heat.off",
            "value.heat.soft",
            "value.heat.balanced",
            "value.heat.strong",
        ]
        .iter()
        .map(|k| k.to_string())
        .collect();
        keys.extend((0..=6).map(|i| format!("thermal.status.{i}")));
        for tab in Tab::ALL {
            keys.push(format!("tab.{}", tab.key()));
            for knob in tab.knobs() {
                keys.push(format!("knob.{}", knob.key()));
                keys.push(format!("knob.{}.desc", knob.key()));
            }
        }
        for preset in ["performance", "balanced", "quality", "custom"] {
            keys.push(format!("preset.{preset}"));
            keys.push(format!("preset.{preset}.desc"));
        }
        for key in keys {
            let key = format!("horizon.boost.{key}");
            let text = en.get(&key).and_then(|v| v.as_str());
            assert!(
                text.is_some_and(|t| !t.is_empty()),
                "{key} has no English text"
            );
            assert!(!text.unwrap().contains('…'), "{key} is cut short");
        }
    }
}
