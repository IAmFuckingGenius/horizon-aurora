//! Android's lifecycle in the game. Leaving the app (Home, the app switcher, a picker
//! over the game) pauses single player and saves the world at once, since Android
//! stops the frames that would otherwise tell the server thread and may end the process
//! in the background without a word; the loop then idles instead of spinning; and
//! memory pressure drops caches the game rebuilds by itself.
// The desktop has no such lifecycle: only the far terrain cap is reached there.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]
use super::{Game, Screen};
use std::time::{Duration, Instant};

/// Frames stopped this long ago: Android paused the activity (or took its window).
const STALLED: Duration = Duration::from_millis(250);
/// How often the process's trim level is read: in play, and with no frames.
const POLL_PLAYING: Duration = Duration::from_secs(5);
const POLL_IDLE: Duration = Duration::from_secs(2);
/// How long memory pressure holds far terrain at half its distance.
const FAR_CAP: Duration = Duration::from_secs(180);

#[derive(Default)]
pub(super) struct Lifecycle {
    /// The world was saved for this stay in the background (reset by the next frame).
    saved_for_background: bool,
    /// The last trim level acted on, and when it was read.
    trim_level: i32,
    polled: Option<Instant>,
    /// Far terrain is held at half its distance until then.
    far_cap_until: Option<Instant>,
    /// Memory trims so far, for the performance report.
    pub trims: u32,
}

impl Game<'_> {
    /// Whether frames have stopped coming: the window is gone, or Android paused the
    /// activity under a picker or the app switcher.
    pub(super) fn frames_stalled(&self) -> bool {
        self.renderer.as_ref().is_some_and(|r| r.suspended)
            || (self.window.is_some()
                && !self.options.offscreen
                && self.previous.elapsed() > STALLED)
    }
    /// The player left the app: open the pause menu where play was going on, tell the
    /// server thread now (its pause saves, as `IntegratedServer.tickServer`'s does) and
    /// ask it for a save that waits for the disk, once per stay in the background.
    pub(super) fn left_app(&mut self) {
        if self.title.is_none() && self.screen == Screen::Playing && self.player.is_some() {
            self.pause_focus = None;
            self.screen = Screen::Paused;
        }
        self.pause = self.pauses_game();
        let Some(server) = &self.integrated else {
            return;
        };
        server.set_paused(self.pause);
        if !self.saved_for_background() {
            server.save_in_background();
            self.lifecycle.saved_for_background = true;
        }
    }
    fn saved_for_background(&self) -> bool {
        self.lifecycle.saved_for_background
    }
    /// From `idle`: while frames are stalled and the window has no focus, the player has
    /// left (`left_app`); the next frame starts a new stay.
    pub(super) fn watch_background(&mut self) {
        if !self.frames_stalled() {
            self.lifecycle.saved_for_background = false;
            return;
        }
        if !self.focused && !self.saved_for_background() && self.player.is_some() {
            self.left_app();
        }
    }
    /// Memory pressure: drop what the game rebuilds by itself (decoded sounds, pooled
    /// mesh buffers, staging buffers) and, when `critical`, hold far terrain at half
    /// its distance for a while.
    pub(super) fn trim_memory(&mut self, reason: &str, critical: bool) {
        let sounds = self.audio.trim_cache();
        let meshes = self.streaming.trim_memory();
        if let Some(r) = self.renderer.as_mut() {
            r.trim_memory();
        }
        if critical && self.far_terrain_choice().0 > 0 {
            self.lifecycle.far_cap_until = Some(Instant::now() + FAR_CAP);
        }
        self.lifecycle.trims += 1;
        log::info!(
            "Horizon: memory pressure ({reason}): released {} KiB of sounds and {} KiB of mesh buffers{}",
            sounds / 1024,
            meshes / 1024,
            if critical {
                ", far terrain at half distance for now"
            } else {
                ""
            }
        );
    }
    /// The far terrain distance under memory pressure: half of `distance` while held.
    pub(super) fn memory_far_cap(&self, distance: u32) -> u32 {
        match self.lifecycle.far_cap_until {
            Some(until) if Instant::now() < until && distance > 0 => (distance / 2).max(32),
            _ => distance,
        }
    }
    /// Reads the process's trim level when due and trims on a new one:
    /// "running low" in play, "background" and above while away.
    pub(super) fn poll_memory_pressure(&mut self) {
        #[cfg(target_os = "android")]
        {
            let stalled = self.frames_stalled();
            let every = if stalled { POLL_IDLE } else { POLL_PLAYING };
            if self.lifecycle.polled.is_some_and(|t| t.elapsed() < every) {
                return;
            }
            self.lifecycle.polled = Some(Instant::now());
            let Some(level) = crate::memory::trim_level() else {
                return;
            };
            let previous = std::mem::replace(&mut self.lifecycle.trim_level, level);
            // "Running low" and "critical" in play; UI_HIDDEN (20) and above describe a
            // hidden app, which acts from "background" on.
            let acts = if stalled {
                level >= crate::memory::BACKGROUND
            } else {
                (crate::memory::RUNNING_LOW..crate::memory::UI_HIDDEN).contains(&level)
            };
            // The level changes only when the system sends a new one.
            if acts && level != previous {
                let critical = level >= crate::memory::RUNNING_CRITICAL;
                self.trim_memory(&format!("trim level {level}"), critical);
            }
        }
    }
    /// The lifecycle's figures for the performance report.
    pub(super) fn lifecycle_json(&self) -> serde_json::Value {
        serde_json::json!({
            "memory_trims": self.lifecycle.trims,
            "trim_level": self.lifecycle.trim_level,
            "far_terrain_capped": self.lifecycle.far_cap_until.is_some_and(|t| Instant::now() < t),
        })
    }
}
