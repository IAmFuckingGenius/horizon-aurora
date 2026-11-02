//! Track total drag distance separately from per-event scroll deltas.
/// Scroll only enough to reveal the selected row, retaining the current offset.
pub(super) fn reveal(scroll: f32, row: [f32; 4], view: [f32; 3]) -> f32 {
    let [top, bottom, maximum] = view;
    let adjustment = if row[1] < top {
        row[1] - top
    } else if row[1] + row[3] > bottom {
        row[1] + row[3] - bottom
    } else {
        0.0
    };
    (scroll + adjustment).clamp(0.0, maximum.max(0.0))
}
pub(super) struct Drag {
    origin: [f64; 2],
    previous_y: f64,
    pub(super) moved: bool,
    /// Scroll speed in dp per second, smoothed over the last moves.
    velocity: f32,
    last: std::time::Instant,
}

/// Below this release speed (dp/s) a list stops with the finger.
const FLING_MIN: f32 = 120.0;
/// How fast a fling slows: velocity × e^(−FRICTION·t).
const FRICTION: f32 = 3.5;
/// A fling ends below this speed.
const FLING_STOP: f32 = 15.0;

impl Drag {
    pub(super) fn new(x: f64, y: f64) -> Self {
        Self {
            origin: [x, y],
            previous_y: y,
            moved: false,
            velocity: 0.0,
            last: std::time::Instant::now(),
        }
    }
    /// The speed a list keeps after the finger lifts: none when the finger rested
    /// before lifting or moved too slowly.
    pub(super) fn release_velocity(&self) -> Option<f32> {
        (self.moved
            && self.last.elapsed() < std::time::Duration::from_millis(80)
            && self.velocity.abs() >= FLING_MIN)
            .then_some(self.velocity)
    }

    pub(super) fn origin_y(&self) -> f64 {
        self.origin[1]
    }

    pub(super) fn update(&mut self, x: f64, y: f64, density: f64) -> f32 {
        let density = density.max(0.01);
        self.moved |= (x - self.origin[0]).hypot(y - self.origin[1]) > 12.0 * density;
        let delta = if self.moved {
            ((self.previous_y - y) / density) as f32
        } else {
            0.0
        };
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32();
        if self.moved && dt > 0.0 {
            // Smoothed so one jittery event does not decide the throw.
            let speed = delta / dt.max(0.004);
            self.velocity = if dt > 0.1 {
                speed
            } else {
                self.velocity * 0.4 + speed * 0.6
            };
        }
        self.last = now;
        self.previous_y = y;
        delta
    }
}

/// A list still sliding after the finger lifted.
pub(super) struct Fling {
    pub(super) velocity: f32,
    last: std::time::Instant,
    /// The open chooser's options rather than the page's list.
    pub(super) chooser: bool,
}
impl Fling {
    pub(super) fn new(velocity: f32, chooser: bool) -> Self {
        Self {
            velocity,
            last: std::time::Instant::now(),
            chooser,
        }
    }
    /// The distance to scroll since the last step, or `None` once it has stopped.
    pub(super) fn step(&mut self) -> Option<f32> {
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.05);
        self.last = now;
        let distance = self.velocity * dt;
        self.velocity *= (-FRICTION * dt).exp();
        (self.velocity.abs() >= FLING_STOP).then_some(distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_scroll_reveals_rows_in_both_directions_and_stops_at_bounds() {
        assert_eq!(
            reveal(0.0, [0.0, 350.0, 100.0, 60.0], [100.0, 400.0, 500.0]),
            10.0
        );
        assert_eq!(
            reveal(200.0, [0.0, 80.0, 100.0, 60.0], [100.0, 400.0, 500.0]),
            180.0
        );
        assert_eq!(
            reveal(20.0, [0.0, 150.0, 100.0, 60.0], [100.0, 400.0, 500.0]),
            20.0
        );
        assert_eq!(
            reveal(490.0, [0.0, 700.0, 100.0, 60.0], [100.0, 400.0, 500.0]),
            500.0
        );
    }

    #[test]
    fn slow_vertical_drag_crosses_the_total_threshold() {
        let mut drag = Drag::new(100.0, 300.0);
        for y in [297.0, 294.0, 291.0, 288.0] {
            assert_eq!(drag.update(100.0, y, 1.0), 0.0);
        }
        assert_eq!(drag.update(100.0, 285.0, 1.0), 3.0);
        assert!(drag.moved);
        assert_eq!(drag.origin_y(), 300.0);
        assert_eq!(drag.update(100.0, 290.0, 1.0), -5.0);
    }

    #[test]
    fn a_fling_slows_down_and_stops() {
        let mut fling = Fling::new(1000.0, false);
        let mut travelled = 0.0;
        let mut steps = 0;
        while let Some(d) = {
            fling.last -= std::time::Duration::from_millis(16);
            fling.step()
        } {
            travelled += d;
            steps += 1;
            assert!(steps < 1000);
        }
        // ≈ v / friction for an exponential decay.
        assert!((200.0..320.0).contains(&travelled), "{travelled}");
    }

    #[test]
    fn density_scales_threshold_and_scroll_together() {
        let mut drag = Drag::new(200.0, 600.0);
        assert_eq!(drag.update(200.0, 580.0, 2.0), 0.0);
        assert_eq!(drag.update(200.0, 570.0, 2.0), 5.0);
        assert!(drag.moved);
    }
}
