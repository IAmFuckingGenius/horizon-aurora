//! Explicit, scrollable selections. Values never cycle on repeated button taps.
use super::{
    Launcher,
    kit::{self, Kit, Style, col},
    pages::Action,
    text::{t, tf},
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
    /// The ways to take a report out of Horizon.
    Report(crate::transfers::Report),
}
pub(super) struct Chooser {
    title: &'static str,
    help: &'static str,
    device: Option<String>,
    choices: Vec<(String, String, Action, bool)>,
    /// An icon per choice for a list of actions; radio squares when empty.
    icons: Vec<&'static str>,
    /// Text after the help, already in the launcher's language, wrapped in full.
    note: Option<String>,
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
        if let Kind::Report(report) = kind {
            self.chooser = Some(self.report_chooser(report));
            return;
        }
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
                    "Game resolution",
                    "The share of the screen's resolution the game draws at. Lower runs faster but looks blurrier; menus keep a usable size.",
                    choices,
                )
            }
            Kind::Orientation => (
                "Screen orientation",
                "The launcher and the game use this orientation.",
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
                "How the game draws through your phone's graphics driver. Applies the next time you press Play.",
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
            // Opened above.
            Kind::Report(_) => return,
            Kind::Profile => {
                let instances = crate::instances::list(&self.home);
                let engines = crate::engines::list(&self.home);
                let mut choices: Vec<_> = instances
                    .iter()
                    .map(|instance| {
                        let engine = match &instance.engine {
                            None => t("built-in engine").to_owned(),
                            Some(id) => engines.iter().find(|e| &e.manifest.id == id).map_or_else(
                                || tf("engine «{}» is not installed", &[id]),
                                |e| tf("engine «{}»", &[&e.manifest.name]),
                            ),
                        };
                        (
                            super::view::instance_name(instance),
                            engine,
                            Action::ChooseProfile(instance.id),
                            instance.id == c.profile,
                        )
                    })
                    .collect();
                choices.push((
                    t("New instance").into(),
                    t("Its own worlds, settings and add-ons, and an engine of its choice").into(),
                    Action::NewInstance,
                    false,
                ));
                (
                    "Choose an instance",
                    "Each instance keeps its own worlds, settings and add-ons, and can run on its own engine.",
                    choices,
                )
            }
        };
        self.chooser = Some(Chooser {
            title,
            help,
            choices,
            icons: Vec::new(),
            note: None,
            device,
            scroll: 0.0,
            rows: Vec::new(),
            view: [0.0; 3],
        });
    }
    /// Save, share or show a report; the launch log also reads its summary here.
    fn report_chooser(&self, report: crate::transfers::Report) -> Chooser {
        use crate::transfers::{Delivery, Report};
        let (title, help, save, share) = match report {
            Report::Diagnostics => (
                "Report for developers",
                "Logs and device details in one ZIP. Nothing is uploaded by itself: review the ZIP before you send it.",
                "Save to a folder",
                "Share",
            ),
            Report::Performance => (
                "Raw performance data",
                "The latest recording as a JSONL file, for your own analysis or for developers.",
                "Save to a folder",
                "Share",
            ),
            Report::LaunchLog => (
                "Launch log",
                "The errors and logs of the last launches as one text file. Passwords, tokens and addresses are removed; review it before you send it.",
                "Save as a text file",
                "Share as text",
            ),
        };
        let note = (report == Report::LaunchLog).then(|| {
            let mut note = match crate::diagnostics::unclean_phase(&self.home) {
                Some(phase) => tf(
                    "The previous session ended without a clean exit (last phase: {}). The game may have crashed, or Android closed it.",
                    &[&phase],
                ),
                None => t("The previous session ended normally.").into(),
            };
            if let Ok(error) = std::fs::read_to_string(self.home.join("android-error.txt"))
                && !error.trim().is_empty()
            {
                note.push('\n');
                note.push_str(&tf(
                    "Last error: {}",
                    &[&crate::diagnostics::redact(error.trim())],
                ));
            }
            note
        });
        Chooser {
            title,
            help,
            device: None,
            choices: vec![
                (
                    save.into(),
                    "Pick a folder or drive with the system file picker.".into(),
                    Action::SendReport(report, Delivery::Save),
                    false,
                ),
                (
                    share.into(),
                    "Send it to another app: mail, a messenger or cloud storage.".into(),
                    Action::SendReport(report, Delivery::Share),
                    false,
                ),
                (
                    "Open in Files".into(),
                    "Shows Horizon Android › Reports in the Files app.".into(),
                    Action::SendReport(report, Delivery::Files),
                    false,
                ),
            ],
            icons: vec!["download", "external-link", "folder"],
            note,
            scroll: 0.0,
            rows: Vec::new(),
            view: [0.0; 3],
        }
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
            icons: Vec::new(),
            note: None,
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
        // The whole help, never cut: in a chooser it scrolls with the options.
        let help: Vec<String> = k
            .wrap(Face::BodyMedium, 13.0, t(self.help), inner)
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect();
        let note: Vec<String> = self
            .note
            .as_deref()
            .map(|n| k.wrap(Face::BodyMedium, 13.0, n, inner))
            .unwrap_or_default();
        let help_h = help.len() as f32 * 18.85
            + if note.is_empty() {
                0.0
            } else {
                8.0 + note.len() as f32 * 18.85
            }
            + if self.device.is_some() { 20.0 } else { 0.0 }
            + 10.0;
        let head = 14.0 + 24.0 + 6.0 + help_h;
        // Each option's name and description, wrapped in full; its row grows.
        let tw = inner - 44.0 - 12.0;
        let layout: Vec<(Vec<String>, Vec<String>, f32)> = self
            .choices
            .iter()
            .map(|(label, detail, _, _)| {
                let label = k.wrap_max(
                    Face::BodySemiBold,
                    15.0,
                    &super::text::t_dyn(label),
                    tw,
                    usize::MAX,
                );
                let detail = k.wrap_max(
                    Face::Body,
                    12.0,
                    &super::text::t_dyn(detail),
                    tw,
                    usize::MAX,
                );
                let text = label.len().max(1) as f32 * 18.0
                    + if detail.is_empty() {
                        0.0
                    } else {
                        2.0 + detail.len() as f32 * 14.0
                    };
                let rh = (text + 18.0).max(if detail.is_empty() { 48.0 } else { 56.0 });
                (label, detail, rh)
            })
            .collect();
        let options: f32 = if confirm {
            0.0
        } else {
            layout.iter().map(|l| l.2 + 6.0).sum::<f32>()
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
        let help_top = cy;
        let clip = k.mark();
        if !confirm {
            cy -= self.scroll;
        }
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
        if !note.is_empty() {
            cy += 8.0;
            for line in &note {
                k.text(Face::BodyMedium, 13.0, 18.85, x + 16.0, cy, col::INK, line);
                cy += 18.85;
            }
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
        let top = help_top;
        let bottom = y + dh - 14.0;
        let maximum = (help_h + options - (bottom - top)).max(0.0);
        self.scroll = self.scroll.clamp(0.0, maximum);
        self.view = [top, bottom, maximum];
        self.rows.clear();
        let mut oy = cy;
        for (i, ((_, _, action, selected), (label, detail, rh))) in
            self.choices.iter().zip(&layout).enumerate()
        {
            let rh = *rh;
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
                if let Some(icon) = self.icons.get(i) {
                    // An action: its icon where a choice has its radio.
                    k.ui.icon(
                        icon,
                        r[0] + 10.0,
                        oy + (rh - 24.0) / 2.0,
                        24.0,
                        col::ACCENT_HI,
                    );
                } else {
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
                }
                let text = label.len().max(1) as f32 * 18.0
                    + if detail.is_empty() {
                        0.0
                    } else {
                        2.0 + detail.len() as f32 * 14.0
                    };
                let mut ty = oy + (rh - text) / 2.0;
                for line in label {
                    k.text(
                        Face::BodySemiBold,
                        15.0,
                        18.0,
                        r[0] + 44.0,
                        ty,
                        col::INK,
                        line,
                    );
                    ty += 18.0;
                }
                ty += 2.0;
                for line in detail {
                    k.text(Face::Body, 12.0, 14.0, r[0] + 44.0, ty, col::MUTED, line);
                    ty += 14.0;
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
