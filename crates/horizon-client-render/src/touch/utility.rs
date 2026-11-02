//! Horizon's utility buttons: the "..." key drawer, the menu mouse toggle, Back,
//! and the virtual mouse's buttons. They sit at fixed spots until the player
//! moves them in the layout editor; `touch_ui` draws and presses them.
use super::layout::{Placement, Show};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Utility {
    /// "...": opens the key drawer (F3, F5, chat, the keyboard, more keys).
    More,
    /// Turns the menu's virtual mouse on and off.
    Mouse,
    /// Escape: closes the screen.
    Back,
    /// The virtual mouse's buttons.
    Lmb,
    Rmb,
}
impl Utility {
    pub const ALL: [Self; 5] = [Self::More, Self::Mouse, Self::Back, Self::Lmb, Self::Rmb];
    pub fn id(self) -> &'static str {
        match self {
            Self::More => "more",
            Self::Mouse => "mouse",
            Self::Back => "back",
            Self::Lmb => "lmb",
            Self::Rmb => "rmb",
        }
    }
    /// The name the editor shows.
    pub fn name(self) -> String {
        crate::lang::get(&format!("horizon.touch.utility.{}.name", self.id()))
    }
    /// The label drawn on the button.
    pub fn label(self, mouse_on: bool) -> String {
        if self == Self::Mouse && mouse_on {
            return crate::lang::get("horizon.touch.utility.mouse_on");
        }
        crate::lang::get(&format!("horizon.touch.utility.{}", self.id()))
    }
    /// Width over height.
    pub fn ratio(self) -> f32 {
        match self {
            Self::More => 33.0 / 20.0,
            _ => 2.5,
        }
    }
    /// Only the drawer means something while playing; the others act on screens.
    pub fn in_game(self) -> bool {
        self == Self::More
    }
    /// Where it sat before it could be moved, in GUI pixels of a `w` × `h` GUI.
    fn fixed(self, w: f32, h: f32) -> [f32; 4] {
        match self {
            Self::More => [3.0, 3.0, 33.0, 20.0],
            Self::Mouse => [w - 108.0, 3.0, 50.0, 20.0],
            Self::Back => [w - 53.0, 3.0, 50.0, 20.0],
            Self::Lmb => [w - 53.0, h - 51.0, 50.0, 20.0],
            Self::Rmb => [w - 53.0, h - 27.0, 50.0, 20.0],
        }
    }
}
/// Where the utility buttons are and when they show.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UtilityLayout {
    /// A moved button's place (centre and height as fractions of the surface);
    /// `None` keeps its fixed spot.
    pub places: [Option<Placement>; 5],
    /// [`Show`] as a byte per button; Auto is "always" for the drawer and "in
    /// menus" for the others.
    pub show: [u8; 5],
}
impl UtilityLayout {
    pub fn show(&self, u: Utility) -> Show {
        match Show::from_u8(self.show[u as usize]) {
            Show::Auto if u.in_game() => Show::Always,
            Show::Auto => Show::Menus,
            Show::Game | Show::Always if !u.in_game() => Show::Menus,
            show => show,
        }
    }
    pub fn set_show(&mut self, u: Utility, show: Show) {
        self.show[u as usize] = show.to_u8();
    }
    /// Shown over screens (`menu`) or while playing.
    pub fn shown(&self, u: Utility, menu: bool) -> bool {
        match self.show(u) {
            Show::Always => true,
            Show::Game => !menu,
            Show::Menus => menu,
            Show::Hidden | Show::Auto => false,
        }
    }
    pub fn opacity(&self, u: Utility) -> f32 {
        self.places[u as usize].map_or(1.0, |p| p.opacity)
    }
    /// The button's rectangle in GUI pixels of a `w` × `h` GUI, kept on screen.
    pub fn rect(&self, u: Utility, [w, h]: [f32; 2]) -> [f32; 4] {
        let Some(p) = self.places[u as usize] else {
            return u.fixed(w, h);
        };
        let rh = (p.size * h).min(h);
        let rw = (rh * u.ratio()).min(w);
        [
            (p.center[0] * w - rw / 2.0).clamp(0.0, (w - rw).max(0.0)),
            (p.center[1] * h - rh / 2.0).clamp(0.0, (h - rh).max(0.0)),
            rw,
            rh,
        ]
    }
    /// The place a button has now, moved or fixed, for the editor to change.
    pub fn place(&self, u: Utility, gui: [f32; 2]) -> Placement {
        if let Some(p) = self.places[u as usize] {
            return p;
        }
        let [x, y, rw, rh] = u.fixed(gui[0], gui[1]);
        Placement {
            center: [(x + rw / 2.0) / gui[0], (y + rh / 2.0) / gui[1]],
            size: rh / gui[1],
            opacity: 1.0,
        }
    }
    pub fn sanitize(&mut self) {
        for p in self.places.iter_mut().flatten() {
            p.sanitize();
        }
        for show in &mut self.show {
            if *show > 4 {
                *show = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unmoved_buttons_keep_their_spots_and_moved_ones_follow_the_surface() {
        let mut layout = UtilityLayout::default();
        assert_eq!(
            layout.rect(Utility::Back, [480.0, 270.0]),
            [427.0, 3.0, 50.0, 20.0]
        );
        assert!(layout.shown(Utility::More, false) && layout.shown(Utility::More, true));
        assert!(!layout.shown(Utility::Back, false) && layout.shown(Utility::Back, true));
        let mut place = layout.place(Utility::Back, [480.0, 270.0]);
        place.center = [0.5, 0.5];
        layout.places[Utility::Back as usize] = Some(place);
        let [x, y, w, h] = layout.rect(Utility::Back, [960.0, 540.0]);
        assert!((x + w / 2.0 - 480.0).abs() < 0.01 && (y + h / 2.0 - 270.0).abs() < 0.01);
        assert!((h - 40.0).abs() < 0.01, "twice the GUI, twice the height");
        // Back means nothing while playing: "always" still shows it over screens only.
        layout.set_show(Utility::Back, Show::Always);
        assert!(!layout.shown(Utility::Back, false));
        layout.set_show(Utility::More, Show::Hidden);
        assert!(!layout.shown(Utility::More, true));
    }
}
