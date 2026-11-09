//! Horizon Boost in the game: the thermal governor's readings and what it changes
//! (frame cap, world resolution, chunk uploads), for the settings in
//! `horizon_client::boost`.
use super::Game;
use horizon_client::boost::{Governor, Heat, Limits, Thermal};
use std::time::Instant;

/// The governor and where its readings come from.
#[derive(Default)]
pub(super) struct ThermalState {
    pub governor: Governor,
    /// Readings the `thermal-test` control command injects in place of the system's
    /// (desktop checks of the governor; Android has its own readings).
    pub test_reading: Option<Thermal>,
    /// The limits last applied to the renderer.
    applied: Option<Limits>,
}

impl ThermalState {
    /// The chunk sections uploaded per frame under the governor (0: unlimited), for the
    /// graphics settings `g`.
    pub fn governor_chunk_updates(&self, g: &horizon_client::graphics::Graphics) -> u32 {
        g.boost
            .limits_at(self.governor.level())
            .chunk_updates(g.chunk_updates)
    }
}

impl Game<'_> {
    /// The governor's limits now under the player's heat settings (none while off).
    pub(super) fn thermal_limits(&self) -> Limits {
        self.settings
            .horizon
            .boost
            .limits_at(self.thermal.governor.level())
    }
    /// Once a frame: a new reading when one is due, and the renderer's resolution
    /// follows the level when it changed.
    pub(super) fn tick_thermal(&mut self) {
        let heat = self.settings.horizon.boost.thermal;
        if heat == Heat::Off {
            if self.thermal.governor.level() != 0 {
                self.thermal.governor.reset();
            }
        } else if let Some(reading) = self.thermal_reading() {
            let before = self.thermal.governor.level();
            let after = self.thermal.governor.update(reading, Instant::now(), heat);
            if after != before {
                log::info!(
                    "Horizon Boost: thermal level {before} -> {after} (status {:?}, headroom {:?})",
                    reading.status,
                    reading.headroom
                );
            }
        }
        let limits = self.thermal_limits();
        if self.thermal.applied != Some(limits) {
            self.thermal.applied = Some(limits);
            self.apply_world_scale();
        }
    }
    fn thermal_reading(&self) -> Option<Thermal> {
        if let Some(test) = self.thermal.test_reading {
            return Some(test);
        }
        #[cfg(target_os = "android")]
        {
            Some(crate::thermal::read())
        }
        #[cfg(not(target_os = "android"))]
        None
    }
    /// The world's resolution scale, anti-aliasing and upscaling, under the governor's
    /// resolution cut.
    pub(super) fn apply_world_scale(&mut self) {
        let limits = self.thermal_limits();
        let g = &self.settings.horizon;
        if let Some(r) = self.renderer.as_mut() {
            r.set_world_post(
                limits.scale(g.resolution_scale) as f32 / 100.0,
                g.fxaa,
                g.taa,
                g.upscaling.ratio(),
                g.upscaling_sharpness as f32 / 100.0,
            );
        }
    }
    /// The thermal state for status replies, performance reports and the overlay.
    pub(super) fn thermal_json(&self) -> serde_json::Value {
        let g = &self.thermal.governor;
        let limits = self.thermal_limits();
        #[cfg(target_os = "android")]
        let source = crate::thermal::source();
        #[cfg(not(target_os = "android"))]
        let source = "unavailable";
        serde_json::json!({
            "adaptive": self.settings.horizon.boost.thermal != Heat::Off,
            "mode": self.settings.horizon.boost.thermal.key(),
            "min_fps": self.settings.horizon.boost.thermal_min_fps,
            "level": g.level(),
            "status": g.last.status,
            "headroom": g.last.headroom,
            "source": if self.thermal.test_reading.is_some() { "test" } else { source },
            "fps_cap": limits.fps,
            "scale_cut": limits.scale_cut,
            "chunk_updates_cap": limits.chunk_updates,
        })
    }
    /// One overlay line: the thermal status and what the governor holds the game to,
    /// or `None` when there is nothing to say (no reading, readout off).
    pub(super) fn thermal_line(&self) -> Option<String> {
        if !self.settings.horizon.boost.thermal_readout {
            return None;
        }
        self.thermal_summary()
    }
}

impl Game<'_> {
    /// Boost's live line: frames per second and, where the system reports it, heat.
    pub(super) fn boost_readout(&self) -> String {
        let fps = self
            .frame_graph
            .averages()
            .map(|(frame_ms, _)| frame_ms)
            .filter(|ms| *ms > 0.0)
            .map(|ms| {
                horizon_client::lang::tr(
                    "horizon.boost.readout.fps",
                    &[&format!("{:.0}", 1000.0 / ms)],
                )
            });
        [fps, self.thermal_summary()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ")
    }
    /// The thermal state and any heat limit, as one line.
    fn thermal_summary(&self) -> Option<String> {
        let g = &self.thermal.governor;
        let status = g.last.status?;
        let name = horizon_client::lang::get(&format!("horizon.boost.thermal.status.{status}"));
        let mut line = horizon_client::lang::tr("horizon.boost.thermal.line", &[&name]);
        if let Some(h) = g.last.headroom {
            line.push_str(&format!(" · {:.0}%", h * 100.0));
        }
        if self.settings.horizon.boost.thermal != Heat::Off && g.level() > 0 {
            let fps = self.thermal_limits().fps.unwrap_or(0);
            line.push_str(" · ");
            line.push_str(&horizon_client::lang::tr(
                "horizon.boost.thermal.capped",
                &[&fps],
            ));
        }
        Some(line)
    }
}
