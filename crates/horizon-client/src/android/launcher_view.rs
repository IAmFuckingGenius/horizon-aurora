//! The launcher's screens in the «Горизонт» look: Home, Add-ons, Worlds and
//! Settings beside a navigation rail (a bottom bar in portrait), with toasts and
//! dialogs over them. Behaviour stays in `launcher_pages.rs`: every control here
//! only draws itself and registers its `Action`.
use super::{
    Launcher,
    kit::{self, Icon, Kit, Style, col},
    pages::Action,
    sky,
    text::{Lang, lang, t, t_dyn, tf},
};
use crate::catalog::Page;
use horizon_client::ui_theme::Face;
use std::time::{Duration, Instant, SystemTime};

/// One world of the profile, for Home's «last world» and the Worlds page.
pub(super) struct WorldRow {
    pub folder: String,
    pub name: String,
    pub mode: i32,
    pub hardcore: bool,
    pub modified: SystemTime,
    pub size: u64,
}

pub(super) struct Toast {
    pub title: String,
    pub detail: String,
    pub error: bool,
    pub at: Instant,
}

/// What the screens remember between frames.
pub(super) struct ViewState {
    pub started: Instant,
    pub worlds: Vec<WorldRow>,
    pub worlds_loaded: Option<Instant>,
    pub addons_installed: bool,
    pub last_addon: Page,
    pub settings_tops: Vec<f32>,
    pub toast: Option<Toast>,
    pub last_status: String,
    pub last_error: Option<String>,
    /// The first byte count and time seen in the download stage, for its speed.
    pub install_rate: Option<(Instant, u64)>,
    pub free: Option<(Instant, u64)>,
    /// The orientation the screen mode was last set for.
    pub landscape: Option<bool>,
    /// A failed world or report transfer: its title and reason, until dismissed.
    pub transfer_error: Option<(String, String)>,
    /// Installed engines (`engines.rs`), read again every few seconds.
    pub engines: Vec<crate::engines::Engine>,
    pub engines_loaded: Option<Instant>,
    /// The engine Delete was tapped for once, and when: a second tap deletes.
    pub engine_delete: Option<(usize, Instant)>,
    /// The Instances page: each instance and how many worlds it has.
    pub instances: Vec<(crate::instances::Instance, usize)>,
    pub instances_loaded: Option<Instant>,
    /// The instance sheet, while it is open.
    pub instance_edit: Option<InstanceEdit>,
    /// The question before an engine is imported.
    pub engine_confirm: bool,
}

/// The instance sheet: a new instance (`id` none) or one being changed.
pub(super) struct InstanceEdit {
    pub id: Option<u32>,
    pub name: String,
    pub engine: Option<String>,
    /// When Delete was tapped once: a second tap within five seconds deletes.
    pub delete_armed: Option<Instant>,
}

/// An instance's name as the player sees it: its own, or the default one.
pub(super) fn instance_name(instance: &crate::instances::Instance) -> String {
    if !instance.name.is_empty() {
        instance.name.clone()
    } else if instance.id == 0 {
        t("Main").into()
    } else {
        tf("Instance {}", &[&instance.id.to_string()])
    }
}
impl Default for ViewState {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            worlds: Vec::new(),
            worlds_loaded: None,
            addons_installed: false,
            last_addon: Page::Mods,
            settings_tops: Vec::new(),
            toast: None,
            last_status: String::new(),
            last_error: None,
            install_rate: None,
            free: None,
            landscape: None,
            transfer_error: None,
            engines: Vec::new(),
            engines_loaded: None,
            engine_delete: None,
            instances: Vec::new(),
            instances_loaded: None,
            instance_edit: None,
            engine_confirm: false,
        }
    }
}

/// The worlds in `saves`, newest first, with their folder sizes.
pub(super) fn load_worlds(saves: &std::path::Path) -> Vec<WorldRow> {
    fn size(path: &std::path::Path) -> u64 {
        std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| match e.metadata() {
                Ok(m) if m.is_dir() => size(&e.path()),
                Ok(m) => m.len(),
                Err(_) => 0,
            })
            .sum()
    }
    horizon_client::title::list_worlds(saves)
        .into_iter()
        .map(|w| WorldRow {
            size: size(&saves.join(&w.folder)),
            folder: w.folder,
            name: w.name,
            mode: w.game_mode,
            hardcore: w.hardcore,
            modified: w.modified,
        })
        .collect()
}

fn mode_name(mode: i32, hardcore: bool) -> &'static str {
    if hardcore {
        return t("Hardcore");
    }
    match mode {
        1 => t("Creative"),
        2 => t("Adventure"),
        3 => t("Spectator"),
        _ => t("Survival"),
    }
}
fn when(time: SystemTime) -> String {
    let days = SystemTime::now()
        .duration_since(time)
        .map_or(0, |d| d.as_secs() / 86_400);
    match days {
        0 => t("today").into(),
        1 => t("yesterday").into(),
        n => tf("{} days ago", &[&n.to_string()]),
    }
}
/// A size in binary units, with a decimal comma in Russian.
fn size(bytes: u64) -> String {
    let mib = bytes as f64 / 1_048_576.0;
    let (value, unit) = if mib >= 1024.0 {
        (mib / 1024.0, t("GiB"))
    } else {
        (mib, t("MiB"))
    };
    let text = if value >= 100.0 || unit == t("MiB") {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    };
    let text = if lang() == Lang::Ru {
        text.replace('.', ",")
    } else {
        text
    };
    format!("{text} {unit}")
}
fn short_count(n: u64) -> String {
    let (value, suffix) = match n {
        0..1_000 => return n.to_string(),
        1_000..1_000_000 => (n as f64 / 1_000.0, t("K")),
        _ => (n as f64 / 1_000_000.0, t("M")),
    };
    let text = if value >= 100.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    };
    let text = if lang() == Lang::Ru {
        text.replace('.', ",")
    } else {
        text
    };
    format!("{text}{suffix}")
}

/// Width of the pager: two 48 dp buttons around the page label.
const PAGER_W: f32 = 188.0;
/// The destinations of the rail.
const NAV: [(Page, &str, &str); 6] = [
    (Page::Play, "home", "Home"),
    (Page::Instances, "gamepad", "Instances"),
    (Page::Mods, "package", "Add-ons"),
    (Page::Worlds, "folder", "Worlds"),
    (Page::Skins, "user", "Skins"),
    (Page::Settings, "sliders", "Settings"),
];
fn nav_selected(page: Page, target: Page) -> bool {
    match target {
        Page::Mods => matches!(
            page,
            Page::Mods | Page::Packs | Page::Shaders | Page::Installed
        ),
        _ => page == target,
    }
}

impl Launcher {
    /// The sky's clock: a half-second beat, so stars and clouds step slowly.
    fn sky_time(&self) -> f32 {
        (self.view.started.elapsed().as_secs_f32() * 2.0).floor() / 2.0
    }
    fn free_space(&mut self) -> Option<u64> {
        if self
            .view
            .free
            .is_none_or(|(at, _)| at.elapsed() > Duration::from_secs(5))
        {
            let free = rustix::fs::statvfs(&self.home)
                .ok()
                .map(|s| s.f_bavail * s.f_frsize);
            self.view.free = free.map(|f| (Instant::now(), f));
        }
        self.view.free.map(|(_, f)| f)
    }
    pub(super) fn refresh_instances(&mut self) {
        self.view.instances = crate::instances::list(&self.home)
            .into_iter()
            .map(|instance| {
                let saves = crate::instances::folder(&self.home, instance.id).join("saves");
                let worlds = std::fs::read_dir(saves)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| e.path().join("level.dat").is_file())
                    .count();
                (instance, worlds)
            })
            .collect();
        self.view.instances_loaded = Some(Instant::now());
    }
    /// The name of an engine as the player sees it: the APK's own, an installed one's,
    /// or that one missing.
    fn engine_label(&self, engine: Option<&str>) -> String {
        match engine {
            None => t("built-in engine").into(),
            Some(id) => match self.view.engines.iter().find(|e| e.manifest.id == id) {
                Some(e) => tf("engine «{}»", &[&e.manifest.name]),
                None => tf("engine «{}» is not installed", &[id]),
            },
        }
    }
    pub(super) fn refresh_engines(&mut self) {
        self.view.engines = crate::engines::list(&self.home);
        self.view.engines_loaded = Some(Instant::now());
    }
    pub(super) fn refresh_worlds(&mut self) {
        let saves = self.catalog.config.game_home(&self.home).join("saves");
        self.view.worlds = load_worlds(&saves);
        self.view.worlds_loaded = Some(Instant::now());
    }
    pub(super) fn toast(
        &mut self,
        title: impl Into<String>,
        detail: impl Into<String>,
        error: bool,
    ) {
        self.view.toast = Some(Toast {
            title: title.into(),
            detail: detail.into(),
            error,
            at: Instant::now(),
        });
    }

    /// Draws the whole launcher frame into `k`.
    pub(super) fn draw_view(&mut self, k: &mut Kit) {
        let (w, h) = (k.ui.w, k.ui.h);
        kit::fill(k.ui, 0.0, 0.0, w, h, col::PAGE);
        let portrait = h > w;
        let main = if portrait {
            [0.0, 0.0, w, h - 64.0]
        } else {
            [96.0, 0.0, w - 96.0, h]
        };
        if self
            .view
            .worlds_loaded
            .is_none_or(|at| at.elapsed() > Duration::from_secs(30))
        {
            self.refresh_worlds();
        }
        if matches!(self.catalog.page, Page::Instances)
            && self
                .view
                .instances_loaded
                .is_none_or(|at| at.elapsed() > Duration::from_secs(10))
        {
            self.refresh_instances();
        }
        if matches!(self.catalog.page, Page::Settings | Page::Instances)
            && self
                .view
                .engines_loaded
                .is_none_or(|at| at.elapsed() > Duration::from_secs(3))
        {
            self.refresh_engines();
        }
        // Messages the old status line showed become toasts, except on the
        // add-on pages, which show their catalogue status in the list.
        let addons = matches!(
            self.catalog.page,
            Page::Mods | Page::Packs | Page::Shaders | Page::Installed
        );
        if self.catalog.status != self.view.last_status {
            self.view.last_status = self.catalog.status.clone();
            if !addons && !self.catalog.status.is_empty() {
                self.toast(t_dyn(&self.catalog.status), String::new(), false);
            }
        }
        let installed = horizon_client::official_install::ready(&self.home);
        if self.error != self.view.last_error {
            self.view.last_error = self.error.clone();
            // An installation error has its own card on Home.
            if let Some(error) = &self.error
                && (installed || self.catalog.page != Page::Play)
            {
                self.toast(t("Something went wrong"), error.clone(), true);
            }
        }
        match self.catalog.page {
            Page::Play => self.draw_home(k, main, portrait),
            Page::Worlds => self.draw_worlds(k, main, portrait),
            Page::Skins => self.draw_skins(k, main, portrait),
            Page::Instances => self.draw_instances(k, main, portrait),
            Page::Settings => self.draw_settings(k, main, portrait),
            _ => self.draw_addons(k, main, portrait),
        }
        self.draw_nav(k, portrait);
        self.draw_toast(k, main);
    }

    fn draw_nav(&mut self, k: &mut Kit, portrait: bool) {
        let (w, h) = (k.ui.w, k.ui.h);
        let page = self.catalog.page;
        if portrait {
            kit::fill(k.ui, 0.0, h - 64.0, w, 64.0, col::SIDE);
            kit::fill(k.ui, 0.0, h - 64.0, w, 2.0, col::RAIL_EDGE);
            let item = (w - 16.0 - 4.0 * (NAV.len() - 1) as f32) / NAV.len() as f32;
            for (i, (target, icon, label)) in NAV.into_iter().enumerate() {
                k.nav(
                    [8.0 + i as f32 * (item + 4.0), h - 60.0, item, 56.0],
                    icon,
                    t(label),
                    nav_selected(page, target),
                    Action::Page(target),
                );
            }
        } else {
            kit::fill(k.ui, 0.0, 0.0, 96.0, h, col::SIDE);
            kit::fill(k.ui, 94.0, 0.0, 2.0, h, col::RAIL_EDGE);
            kit::brand(k.ui, 43.0, 12.0);
            let step = ((h - 56.0 - 8.0) / NAV.len() as f32).clamp(48.0, 66.0);
            for (i, (target, icon, label)) in NAV.into_iter().enumerate() {
                k.nav(
                    [23.0, 56.0 + i as f32 * step, 72.0, step - 4.0],
                    icon,
                    t(label),
                    nav_selected(page, target),
                    Action::Page(target),
                );
            }
        }
    }

    fn draw_toast(&mut self, k: &mut Kit, [mx, _, mw, mh]: [f32; 4]) {
        let expired = self.view.toast.as_ref().is_some_and(|toast| {
            toast.at.elapsed()
                > if toast.error {
                    Duration::from_secs(8)
                } else {
                    Duration::from_millis(3200)
                }
        });
        if expired {
            self.view.toast = None;
        }
        let Some(toast) = &self.view.toast else {
            return;
        };
        let max = (mw - 32.0).min(520.0);
        let text_w = max - 16.0 - 24.0 - 12.0 - 16.0;
        // Titles and details wrap in full instead of ending in an ellipsis.
        let title = k.wrap_max(Face::BodyBold, 14.0, &toast.title, text_w, usize::MAX);
        let detail = k.wrap_max(Face::BodyMedium, 12.0, &toast.detail, text_w, usize::MAX);
        let content = title
            .iter()
            .map(|l| k.measure(Face::BodyBold, 14.0, l))
            .chain(detail.iter().map(|l| k.measure(Face::BodyMedium, 12.0, l)))
            .fold(0.0, f32::max)
            .min(text_w);
        let title_h = title.len().max(1) as f32 * 16.8;
        let tw = 16.0 + 24.0 + 12.0 + content + 16.0;
        let th = (28.0 + title_h + detail.len() as f32 * 14.4).max(56.0);
        let x = mx + (mw - tw) / 2.0;
        // Portrait keeps Play at the bottom: the toast comes down from the top.
        let y = if k.ui.h > k.ui.w {
            16.0
        } else {
            mh - th - 16.0
        };
        kit::plate(k.ui, [x, y, tw, th], 2, col::CONTROL, col::CONTROL_RING);
        let icon = if toast.error {
            Icon::Sprite(kit::ALERT)
        } else {
            Icon::Pixel("check")
        };
        kit::icon(
            k.ui,
            icon,
            x + 16.0,
            y + (th - 24.0) / 2.0,
            24.0,
            if toast.error {
                col::DANGER
            } else {
                col::SUCCESS
            },
        );
        let text_top = y + (th - title_h - detail.len() as f32 * 14.4) / 2.0;
        for (i, line) in title.iter().enumerate() {
            k.text(
                Face::BodyBold,
                14.0,
                16.8,
                x + 52.0,
                text_top + i as f32 * 16.8,
                col::INK,
                line,
            );
        }
        for (i, line) in detail.iter().enumerate() {
            k.text(
                Face::BodyMedium,
                12.0,
                14.4,
                x + 52.0,
                text_top + title_h + i as f32 * 14.4,
                col::INK_SOFT,
                line,
            );
        }
    }

    // ---- Home ----

    fn draw_home(&mut self, k: &mut Kit, main: [f32; 4], portrait: bool) {
        let [mx, my, mw, mh] = main;
        sky::full(k.ui, main, self.sky_time());
        let installed = horizon_client::official_install::ready(&self.home);
        let installing = self.worker.is_some();
        let failed = !installed && !installing && self.error.is_some();
        let x0 = mx + 16.0;
        let w0 = mw - 32.0;
        let top = my + 16.0;
        // Top row: status line and the account chip.
        let (dot, state) = if installing {
            (col::ACCENT_HI, t("downloading"))
        } else if failed {
            (col::DANGER, t("download failed"))
        } else if installed {
            (col::SUCCESS, t("ready"))
        } else {
            (col::MUTED, t("not installed"))
        };
        let name = if let (Some(profile), true) = (&self.login.profile, self.login.signed_in) {
            profile.clone()
        } else if self.catalog.config.name.trim().is_empty() {
            t("Player").into()
        } else {
            self.catalog.config.name.clone()
        };
        let (status, sign_in) = if self.login.signed_in {
            (t("Microsoft"), "")
        } else if self.login.pending() {
            (t("Signing in…"), "")
        } else {
            (t("Offline"), t("Sign in"))
        };
        let chip_w = k.chip(x0 + w0, top, &name, status, sign_in, Action::Account);
        let status_text = format!("Minecraft {} · {state}", horizon_core::VANILLA_VERSION);
        let status_w = w0 - chip_w - 16.0 - 24.0;
        if k.measure(Face::Mono, 11.0, &status_text.to_uppercase()) + 12.0 <= status_w {
            k.status_line(x0, top + 17.0, dot, &status_text);
        } else {
            // A narrow screen: the version, and the state under it.
            k.status_line(
                x0,
                top + 10.0,
                dot,
                &format!("Minecraft {}", horizon_core::VANILLA_VERSION),
            );
            k.spaced(
                Face::Mono,
                11.0,
                14.0,
                x0 + 16.0,
                top + 26.0,
                0.66,
                col::INK_SOFT,
                state,
            );
        }
        let body = top + 60.0;
        if installing || (!installed && !failed) {
            self.draw_install(
                k,
                [x0, body, w0, my + mh - 16.0 - body],
                portrait,
                installing,
            );
            return;
        }
        if failed {
            self.draw_install_error(k, [x0, body, w0, my + mh - 16.0 - body], portrait);
            return;
        }
        // Left column: wordmark, a line about the profile, Play and the profile.
        let card_w = if portrait {
            w0
        } else {
            (w0 * 0.43).clamp(260.0, 320.0)
        };
        let left_w = if portrait { w0 } else { w0 - 20.0 - card_w };
        let display = if portrait { 44.0 } else { 52.0 };
        k.text(
            Face::PixelDisplay,
            display,
            display,
            x0,
            body + if portrait { 20.0 } else { 4.0 },
            col::INK,
            "Horizon",
        );
        let profile = self.profile_name();
        let last = self.view.worlds.first();
        let line = match last {
            Some(world) => tf(
                "Instance «{}». Last world: «{}», {}.",
                &[&profile, &world.name, &when(world.modified)],
            ),
            None => tf(
                "Instance «{}». No worlds yet: create one from Play.",
                &[&profile],
            ),
        };
        let para_y = body + if portrait { 72.0 } else { 64.0 };
        k.paragraph(
            Face::BodyMedium,
            14.0,
            20.3,
            x0,
            para_y,
            left_w.min(330.0),
            2,
            col::INK_SOFT,
            &line,
        );
        let actions_y = para_y + 58.6;
        let play_w = k
            .button_width(Style::PrimaryXl, Some(Icon::Pixel("play")), t("Play"))
            .max(184.0);
        let card_h = 234.0;
        let card = if portrait {
            // Portrait stacks from the bottom, within thumb reach: Play last, the
            // profile above it, the last-world card above that (each optional
            // except Play; the card is off by default).
            let mut bottom = my + mh - 16.0;
            bottom -= 56.0;
            k.button(
                [x0, bottom, w0, 56.0],
                Style::PrimaryXl,
                Some(Icon::Pixel("play")),
                t("Play"),
                Some(Action::Play),
            );
            if !self.catalog.config.home_hide_profile {
                bottom -= 66.0;
                k.dropdown(
                    [x0, bottom, w0, 56.0],
                    Some(t("Instance")),
                    &profile,
                    Action::Profile,
                );
            }
            if !self.catalog.config.home_last_world || bottom - 12.0 - card_h < para_y + 60.0 {
                return;
            }
            [x0, bottom - 12.0 - card_h, w0, card_h]
        } else {
            let profile_beside = play_w + 10.0 + 150.0 <= left_w;
            k.button(
                [x0, actions_y, play_w, 56.0],
                Style::PrimaryXl,
                Some(Icon::Pixel("play")),
                t("Play"),
                Some(Action::Play),
            );
            let profile_rect = if profile_beside {
                [
                    x0 + play_w + 10.0,
                    actions_y,
                    (left_w - play_w - 10.0).min(200.0),
                    56.0,
                ]
            } else {
                [x0, actions_y + 66.0, left_w.min(260.0), 56.0]
            };
            k.dropdown(profile_rect, Some(t("Instance")), &profile, Action::Profile);
            [x0 + w0 - card_w, body, card_w, card_h]
        };
        let [cx, cy, cw, _] = card;
        k.panel(card, true);
        let ix = cx + 16.0;
        let iw = cw - 32.0;
        k.section_label(ix, cy + 14.0, t("Last world"));
        if let Some(world) = self.view.worlds.first() {
            thumb(k, ix, cy + 36.0, &world.name);
            let detail = format!(
                "{} · {}",
                mode_name(world.mode, world.hardcore),
                when(world.modified)
            );
            let name = world.name.clone();
            k.text_fit(
                Face::BodySemiBold,
                15.0,
                18.0,
                ix + 76.0,
                cy + 38.0,
                iw - 76.0,
                col::INK,
                &name,
            );
            k.text_fit(
                Face::Body,
                12.0,
                14.0,
                ix + 76.0,
                cy + 56.0,
                iw - 76.0,
                col::MUTED,
                &detail,
            );
            k.button(
                [ix, cy + 80.0, iw, 48.0],
                Style::Secondary,
                None,
                t("Continue"),
                Some(Action::ContinueWorld),
            );
        } else {
            k.paragraph(
                Face::BodyMedium,
                13.0,
                18.85,
                ix,
                cy + 40.0,
                iw,
                3,
                col::MUTED,
                t("Your worlds appear here. Press Play, then Singleplayer, to create one."),
            );
        }
        kit::fill(k.ui, ix, cy + 140.0, iw, 2.0, col::RULE);
        k.section_label(ix, cy + 152.0, t("Quick join a server"));
        let join_w = k.button_width(Style::Secondary, None, t("Join"));
        let server = self.catalog.config.server.clone();
        let editing = self.catalog.editing == Some("server");
        let caret = k.field(
            [ix, cy + 172.0, iw - join_w - 8.0, 48.0],
            Some("server"),
            &server,
            t("Server address"),
            Action::Field("server"),
            editing,
        );
        self.caret(k, editing, caret, cy + 172.0);
        let join_rect = [ix + iw - join_w, cy + 172.0, join_w, 48.0];
        if server.trim().is_empty() {
            k.button(join_rect, Style::Disabled, None, t("Join"), None);
        } else {
            k.button(
                join_rect,
                Style::Secondary,
                None,
                t("Join"),
                Some(Action::JoinServer),
            );
        }
    }

    fn profile_name(&self) -> String {
        instance_name(&crate::instances::get(
            &self.home,
            self.catalog.config.profile,
        ))
    }

    /// A blinking caret after a field's text while it is being edited.
    fn caret(&self, k: &mut Kit, editing: bool, x: f32, field_y: f32) {
        let blink = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .is_ok_and(|t| t.as_millis() % 1000 < 500);
        if editing && blink {
            kit::fill(k.ui, x + 1.0, field_y + 14.0, 2.0, 20.0, col::INK);
        }
    }

    fn draw_install(&mut self, k: &mut Kit, r: [f32; 4], portrait: bool, installing: bool) {
        use horizon_client::official_install::InstallStage;
        let [x, y, w, h] = r;
        let progress = *self.install.lock().unwrap();
        let panel_h = if portrait { h } else { h.min(262.0) };
        k.panel([x, y, w, panel_h], true);
        let ix = x + 16.0;
        let iw = w - 32.0;
        k.text_fit(
            Face::BodyBold,
            18.0,
            22.5,
            ix,
            y + 14.0,
            iw,
            col::INK,
            if installing {
                t("Getting ready for the first start")
            } else {
                t("Minecraft 26.3 is not installed yet")
            },
        );
        let subtitle = tf(
            "Minecraft {} · instance «{}»",
            &[horizon_core::VANILLA_VERSION, &self.profile_name()],
        );
        k.text_fit(
            Face::Mono,
            12.0,
            16.0,
            ix,
            y + 38.5,
            iw,
            col::MUTED,
            &subtitle,
        );
        let body = y + 64.5;
        let (left_w, rx, rw, ry) = if portrait {
            (iw, ix, iw, body + 6.0 * 30.0 + 10.0)
        } else {
            let left = (iw - 20.0) / 2.1;
            (left, ix + left + 20.0, iw - left - 20.0, body)
        };
        let current = InstallStage::ALL
            .iter()
            .position(|s| *s == progress.stage)
            .unwrap_or(0);
        const STAGES: [&str; 6] = [
            "Checking the version",
            "Downloading resources and sounds",
            "Unpacking",
            "Preparing worlds",
            "Touch controls",
            "Finishing",
        ];
        for (i, label) in STAGES.iter().enumerate() {
            let sy = body + i as f32 * 30.0;
            let (state_color, face) = if !installing {
                (col::MUTED, Face::BodyMedium)
            } else if i < current {
                (col::INK_SOFT, Face::BodyMedium)
            } else if i == current {
                (col::INK, Face::BodyBold)
            } else {
                (col::MUTED, Face::BodyMedium)
            };
            if installing && i < current {
                k.ui.icon("check", ix, sy + 2.0, 24.0, col::SUCCESS);
            } else if installing && i == current {
                k.ui.icon("download", ix, sy + 2.0, 24.0, col::ACCENT_HI);
            } else {
                let n = (i + 1).to_string();
                let nw = k.measure(Face::Mono, 12.0, &n);
                k.text(
                    Face::Mono,
                    12.0,
                    16.0,
                    ix + 12.0 - nw / 2.0,
                    sy + 6.0,
                    col::MUTED,
                    &n,
                );
            }
            k.text_fit(
                face,
                14.0,
                17.0,
                ix + 34.0,
                sy + 5.5,
                left_w - 34.0,
                state_color,
                t(label),
            );
        }
        // Right column: size, free space, progress, Cancel or Install.
        let downloading =
            installing && progress.stage == InstallStage::Download && progress.total > 0;
        let total = if downloading {
            progress.total
        } else {
            502 * 1_048_576
        };
        k.ui.icon("download", rx, ry, 24.0, col::INK_SOFT);
        let size_line = tf("Download {}, about 1.1 GiB on disk", &[&size(total)]);
        k.text_fit(
            Face::BodyMedium,
            13.0,
            18.85,
            rx + 32.0,
            ry + 2.6,
            rw - 32.0,
            col::INK_SOFT,
            &size_line,
        );
        if let Some(free) = self.free_space() {
            let enough = free > 1_300 * 1_048_576;
            k.ui.icon(
                if enough { "check" } else { "close" },
                rx,
                ry + 32.0,
                24.0,
                if enough { col::SUCCESS } else { col::DANGER },
            );
            let line = if enough {
                tf("{} free: enough space", &[&size(free)])
            } else {
                tf("Only {} free: free up about 1.3 GiB first", &[&size(free)])
            };
            k.text_fit(
                Face::BodyMedium,
                13.0,
                18.85,
                rx + 32.0,
                ry + 34.6,
                rw - 32.0,
                if enough { col::SUCCESS } else { col::DANGER },
                &line,
            );
        }
        let py = ry + 64.0;
        if installing {
            // Bytes dominate: the download is 85% of the bar, the rest share 15%.
            let download = InstallStage::ALL
                .iter()
                .position(|s| *s == InstallStage::Download)
                .unwrap_or(1);
            let fraction = if progress.total > 0 {
                (progress.done as f32 / progress.total as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let overall = if current < download {
                0.0
            } else if current == download {
                0.85 * fraction
            } else {
                0.85 + 0.15 * (current - download) as f32 / (STAGES.len() - download) as f32
            };
            let numbers = if downloading {
                format!(
                    "{} / {}",
                    size(progress.done).replace(&format!(" {}", t("MiB")), ""),
                    size(progress.total)
                )
            } else if progress.stage == InstallStage::Controls && progress.total > 0 {
                format!("{} / {}", progress.done, progress.total)
            } else {
                t(STAGES[current.min(5)]).to_owned()
            };
            k.text_fit(
                Face::Mono,
                16.0,
                21.0,
                rx,
                py,
                rw - 60.0,
                col::INK,
                &numbers,
            );
            let pct = format!("{}%", (overall * 100.0).round() as u32);
            let pw = k.measure(Face::Mono, 13.0, &pct);
            k.text(
                Face::Mono,
                13.0,
                17.0,
                rx + rw - pw,
                py + 3.0,
                col::ACCENT_HI,
                &pct,
            );
            let bar_y = py + 27.0;
            kit::fill(k.ui, rx, bar_y, rw, 12.0, col::RULE);
            let fw = rw * overall;
            kit::fill(k.ui, rx, bar_y, fw, 2.0, col::ACCENT_HI);
            kit::fill(k.ui, rx, bar_y + 2.0, fw, 10.0, col::ACCENT);
            let mut gap = 10.0;
            while gap < rw {
                kit::fill(k.ui, rx + gap, bar_y, 2.0, 12.0, col::PLATE);
                gap += 12.0;
            }
            // Speed and time left, from the first download sample.
            let sub = if downloading {
                let (at, start) = *self
                    .view
                    .install_rate
                    .get_or_insert((Instant::now(), progress.done));
                let seconds = at.elapsed().as_secs_f32();
                if seconds > 2.0 && progress.done > start {
                    let rate = (progress.done - start) as f32 / seconds;
                    let left = (progress.total.saturating_sub(progress.done)) as f32 / rate;
                    let minutes = (left / 60.0).ceil().max(1.0) as u32;
                    tf(
                        "{}/s · about {} min left",
                        &[&size(rate as u64), &minutes.to_string()],
                    )
                } else {
                    t("Measuring the speed…").into()
                }
            } else {
                t("Keep Horizon open; downloaded files are kept for a retry.").into()
            };
            k.text_fit(
                Face::BodyMedium,
                12.0,
                17.4,
                rx,
                bar_y + 16.0,
                rw,
                col::MUTED,
                &sub,
            );
            let cancel_w =
                k.button_width(Style::Secondary, Some(Icon::Pixel("close")), t("Cancel"));
            k.button(
                [
                    rx + rw - cancel_w,
                    y + panel_h - 16.0 - 48.0,
                    cancel_w,
                    48.0,
                ],
                Style::Secondary,
                Some(Icon::Pixel("close")),
                t("Cancel"),
                Some(Action::CancelInstall),
            );
        } else {
            self.view.install_rate = None;
            k.paragraph(
                Face::BodyMedium,
                13.0,
                18.85,
                rx,
                py,
                rw,
                3,
                col::MUTED,
                t("Horizon downloads the game files from Mojang's servers and checks every file. You need about 1.3 GiB free."),
            );
            let install_w =
                k.button_width(Style::Primary, Some(Icon::Pixel("download")), t("Install"));
            k.button(
                [
                    rx + rw - install_w,
                    y + panel_h - 16.0 - 48.0,
                    install_w,
                    48.0,
                ],
                Style::Primary,
                Some(Icon::Pixel("download")),
                t("Install"),
                Some(Action::Play),
            );
        }
    }

    fn draw_install_error(&mut self, k: &mut Kit, r: [f32; 4], portrait: bool) {
        let [x, y, w, h] = r;
        let ix = x + 16.0;
        let iw = w - 32.0;
        let error = self.error.clone().unwrap_or_default();
        let cancelled = error == horizon_client::official_install::CANCELLED_MESSAGE;
        let para_w = iw.min(560.0);
        let title = if cancelled {
            t("Download cancelled")
        } else {
            t("Couldn't download the resources")
        };
        let body = if cancelled {
            t("Downloaded files are kept: Retry continues from where it stopped.")
        } else {
            t(
                "Downloaded files are kept: Retry continues from where it stopped. Check the connection, VPN or Private DNS.",
            )
        };
        let lines = k.wrap(Face::BodyMedium, 14.0, body, para_w).len().min(3) as f32;
        let panel_h = if portrait {
            h
        } else {
            (14.0 + 24.0 + 8.0 + lines * 20.3 + 6.0 + 18.85 + 32.0 + 12.0 + 48.0 + 14.0).min(h)
        };
        k.panel([x, y, w, panel_h], true);
        kit::icon(
            k.ui,
            Icon::Sprite(kit::ALERT),
            ix,
            y + 14.0,
            24.0,
            col::DANGER,
        );
        k.text_fit(
            Face::BodyBold,
            18.0,
            22.5,
            ix + 34.0,
            y + 14.8,
            iw - 34.0,
            col::INK,
            title,
        );
        let mut cy = y + 46.0;
        cy += k.paragraph(
            Face::BodyMedium,
            14.0,
            20.3,
            ix,
            cy,
            para_w,
            3,
            col::INK_SOFT,
            body,
        ) + 6.0;
        cy += k.paragraph(
            Face::BodyMedium,
            13.0,
            18.85,
            ix,
            cy,
            para_w,
            2,
            col::MUTED,
            t("If it keeps failing, clear the download cache: everything downloads again."),
        );
        if !cancelled {
            let code = k.ui.fit_text(Face::Mono, 12.0, &error, para_w - 16.0);
            let cw = k.measure(Face::Mono, 12.0, &code) + 16.0;
            kit::fill(k.ui, ix, cy + 2.0, cw, 24.0, col::SIDE);
            k.text(
                Face::Mono,
                12.0,
                16.0,
                ix + 8.0,
                cy + 6.0,
                col::INK_SOFT,
                &code,
            );
            cy += 38.0;
        } else {
            cy += 12.0;
        }
        let retry_w = k.button_width(Style::Primary, Some(Icon::Pixel("reload")), t("Retry"));
        k.button(
            [ix, cy, retry_w, 48.0],
            Style::Primary,
            Some(Icon::Pixel("reload")),
            t("Retry"),
            Some(Action::Play),
        );
        let clear_w = k.button_width(
            Style::Secondary,
            Some(Icon::Pixel("trash")),
            t("Clear download cache"),
        );
        let (clear_x, clear_y) = if ix + retry_w + 8.0 + clear_w <= ix + iw {
            (ix + retry_w + 8.0, cy)
        } else {
            (ix, cy + 56.0)
        };
        k.button(
            [clear_x, clear_y, clear_w.min(iw), 48.0],
            Style::Secondary,
            Some(Icon::Pixel("trash")),
            t("Clear download cache"),
            Some(Action::ClearDownloadCache),
        );
    }

    // ---- Add-ons ----

    fn draw_addons(&mut self, k: &mut Kit, main: [f32; 4], portrait: bool) {
        use horizon_client::content_files::Kind;
        let [mx, my, mw, mh] = main;
        if self.catalog.page == Page::Installed {
            self.catalog.page = Page::Mods;
            self.view.addons_installed = true;
        }
        let page = self.catalog.page;
        let installed_view = self.view.addons_installed;
        sky::band(k.ui, [mx, my, mw, 72.0], self.sky_time());
        let x0 = mx + 16.0;
        let w0 = mw - 32.0;
        let top = my + if portrait { 14.0 } else { 18.0 };
        k.text(
            Face::Pixel,
            32.0,
            32.0,
            x0,
            top + if portrait { 2.0 } else { 10.0 },
            col::INK,
            t("Add-ons"),
        );
        let kinds = [
            (
                Some("package"),
                t("Mods"),
                Action::Page(Page::Mods),
                page == Page::Mods,
            ),
            (
                Some("image"),
                t("Resource packs"),
                Action::Page(Page::Packs),
                page == Page::Packs,
            ),
            (
                Some("star"),
                t("Shaders"),
                Action::Page(Page::Shaders),
                page == Page::Shaders,
            ),
        ];
        let natural: f32 = kinds
            .iter()
            .map(|(_, l, _, _)| 58.0 + k.measure(Face::BodySemiBold, 14.0, l))
            .sum();
        let title_w = k.measure(Face::Pixel, 32.0, t("Add-ons"));
        let mut y = top + 60.0;
        if portrait || natural + title_w + 20.0 > w0 {
            k.segmented([x0, top + 42.0, w0, 48.0], &kinds, true);
            y = top + 102.0;
        } else {
            k.segmented([x0 + w0 - natural, top + 2.0, natural, 48.0], &kinds, false);
        }
        // Toolbar: Browse / Installed, the search field, and the pager or import.
        // Narrow screens give the field its own row and move the pager under the list.
        let narrow = portrait || w0 < 600.0;
        let tabs = [
            (None, t("Browse"), Action::AddonView(false), !installed_view),
            (
                None,
                t("Installed"),
                Action::AddonView(true),
                installed_view,
            ),
        ];
        let tabs_w = if narrow {
            w0
        } else {
            tabs.iter()
                .map(|(_, l, _, _)| 28.0 + k.measure(Face::BodySemiBold, 14.0, l))
                .sum::<f32>()
        };
        k.segmented([x0, y, tabs_w, 48.0], &tabs, narrow);
        let kind = match page {
            Page::Shaders => Kind::Shaders,
            Page::Packs => Kind::Packs,
            _ => Kind::Mods,
        };
        let (field_x, field_y) = if narrow {
            (x0, y + 58.0)
        } else {
            (x0 + tabs_w + 10.0, y)
        };
        let import_w = k.button_width(
            Style::Secondary,
            Some(Icon::Pixel("plus")),
            t("Import file"),
        );
        let right_w = match (installed_view, narrow) {
            (true, true) => 48.0 + 8.0 + 48.0,
            (true, false) => import_w + 8.0 + 48.0,
            (false, true) => 0.0,
            (false, false) => PAGER_W,
        };
        let field_w = x0 + w0 - field_x - if right_w > 0.0 { right_w + 10.0 } else { 0.0 };
        let placeholder = match page {
            Page::Packs => t("Search resource packs"),
            Page::Shaders => t("Search shaders"),
            _ => t("Search mods or paste an .hmod link"),
        };
        let editing = self.catalog.editing == Some("search");
        let query = self.catalog.query.clone();
        let caret = k.field(
            [field_x, field_y, field_w, 48.0],
            Some("search"),
            &query,
            placeholder,
            Action::Field("search"),
            editing,
        );
        self.caret(k, editing, caret, field_y);
        let rx = field_x + field_w + 10.0;
        if installed_view {
            let iw = if narrow { 48.0 } else { import_w };
            k.button(
                [rx, field_y, iw, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("plus")),
                if narrow { "" } else { t("Import file") },
                Some(Action::Import(kind)),
            );
            k.button(
                [rx + iw + 8.0, field_y, 48.0, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("folder")),
                "",
                Some(Action::OpenFolder(kind)),
            );
        } else if !narrow {
            self.pager(k, rx, field_y);
        }
        // The list, and on narrow screens the pager under it.
        let pager_h = if !installed_view && narrow { 56.0 } else { 0.0 };
        let list_top = field_y + 60.0;
        let panel = [x0, list_top, w0, my + mh - 16.0 - pager_h - list_top];
        k.panel(panel, false);
        if pager_h > 0.0 {
            self.pager(k, x0 + (w0 - PAGER_W) / 2.0, panel[1] + panel[3] + 8.0);
        }
        let lx = x0 + 6.0;
        let lw = w0 - 12.0;
        let mut ly = list_top + 6.0;
        let lb = list_top + panel[3] - 6.0;
        // Source choice for mods and shaders: Horizon's registry or Modrinth.
        if !installed_view && matches!(page, Page::Mods | Page::Shaders) {
            use crate::shader_catalog::Origin;
            let current = if page == Page::Mods {
                self.catalog.config.mod_source
            } else {
                self.catalog.config.shader_source
            };
            let source = |origin: Origin| {
                if page == Page::Mods {
                    Action::ModSource(origin)
                } else {
                    Action::ShaderSource(origin)
                }
            };
            let items = [
                (
                    None,
                    "Horizon",
                    source(Origin::Horizon),
                    current == Origin::Horizon,
                ),
                (
                    None,
                    "Modrinth",
                    source(Origin::Modrinth),
                    current == Origin::Modrinth,
                ),
            ];
            let seg_w = (lw - 12.0).min(220.0);
            k.segmented([lx + 6.0, ly + 4.0, seg_w, 40.0], &items, true);
            let hint = match (page, current) {
                (Page::Mods, Origin::Modrinth) => t("Java mods: browse only, they cannot run here"),
                (Page::Mods, _) => t("Sandboxed .hmod mods"),
                (_, Origin::Modrinth) => t("Shader ZIPs: experimental"),
                _ => t("Horizon .hshader packs"),
            };
            let beside = lw - seg_w - 30.0;
            if beside >= 180.0 {
                let lines = k.wrap_max(Face::Body, 12.0, hint, beside, 2);
                let top = ly + 4.0 + (40.0 - lines.len() as f32 * 14.0) / 2.0;
                for (i, line) in lines.iter().enumerate() {
                    k.text(
                        Face::Body,
                        12.0,
                        14.0,
                        lx + seg_w + 18.0,
                        top + i as f32 * 14.0,
                        col::MUTED,
                        line,
                    );
                }
                ly += 52.0;
            } else {
                let lines = k.wrap_max(Face::Body, 12.0, hint, lw - 12.0, 2);
                for (i, line) in lines.iter().enumerate() {
                    k.text(
                        Face::Body,
                        12.0,
                        14.0,
                        lx + 6.0,
                        ly + 52.0 + i as f32 * 14.0,
                        col::MUTED,
                        line,
                    );
                }
                ly += 52.0 + lines.len() as f32 * 14.0 + 8.0;
            }
        }
        let rows = if installed_view {
            self.addon_installed_rows(k, kind, [lx, ly, lw, lb - ly])
        } else {
            self.addon_browse_rows(k, kind, [lx, ly, lw, lb - ly])
        };
        if rows == 0 {
            self.addon_empty(k, kind, [lx, ly, lw, lb - ly], installed_view);
        }
    }

    /// Previous · «Page n» · Next, `PAGER_W` wide.
    fn pager(&mut self, k: &mut Kit, x: f32, y: f32) {
        k.button(
            [x, y, 48.0, 48.0],
            Style::Ghost,
            Some(Icon::Sprite(kit::CHEVRON_LEFT)),
            "",
            (self.catalog.offset > 0).then_some(Action::Previous),
        );
        let label = tf("Page {}", &[&(self.catalog.offset / 12 + 1).to_string()]);
        let lw = k.measure(Face::Mono, 12.0, &label);
        k.text(
            Face::Mono,
            12.0,
            16.0,
            x + 52.0 + (84.0 - lw) / 2.0,
            y + 16.0,
            col::INK_SOFT,
            &label,
        );
        k.button(
            [x + 140.0, y, 48.0, 48.0],
            Style::Ghost,
            Some(Icon::Pixel("chevron-right")),
            "",
            (self.catalog.results.len() >= 12).then_some(Action::Next),
        );
    }

    fn list_scroll(&mut self, top: f32, bottom: f32, content: f32) -> f32 {
        let maximum = (content - (bottom - top)).max(0.0);
        self.catalog.scroll = self.catalog.scroll.clamp(0.0, maximum);
        self.list_view = Some([top, bottom, maximum]);
        self.catalog.scroll
    }

    fn addon_browse_rows(
        &mut self,
        k: &mut Kit,
        _kind: horizon_client::content_files::Kind,
        [x, y, w, h]: [f32; 4],
    ) -> usize {
        let page = self.catalog.page;
        // One entry per row: (seed, title, detail, trailing).
        enum Trailing {
            Toggle(&'static str, bool),
            Install(usize, &'static str),
            State(String),
        }
        let mut entries: Vec<(String, String, String, Trailing)> = Vec::new();
        if page == Page::Mods
            && self.catalog.config.mod_source == crate::shader_catalog::Origin::Horizon
        {
            let query = self.catalog.query.to_lowercase();
            for (id, title, description) in horizon_client::mod_list::ANDROID_BUNDLED {
                if id.contains(&query) || title.to_lowercase().contains(&query) {
                    let enabled = self.catalog.bundled_enabled.contains(id);
                    entries.push((
                        id.to_owned(),
                        title.to_owned(),
                        t_dyn(description),
                        Trailing::Toggle(id, enabled),
                    ));
                }
            }
        }
        for (i, p) in self.catalog.results.iter().enumerate() {
            if page == Page::Mods
                && p.source == crate::browser::SourceId::Horizon
                && horizon_client::mod_list::ANDROID_BUNDLED
                    .iter()
                    .any(|(id, _, _)| *id == p.id)
            {
                continue;
            }
            let java = p.kind == crate::browser::Kind::Mod
                && p.source == crate::browser::SourceId::Modrinth;
            let trailing = if let Some(state) = self.catalog.download_label(p) {
                Trailing::State(state)
            } else if java {
                Trailing::State(t("Java mod").to_owned())
            } else if crate::catalog::source_only_mod(p) {
                Trailing::State(t(".hnmod: later").to_owned())
            } else if p.kind == crate::browser::Kind::Mod && !cfg!(target_pointer_width = "64") {
                Trailing::State(t("64-bit only").to_owned())
            } else {
                Trailing::Install(i, static_label(self.catalog.install_button(p)))
            };
            let detail = if p.downloads > 0 {
                format!(
                    "{} · {} {}",
                    p.author,
                    short_count(p.downloads),
                    t("downloads")
                )
            } else {
                p.author.clone()
            };
            entries.push((p.id.clone(), p.title.clone(), detail, trailing));
        }
        if entries.is_empty() {
            return 0;
        }
        // Wide buttons where there is room; on narrow rows an icon, so the title
        // keeps its width.
        let label_w = k.button_width(
            Style::Secondary,
            Some(Icon::Pixel("download")),
            t("Install"),
        );
        let icon_only = w - 64.0 - 24.0 - label_w < w * 0.5;
        let bw = if icon_only { 48.0 } else { label_w };
        let text_w = w - 64.0 - 12.0 - bw - 12.0;
        let layout: Vec<(Vec<String>, Vec<String>, f32)> = entries
            .iter()
            .map(|(_, title, detail, _)| {
                let title = k.wrap_max(Face::BodySemiBold, 15.0, title, text_w, 2);
                let detail = k.wrap_max(Face::Body, 12.0, detail, text_w, 2);
                let height =
                    (16.0 + title.len() as f32 * 18.0 + 2.0 + detail.len() as f32 * 14.0).max(64.0);
                (title, detail, height)
            })
            .collect();
        let status_h = if self.catalog.status.is_empty() {
            0.0
        } else {
            34.0
        };
        let content = layout.iter().map(|l| l.2).sum::<f32>() + status_h;
        let scroll = self.list_scroll(y, y + h, content);
        let clip = k.mark();
        let mut ry = y - scroll;
        for (row, ((seed, _, _, trailing), (title, detail, rh))) in
            entries.iter().zip(&layout).enumerate()
        {
            let button = [x + w - 12.0 - bw, ry + (rh - 48.0) / 2.0, bw, 48.0];
            // Built-in mods switch on and off like installed ones.
            let switch = [x + w - 12.0 - 60.0, ry + (rh - 36.0) / 2.0, 60.0, 36.0];
            match trailing {
                Trailing::Toggle(id, _) => {
                    self.list_targets.push((switch, Action::ToggleBundled(id)))
                }
                Trailing::Install(i, _) => self.list_targets.push((button, Action::Install(*i))),
                Trailing::State(_) => {}
            }
            if ry + rh > y && ry < y + h {
                divider(k, row + 1, [x, ry, w]);
                identicon(k, x + 12.0, ry + (rh - 40.0) / 2.0, seed);
                let mut ty =
                    ry + (rh - title.len() as f32 * 18.0 - 2.0 - detail.len() as f32 * 14.0) / 2.0;
                for line in title {
                    k.text(Face::BodySemiBold, 15.0, 18.0, x + 64.0, ty, col::INK, line);
                    ty += 18.0;
                }
                ty += 2.0;
                for line in detail {
                    k.text(Face::Body, 12.0, 14.0, x + 64.0, ty, col::MUTED, line);
                    ty += 14.0;
                }
                match trailing {
                    Trailing::Toggle(id, enabled) => {
                        k.switch(switch[0], switch[1], *enabled, Action::ToggleBundled(id));
                    }
                    Trailing::Install(i, label) => k.button(
                        button,
                        Style::Secondary,
                        Some(Icon::Pixel("download")),
                        if icon_only { "" } else { t(label) },
                        Some(Action::Install(*i)),
                    ),
                    Trailing::State(state) => {
                        let lines = k.wrap_max(Face::BodyMedium, 12.0, state, bw.max(96.0), 2);
                        let mut sy = ry + (rh - lines.len() as f32 * 15.0) / 2.0;
                        for line in &lines {
                            let sw = k.measure(Face::BodyMedium, 12.0, line);
                            k.text(
                                Face::BodyMedium,
                                12.0,
                                15.0,
                                x + w - 12.0 - sw,
                                sy,
                                col::MUTED,
                                line,
                            );
                            sy += 15.0;
                        }
                    }
                }
            }
            ry += rh;
        }
        if status_h > 0.0 && ry + 6.0 < y + h {
            let status = t_dyn(&self.catalog.status);
            k.paragraph(
                Face::Body,
                12.0,
                14.0,
                x + 12.0,
                ry + 6.0,
                w - 24.0,
                2,
                col::MUTED,
                &status,
            );
        }
        k.clip(clip, [x, y, w, h]);
        entries.len()
    }

    fn addon_installed_rows(
        &mut self,
        k: &mut Kit,
        kind: horizon_client::content_files::Kind,
        [x, y, w, h]: [f32; 4],
    ) -> usize {
        let query = self.catalog.query.to_lowercase();
        let items: Vec<usize> = self
            .catalog
            .local
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == kind && e.title.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        if items.is_empty() {
            return 0;
        }
        let narrow =
            kind == horizon_client::content_files::Kind::Mods && !cfg!(target_pointer_width = "64");
        let remove_label = k.button_width(Style::Danger, Some(Icon::Pixel("trash")), t("Remove"));
        let icon_only = w - 64.0 - 24.0 - 68.0 - remove_label < w * 0.5;
        let remove_w = if icon_only { 48.0 } else { remove_label };
        let switch_w = if narrow { 0.0 } else { 68.0 };
        let text_w = w - 64.0 - 12.0 - switch_w - remove_w - 12.0;
        let layout: Vec<(Vec<String>, f32)> = items
            .iter()
            .map(|i| {
                let title = k.wrap_max(
                    Face::BodySemiBold,
                    15.0,
                    &self.catalog.local[*i].title,
                    text_w,
                    2,
                );
                let height = (16.0 + title.len() as f32 * 18.0 + 2.0 + 14.0).max(64.0);
                (title, height)
            })
            .collect();
        let scroll = self.list_scroll(y, y + h, layout.iter().map(|l| l.1).sum());
        let clip = k.mark();
        let mut ry = y - scroll;
        for (row, (i, (title, rh))) in items.iter().copied().zip(&layout).enumerate() {
            let rh = *rh;
            let remove = [
                x + w - 12.0 - remove_w,
                ry + (rh - 48.0) / 2.0,
                remove_w,
                48.0,
            ];
            let switch = [remove[0] - 68.0, ry + (rh - 36.0) / 2.0, 60.0, 36.0];
            if !narrow {
                self.list_targets.push((switch, Action::ToggleLocal(i)));
            }
            self.list_targets.push((remove, Action::DeleteLocal(i)));
            if ry + rh > y && ry < y + h {
                divider(k, row + 1, [x, ry, w]);
                let (enabled, file) = (
                    self.catalog.local[i].enabled,
                    self.catalog.local[i].file.clone(),
                );
                identicon(k, x + 12.0, ry + (rh - 40.0) / 2.0, &file);
                let mut ty = ry + (rh - title.len() as f32 * 18.0 - 16.0) / 2.0;
                for line in title {
                    k.text(Face::BodySemiBold, 15.0, 18.0, x + 64.0, ty, col::INK, line);
                    ty += 18.0;
                }
                let (dot, state) = if narrow {
                    (col::MUTED, t("64-bit only"))
                } else if enabled {
                    (col::SUCCESS, t("Enabled"))
                } else {
                    (col::MUTED, t("Disabled"))
                };
                kit::fill(k.ui, x + 64.0, ty + 5.0, 8.0, 8.0, dot);
                k.text(Face::Body, 12.0, 14.0, x + 78.0, ty + 2.0, dot, state);
                if !narrow {
                    k.switch(switch[0], switch[1], enabled, Action::ToggleLocal(i));
                }
                k.button(
                    remove,
                    Style::Danger,
                    Some(Icon::Pixel("trash")),
                    if icon_only { "" } else { t("Remove") },
                    Some(Action::DeleteLocal(i)),
                );
            }
            ry += rh;
        }
        k.clip(clip, [x, y, w, h]);
        items.len()
    }

    /// The empty list: an icon, a title, a line of help and the actions, shrunk to
    /// fit the panel (the icon goes first, then help lines), never past its edge.
    fn addon_empty(
        &mut self,
        k: &mut Kit,
        kind: horizon_client::content_files::Kind,
        [x, y, w, h]: [f32; 4],
        installed: bool,
    ) {
        let searching = !installed && self.catalog.status.starts_with("Searching");
        let (icon, title, body) = if searching {
            (
                "reload",
                t("Searching…"),
                t("Results appear here in a moment."),
            )
        } else if installed {
            (
                "package",
                t("Nothing installed yet"),
                t("Install add-ons from Browse, or import a file from the phone."),
            )
        } else {
            (
                "search",
                t("Nothing found"),
                t("Try another search or import a file from the phone."),
            )
        };
        let pw = (w - 24.0).min(420.0);
        let import_w = k.button_width(
            Style::Secondary,
            Some(Icon::Pixel("plus")),
            t("Import file"),
        );
        let clear_w = k.button_width(Style::Secondary, None, t("Clear search"));
        let both = !installed && !self.catalog.query.is_empty();
        let buttons_row = if both {
            clear_w + 8.0 + import_w
        } else {
            import_w
        };
        // Buttons stack when they do not fit side by side.
        let buttons_h = if searching {
            0.0
        } else if buttons_row > w - 24.0 && both {
            48.0 + 8.0 + 48.0 + 6.0
        } else {
            48.0 + 6.0
        };
        let mut lines = k.wrap_max(Face::BodyMedium, 14.0, body, pw, usize::MAX);
        let mut show_icon = true;
        let block = |icon: bool, lines: usize| {
            (if icon { 56.0 } else { 0.0 }) + 22.0 + 8.0 + lines as f32 * 20.3 + buttons_h
        };
        if block(true, lines.len()) > h - 8.0 {
            show_icon = false;
        }
        if block(show_icon, lines.len()) > h - 8.0 {
            lines = k.wrap_max(Face::BodyMedium, 14.0, body, pw, 1);
        }
        let mut cy = y + ((h - block(show_icon, lines.len())) / 2.0).max(4.0);
        if show_icon {
            k.ui.icon(icon, x + (w - 48.0) / 2.0, cy, 48.0, col::MUTED);
            cy += 56.0;
        }
        let tw = k.measure(Face::BodyBold, 18.0, title);
        k.text(
            Face::BodyBold,
            18.0,
            22.0,
            x + (w - tw) / 2.0,
            cy,
            col::INK,
            title,
        );
        cy += 30.0;
        for line in &lines {
            let lw = k.measure(Face::BodyMedium, 14.0, line);
            k.text(
                Face::BodyMedium,
                14.0,
                20.3,
                x + (w - lw) / 2.0,
                cy,
                col::INK_SOFT,
                line,
            );
            cy += 20.3;
        }
        if searching {
            return;
        }
        cy += 6.0;
        if !both {
            k.button(
                [x + (w - import_w) / 2.0, cy, import_w, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("plus")),
                t("Import file"),
                Some(Action::Import(kind)),
            );
        } else if buttons_row <= w - 24.0 {
            let bx = x + (w - buttons_row) / 2.0;
            k.button(
                [bx, cy, clear_w, 48.0],
                Style::Secondary,
                None,
                t("Clear search"),
                Some(Action::ClearSearch),
            );
            k.button(
                [bx + clear_w + 8.0, cy, import_w, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("plus")),
                t("Import file"),
                Some(Action::Import(kind)),
            );
        } else {
            k.button(
                [x + (w - clear_w) / 2.0, cy, clear_w, 48.0],
                Style::Secondary,
                None,
                t("Clear search"),
                Some(Action::ClearSearch),
            );
            k.button(
                [x + (w - import_w) / 2.0, cy + 56.0, import_w, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("plus")),
                t("Import file"),
                Some(Action::Import(kind)),
            );
        }
    }

    // ---- Worlds ----

    fn draw_worlds(&mut self, k: &mut Kit, main: [f32; 4], portrait: bool) {
        let [mx, my, mw, mh] = main;
        sky::band(k.ui, [mx, my, mw, 72.0], self.sky_time());
        let x0 = mx + 16.0;
        let w0 = mw - 32.0;
        let top = my + if portrait { 16.0 } else { 12.0 };
        let heading = t("Worlds & files");
        k.text(Face::Pixel, 32.0, 32.0, x0, top, col::INK, heading);
        let total: u64 = self.view.worlds.iter().map(|w| w.size).sum();
        let mut subtitle = format!(
            "{} · {}",
            match self.view.worlds.len() {
                1 => t("1 world").to_owned(),
                n => tf("{} worlds", &[&n.to_string()]),
            },
            size(total)
        );
        if let Some(free) = self.free_space() {
            subtitle.push_str(&format!(" · {}", tf("{} free", &[&size(free)])));
        }
        // Import and Open in Files: beside the title when there is room, else
        // under it, side by side when both labels fit, one per row when not.
        let (import, files) = (t("Import world"), t("Open in Files"));
        let import_w = k.button_width(Style::Secondary, Some(Icon::Pixel("plus")), import);
        let files_w = k.button_width(Style::Secondary, Some(Icon::Pixel("folder")), files);
        let heading_w =
            k.measure(Face::Pixel, 32.0, heading)
                .max(k.measure(Face::Mono, 12.0, &subtitle));
        let beside = !portrait && heading_w + 24.0 + import_w + 8.0 + files_w <= w0;
        let half = (w0 - 8.0) / 2.0;
        let buttons = if beside {
            [
                [x0 + w0 - files_w - 8.0 - import_w, top, import_w, 48.0],
                [x0 + w0 - files_w, top, files_w, 48.0],
            ]
        } else if half >= import_w.max(files_w) {
            [
                [x0, top + 60.0, half, 48.0],
                [x0 + half + 8.0, top + 60.0, half, 48.0],
            ]
        } else {
            [[x0, top + 60.0, w0, 48.0], [x0, top + 116.0, w0, 48.0]]
        };
        k.text_fit(
            Face::Mono,
            12.0,
            16.0,
            x0,
            top + 32.0,
            if beside {
                buttons[0][0] - 16.0 - x0
            } else {
                w0
            },
            col::INK_SOFT,
            &subtitle,
        );
        k.button(
            buttons[0],
            Style::Secondary,
            Some(Icon::Pixel("plus")),
            import,
            Some(Action::ImportWorld),
        );
        k.button(
            buttons[1],
            Style::Secondary,
            Some(Icon::Pixel("folder")),
            files,
            Some(Action::WorldFiles),
        );
        let list_top = if beside {
            my + 92.0
        } else {
            buttons[1][1] + 48.0 + 12.0
        };
        let panel = [x0, list_top, w0, my + mh - 16.0 - list_top];
        k.panel(panel, false);
        let (lx, ly, lw, lh) = (x0 + 6.0, list_top + 6.0, w0 - 12.0, panel[3] - 12.0);
        if self.view.worlds.is_empty() {
            let title = t("No worlds yet");
            let tw = k.measure(Face::BodyBold, 18.0, title);
            let body = t(
                "Press Play, then Singleplayer, to create one, or bring one in with Import world. Copied world folders appear here too.",
            );
            // The whole text, never cut: the block centres on its own height.
            let lines: Vec<String> = k
                .wrap(Face::BodyMedium, 14.0, body, lw.min(420.0))
                .into_iter()
                .filter(|l| !l.is_empty())
                .collect();
            let block = 56.0 + 30.0 + lines.len() as f32 * 20.3;
            let cy = (ly + (lh - block) / 2.0).max(ly) + 56.0;
            k.ui.icon(
                "folder",
                lx + (lw - 48.0) / 2.0,
                cy - 56.0,
                48.0,
                col::MUTED,
            );
            k.text(
                Face::BodyBold,
                18.0,
                22.0,
                lx + (lw - tw) / 2.0,
                cy,
                col::INK,
                title,
            );
            for (i, line) in lines.iter().enumerate() {
                let w = k.measure(Face::BodyMedium, 14.0, line);
                k.text(
                    Face::BodyMedium,
                    14.0,
                    20.3,
                    lx + (lw - w) / 2.0,
                    cy + 30.0 + i as f32 * 20.3,
                    col::INK_SOFT,
                    line,
                );
            }
            return;
        }
        // The widest arrangement that still leaves a name 180 dp: labelled
        // buttons and a size column, then Export as an icon, then Play too,
        // then the size moves into the details.
        let play_label = k.button_width(Style::Secondary, Some(Icon::Pixel("play")), t("Play"));
        let export_label =
            k.button_width(Style::Secondary, Some(Icon::Pixel("archive")), t("Export"));
        let fits = |play: f32, export: f32, size_col: f32| {
            lw - 88.0 - 12.0 - play - 8.0 - export - size_col - 12.0
        };
        let (play_w, export_w, size_w) = [
            (play_label, export_label, 72.0),
            (play_label, 48.0, 72.0),
            (48.0, 48.0, 72.0),
            (48.0, 48.0, 0.0),
        ]
        .into_iter()
        .find(|&(p, e, s)| fits(p, e, s) >= 180.0)
        .unwrap_or((48.0, 48.0, 0.0));
        let compact = size_w == 0.0;
        let text_w = fits(play_w, export_w, size_w).max(40.0);
        // Names and details wrap in full; the row grows with them.
        let layout: Vec<(Vec<String>, Vec<String>, f32)> = self
            .view
            .worlds
            .iter()
            .map(|world| {
                let (mode, when_text) =
                    (mode_name(world.mode, world.hardcore), when(world.modified));
                let mut detail = if compact {
                    format!("{mode} · {when_text} · {}", size(world.size))
                } else {
                    format!("{mode} · {when_text}")
                };
                // Two worlds may share a name (an import beside the original):
                // their folders tell them apart.
                if world.folder != world.name {
                    detail = format!("{} · {detail}", world.folder);
                }
                let wrap = |face, size, text: &str| -> Vec<String> {
                    k.wrap(face, size, text, text_w)
                        .into_iter()
                        .filter(|l| !l.is_empty())
                        .collect()
                };
                let name = wrap(Face::BodySemiBold, 15.0, &world.name);
                let lines = wrap(Face::Body, 12.0, &detail);
                let text = name.len().max(1) as f32 * 18.0 + 2.0 + lines.len() as f32 * 14.0;
                (name, lines, (text + 28.0).max(64.0))
            })
            .collect();
        let content: f32 = layout.iter().map(|(_, _, h)| h).sum();
        let scroll = self.list_scroll(ly, ly + lh, content);
        let clip = k.mark();
        let mut ry = ly - scroll;
        for (i, (name_lines, lines, rh)) in layout.into_iter().enumerate() {
            let by = ry + (rh - 48.0) / 2.0;
            let play_x = lx + lw - 12.0 - play_w;
            let export_x = play_x - 8.0 - export_w;
            self.list_targets
                .push(([export_x, by, export_w, 48.0], Action::ExportWorld(i)));
            self.list_targets
                .push(([play_x, by, play_w, 48.0], Action::OpenWorld(i)));
            if ry + rh <= ly || ry >= ly + lh {
                ry += rh;
                continue;
            }
            divider(k, i + 1, [lx, ry, lw]);
            let world = &self.view.worlds[i];
            let (name, bytes) = (world.name.clone(), world.size);
            thumb(k, lx + 12.0, ry + (rh - 36.0) / 2.0, &name);
            let text_h = name_lines.len().max(1) as f32 * 18.0 + 2.0 + lines.len() as f32 * 14.0;
            let mut ty = ry + (rh - text_h) / 2.0;
            for line in &name_lines {
                // Only a single word wider than the column is cut.
                k.text_fit(
                    Face::BodySemiBold,
                    15.0,
                    18.0,
                    lx + 88.0,
                    ty,
                    text_w,
                    col::INK,
                    line,
                );
                ty += 18.0;
            }
            ty += 2.0;
            for line in &lines {
                k.text(Face::Body, 12.0, 14.0, lx + 88.0, ty, col::MUTED, line);
                ty += 14.0;
            }
            if !compact {
                let s = size(bytes);
                let sw = k.measure(Face::Mono, 12.0, &s);
                k.text(
                    Face::Mono,
                    12.0,
                    16.0,
                    export_x - 12.0 - sw,
                    ry + (rh - 16.0) / 2.0,
                    col::INK_SOFT,
                    &s,
                );
            }
            k.button(
                [export_x, by, export_w, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("archive")),
                if export_w > 48.0 { t("Export") } else { "" },
                Some(Action::ExportWorld(i)),
            );
            k.button(
                [play_x, by, play_w, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("play")),
                if play_w > 48.0 { t("Play") } else { "" },
                Some(Action::OpenWorld(i)),
            );
            ry += rh;
        }
        k.clip(clip, [lx, ly, lw, lh]);
    }

    // ---- Instances ----

    fn draw_instances(&mut self, k: &mut Kit, main: [f32; 4], portrait: bool) {
        let [mx, my, mw, mh] = main;
        sky::band(k.ui, [mx, my, mw, 72.0], self.sky_time());
        let x0 = mx + 16.0;
        let w0 = mw - 32.0;
        let top = my + if portrait { 16.0 } else { 12.0 };
        let heading = t("Instances");
        k.text(Face::Pixel, 32.0, 32.0, x0, top, col::INK, heading);
        let subtitle = t("Each has its own worlds, settings and add-ons, and its engine");
        let new = t("New instance");
        let new_w = k.button_width(Style::Secondary, Some(Icon::Pixel("plus")), new);
        let heading_w =
            k.measure(Face::Pixel, 32.0, heading)
                .max(k.measure(Face::Mono, 12.0, subtitle));
        let beside = !portrait && heading_w + 24.0 + new_w <= w0;
        let button = if beside {
            [x0 + w0 - new_w, top, new_w, 48.0]
        } else {
            [x0, top + 60.0, w0.min(new_w.max(240.0)), 48.0]
        };
        k.text_fit(
            Face::Mono,
            12.0,
            16.0,
            x0,
            top + 32.0,
            if beside { button[0] - 16.0 - x0 } else { w0 },
            col::INK_SOFT,
            subtitle,
        );
        k.button(
            button,
            Style::Secondary,
            Some(Icon::Pixel("plus")),
            new,
            Some(Action::NewInstance),
        );
        let list_top = if beside { my + 92.0 } else { button[1] + 60.0 };
        let panel = [x0, list_top, w0, my + mh - 16.0 - list_top];
        k.panel(panel, false);
        let (lx, ly, lw, lh) = (x0 + 6.0, list_top + 6.0, w0 - 12.0, panel[3] - 12.0);
        let play_label = k.button_width(Style::Secondary, Some(Icon::Pixel("play")), t("Play"));
        let edit_label =
            k.button_width(Style::Secondary, Some(Icon::Pixel("sliders")), t("Change"));
        let (play_w, edit_w) = if lw - 88.0 - play_label - edit_label - 32.0 >= 180.0 {
            (play_label, edit_label)
        } else {
            (48.0, 48.0)
        };
        let text_w = (lw - 88.0 - play_w - edit_w - 32.0).max(40.0);
        let rows: Vec<_> = self.view.instances.clone();
        let rh = 76.0;
        let scroll = self.list_scroll(ly, ly + lh, rows.len() as f32 * rh);
        let clip = k.mark();
        let mut ry = ly - scroll;
        for (i, (instance, worlds)) in rows.iter().enumerate() {
            let by = ry + (rh - 48.0) / 2.0;
            let play_x = lx + lw - 12.0 - play_w;
            let edit_x = play_x - 8.0 - edit_w;
            let id = instance.id;
            let row = [lx, ry, edit_x - 8.0 - lx, rh];
            self.list_targets.push((row, Action::SelectInstance(id)));
            self.list_targets
                .push(([edit_x, by, edit_w, 48.0], Action::EditInstance(id)));
            self.list_targets
                .push(([play_x, by, play_w, 48.0], Action::PlayInstance(id)));
            if ry + rh <= ly || ry >= ly + lh {
                ry += rh;
                continue;
            }
            let active = id == self.catalog.config.profile;
            if k.pressed(row, Some(Action::SelectInstance(id))) {
                kit::fill(k.ui, row[0], row[1], row[2], row[3], col::CONTROL_PRESSED);
            }
            if active {
                kit::fill(k.ui, lx, ry + 8.0, 3.0, rh - 16.0, col::ACCENT_HI);
            }
            divider(k, i + 1, [lx, ry, lw]);
            let name = instance_name(instance);
            thumb(k, lx + 12.0, ry + (rh - 36.0) / 2.0, &name);
            k.text_fit(
                Face::BodySemiBold,
                15.0,
                18.0,
                lx + 88.0,
                ry + 14.0,
                text_w,
                if active { col::ACCENT_HI } else { col::INK },
                &name,
            );
            let mut details = self.engine_label(instance.engine.as_deref());
            details.push_str(" · ");
            details.push_str(&match worlds {
                1 => t("1 world").to_owned(),
                n => tf("{} worlds", &[&n.to_string()]),
            });
            if let Some(at) = instance.played {
                let time = SystemTime::UNIX_EPOCH + Duration::from_secs(at);
                details.push_str(&format!(" · {}", tf("played {}", &[&when(time)])));
            }
            if active {
                details = format!("{} · {details}", t("in use"));
            }
            for (j, line) in k
                .wrap(Face::Body, 12.0, &details, text_w)
                .into_iter()
                .take(2)
                .enumerate()
            {
                k.text(
                    Face::Body,
                    12.0,
                    14.0,
                    lx + 88.0,
                    ry + 36.0 + j as f32 * 14.0,
                    col::MUTED,
                    &line,
                );
            }
            k.hit(row, Action::SelectInstance(id));
            k.button(
                [edit_x, by, edit_w, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("sliders")),
                if edit_w > 48.0 { t("Change") } else { "" },
                Some(Action::EditInstance(id)),
            );
            k.button(
                [play_x, by, play_w, 48.0],
                if active {
                    Style::Primary
                } else {
                    Style::Secondary
                },
                Some(Icon::Pixel("play")),
                if play_w > 48.0 { t("Play") } else { "" },
                Some(Action::PlayInstance(id)),
            );
            ry += rh;
        }
        k.clip(clip, [lx, ly, lw, lh]);
    }

    /// The instance sheet: its name, its engine, and Delete.
    pub(super) fn draw_instance_edit(&mut self, k: &mut Kit) {
        let Some(edit) = &self.view.instance_edit else {
            return;
        };
        let (w, h) = (k.ui.w, k.ui.h);
        kit::fill(k.ui, 0.0, 0.0, w, h, col::SCRIM);
        let typing = self.catalog.editing == Some("instance_name");
        let engines: Vec<(Option<usize>, String, String)> = std::iter::once((
            None,
            t("Built-in engine").to_owned(),
            format!("Horizon Aurora {}", crate::backend::VERSION),
        ))
        .chain(self.view.engines.iter().enumerate().map(|(i, e)| {
            let m = &e.manifest;
            (
                Some(i),
                m.name.clone(),
                format!("{} · {}", m.version, m.commit),
            )
        }))
        .collect();
        let sw = w.min(640.0);
        let wide = sw >= 560.0;
        let cols = if wide { 2 } else { 1 };
        let engine_rows = engines.len().div_ceil(cols) as f32;
        let want = if typing {
            200.0
        } else {
            80.0 + 96.0 + 28.0 + engine_rows * 56.0 + 16.0 + 64.0
        };
        let sh = want.min(h - 12.0);
        let (x, y) = ((w - sw) / 2.0, h - sh);
        k.panel([x, y, sw, sh + 8.0], false);
        kit::fill(
            k.ui,
            x + (sw - 40.0) / 2.0,
            y + 10.0,
            40.0,
            4.0,
            col::CONTROL_RING,
        );
        let title = match edit.id {
            None => t("New instance").to_owned(),
            Some(_) => tf("Instance «{}»", &[&edit.name]),
        };
        k.text_fit(
            Face::BodyBold,
            20.0,
            24.0,
            x + 16.0,
            y + 34.0,
            sw - 96.0,
            col::INK,
            &title,
        );
        k.button(
            [x + sw - 16.0 - 48.0, y + 22.0, 48.0, 48.0],
            Style::Ghost,
            Some(Icon::Pixel("close")),
            "",
            Some(Action::CloseInstanceEdit),
        );
        let (bx, bw) = (x + 16.0, sw - 32.0);
        let mut cy = y + 80.0;
        k.text(Face::Body, 12.0, 17.0, bx, cy, col::MUTED, t("Name"));
        cy += 20.0;
        let caret = k.field(
            [bx, cy, bw, 48.0],
            None,
            &edit.name,
            t("Instance name"),
            Action::Field("instance_name"),
            typing,
        );
        self.caret(k, typing, caret, cy);
        cy += 56.0;
        if typing {
            let done_w = k.button_width(Style::Secondary, Some(Icon::Pixel("check")), t("Done"));
            k.button(
                [bx, cy, done_w, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("check")),
                t("Done"),
                Some(Action::FinishNameEdit),
            );
            return;
        }
        cy += 20.0;
        k.text(Face::Body, 12.0, 17.0, bx, cy, col::MUTED, t("Engine"));
        cy += 22.0;
        let cw = (bw - 8.0 * (cols - 1) as f32) / cols as f32;
        let selected = |e: &Option<usize>| match e {
            None => edit.engine.is_none(),
            Some(i) => {
                edit.engine.as_deref() == self.view.engines.get(*i).map(|e| e.manifest.id.as_str())
            }
        };
        for (i, (index, name, detail)) in engines.iter().enumerate() {
            let r = [
                bx + (i % cols) as f32 * (cw + 8.0),
                cy + (i / cols) as f32 * 56.0,
                cw,
                48.0,
            ];
            if r[1] + 48.0 > y + sh - 64.0 {
                break;
            }
            let on = selected(index);
            let action = Action::InstanceEngine(*index);
            let pressed = k.pressed(r, Some(action));
            kit::fill(
                k.ui,
                r[0],
                r[1],
                r[2],
                r[3],
                if pressed {
                    col::CONTROL_PRESSED
                } else {
                    col::CONTROL
                },
            );
            kit::frame(k.ui, r, if on { col::ACCENT } else { col::CONTROL_RING });
            k.ui.icon("box", r[0] + 12.0, r[1] + 12.0, 24.0, col::INK_SOFT);
            let tw = r[2] - 48.0 - if on { 40.0 } else { 12.0 };
            k.text_fit(
                Face::BodySemiBold,
                14.0,
                17.0,
                r[0] + 48.0,
                r[1] + 7.0,
                tw,
                col::INK,
                name,
            );
            k.text_fit(
                Face::Mono,
                11.0,
                14.0,
                r[0] + 48.0,
                r[1] + 27.0,
                tw,
                col::MUTED,
                detail,
            );
            if on {
                k.ui.icon("check", r[0] + r[2] - 32.0, r[1] + 12.0, 24.0, col::SUCCESS);
            }
            k.hit(r, action);
        }
        // Save, and Delete for an instance that has its own folder.
        let by = y + sh - 60.0;
        let save = if edit.id.is_some() {
            t("Save")
        } else {
            t("Create")
        };
        let save_w = k
            .button_width(Style::Primary, Some(Icon::Pixel("check")), save)
            .max(160.0);
        k.button(
            [x + sw - 16.0 - save_w, by, save_w, 48.0],
            Style::Primary,
            Some(Icon::Pixel("check")),
            save,
            Some(Action::SaveInstance),
        );
        if let Some(id) = edit.id
            && id != 0
        {
            let armed = edit
                .delete_armed
                .is_some_and(|at| at.elapsed() < Duration::from_secs(5));
            let label = if armed {
                t("Tap again: its worlds go too")
            } else {
                t("Delete")
            };
            let del_w = k.button_width(Style::Danger, Some(Icon::Pixel("trash")), label);
            k.button(
                [bx, by, del_w.min(bw - save_w - 8.0), 48.0],
                Style::Danger,
                Some(Icon::Pixel("trash")),
                label,
                Some(Action::DeleteInstance),
            );
        }
    }

    // ---- Skins ----

    fn draw_skins(&mut self, k: &mut Kit, main: [f32; 4], portrait: bool) {
        let [mx, my, mw, mh] = main;
        sky::band(k.ui, [mx, my, mw, 72.0], self.sky_time());
        let x0 = mx + 16.0;
        let w0 = mw - 32.0;
        let top = my + if portrait { 16.0 } else { 12.0 };
        k.text(
            Face::Pixel,
            32.0,
            32.0,
            x0,
            top,
            col::INK,
            t("Skins & capes"),
        );
        let owner = self.wardrobe.owner(&self.catalog.config.name);
        k.text_fit(
            Face::Mono,
            12.0,
            16.0,
            x0,
            top + 32.0,
            w0,
            col::INK_SOFT,
            &owner,
        );
        let body_top = top + 60.0;
        let bottom = my + mh - 16.0;
        // Landscape: the preview on the left, the list beside it. Portrait: the
        // preview above, as tall as the screen allows.
        let (preview, list) = if !portrait && w0 >= 520.0 {
            let pw = (w0 * 0.38).clamp(220.0, 320.0);
            (
                [x0, body_top, pw, bottom - body_top],
                [x0 + pw + 16.0, body_top, w0 - pw - 16.0, bottom - body_top],
            )
        } else {
            let ph = ((bottom - body_top) * 0.45).clamp(220.0, 360.0);
            (
                [x0, body_top, w0, ph],
                [x0, body_top + ph + 8.0, w0, bottom - body_top - ph - 8.0],
            )
        };
        self.wardrobe.draw_preview(k, preview);
        let scroll = self.catalog.scroll;
        let ([_, ly, _, lh], content) =
            self.wardrobe
                .draw_list(k, list, scroll, &mut self.list_targets);
        self.list_scroll(ly, ly + lh, content);
    }

    // ---- Settings ----

    fn draw_settings(&mut self, k: &mut Kit, main: [f32; 4], portrait: bool) {
        let [mx, my, mw, mh] = main;
        sky::band(k.ui, [mx, my, mw, 72.0], self.sky_time());
        let x0 = mx + 16.0;
        let w0 = mw - 32.0;
        let top = my + 16.0;
        k.text(Face::Pixel, 32.0, 32.0, x0, top, col::INK, t("Settings"));
        let body = my + 84.0;
        let bottom = my + mh - 16.0;
        let sections = self.settings_sections();
        // The index (landscape): jump to a section. Its rows shrink to fit the
        // height (full-screen landscape is short); a screen too short even for
        // the smallest rows gets the single column instead.
        let index_step = ((bottom - body + 2.0) / sections.len().max(1) as f32).min(46.0);
        let (cx, cw) = if portrait || w0 < 560.0 || index_step < 34.0 {
            (x0, w0)
        } else {
            let selected = self
                .view
                .settings_tops
                .iter()
                .rposition(|top| *top <= self.catalog.scroll + 8.0)
                .unwrap_or(0);
            for (i, (icon, title, _)) in sections.iter().enumerate() {
                let r = [x0, body + i as f32 * index_step, 196.0, index_step - 2.0];
                let action = Action::SettingsSection(i as u8);
                let on = i == selected;
                if on {
                    kit::plate(k.ui, r, 2, col::CONTROL, col::CONTROL_RING);
                } else if k.pressed(r, Some(action)) {
                    kit::stepped(k.ui, r, 2, 4.0, col::CONTROL);
                }
                k.ui.icon(
                    icon,
                    x0 + 10.0,
                    r[1] + (r[3] - 24.0) / 2.0,
                    24.0,
                    if on { col::ACCENT_HI } else { col::MUTED },
                );
                let lines = if k.measure(Face::BodySemiBold, 13.0, t(title)) > 142.0 {
                    2.0
                } else {
                    1.0
                };
                k.paragraph(
                    Face::BodySemiBold,
                    13.0,
                    14.95,
                    x0 + 44.0,
                    r[1] + (r[3] - lines * 14.95) / 2.0,
                    142.0,
                    2,
                    if on { col::INK } else { col::MUTED },
                    t(title),
                );
                k.hit(r, action);
            }
            (x0 + 210.0, w0 - 210.0)
        };
        // The column: section titles and panels of rows.
        let control_w = if portrait || cw < 440.0 {
            cw - 28.0
        } else {
            236.0
        };
        let stacked_rows = portrait || cw < 440.0;
        // Each row's wrapped title and description and its height: text wraps
        // (title up to 2 lines, description up to 3) and the row grows with it.
        let layouts: Vec<Vec<(Vec<String>, Vec<String>, f32, bool)>> = sections
            .iter()
            .map(|(_, _, rows)| {
                rows.iter()
                    .map(|row| {
                        let below = stacked_rows
                            && !matches!(row.control, Control::Switch(..) | Control::Text(_))
                            || matches!(row.control, Control::Segmented(_));
                        let control = match &row.control {
                            _ if below => 0.0,
                            Control::Switch(..) => 60.0,
                            Control::Text(_) => 0.0,
                            Control::Button(style, icon, label, _) => {
                                k.button_width(*style, Some(Icon::Pixel(icon)), label)
                            }
                            Control::Pair(a, b) => {
                                k.button_width(a.0, Some(Icon::Pixel(a.1)), a.2)
                                    + 8.0
                                    + k.button_width(b.0, Some(Icon::Pixel(b.1)), b.2)
                            }
                            _ => control_w,
                        };
                        let label_w = cw - 28.0 - if control > 0.0 { control + 16.0 } else { 0.0 };
                        // Titles and descriptions wrap in full: the row grows.
                        let title =
                            k.wrap_max(Face::BodySemiBold, 14.0, &row.title, label_w, usize::MAX);
                        let face = if row.mono { Face::Mono } else { Face::Body };
                        let description = if row.description.is_empty() {
                            Vec::new()
                        } else {
                            k.wrap_max(face, 12.0, &row.description, label_w, usize::MAX)
                        };
                        let text = title.len() as f32 * 17.0
                            + if description.is_empty() {
                                0.0
                            } else {
                                3.0 + description.len() as f32 * 14.0
                            };
                        let height = if below {
                            10.0 + text + 8.0 + 48.0 + 10.0
                        } else {
                            (text + 24.0).max(64.0)
                        };
                        (title, description, height, below)
                    })
                    .collect()
            })
            .collect();
        let content: f32 = layouts
            .iter()
            .map(|rows| 30.0 + 4.0 + rows.iter().map(|r| r.2).sum::<f32>() + 14.0)
            .sum::<f32>()
            + 24.0;
        let scroll = self.list_scroll(body, bottom, content);
        let mut tops = Vec::new();
        let mut y = body + 4.0 - scroll;
        // Everything in the column is cut to it: sections slide in and out of view
        // instead of popping.
        let clip = k.mark();
        for ((icon, title, rows), layout) in sections.into_iter().zip(layouts) {
            tops.push(y + scroll - body - 4.0);
            if y + 24.0 > body && y < bottom {
                k.ui.icon(icon, cx + 6.0, y, 24.0, col::ACCENT_HI);
                k.text(
                    Face::BodyBold,
                    15.0,
                    18.0,
                    cx + 38.0,
                    y + 3.0,
                    col::INK_SOFT,
                    t(title),
                );
            }
            y += 30.0;
            let panel_h: f32 = layout.iter().map(|r| r.2).sum::<f32>() + 4.0;
            if y + panel_h > body && y < bottom {
                kit::plate(k.ui, [cx, y, cw, panel_h], 2, col::PLATE, col::PLATE_RING);
            }
            let mut ry = y + 2.0;
            for (index, (row, (title_lines, description, rh, below))) in
                rows.iter().zip(layout).enumerate()
            {
                let text_h = title_lines.len() as f32 * 17.0
                    + if description.is_empty() {
                        0.0
                    } else {
                        3.0 + description.len() as f32 * 14.0
                    };
                let control_rect = if below {
                    [
                        cx + 14.0,
                        ry + 10.0 + text_h + 8.0,
                        if matches!(row.control, Control::Segmented(_)) {
                            cw - 28.0
                        } else {
                            control_w
                        },
                        48.0,
                    ]
                } else {
                    [
                        cx + cw - 14.0 - control_w,
                        ry + (rh - 48.0) / 2.0,
                        control_w,
                        48.0,
                    ]
                };
                for action in row.control.actions() {
                    self.list_targets.push((control_rect, action));
                }
                if ry + rh > body && ry < bottom {
                    if index > 0 {
                        kit::fill(k.ui, cx + 14.0, ry - 1.0, cw - 28.0, 2.0, col::RULE);
                    }
                    let mut ty = if below {
                        ry + 10.0
                    } else {
                        ry + (rh - text_h) / 2.0
                    };
                    for line in &title_lines {
                        k.text(
                            Face::BodySemiBold,
                            14.0,
                            17.0,
                            cx + 14.0,
                            ty,
                            col::INK,
                            line,
                        );
                        ty += 17.0;
                    }
                    ty += 3.0;
                    let face = if row.mono { Face::Mono } else { Face::Body };
                    for line in &description {
                        k.text(
                            face,
                            12.0,
                            14.0,
                            cx + 14.0,
                            ty,
                            if row.mono { col::INK_SOFT } else { col::MUTED },
                            line,
                        );
                        ty += 14.0;
                    }
                    match &row.control {
                        Control::Dropdown(value, action) => {
                            k.dropdown(control_rect, None, value, *action)
                        }
                        Control::Switch(on, action) => {
                            k.switch(cx + cw - 14.0 - 60.0, ry + (rh - 36.0) / 2.0, *on, *action);
                        }
                        Control::Button(style, icon, label, action) => {
                            let bw = k.button_width(*style, Some(Icon::Pixel(icon)), label);
                            let r = if below {
                                [control_rect[0], control_rect[1], bw.min(control_w), 48.0]
                            } else {
                                [cx + cw - 14.0 - bw, control_rect[1], bw, 48.0]
                            };
                            k.button(r, *style, Some(Icon::Pixel(icon)), label, Some(*action));
                        }
                        Control::Pair(a, b) => {
                            let aw = k.button_width(a.0, Some(Icon::Pixel(a.1)), a.2);
                            let bw = k.button_width(b.0, Some(Icon::Pixel(b.1)), b.2);
                            let x = if below {
                                control_rect[0]
                            } else {
                                cx + cw - 14.0 - bw - 8.0 - aw
                            };
                            let y = control_rect[1];
                            k.button(
                                [x, y, aw, 48.0],
                                a.0,
                                Some(Icon::Pixel(a.1)),
                                a.2,
                                Some(a.3),
                            );
                            k.button(
                                [x + aw + 8.0, y, bw, 48.0],
                                b.0,
                                Some(Icon::Pixel(b.1)),
                                b.2,
                                Some(b.3),
                            );
                        }
                        Control::Segmented(items) => {
                            let items: Vec<_> = items
                                .iter()
                                .map(|(l, a, s)| (None, l.as_str(), *a, *s))
                                .collect();
                            k.segmented(control_rect, &items, true);
                        }
                        Control::Text(_) => {}
                    }
                }
                ry += rh;
            }
            y += panel_h + 14.0;
        }
        k.clip(clip, [cx, body, cw, bottom - body]);
        self.view.settings_tops = tops;
    }

    fn settings_sections(&self) -> Vec<(&'static str, &'static str, Vec<Row>)> {
        use crate::catalog::InputHost;
        let c = &self.catalog.config;
        let window = self.window.as_ref().map(|w| w.inner_size());
        let resolution = window.map_or(String::new(), |s| {
            let p = c.render_scale.percent();
            format!(
                "{} × {}",
                (s.width as f32 * p as f32 / 100.0).round(),
                (s.height as f32 * p as f32 / 100.0).round()
            )
        });
        let resolution_note = format!("{resolution}. {}", t("Lower is faster but blurrier."));
        let renderer_note = match c.renderer {
            crate::catalog::RendererMode::Vulkan => t("Recommended: fastest on most phones"),
            crate::catalog::RendererMode::OpenGl => {
                t("For phones with old Vulkan drivers: slower, but works almost everywhere.")
            }
            _ => t(
                "Vulkan with driver workarounds: pick it if regular Vulkan shows glitches or crashes.",
            ),
        };
        let account = if let (Some(name), true) = (&self.login.profile, self.login.signed_in) {
            (
                tf("{} · Microsoft", &[name]),
                t("Servers that check the licence work"),
            )
        } else {
            let name = if c.name.trim().is_empty() {
                t("Player").to_owned()
            } else {
                c.name.clone()
            };
            (
                tf("{} · offline", &[&name]),
                t("Singleplayer and servers without licence checks"),
            )
        };
        vec![
            (
                "image",
                "Graphics",
                vec![
                    Row::new(
                        t("Renderer"),
                        renderer_note,
                        Control::Dropdown(t_dyn(c.renderer.label()), Action::Renderer),
                    ),
                    Row::new(
                        t("Resolution"),
                        &resolution_note,
                        Control::Dropdown(
                            if self.catalog.editing == Some("render_scale") {
                                format!("{}…", self.catalog.render_scale_text)
                            } else {
                                format!("{}%", c.render_scale.percent())
                            },
                            Action::RenderScale,
                        ),
                    ),
                    Row::new(
                        t("Aspect ratio"),
                        t(
                            "The shape of the game picture. A shape other than the screen's leaves black bars at the edges.",
                        ),
                        Control::Dropdown(t_dyn(c.screen.aspect.label()), Action::Aspect),
                    ),
                    Row::new(
                        t("Refresh rate"),
                        t(
                            "How often the screen redraws. Higher is smoother but uses more battery.",
                        ),
                        Control::Dropdown(t_dyn(&c.device.display_label()), Action::DisplayHz),
                    ),
                ],
            ),
            (
                "zap",
                "Performance",
                vec![
                    Row::new(
                        t("Thread limit"),
                        t(
                            "How many processor threads load and build the world around you. Fewer heat the phone less, but the world loads slower.",
                        ),
                        Control::Dropdown(
                            if c.worker_limit == 0 {
                                t("Auto").to_owned()
                            } else {
                                c.worker_limit.to_string()
                            },
                            Action::WorkerLimit,
                        ),
                    ),
                    Row::new(
                        t("RAM cache"),
                        t(
                            "Memory where the game keeps sounds and chunk geometry to reuse. More means fewer stutters on phones with plenty of RAM.",
                        ),
                        Control::Dropdown(
                            if c.cache_budget_mib == 0 {
                                t("Default (80 MiB)").to_owned()
                            } else {
                                format!("{} {}", c.cache_budget_mib, t("MiB"))
                            },
                            Action::CacheBudget,
                        ),
                    ),
                    Row::new(
                        t("Performance hints"),
                        t(
                            "Tells Android how much processor power the game needs (ADPF), so frames come at a steadier pace.",
                        ),
                        Control::Switch(c.device.performance_hints, Action::PerformanceHints),
                    ),
                    Row::new(
                        t("Download cache"),
                        t(
                            "Minecraft files left after installing, so an interrupted install can continue. Clearing frees space.",
                        ),
                        Control::Button(
                            Style::Secondary,
                            "trash",
                            t("Clear"),
                            Action::ClearDownloadCache,
                        ),
                    ),
                ],
            ),
            (
                "gamepad",
                "Input",
                vec![Row::new(
                    t("Controls"),
                    t(
                        "Auto uses touch controls and switches to a keyboard or mouse as soon as one is connected.",
                    ),
                    Control::Segmented(vec![
                        (
                            t("Touch").to_owned(),
                            Action::ChooseInputHost(InputHost::Touch),
                            c.input_host == InputHost::Touch,
                        ),
                        (
                            t("Keyboard & mouse").to_owned(),
                            Action::ChooseInputHost(InputHost::KeyboardMouse),
                            c.input_host == InputHost::KeyboardMouse,
                        ),
                        (
                            t("Auto").to_owned(),
                            Action::ChooseInputHost(InputHost::Auto),
                            c.input_host == InputHost::Auto,
                        ),
                    ]),
                )],
            ),
            (
                "home",
                "Home screen",
                vec![
                    Row::new(
                        t("Profile above Play"),
                        t("Shows the profile picker above Play when the phone is held upright."),
                        Control::Switch(!c.home_hide_profile, Action::HomeProfile),
                    ),
                    Row::new(
                        t("Last world and quick join"),
                        t(
                            "Shows the last world and server quick join above Play when the phone is held upright.",
                        ),
                        Control::Switch(c.home_last_world, Action::HomeLastWorld),
                    ),
                ],
            ),
            (
                "monitor",
                "Display",
                vec![
                    {
                        use crate::catalog::LauncherLanguage as L;
                        let choice = c.launcher_language;
                        // Named in both languages, so it is found in either.
                        Row::new(
                            "Language · Язык",
                            t(
                                "The launcher's language. Automatic follows Android's. The game has its own in its Options.",
                            ),
                            Control::Segmented(vec![
                                (
                                    t("Automatic").into(),
                                    Action::LauncherLanguage(L::Auto),
                                    choice == L::Auto,
                                ),
                                (
                                    "English".into(),
                                    Action::LauncherLanguage(L::English),
                                    choice == L::English,
                                ),
                                (
                                    "Русский".into(),
                                    Action::LauncherLanguage(L::Russian),
                                    choice == L::Russian,
                                ),
                            ]),
                        )
                    },
                    Row::new(
                        t("Screen orientation"),
                        t("The launcher and the game open in this orientation."),
                        Control::Dropdown(t_dyn(c.orientation.label()), Action::Orientation),
                    ),
                    Row::new(
                        t("Full screen in the game"),
                        t("Hides Android's status bar and navigation buttons while you play."),
                        Control::Switch(c.screen.immersive, Action::Immersive),
                    ),
                    Row::new(
                        t("Avoid the camera cutout"),
                        t(
                            "Keeps the game picture out of the camera cutout, so nothing hides behind it.",
                        ),
                        Control::Switch(c.screen.avoid_cutout, Action::Cutout),
                    ),
                ],
            ),
            (
                "user",
                "Account",
                vec![
                    Row::new(
                        &account.0,
                        account.1,
                        Control::Button(
                            Style::Secondary,
                            "user",
                            if self.login.signed_in {
                                t("Account")
                            } else {
                                t("Sign in")
                            },
                            Action::Account,
                        ),
                    ),
                    Row::new(
                        t("Instance"),
                        t(
                            "Each instance keeps its own worlds, settings and add-ons, and can run on its own engine. Manage them on the Instances page.",
                        ),
                        Control::Dropdown(self.profile_name(), Action::Profile),
                    ),
                ],
            ),
            ("box", "Engines", self.engine_rows()),
            (
                "bug",
                "Diagnostics & about",
                vec![
                    Row::new(
                        t("Launch log"),
                        t(
                            "The errors and logs of the last game launches. Read the summary, save it as a text file or share it.",
                        ),
                        Control::Button(
                            Style::Secondary,
                            "terminal",
                            t("Open"),
                            Action::ReportMenu(crate::transfers::Report::LaunchLog),
                        ),
                    ),
                    Row::new(
                        t("Report for developers"),
                        t("Saves logs and device details into one ZIP to attach to a bug report."),
                        Control::Button(
                            Style::Secondary,
                            "archive",
                            t("Export"),
                            Action::ReportMenu(crate::transfers::Report::Diagnostics),
                        ),
                    ),
                    Row::new(
                        t("Record performance"),
                        t(
                            "Records frame times for up to 30 minutes, to show developers where the game slows down.",
                        ),
                        Control::Switch(c.record_performance, Action::RecordPerformance),
                    ),
                    Row::new(
                        t("GPU profiling"),
                        t(
                            "Adds to the recording how long the GPU spends on each stage. May cost a little FPS.",
                        ),
                        Control::Switch(c.device.gpu_profile, Action::GpuProfile),
                    ),
                    Row::new(
                        t("Raw performance data"),
                        t(
                            "Exports the recorded measurements as JSONL files for your own analysis.",
                        ),
                        Control::Button(
                            Style::Secondary,
                            "download",
                            t("Export"),
                            Action::ReportMenu(crate::transfers::Report::Performance),
                        ),
                    ),
                    Row::new(
                        t("Game files"),
                        t("Opens the game folder (worlds, mods, packs, reports) in the Files app."),
                        Control::Button(Style::Secondary, "folder", t("Open"), Action::GameFiles),
                    ),
                    Row::mono(
                        t("Version"),
                        &format!(
                            "Horizon Aurora {} · Minecraft {} · {}",
                            crate::backend::VERSION,
                            horizon_core::VANILLA_VERSION,
                            std::env::consts::ARCH
                        ),
                        Control::Text(()),
                    ),
                    Row::new(
                        t("Source code"),
                        "github.com/IAmFuckingGenius/horizon-aurora",
                        Control::Button(
                            Style::Ghost,
                            "external-link",
                            "GitHub",
                            Action::Repository,
                        ),
                    ),
                ],
            ),
        ]
    }

    /// Settings › Engines: the engine running now, import, and each installed one.
    fn engine_rows(&self) -> Vec<Row> {
        let commit = option_env!("HORIZON_ANDROID_BUILD_COMMIT").unwrap_or("local");
        let running = crate::engines::running();
        let mut rows = vec![match &running {
            None => Row::mono(
                t("Engine in use: built into the app"),
                &format!("Horizon Aurora {} · {commit}", crate::backend::VERSION),
                Control::Text(()),
            ),
            Some(id) => {
                let name = self
                    .view
                    .engines
                    .iter()
                    .find(|e| &e.manifest.id == id)
                    .map_or(id.clone(), |e| e.manifest.name.clone());
                Row::new(
                    &tf("Engine in use: «{}»", &[&name]),
                    t(
                        "Started for this launch only. The next start of the app uses the built-in engine again.",
                    ),
                    Control::Button(
                        Style::Secondary,
                        "reload",
                        t("Built-in engine"),
                        Action::BuiltInEngine,
                    ),
                )
            }
        }];
        rows.push(Row::new(
            t("Add an engine"),
            t("An engine is the game built on a computer, with your native mods compiled in, and packed into a .hengine file by tools/android_engine_pack.py. It runs without reinstalling the app; your worlds, settings and account stay shared."),
            Control::Button(Style::Secondary, "plus", t("Import"), Action::ImportEngine),
        ));
        for (i, engine) in self.view.engines.iter().enumerate() {
            let m = &engine.manifest;
            let confirming = self
                .view
                .engine_delete
                .is_some_and(|(at, when)| at == i && when.elapsed() < Duration::from_secs(5));
            let mut details = format!(
                "{} · {} · {}",
                if m.version.is_empty() {
                    "?"
                } else {
                    &m.version
                },
                if m.commit.is_empty() { "?" } else { &m.commit },
                size(m.size)
            );
            if m.dirty {
                details.push_str(&format!(" · {}", t("uncommitted changes")));
            }
            if let Some(day) = m.built.get(..10) {
                details.push_str(&format!(" · {day}"));
            }
            rows.push(Row::mono(
                &m.name,
                &details,
                Control::Pair(
                    (Style::Primary, "play", t("Start"), Action::LaunchEngine(i)),
                    (
                        Style::Danger,
                        "trash",
                        if confirming {
                            t("Tap again")
                        } else {
                            t("Delete")
                        },
                        Action::DeleteEngine(i),
                    ),
                ),
            ));
        }
        rows
    }

    // ---- dialogs ----

    /// The question before an engine is imported: it is code with all the app's rights.
    pub(super) fn draw_engine_confirm(&mut self, k: &mut Kit) {
        let (w, h) = (k.ui.w, k.ui.h);
        kit::fill(k.ui, 0.0, 0.0, w, h, col::SCRIM);
        let dw = (w - 32.0).min(460.0);
        let title = t("Install an engine?");
        let body = t(
            "An engine is the game's code. It runs with all of Horizon's rights: your worlds, accounts and files. Install only an engine you built yourself or got from someone you trust.",
        );
        let title_lines = k.wrap_max(Face::BodyBold, 20.0, title, dw - 64.0, usize::MAX);
        let lines = k.wrap_max(Face::BodyMedium, 14.0, body, dw - 32.0, usize::MAX);
        let dh = 14.0
            + title_lines.len() as f32 * 24.0
            + 10.0
            + lines.len() as f32 * 20.3
            + 16.0
            + 48.0
            + 14.0;
        let (x, y) = ((w - dw) / 2.0, ((h - dh) / 2.0).max(6.0));
        k.panel([x, y, dw, dh], false);
        kit::icon(
            k.ui,
            Icon::Sprite(kit::ALERT),
            x + 16.0,
            y + 14.0,
            24.0,
            col::DANGER,
        );
        let mut cy = y + 14.0;
        for line in &title_lines {
            k.text(Face::BodyBold, 20.0, 24.0, x + 48.0, cy, col::INK, line);
            cy += 24.0;
        }
        cy += 10.0;
        for line in &lines {
            k.text(
                Face::BodyMedium,
                14.0,
                20.3,
                x + 16.0,
                cy,
                col::INK_SOFT,
                line,
            );
            cy += 20.3;
        }
        let choose = t("Choose the file");
        let choose_w = k.button_width(Style::Primary, Some(Icon::Pixel("plus")), choose);
        let cancel_w = k.button_width(Style::Secondary, None, t("Cancel"));
        let by = y + dh - 14.0 - 48.0;
        k.button(
            [x + dw - 16.0 - choose_w, by, choose_w, 48.0],
            Style::Primary,
            Some(Icon::Pixel("plus")),
            choose,
            Some(Action::ConfirmEngineImport),
        );
        k.button(
            [
                x + dw - 16.0 - choose_w - 8.0 - cancel_w,
                by,
                cancel_w,
                48.0,
            ],
            Style::Secondary,
            None,
            t("Cancel"),
            Some(Action::CancelEngineImport),
        );
    }

    /// The exit question.
    pub(super) fn draw_exit(&mut self, k: &mut Kit) {
        let (w, h) = (k.ui.w, k.ui.h);
        kit::fill(k.ui, 0.0, 0.0, w, h, col::SCRIM);
        let dw = (w - 32.0).min(380.0);
        let body = t("Your worlds are saved. Downloads, if any, stop and resume next time.");
        let lines = k.wrap(Face::BodyMedium, 14.0, body, dw - 32.0).len().min(4) as f32;
        let dh = 14.0 + 24.0 + 8.0 + lines * 20.3 + 14.0 + 48.0 + 14.0;
        let (x, y) = ((w - dw) / 2.0, (h - dh) / 2.0);
        k.panel([x, y, dw, dh], false);
        k.text(
            Face::BodyBold,
            20.0,
            24.0,
            x + 16.0,
            y + 14.0,
            col::INK,
            t("Leave Horizon?"),
        );
        k.paragraph(
            Face::BodyMedium,
            14.0,
            20.3,
            x + 16.0,
            y + 46.0,
            dw - 32.0,
            4,
            col::INK_SOFT,
            body,
        );
        let leave_w = k.button_width(Style::Danger, None, t("Leave"));
        let stay_w = k.button_width(Style::Secondary, None, t("Stay"));
        let by = y + dh - 14.0 - 48.0;
        k.button(
            [x + dw - 16.0 - leave_w, by, leave_w, 48.0],
            Style::Danger,
            None,
            t("Leave"),
            Some(Action::ConfirmExit),
        );
        k.button(
            [x + dw - 16.0 - leave_w - 8.0 - stay_w, by, stay_w, 48.0],
            Style::Secondary,
            None,
            t("Stay"),
            Some(Action::Dismiss),
        );
    }
}

impl Launcher {
    /// A world or report transfer over everything: its progress and Cancel, or
    /// why it failed and OK. Every text wraps in full; the dialog grows.
    pub(super) fn draw_transfer(&mut self, k: &mut Kit) {
        use crate::transfers::{Job, Stage};
        let running = crate::transfers::running();
        let (title, body, error) = match (&running, &self.view.transfer_error) {
            (Some(r), _) => (
                match r.job {
                    Job::ExportWorld => t("Exporting the world"),
                    Job::ImportWorld => t("Importing a world"),
                    Job::SaveFile => t("Saving the file"),
                    Job::ImportEngine => t("Installing an engine"),
                }
                .to_owned(),
                r.name.clone(),
                false,
            ),
            (None, Some((title, detail))) => (title.clone(), detail.clone(), true),
            (None, None) => return,
        };
        let (w, h) = (k.ui.w, k.ui.h);
        kit::fill(k.ui, 0.0, 0.0, w, h, col::SCRIM);
        let dw = (w - 32.0).min(440.0);
        let inner = dw - 32.0;
        let title_x = if error { 16.0 + 32.0 } else { 16.0 };
        let title_lines = k.wrap_max(
            Face::BodyBold,
            20.0,
            &title,
            dw - title_x - 16.0,
            usize::MAX,
        );
        let (body_face, body_size, body_line) = if error {
            (Face::BodyMedium, 14.0, 20.3)
        } else {
            (Face::BodySemiBold, 15.0, 18.0)
        };
        let body_lines = k.wrap_max(body_face, body_size, &body, inner, usize::MAX);
        // The progress block: stage and percent, the bar, sizes and files.
        let mut fraction = 0.0;
        let mut pct = String::new();
        let mut stage_lines = Vec::new();
        let mut number_lines = Vec::new();
        if let Some(r) = &running {
            let stage = match (r.cancelling, r.stage, r.job) {
                (true, ..) => t("Stopping and cleaning up"),
                (_, Stage::Receiving, Job::ImportEngine) => t("Copying the engine into Horizon"),
                (_, _, Job::ImportEngine) => t("Checking and installing the engine"),
                (_, Stage::Receiving, _) => t("Copying the ZIP into Horizon"),
                (_, Stage::Checking, _) => t("Checking the ZIP"),
                (_, Stage::Writing, Job::ExportWorld) => t("Packing the world into a ZIP"),
                (_, Stage::Writing, Job::ImportWorld) => t("Unpacking the world"),
                (_, Stage::Writing, Job::SaveFile) => t("Writing the file"),
            };
            let p = r.progress;
            if p.total_bytes > 0 {
                fraction = (p.bytes as f32 / p.total_bytes as f32).clamp(0.0, 1.0);
                pct = format!("{}%", (fraction * 100.0).floor() as u32);
            }
            let pw = k.measure(Face::Mono, 13.0, &pct);
            stage_lines = k.wrap_max(Face::BodyMedium, 13.0, stage, inner - pw - 8.0, usize::MAX);
            let mut numbers = if p.total_bytes > 0 {
                format!("{} / {}", size(p.bytes), size(p.total_bytes))
            } else if p.bytes > 0 {
                size(p.bytes)
            } else {
                String::new()
            };
            if p.total_files > 0 {
                numbers.push_str(&format!(
                    " · {}",
                    tf(
                        "{} of {} files",
                        &[&p.files.to_string(), &p.total_files.to_string()]
                    )
                ));
            }
            number_lines = k.wrap_max(Face::Mono, 12.0, &numbers, inner, usize::MAX);
        }
        let progress_h = if running.is_some() {
            8.0 + stage_lines.len().max(1) as f32 * 18.0
                + 8.0
                + 12.0
                + 8.0
                + number_lines.len() as f32 * 16.0
        } else {
            0.0
        };
        let dh = (14.0
            + title_lines.len() as f32 * 24.0
            + 8.0
            + body_lines.len() as f32 * body_line
            + progress_h
            + 16.0
            + 48.0
            + 14.0)
            .min(h - 24.0);
        let (x, y) = ((w - dw) / 2.0, ((h - dh) / 2.0).max(12.0));
        k.panel([x, y, dw, dh], false);
        let mut cy = y + 14.0;
        if error {
            kit::icon(
                k.ui,
                Icon::Sprite(kit::ALERT),
                x + 16.0,
                cy,
                24.0,
                col::DANGER,
            );
        }
        for line in &title_lines {
            k.text(Face::BodyBold, 20.0, 24.0, x + title_x, cy, col::INK, line);
            cy += 24.0;
        }
        cy += 8.0;
        for line in &body_lines {
            k.text(
                body_face,
                body_size,
                body_line,
                x + 16.0,
                cy,
                if error { col::INK_SOFT } else { col::INK },
                line,
            );
            cy += body_line;
        }
        let by = y + dh - 14.0 - 48.0;
        let Some(r) = running else {
            let ok_w = k.button_width(Style::Secondary, None, t("OK"));
            k.button(
                [x + dw - 16.0 - ok_w, by, ok_w, 48.0],
                Style::Secondary,
                None,
                t("OK"),
                Some(Action::DismissTransfer),
            );
            return;
        };
        cy += 8.0;
        let pw = k.measure(Face::Mono, 13.0, &pct);
        k.text(
            Face::Mono,
            13.0,
            18.0,
            x + dw - 16.0 - pw,
            cy,
            col::ACCENT_HI,
            &pct,
        );
        for line in &stage_lines {
            k.text(Face::BodyMedium, 13.0, 18.0, x + 16.0, cy, col::MUTED, line);
            cy += 18.0;
        }
        cy += 8.0;
        // The install page's segmented bar.
        kit::fill(k.ui, x + 16.0, cy, inner, 12.0, col::RULE);
        let fw = inner * fraction;
        kit::fill(k.ui, x + 16.0, cy, fw, 2.0, col::ACCENT_HI);
        kit::fill(k.ui, x + 16.0, cy + 2.0, fw, 10.0, col::ACCENT);
        let mut gap = 10.0;
        while gap < inner {
            kit::fill(k.ui, x + 16.0 + gap, cy, 2.0, 12.0, col::PLATE);
            gap += 12.0;
        }
        cy += 12.0 + 8.0;
        for line in &number_lines {
            k.text(Face::Mono, 12.0, 16.0, x + 16.0, cy, col::INK_SOFT, line);
            cy += 16.0;
        }
        let label = t("Cancel");
        let cancel_w = k.button_width(Style::Secondary, Some(Icon::Pixel("close")), label);
        let rect = [x + dw - 16.0 - cancel_w, by, cancel_w, 48.0];
        if r.cancelling {
            k.button(
                rect,
                Style::Disabled,
                Some(Icon::Pixel("close")),
                label,
                None,
            );
        } else {
            k.button(
                rect,
                Style::Secondary,
                Some(Icon::Pixel("close")),
                label,
                Some(Action::CancelTransfer),
            );
        }
    }
}

/// A settings row.
struct Row {
    title: String,
    description: String,
    mono: bool,
    control: Control,
}
impl Row {
    fn new(title: &str, description: &str, control: Control) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            mono: false,
            control,
        }
    }
    fn mono(title: &str, description: &str, control: Control) -> Self {
        Self {
            mono: true,
            ..Self::new(title, description, control)
        }
    }
}
type ButtonSpec = (Style, &'static str, &'static str, Action);
enum Control {
    Dropdown(String, Action),
    Switch(bool, Action),
    Button(Style, &'static str, &'static str, Action),
    /// Two buttons side by side (start and delete).
    Pair(ButtonSpec, ButtonSpec),
    Segmented(Vec<(String, Action, bool)>),
    Text(()),
}
impl Control {
    fn actions(&self) -> Vec<Action> {
        match self {
            Self::Dropdown(_, a) | Self::Switch(_, a) | Self::Button(_, _, _, a) => vec![*a],
            Self::Pair(a, b) => vec![a.3, b.3],
            Self::Segmented(items) => items.iter().map(|(_, a, _)| *a).collect(),
            Self::Text(_) => Vec::new(),
        }
    }
}

/// Install-button labels come from the catalogue as English words.
fn static_label(label: &str) -> &'static str {
    match label {
        "Install" => "Install",
        "Update" => "Update",
        "Reinstall" => "Reinstall",
        "Installed" => "Installed",
        "Retry" => "Retry",
        _ => "Install",
    }
}

/// The 2 dp rule between list rows (none above the first).
fn divider(k: &mut Kit, row: usize, [x, y, w]: [f32; 3]) {
    if row > 1 {
        kit::fill(k.ui, x + 12.0, y - 1.0, w - 24.0, 2.0, col::RULE);
    }
}

/// An 8 × 8 mirrored identicon for an add-on without an icon.
fn identicon(k: &mut Kit, x: f32, y: f32, seed_text: &str) {
    let mut seed: u32 = seed_text.bytes().fold(2_166_136_261u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16_777_619)
    });
    seed = seed.max(1);
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed >> 8) as f32 / 16_777_216.0
    };
    let hue = next() * 360.0;
    let bg = hsl(hue, 0.38, 0.22);
    let a = hsl(hue, 0.62, 0.58);
    let b = hsl(hue + 30.0, 0.70, 0.74);
    for row in 0..8 {
        for col_index in 0..4 {
            let v = next();
            let color = if col_index == 0 || row == 0 || row == 7 {
                bg
            } else if v < 0.45 {
                a
            } else if v < 0.6 {
                b
            } else {
                bg
            };
            for c in [col_index, 7 - col_index] {
                kit::fill(
                    k.ui,
                    x + c as f32 * 5.0,
                    y + row as f32 * 5.0,
                    5.0,
                    5.0,
                    color,
                );
            }
        }
    }
}
fn hsl(h: f32, s: f32, l: f32) -> u32 {
    let h = h.rem_euclid(360.0) / 360.0;
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f32| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round() as u32
    };
    horizon_client::ui_theme::color::hex(
        channel(h + 1.0 / 3.0) << 16 | channel(h) << 8 | channel(h - 1.0 / 3.0),
        255,
    )
}
/// A 32 × 18 pixel landscape standing in for a world's icon.
fn thumb(k: &mut Kit, x: f32, y: f32, name: &str) {
    let mut seed: u32 = name
        .bytes()
        .fold(40u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)))
        .max(1);
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed >> 8) as f32 / 16_777_216.0
    };
    let palettes = [
        (
            [0x5b8fe0, 0x73a3ea, 0x8fb8f2],
            0x5fb83a,
            0x7a5232,
            0x6b6b6b,
            0x2f7d22,
        ),
        (
            [0xe8a05a, 0xf0b878, 0xf6cf98],
            0xe8d28a,
            0xd8bd6e,
            0xb89a52,
            0x4f8a2a,
        ),
        (
            [0x8aa3c8, 0xa3b8d6, 0xbfcde3],
            0xf4f7fb,
            0x7a5232,
            0x6b6b6b,
            0x2a5a3a,
        ),
    ];
    let (sky, top, body, deep, tree) = palettes[(next() * 3.0) as usize % 3];
    let c = |rgb: u32| horizon_client::ui_theme::color::hex(rgb, 255);
    for (i, s) in sky.iter().enumerate() {
        kit::fill(
            k.ui,
            x,
            y + i as f32 * 8.0,
            64.0,
            if i == 2 { 12.0 } else { 8.0 },
            c(*s),
        );
    }
    let mut height = 10 + (next() * 3.0) as i32;
    for column in 0..32 {
        if next() < 0.35 {
            height += if next() < 0.5 { -1 } else { 1 };
        }
        height = height.clamp(8, 14);
        let gx = x + column as f32 * 2.0;
        let gy = y + height as f32 * 2.0;
        kit::fill(k.ui, gx, gy, 2.0, 2.0, c(top));
        kit::fill(k.ui, gx, gy + 2.0, 2.0, 6.0, c(body));
        kit::fill(
            k.ui,
            gx,
            gy + 8.0,
            2.0,
            (36.0 - height as f32 * 2.0 - 8.0).max(0.0),
            c(deep),
        );
        if column > 2 && column < 29 && next() < 0.09 {
            kit::fill(k.ui, gx, gy - 4.0, 2.0, 4.0, c(0x6b4a2b));
            kit::fill(k.ui, gx - 2.0, gy - 10.0, 6.0, 6.0, c(tree));
        }
    }
}
