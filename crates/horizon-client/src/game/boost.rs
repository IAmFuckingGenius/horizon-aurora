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

/// Horizon Boost's sheet while it is open over the options.
pub(super) struct BoostScreen {
    ui: horizon_client::boost_ui::BoostUi,
    /// The mouse's last position (physical pixels), for its buttons.
    cursor: [f32; 2],
    /// The finger driving the sheet; others are ignored.
    finger: Option<u64>,
}

impl BoostScreen {
    /// A tap's action, for the control socket's `boost tab`.
    pub fn act(
        &mut self,
        act: horizon_client::boost_ui::Act,
        s: &mut horizon_client::settings::Settings,
    ) -> horizon_client::boost_ui::Outcome {
        self.ui.act(act, s)
    }
}

impl Game<'_> {
    pub(super) fn open_boost(&mut self) {
        self.capture(false);
        self.boost_screen = Some(BoostScreen {
            ui: Default::default(),
            cursor: [self.cursor.0, self.cursor.1],
            finger: None,
        });
    }
    pub(super) fn close_boost(&mut self) {
        if self.boost_screen.take().is_some() {
            self.audio.play_button_click();
        }
    }
    /// Input while the sheet is open: it takes every pointer, wheel and Escape (or
    /// Back) event; true when it took `event`.
    pub(super) fn boost_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        use horizon_client::boost_ui::Outcome;
        use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
        use winit::keyboard::{KeyCode, PhysicalKey};
        let Some(screen) = self.boost_screen.as_mut() else {
            return false;
        };
        let outcome = match event {
            WindowEvent::Touch(t) => {
                let at = [t.location.x as f32, t.location.y as f32];
                match t.phase {
                    TouchPhase::Started if screen.finger.is_none() => {
                        screen.finger = Some(t.id);
                        screen.ui.pointer_down(at);
                        Outcome::None
                    }
                    TouchPhase::Moved if screen.finger == Some(t.id) => {
                        screen.ui.pointer_move(at);
                        Outcome::None
                    }
                    TouchPhase::Ended if screen.finger == Some(t.id) => {
                        screen.finger = None;
                        screen.ui.pointer_up(at, &mut self.settings)
                    }
                    TouchPhase::Cancelled if screen.finger == Some(t.id) => {
                        screen.finger = None;
                        screen.ui.cancel();
                        Outcome::None
                    }
                    _ => Outcome::None,
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                screen.cursor = [position.x as f32, position.y as f32];
                screen.ui.pointer_move(screen.cursor);
                Outcome::None
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if *state == ElementState::Pressed {
                    screen.ui.pointer_down(screen.cursor);
                    Outcome::None
                } else {
                    screen.ui.pointer_up(screen.cursor, &mut self.settings)
                }
            }
            WindowEvent::MouseInput { .. } => Outcome::None,
            WindowEvent::MouseWheel { delta, .. } => {
                screen.ui.wheel(match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                });
                Outcome::None
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && matches!(
                        event.physical_key,
                        PhysicalKey::Code(KeyCode::Escape | KeyCode::BrowserBack)
                    )
                {
                    Outcome::Close
                } else {
                    Outcome::None
                }
            }
            _ => return false,
        };
        match outcome {
            Outcome::None => {}
            Outcome::Clicked => self.audio.play_button_click(),
            Outcome::Changed => {
                self.audio.play_button_click();
                self.settings.save(&Self::options_path());
                self.apply_settings();
            }
            Outcome::Close => self.close_boost(),
        }
        true
    }
    /// The sheet over everything else in `hud`, when it is open.
    pub(super) fn draw_boost(&mut self, hud: &mut horizon_client::hud::HudBatch) {
        if self.boost_screen.is_none() {
            return;
        }
        let (frame_ms, gpu_ms) = self.frame_graph.averages().unwrap_or((0.0, 0.0));
        let readout = horizon_client::boost_ui::Readout {
            fps: if frame_ms > 0.0 {
                1000.0 / frame_ms
            } else {
                0.0
            },
            frame_ms,
            gpu_ms: (gpu_ms > 0.0).then_some(gpu_ms),
            thermal: self.thermal_summary(),
            thermal_level: if self.settings.horizon.boost.thermal == Heat::Off {
                0
            } else {
                self.thermal.governor.level()
            },
        };
        let density = self
            .window
            .as_ref()
            .map_or(1.0, |w| w.scale_factor() as f32);
        if let Some(screen) = self.boost_screen.as_mut() {
            horizon_client::boost_ui::draw(
                &mut screen.ui,
                &self.settings,
                &readout,
                hud,
                &self.gui.atlas,
                density,
            );
        }
    }
    /// The thermal line for the sheet: as the overlay's, whatever its readout setting.
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
