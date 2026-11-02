//! Explicit, scrollable selections. Values never cycle on repeated button taps.
use super::{
    Launcher,
    kit::{self, Kit, Style, col},
    pages::Action,
    text::t,
};
use crate::catalog::RendererMode;
use horizon_client::ui_theme::Face;

pub(super) enum Kind {
    Aspect,
    DisplayHz,
    CacheBudget,
    WorkerLimit,
    Renderer,
    RenderScale,
    Profile,
    Orientation,
}
pub(super) struct Chooser {
    title: &'static str,
    help: &'static str,
    device: Option<String>,
    choices: Vec<(String, String, Action, bool)>,
    pub scroll: f32,
    /// The option rows as last drawn, for D-pad navigation.
    rows: Vec<([f32; 4], Action)>,
    /// The scrolling options area: top, bottom, maximum scroll.
    view: [f32; 3],
}
impl Launcher {
    pub(super) fn open_chooser(&mut self, kind: Kind) {
        crate::platform::hide();
        self.catalog.editing = None;
        let device = if matches!(kind, Kind::Renderer) {
            self.renderer.as_ref().map(|r| {
                format!(
                    "GPU: {} / {} {}",
                    r.info.name, r.info.driver, r.info.driver_info
                )
            })
        } else {
            None
        };
        let c = &self.catalog.config;
        let (title, help, choices) = match kind {
            Kind::Aspect => (
                "Aspect ratio",
                "Fits the whole picture without stretching. In portrait, width and height swap.",
                [
                    crate::display::Aspect::Native,
                    crate::display::Aspect::Wide,
                    crate::display::Aspect::Classic,
                    crate::display::Aspect::Square,
                ]
                .into_iter()
                .map(|a| {
                    (
                        a.label().into(),
                        String::new(),
                        Action::ChooseAspect(a),
                        c.screen.aspect == a,
                    )
                })
                .collect(),
            ),
            Kind::CacheBudget => (
                "Reusable RAM cache",
                "Sounds and mesh buffers the game keeps for reuse. Not a limit on all memory. Applies on the next Play.",
                [0, 16, 32, 64, 128, 256]
                    .into_iter()
                    .map(|mib| {
                        (
                            if mib == 0 {
                                "Engine default (80 MiB)".into()
                            } else {
                                format!("{mib} MiB")
                            },
                            String::new(),
                            Action::ChooseCacheBudget(mib),
                            c.cache_budget_mib == mib,
                        )
                    })
                    .collect(),
            ),
            Kind::WorkerLimit => (
                "Chunk worker limit",
                "A ceiling for the threads that generate and build chunks. Fewer ease the CPU but load chunks slower. Applies on the next Play.",
                (0..=64u32)
                    .map(|limit| {
                        (
                            if limit == 0 {
                                "No additional limit".into()
                            } else {
                                format!("{limit} worker(s)")
                            },
                            String::new(),
                            Action::ChooseWorkerLimit(limit),
                            c.worker_limit == limit,
                        )
                    })
                    .collect(),
            ),
            Kind::DisplayHz => (
                "Display refresh rate",
                "An Android display preference, not a game FPS cap. Unsupported rates may be ignored.",
                crate::device_preferences::DISPLAY_RATES
                    .into_iter()
                    .map(|hz| {
                        (
                            if hz == 0 {
                                "System default".into()
                            } else {
                                format!("{hz} Hz")
                            },
                            String::new(),
                            Action::ChooseDisplayHz(hz),
                            c.device.display_hz == hz,
                        )
                    })
                    .collect(),
            ),
            Kind::RenderScale => {
                let mut choices: Vec<_> = crate::display::RenderScale::PRESETS
                    .into_iter()
                    .map(|percent| {
                        (
                            format!("{percent}%"),
                            if percent == 100 {
                                "Native resolution".into()
                            } else {
                                "Lower resolution for the whole picture".into()
                            },
                            Action::ChooseRenderScale(crate::display::RenderScale::new(percent)),
                            c.render_scale.percent() == percent,
                        )
                    })
                    .collect();
                choices.push((
                    "Custom percentage...".into(),
                    "Enter a value from 25 to 100".into(),
                    Action::Field("render_scale"),
                    false,
                ));
                (
                    "Client resolution",
                    "Small screens retain a usable menu size.",
                    choices,
                )
            }
            Kind::Orientation => (
                "Screen orientation",
                "Applies to the launcher and game. Does not change touch bindings.",
                crate::catalog::Orientation::ALL
                    .into_iter()
                    .map(|mode| {
                        (
                            mode.label().into(),
                            String::new(),
                            Action::ChooseOrientation(mode),
                            c.orientation == mode,
                        )
                    })
                    .collect(),
            ),
            Kind::Renderer => (
                "Rendering mode",
                "Uses the system GPU driver. Applies on the next game launch.",
                vec![
                    (
                        "OpenGL ES / experimental".into(),
                        "For older GPUs. Requires ES 3.0+ and sufficient driver limits.".into(),
                        Action::ChooseRenderer(RendererMode::OpenGl),
                        c.renderer == RendererMode::OpenGl,
                    ),
                    (
                        "Vulkan / GPU batching".into(),
                        "Groups terrain draws and enables supported GPU culling.".into(),
                        Action::ChooseRenderer(RendererMode::Vulkan),
                        c.renderer == RendererMode::Vulkan,
                    ),
                    (
                        "Vulkan / compatibility".into(),
                        "Direct terrain draws for drivers with missing or transparent blocks."
                            .into(),
                        Action::ChooseRenderer(RendererMode::VulkanCompatibility),
                        c.renderer == RendererMode::VulkanCompatibility,
                    ),
                ],
            ),
            Kind::Profile => {
                let mut ids = vec![0, c.profile];
                ids.extend(
                    std::fs::read_dir(self.home.join("profiles"))
                        .into_iter()
                        .flatten()
                        .flatten()
                        .filter(|e| e.path().is_dir())
                        .filter_map(|e| {
                            e.file_name()
                                .to_str()?
                                .strip_prefix("profile-")?
                                .parse::<u32>()
                                .ok()
                        }),
                );
                ids.sort_unstable();
                ids.dedup();
                let next = ids.last().copied().unwrap_or(0).checked_add(1);
                let mut choices: Vec<_> = ids
                    .into_iter()
                    .map(|id| {
                        (
                            if id == 0 {
                                "Main".into()
                            } else {
                                format!("Profile {id}")
                            },
                            if id == 0 {
                                "Default profile".into()
                            } else {
                                "Separate saves, mods and packs".into()
                            },
                            Action::ChooseProfile(id),
                            id == c.profile,
                        )
                    })
                    .collect();
                if let Some(id) = next {
                    choices.push((
                        "Create profile".into(),
                        format!("Create Profile {id}"),
                        Action::ChooseProfile(id),
                        false,
                    ));
                }
                (
                    "Choose profile",
                    "Each profile keeps its own game settings and saves.",
                    choices,
                )
            }
        };
        self.chooser = Some(Chooser {
            title,
            help,
            choices,
            device,
            scroll: 0.0,
            rows: Vec::new(),
            view: [0.0; 3],
        });
    }
}
impl Chooser {
    pub(super) fn confirm_delete(name: String) -> Self {
        Self {
            title: "Remove add-on?",
            help: "Deletes only the selected file from this profile. This cannot be undone.",
            device: Some(name),
            choices: vec![
                (
                    "Remove".into(),
                    String::new(),
                    Action::ConfirmDeleteLocal,
                    false,
                ),
                ("Cancel".into(), String::new(), Action::Dismiss, true),
            ],
            scroll: 0.0,
            rows: Vec::new(),
            view: [0.0; 3],
        }
    }
    pub(super) fn navigation(&self) -> Vec<([f32; 4], Action)> {
        self.rows.clone()
    }
    pub(super) fn viewport(&self) -> [f32; 3] {
        self.view
    }
    /// A dialog over a scrim: radio options with their descriptions, or, for a
    /// confirmation without descriptions, a row of buttons.
    pub fn draw(&mut self, k: &mut Kit) {
        let (w, h) = (k.ui.w, k.ui.h);
        kit::fill(k.ui, 0.0, 0.0, w, h, col::SCRIM);
        let dw = (w - 32.0).min(460.0);
        let inner = dw - 32.0;
        let confirm = self.choices.len() <= 2 && self.choices.iter().all(|c| c.1.is_empty());
        // The whole help when it fits; a long one ends in an ellipsis, never mid-word.
        let help = k.wrap_max(
            Face::BodyMedium,
            13.0,
            t(self.help),
            inner,
            if h < 420.0 { 3 } else { 5 },
        );
        let head = 14.0
            + 24.0
            + 6.0
            + help.len() as f32 * 18.85
            + if self.device.is_some() { 20.0 } else { 0.0 }
            + 10.0;
        let row_h = |detail: &str| if detail.is_empty() { 48.0 } else { 56.0 };
        let options: f32 = if confirm {
            0.0
        } else {
            self.choices.iter().map(|c| row_h(&c.1) + 6.0).sum::<f32>()
        };
        // Confirmations end in buttons; choosers close from their header.
        let foot = if confirm { 8.0 + 48.0 + 14.0 } else { 14.0 };
        let dh = (head + options + foot).min(h - 24.0);
        let (x, y) = ((w - dw) / 2.0, (h - dh) / 2.0);
        k.panel([x, y, dw, dh], false);
        k.text_fit(
            Face::BodyBold,
            20.0,
            24.0,
            x + 16.0,
            y + 14.0,
            inner - if confirm { 0.0 } else { 52.0 },
            col::INK,
            t(self.title),
        );
        if !confirm {
            k.button(
                [x + dw - 16.0 - 48.0, y + 2.0, 48.0, 48.0],
                Style::Ghost,
                Some(super::kit::Icon::Pixel("close")),
                "",
                Some(Action::Dismiss),
            );
        }
        let mut cy = y + 44.0;
        for line in &help {
            k.text(
                Face::BodyMedium,
                13.0,
                18.85,
                x + 16.0,
                cy,
                col::INK_SOFT,
                line,
            );
            cy += 18.85;
        }
        if let Some(device) = &self.device {
            k.text_fit(
                Face::Mono,
                12.0,
                16.0,
                x + 16.0,
                cy + 2.0,
                inner,
                col::MUTED,
                device,
            );
            cy += 20.0;
        }
        cy += 10.0;
        let foot_y = y + dh - 14.0 - 48.0;
        if confirm {
            // Buttons, right-aligned: the safe choice last, nearest the thumb.
            let mut bx = x + dw - 16.0;
            for (label, _, action, _) in self.choices.iter().rev() {
                let style = if *action == Action::ConfirmDeleteLocal {
                    Style::Danger
                } else {
                    Style::Secondary
                };
                let bw = k.button_width(style, None, t(static_str(label)));
                bx -= bw;
                k.button(
                    [bx, foot_y, bw, 48.0],
                    style,
                    None,
                    t(static_str(label)),
                    Some(*action),
                );
                bx -= 8.0;
            }
            self.rows.clear();
            self.view = [0.0; 3];
            return;
        }
        let top = cy;
        let bottom = y + dh - 14.0;
        let maximum = (options - (bottom - top)).max(0.0);
        self.scroll = self.scroll.clamp(0.0, maximum);
        self.view = [top, bottom, maximum];
        self.rows.clear();
        let mut oy = top - self.scroll;
        let clip = k.mark();
        for (label, detail, action, selected) in &self.choices {
            let rh = row_h(detail);
            let r = [x + 16.0, oy, inner, rh];
            self.rows.push((r, *action));
            if oy + rh > top && oy < bottom {
                let pressed = k.pressed(r, Some(*action));
                kit::plate(
                    k.ui,
                    r,
                    1,
                    if pressed {
                        col::CONTROL_PRESSED
                    } else {
                        col::CONTROL
                    },
                    if *selected {
                        col::ACCENT_HI
                    } else {
                        col::CONTROL_RING
                    },
                );
                // A square radio: a 2 dp frame, and a dot when chosen.
                let (rx, ry) = (r[0] + 12.0, oy + (rh - 20.0) / 2.0);
                let ring = if *selected {
                    col::ACCENT_HI
                } else {
                    col::MUTED
                };
                kit::frame(k.ui, [rx, ry, 20.0, 20.0], ring);
                if *selected {
                    kit::fill(k.ui, rx + 6.0, ry + 6.0, 8.0, 8.0, col::ACCENT_HI);
                }
                let tw = inner - 44.0 - 12.0;
                let label = &super::text::t_dyn(label);
                let detail = &super::text::t_dyn(detail);
                if detail.is_empty() {
                    k.text_fit(
                        Face::BodySemiBold,
                        15.0,
                        18.0,
                        r[0] + 44.0,
                        oy + 15.0,
                        tw,
                        col::INK,
                        label,
                    );
                } else {
                    k.text_fit(
                        Face::BodySemiBold,
                        15.0,
                        18.0,
                        r[0] + 44.0,
                        oy + 9.0,
                        tw,
                        col::INK,
                        label,
                    );
                    k.text_fit(
                        Face::Body,
                        12.0,
                        14.0,
                        r[0] + 44.0,
                        oy + 29.0,
                        tw,
                        col::MUTED,
                        detail,
                    );
                }
                k.hit(r, *action);
            }
            oy += rh + 6.0;
        }
        k.clip(clip, [x + 12.0, top, inner + 8.0, bottom - top]);
    }
}

/// Confirmation labels are fixed English words; give them a static lifetime for `t`.
fn static_str(label: &str) -> &'static str {
    match label {
        "Remove" => "Remove",
        "Cancel" => "Cancel",
        _ => "OK",
    }
}
