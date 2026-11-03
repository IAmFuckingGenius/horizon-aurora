//! Horizon Boost: the speed settings Horizon adds beside vanilla's and its own graphics
//! settings, and the thermal governor that eases off while a phone is hot.
//!
//! Stored in `horizon-graphics.txt` with the rest of [`crate::graphics::Graphics`]. Every
//! default draws what vanilla draws: rain at full density, every splash, sun, moon and
//! stars, vanilla's fog, every texture animation. The thermal governor starts in its
//! soft mode, which acts only once Android reports severe heat; the player can turn it
//! off, make it act sooner, and set the lowest frame cap it may impose.
use std::time::{Duration, Instant};

/// The Boost settings (`Graphics::boost`).
#[derive(Clone, Debug, PartialEq)]
pub struct Boost {
    /// Share of rain and snow columns drawn, in percent: 100 (vanilla), 50 or 25.
    pub weather_density: u32,
    /// Share of rain splashes spawned, in percent: 100 (vanilla), 50 or 0. The rain's
    /// sound still plays with none.
    pub rain_splashes: u32,
    /// Stars at night (the sky's other parts follow `Graphics::sky`).
    pub stars: bool,
    /// The sun and the moon.
    pub sun_moon: bool,
    /// Where the fog at the edge of the render distance starts, in percent of that
    /// distance; 0 is vanilla's (the last tenth, 4 to 64 blocks).
    pub fog_start: u32,
    pub animate_water: bool,
    pub animate_lava: bool,
    pub animate_fire: bool,
    pub animate_portal: bool,
    /// Every other animated block texture (sea lanterns, prismarine, magma, kelp...).
    pub animate_other: bool,
    /// How readily the frame cap, then the resolution and chunk uploads, go down
    /// while the system reports thermal pressure ([`Governor`]); back once it has
    /// cooled down. [`Heat::Off`] never limits anything.
    pub thermal: Heat,
    /// The lowest frame cap the governor may impose, in frames per second.
    pub thermal_min_fps: u32,
    /// Whether the governor's last step may lower the resolution.
    pub thermal_resolution: bool,
    /// Show the thermal state on the performance overlay.
    pub thermal_readout: bool,
}
impl Default for Boost {
    fn default() -> Self {
        Self {
            weather_density: 100,
            rain_splashes: 100,
            stars: true,
            sun_moon: true,
            fog_start: 0,
            animate_water: true,
            animate_lava: true,
            animate_fire: true,
            animate_portal: true,
            animate_other: true,
            thermal: Heat::Soft,
            thermal_min_fps: 30,
            thermal_resolution: true,
            thermal_readout: true,
        }
    }
}

/// How readily the thermal governor acts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Heat {
    /// Never limits anything: the player's settings hold however hot it gets.
    Off,
    /// Only once Android reports severe heat (the system is already throttling).
    #[default]
    Soft,
    /// From moderate heat and a forecast of severe heat within 10 seconds.
    Balanced,
    /// From light heat: the phone stays cooler, the frame rate drops sooner.
    Strong,
}
impl Heat {
    pub const ALL: [Heat; 4] = [Heat::Off, Heat::Soft, Heat::Balanced, Heat::Strong];
    pub fn key(self) -> &'static str {
        match self {
            Heat::Off => "off",
            Heat::Soft => "soft",
            Heat::Balanced => "balanced",
            Heat::Strong => "strong",
        }
    }
    fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|h| h.key() == value)
    }
}

/// The choices of the stepped settings.
pub const DENSITIES: [u32; 3] = [100, 50, 25];
pub const SPLASHES: [u32; 3] = [100, 50, 0];
pub const FOG_STARTS: [u32; 5] = [0, 80, 60, 40, 20];
/// The choices of [`Boost::thermal_min_fps`].
pub const THERMAL_MIN_FPS: [u32; 5] = [30, 45, 60, 90, 120];

fn flag(v: bool) -> &'static str {
    if v { "true" } else { "false" }
}
fn nearest(choices: &[u32], value: u32) -> u32 {
    *choices
        .iter()
        .min_by_key(|&&c| c.abs_diff(value))
        .unwrap_or(&choices[0])
}

impl Boost {
    /// One `horizon-graphics.txt` line; false when `key` is not a Boost key.
    pub fn parse_line(&mut self, key: &str, value: &str) -> bool {
        let flag = |d: bool| match value {
            "true" => true,
            "false" => false,
            _ => d,
        };
        let number = |d: u32| value.parse().unwrap_or(d);
        match key {
            "weatherDensity" => {
                self.weather_density = nearest(&DENSITIES, number(self.weather_density))
            }
            "rainSplashes" => self.rain_splashes = nearest(&SPLASHES, number(self.rain_splashes)),
            "stars" => self.stars = flag(self.stars),
            "sunMoon" => self.sun_moon = flag(self.sun_moon),
            "fogStart" => self.fog_start = nearest(&FOG_STARTS, number(self.fog_start)),
            "animateWater" => self.animate_water = flag(self.animate_water),
            "animateLava" => self.animate_lava = flag(self.animate_lava),
            "animateFire" => self.animate_fire = flag(self.animate_fire),
            "animatePortal" => self.animate_portal = flag(self.animate_portal),
            "animateOther" => self.animate_other = flag(self.animate_other),
            "thermalMode" => self.thermal = Heat::parse(value).unwrap_or(self.thermal),
            // Before the modes: off stays off, on becomes the default mode.
            "thermalAdaptive" => {
                if value == "false" {
                    self.thermal = Heat::Off
                }
            }
            "thermalMinFps" => {
                self.thermal_min_fps = nearest(&THERMAL_MIN_FPS, number(self.thermal_min_fps))
            }
            "thermalResolution" => self.thermal_resolution = flag(self.thermal_resolution),
            "thermalReadout" => self.thermal_readout = flag(self.thermal_readout),
            _ => return false,
        }
        true
    }
    pub fn serialize(&self) -> String {
        format!(
            "weatherDensity:{}\nrainSplashes:{}\nstars:{}\nsunMoon:{}\nfogStart:{}\nanimateWater:{}\nanimateLava:{}\nanimateFire:{}\nanimatePortal:{}\nanimateOther:{}\nthermalMode:{}\nthermalMinFps:{}\nthermalResolution:{}\nthermalReadout:{}\n",
            self.weather_density,
            self.rain_splashes,
            flag(self.stars),
            flag(self.sun_moon),
            self.fog_start,
            flag(self.animate_water),
            flag(self.animate_lava),
            flag(self.animate_fire),
            flag(self.animate_portal),
            flag(self.animate_other),
            self.thermal.key(),
            self.thermal_min_fps,
            flag(self.thermal_resolution),
            flag(self.thermal_readout),
        )
    }
    /// Whether the rain or snow column at `x`, `z` is drawn. The choice is fixed per
    /// column (a hash of its position), so the columns kept don't flicker as the
    /// camera moves.
    pub fn keeps_weather_column(&self, x: i32, z: i32) -> bool {
        if self.weather_density >= 100 {
            return true;
        }
        let h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (z as u32).wrapping_mul(0x85EB_CA77);
        let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
        (h >> 24) % 4 < self.weather_density / 25
    }
    /// The rain splashes a tick spawns out of vanilla's `count`.
    pub fn splashes(&self, count: i32) -> i32 {
        (count as i64 * i64::from(self.rain_splashes) / 100) as i32
    }
    /// Whether the animated block sprite `name` (a texture id such as
    /// `minecraft:block/water_still`) keeps moving.
    pub fn animates(&self, name: &str) -> bool {
        let path = name.rsplit('/').next().unwrap_or(name);
        match sprite_kind(path) {
            Kind::Water => self.animate_water,
            Kind::Lava => self.animate_lava,
            Kind::Fire => self.animate_fire,
            Kind::Portal => self.animate_portal,
            Kind::Other => self.animate_other,
        }
    }
    /// Whether every texture animation runs (nothing to hold still).
    pub fn animates_everything(&self) -> bool {
        self.animate_water
            && self.animate_lava
            && self.animate_fire
            && self.animate_portal
            && self.animate_other
    }
    /// What the governor at `level` holds the game to under these settings: never
    /// a frame cap under [`Self::thermal_min_fps`], no resolution cut unless allowed,
    /// nothing at all with the governor off.
    pub fn limits_at(&self, level: usize) -> Limits {
        if self.thermal == Heat::Off {
            return Limits::NONE;
        }
        let mut limits = LEVELS[level.min(LEVELS.len() - 1)];
        limits.fps = limits.fps.map(|fps| fps.max(self.thermal_min_fps as i32));
        if !self.thermal_resolution {
            limits.scale_cut = 0;
        }
        limits
    }
    /// The edge fog's start for a render distance `end` (in blocks), where it ends:
    /// `None` keeps vanilla's.
    pub fn fog_start_at(&self, end: f32) -> Option<f32> {
        (self.fog_start > 0).then(|| end * self.fog_start as f32 / 100.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Water,
    Lava,
    Fire,
    Portal,
    Other,
}
fn sprite_kind(path: &str) -> Kind {
    if path.starts_with("water_") {
        Kind::Water
    } else if path.starts_with("lava_") {
        Kind::Lava
    } else if path.starts_with("fire_")
        || path.starts_with("soul_fire_")
        || path.ends_with("campfire_fire")
    {
        Kind::Fire
    } else if path == "nether_portal" {
        Kind::Portal
    } else {
        Kind::Other
    }
}

/// How hot the system says the device is (`AThermal_getCurrentThermalStatus`, and
/// `AThermal_getThermalHeadroom`: 1.0 is where it reaches severe throttling).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Thermal {
    /// 0 none, 1 light, 2 moderate, 3 severe, 4 critical, 5 emergency, 6 shutdown.
    pub status: Option<i32>,
    pub headroom: Option<f32>,
}

/// What the governor holds the game to at a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Frames per second at most (`None`: the player's cap).
    pub fps: Option<i32>,
    /// Points taken off the resolution scale (never below 50%).
    pub scale_cut: u32,
    /// Chunk sections uploaded per frame at most (`None`: the player's setting).
    pub chunk_updates: Option<u32>,
}
impl Limits {
    pub const NONE: Self = Self {
        fps: None,
        scale_cut: 0,
        chunk_updates: None,
    };
    /// `limit` (the frame cap the game would use) under these limits.
    pub fn fps_cap(&self, limit: i32) -> i32 {
        self.fps.map_or(limit, |cap| limit.min(cap))
    }
    /// `scale` (the player's resolution scale, in percent) under these limits.
    pub fn scale(&self, scale: u32) -> u32 {
        if self.scale_cut == 0 {
            scale
        } else {
            scale.saturating_sub(self.scale_cut).max(50).min(scale)
        }
    }
    /// `updates` (the player's chunk updates per frame, 0 unlimited) under these limits.
    pub fn chunk_updates(&self, updates: u32) -> u32 {
        match (self.chunk_updates, updates) {
            (None, u) => u,
            (Some(cap), 0) => cap,
            (Some(cap), u) => u.min(cap),
        }
    }
}

/// Levels of the governor: 0 changes nothing.
pub const LEVELS: [Limits; 4] = [
    Limits::NONE,
    Limits {
        fps: Some(60),
        scale_cut: 0,
        chunk_updates: None,
    },
    Limits {
        fps: Some(45),
        scale_cut: 0,
        chunk_updates: Some(4),
    },
    Limits {
        fps: Some(30),
        scale_cut: 20,
        chunk_updates: Some(2),
    },
];

/// The level a reading asks for in `heat` mode, and the level it allows going back
/// down to (lower thresholds, so a reading on a boundary doesn't make the level flap).
fn wanted(t: Thermal, heat: Heat) -> (usize, usize) {
    let status = t.status.unwrap_or(0).max(0) as usize;
    // Headroom thresholds for levels 1, 2 and 3, rising and falling; status: the
    // status that asks for the first level (each status above it, one more level).
    let (up_at, down_at, first_status): ([f32; 3], [f32; 3], usize) = match heat {
        Heat::Off => return (0, 0),
        // Severe heat or a forecast of it: the system already throttles.
        Heat::Soft => (
            [1.0, f32::INFINITY, f32::INFINITY],
            [0.92, f32::INFINITY, f32::INFINITY],
            3,
        ),
        Heat::Balanced => ([0.75, 0.85, 0.95], [0.65, 0.78, 0.88], 2),
        Heat::Strong => ([0.65, 0.75, 0.85], [0.55, 0.68, 0.78], 1),
    };
    let level = |h: f32, at: [f32; 3]| at.iter().filter(|&&a| h >= a).count();
    let up = |h: f32| level(h, up_at);
    let down = |h: f32| level(h, down_at);
    let by_status = if status >= first_status {
        (status - first_status + 1).min(3)
    } else {
        0
    };
    let (h_up, h_down) = t
        .headroom
        .filter(|h| h.is_finite())
        .map_or((0, 0), |h| (up(h), down(h)));
    (by_status.max(h_up), by_status.max(h_down))
}

/// Steps the frame cap down as the device heats up, and back up one level at a time
/// once readings have stayed below the level's lower threshold for [`Governor::COOL`].
#[derive(Clone, Debug, Default)]
pub struct Governor {
    level: usize,
    /// Since when readings allowed a lower level.
    cool_since: Option<Instant>,
    pub last: Thermal,
}
impl Governor {
    /// How long readings must allow a lower level before the governor steps down.
    pub const COOL: Duration = Duration::from_secs(30);
    pub fn level(&self) -> usize {
        self.level
    }
    /// The limits of the current level with the default settings.
    pub fn limits(&self) -> Limits {
        LEVELS[self.level]
    }
    /// A new reading at `now` in `heat` mode: the level rises at once, and falls a
    /// step at a time.
    pub fn update(&mut self, reading: Thermal, now: Instant, heat: Heat) -> usize {
        self.last = reading;
        let (up, down) = wanted(reading, heat);
        if up > self.level {
            self.level = up;
            self.cool_since = None;
        } else if down < self.level {
            let since = *self.cool_since.get_or_insert(now);
            if now.duration_since(since) >= Self::COOL {
                self.level -= 1;
                self.cool_since = (down < self.level).then_some(now);
            }
        } else {
            self.cool_since = None;
        }
        self.level
    }
    /// Back to level 0 (the setting was turned off).
    pub fn reset(&mut self) {
        self.level = 0;
        self.cool_since = None;
    }
}

/// A preset's Boost values (the rest of the preset is `graphics::apply_preset`).
pub fn apply_preset(preset: crate::graphics::Preset, b: &mut Boost) {
    use crate::graphics::Preset;
    let (density, splashes) = match preset {
        Preset::Performance => (50, 50),
        Preset::Balanced | Preset::Quality | Preset::Ultra => (100, 100),
        Preset::Custom => return,
    };
    b.weather_density = density;
    b.rain_splashes = splashes;
    b.stars = true;
    b.sun_moon = true;
    b.fog_start = 0;
    b.animate_water = true;
    b.animate_lava = true;
    b.animate_fire = true;
    b.animate_portal = true;
    b.animate_other = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_draw_what_vanilla_draws() {
        let b = Boost::default();
        assert!((-40..40).all(|x| (-40..40).all(|z| b.keeps_weather_column(x, z))));
        assert_eq!(b.splashes(99), 99);
        assert!(b.animates_everything());
        assert_eq!(b.fog_start_at(192.0), None);
        assert!(b.stars && b.sun_moon);
    }

    #[test]
    fn round_trips_and_snaps_to_choices() {
        let b = Boost {
            weather_density: 25,
            rain_splashes: 0,
            stars: false,
            sun_moon: false,
            fog_start: 60,
            animate_water: false,
            animate_lava: true,
            animate_fire: false,
            animate_portal: false,
            animate_other: false,
            thermal: Heat::Strong,
            thermal_min_fps: 60,
            thermal_resolution: false,
            thermal_readout: false,
        };
        let mut parsed = Boost::default();
        for line in b.serialize().lines() {
            let (k, v) = line.split_once(':').unwrap();
            assert!(parsed.parse_line(k, v), "{k}");
        }
        assert_eq!(parsed, b);
        let mut odd = Boost::default();
        odd.parse_line("weatherDensity", "40");
        odd.parse_line("rainSplashes", "nonsense");
        odd.parse_line("fogStart", "95");
        assert_eq!(odd.weather_density, 50);
        assert_eq!(odd.rain_splashes, 100);
        assert_eq!(odd.fog_start, 80);
        assert!(!odd.parse_line("sky", "true"));
    }

    #[test]
    fn density_keeps_its_share_of_columns_and_stays_put() {
        for (density, share) in [(50, 0.5), (25, 0.25)] {
            let b = Boost {
                weather_density: density,
                ..Boost::default()
            };
            let kept = (-64..64)
                .flat_map(|x| (-64..64).map(move |z| (x, z)))
                .filter(|&(x, z)| b.keeps_weather_column(x, z))
                .count() as f64
                / (128.0 * 128.0);
            assert!((kept - share).abs() < 0.03, "{density}: {kept}");
            // Every column kept at 25% is also kept at 50%.
            let half = Boost {
                weather_density: 50,
                ..Boost::default()
            };
            assert!(
                (-64..64)
                    .flat_map(|x| (-64..64).map(move |z| (x, z)))
                    .all(|(x, z)| !b.keeps_weather_column(x, z) || half.keeps_weather_column(x, z))
            );
        }
    }

    #[test]
    fn animation_groups_by_texture() {
        let b = Boost {
            animate_water: false,
            animate_fire: false,
            ..Boost::default()
        };
        for still in [
            "minecraft:block/water_still",
            "minecraft:block/water_flow",
            "minecraft:block/fire_0",
            "minecraft:block/soul_fire_1",
            "minecraft:block/campfire_fire",
            "minecraft:block/soul_campfire_fire",
        ] {
            assert!(!b.animates(still), "{still}");
        }
        for moving in [
            "minecraft:block/lava_still",
            "minecraft:block/nether_portal",
            "minecraft:block/sea_lantern",
            "minecraft:block/prismarine",
            "minecraft:block/campfire_log_lit",
        ] {
            assert!(b.animates(moving), "{moving}");
        }
    }

    #[test]
    fn the_governor_changes_nothing_while_cool() {
        let mut g = Governor::default();
        let now = Instant::now();
        for (status, headroom) in [(None, None), (Some(0), Some(0.3)), (Some(1), Some(0.7))] {
            assert_eq!(
                g.update(Thermal { status, headroom }, now, Heat::Balanced),
                0
            );
            assert_eq!(g.limits(), Limits::NONE);
        }
        assert_eq!(Limits::NONE.fps_cap(120), 120);
        assert_eq!(Limits::NONE.scale(100), 100);
        assert_eq!(Limits::NONE.chunk_updates(0), 0);
    }

    #[test]
    fn the_governor_rises_at_once_and_cools_down_slowly() {
        let mut g = Governor::default();
        let t0 = Instant::now();
        let hot = Thermal {
            status: Some(3),
            headroom: Some(0.9),
        };
        assert_eq!(g.update(hot, t0, Heat::Balanced), 2);
        assert_eq!(g.limits().fps_cap(120), 45);
        assert_eq!(g.limits().fps_cap(30), 30);
        // Critical: the resolution goes down too, never under 50%.
        let critical = Thermal {
            status: Some(4),
            headroom: None,
        };
        assert_eq!(g.update(critical, t0, Heat::Balanced), 3);
        assert_eq!(g.limits().scale(100), 80);
        assert_eq!(g.limits().scale(60), 50);
        assert_eq!(g.limits().scale(50), 50);
        assert_eq!(g.limits().chunk_updates(0), 2);
        assert_eq!(g.limits().chunk_updates(1), 1);
        // A reading between the thresholds holds the level.
        let warm = Thermal {
            status: Some(1),
            headroom: Some(0.9),
        };
        assert_eq!(
            g.update(warm, t0 + Duration::from_secs(60), Heat::Balanced),
            3
        );
        // Cool readings step down one level per COOL period.
        let cool = Thermal {
            status: Some(0),
            headroom: Some(0.2),
        };
        let mut t = t0 + Duration::from_secs(61);
        assert_eq!(g.update(cool, t, Heat::Balanced), 3);
        t += Governor::COOL;
        assert_eq!(g.update(cool, t, Heat::Balanced), 2);
        assert_eq!(
            g.update(cool, t + Duration::from_secs(1), Heat::Balanced),
            2
        );
        t += Governor::COOL;
        assert_eq!(g.update(cool, t, Heat::Balanced), 1);
        t += Governor::COOL;
        assert_eq!(g.update(cool, t, Heat::Balanced), 0);
        assert_eq!(g.limits(), Limits::NONE);
        // A hot reading in the middle of cooling restarts the wait.
        assert_eq!(g.update(hot, t, Heat::Balanced), 2);
        assert_eq!(
            g.update(cool, t + Duration::from_secs(20), Heat::Balanced),
            2
        );
        assert_eq!(
            g.update(hot, t + Duration::from_secs(25), Heat::Balanced),
            2
        );
        assert_eq!(
            g.update(cool, t + Duration::from_secs(40), Heat::Balanced),
            2
        );
        assert_eq!(
            g.update(cool, t + Duration::from_secs(69), Heat::Balanced),
            2
        );
        assert_eq!(
            g.update(cool, t + Duration::from_secs(70), Heat::Balanced),
            1
        );
    }

    #[test]
    fn the_player_chooses_how_readily_it_acts_and_how_far() {
        let now = Instant::now();
        let moderate = Thermal {
            status: Some(2),
            headroom: Some(0.9),
        };
        let severe = Thermal {
            status: Some(3),
            headroom: Some(0.9),
        };
        let light = Thermal {
            status: Some(1),
            headroom: Some(0.6),
        };
        let level = |heat, reading| Governor::default().update(reading, now, heat);
        // Off never acts, however hot.
        for reading in [moderate, severe, light] {
            assert_eq!(level(Heat::Off, reading), 0);
        }
        let off = Boost {
            thermal: Heat::Off,
            ..Boost::default()
        };
        assert_eq!(off.limits_at(3), Limits::NONE);
        // Soft waits for severe heat; Balanced acts at moderate; Strong at light.
        assert_eq!(level(Heat::Soft, moderate), 0);
        assert_eq!(level(Heat::Soft, severe), 1);
        assert_eq!(level(Heat::Balanced, moderate), 2);
        assert_eq!(level(Heat::Balanced, light), 0);
        assert_eq!(level(Heat::Strong, light), 1);
        // The frame cap never goes under the player's floor.
        let floor = Boost {
            thermal_min_fps: 60,
            ..Boost::default()
        };
        assert_eq!(floor.limits_at(2).fps_cap(120), 60);
        assert_eq!(floor.limits_at(3).fps_cap(120), 60);
        assert_eq!(Boost::default().limits_at(3).fps_cap(120), 30);
        // The resolution stays when the player keeps it.
        let sharp = Boost {
            thermal_resolution: false,
            ..Boost::default()
        };
        assert_eq!(sharp.limits_at(3).scale(100), 100);
        assert_eq!(Boost::default().limits_at(3).scale(100), 80);
        // A file from before the modes: off stays off, on takes the default mode.
        let mut old = Boost::default();
        old.parse_line("thermalAdaptive", "false");
        assert_eq!(old.thermal, Heat::Off);
        let mut old = Boost::default();
        old.parse_line("thermalAdaptive", "true");
        assert_eq!(old.thermal, Heat::Soft);
    }

    #[test]
    fn headroom_alone_drives_it_where_status_is_missing() {
        let mut g = Governor::default();
        let now = Instant::now();
        let reading = |h| Thermal {
            status: None,
            headroom: Some(h),
        };
        assert_eq!(g.update(reading(0.74), now, Heat::Balanced), 0);
        assert_eq!(g.update(reading(0.76), now, Heat::Balanced), 1);
        assert_eq!(g.update(reading(f32::NAN), now, Heat::Balanced), 1);
        assert_eq!(g.update(reading(0.96), now, Heat::Balanced), 3);
    }

    #[test]
    fn presets_keep_vanilla_looks_except_performance() {
        use crate::graphics::Preset;
        for preset in [Preset::Balanced, Preset::Quality, Preset::Ultra] {
            let mut b = Boost {
                weather_density: 25,
                animate_water: false,
                ..Boost::default()
            };
            apply_preset(preset, &mut b);
            assert_eq!(b, Boost::default(), "{preset:?}");
        }
        let mut b = Boost::default();
        apply_preset(Preset::Performance, &mut b);
        assert_eq!((b.weather_density, b.rain_splashes), (50, 50));
        assert!(b.animates_everything(), "animations stay on (#2064)");
        let mut custom = Boost {
            stars: false,
            ..Boost::default()
        };
        apply_preset(Preset::Custom, &mut custom);
        assert!(!custom.stars);
    }
}
