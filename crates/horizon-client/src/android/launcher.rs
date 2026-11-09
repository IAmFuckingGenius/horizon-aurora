//! A native Rust installer and version screen, sharing the game's Vulkan renderer.
use crate::catalog::{Catalog, Page};
use crate::game;
#[path = "chooser.rs"]
mod chooser;
#[path = "launcher_gesture.rs"]
mod gesture;
#[path = "launcher_kit.rs"]
mod kit;
#[path = "login.rs"]
mod login;
#[path = "launcher_pages.rs"]
mod pages;
#[path = "launcher_sky.rs"]
mod sky;
#[path = "launcher_text.rs"]
pub(crate) mod text;
#[path = "launcher_view.rs"]
mod view;
#[path = "wardrobe.rs"]
mod wardrobe;
use horizon_client::{
    Result,
    gfx::Renderer,
    hud::{HudAtlas, HudBatch},
    ui_theme::Ui,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, TouchPhase, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    window::{Window, WindowId},
};

struct PreparedAccount {
    key: String,
    file: PathBuf,
    appearance: super::skins::cloud::Cache,
}
pub(super) struct Launcher {
    home: PathBuf,
    controller_edges: horizon_client::gamepad::Edges,
    controller_repeat: Option<(horizon_client::gamepad::Button, Instant, bool)>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    gui: Option<game::UiResources>,
    pub(super) audio: Option<horizon_client::audio::Audio>,
    game: Option<Box<dyn ApplicationHandler>>,
    cursor: (f64, f64),
    mouse_down: bool,
    feedback: Option<(pages::Action, Instant)>,
    install: Arc<Mutex<horizon_client::official_install::InstallProgress>>,
    worker: Option<mpsc::Receiver<std::result::Result<(), String>>>,
    error: Option<String>,
    catalog: Catalog,
    targets: Vec<([f32; 4], pages::Action)>,
    list_targets: Vec<([f32; 4], pages::Action)>,
    list_view: Option<[f32; 3]>,
    pending_focus: Option<pages::Action>,
    drag: Option<gesture::Drag>,
    chooser: Option<chooser::Chooser>,
    exit_confirmation: bool,
    login: login::Login,
    wardrobe: wardrobe::Wardrobe,
    session_worker: Option<mpsc::Receiver<std::result::Result<PreparedAccount, String>>>,
    session_ready: bool,
    prepared_session: Option<PathBuf>,
    focus: Option<usize>,
    /// The diagnostic ZIP being written, and where it goes when it is ready.
    diagnostic_export: Option<(
        mpsc::Receiver<std::result::Result<PathBuf, String>>,
        crate::transfers::Delivery,
    )>,
    /// Where Play goes once the session is ready: the title screen, the quick-join
    /// server or straight into a world.
    launch_target: LaunchTarget,
    /// What the launch request this start was made for asks to open (an installed
    /// engine started from Settings › Engines); opened once the launcher is ready.
    start_request: Option<serde_json::Value>,
    view: view::ViewState,
    fling: Option<gesture::Fling>,
}

#[derive(Clone, Default, PartialEq)]
pub(super) enum LaunchTarget {
    #[default]
    Title,
    Server,
    World(String),
}

impl Launcher {
    /// Restarts the app on `engine` (`None`: the APK's own) for one launch: the boot
    /// library reads the request on the next start (`crates/horizon-boot`), and the
    /// engine opens `target` there. Without a target the launcher just starts again on
    /// the APK's engine. The process ends here; Android starts a new one for the
    /// restarted activity.
    pub(super) fn restart_on(&mut self, engine: Option<&str>, target: Option<serde_json::Value>) {
        if let Some(target) = target
            && let Err(error) = crate::engines::request(&self.home, engine, target)
        {
            self.toast(
                text::t("Could not start the engine"),
                text::t_dyn(&error),
                true,
            );
            return;
        }
        if let Err(error) = self.catalog.config.save(&self.home) {
            log::warn!("Launcher settings before an engine switch: {error}");
        }
        if !crate::platform::restart_app() {
            let _ = std::fs::remove_file(crate::engines::folder(&self.home).join("launch.json"));
            self.toast(
                text::t("Could not restart Horizon"),
                text::t("Close the app and open it again to switch the engine."),
                true,
            );
            return;
        }
        log::info!("Restarting on engine {}", engine.unwrap_or("built-in"));
        crate::diagnostics::finish();
        crate::platform::finish();
        std::process::exit(0);
    }
    /// Plays the chosen instance (the profile in use) with `launch_target`: here when
    /// this engine is the instance's, else after restarting on the instance's engine,
    /// which opens the same target.
    pub(super) fn play(&mut self, events: &ActiveEventLoop) -> Result<()> {
        let id = self.catalog.config.profile;
        let instance = crate::instances::get(&self.home, id);
        if instance.engine != crate::engines::running() {
            let mut target = match &self.launch_target {
                LaunchTarget::Title => serde_json::json!({ "open": "title" }),
                LaunchTarget::World(folder) => serde_json::json!({ "world": folder }),
                LaunchTarget::Server => {
                    serde_json::json!({ "server": self.catalog.config.server.trim() })
                }
            };
            target["instance"] = id.into();
            self.restart_on(instance.engine.as_deref(), Some(target));
            return Ok(());
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        if let Err(error) = crate::instances::played(&self.home, id, now) {
            log::warn!("Instance {id}: {error}");
        }
        self.action(events)
    }
    pub(super) fn refresh_wardrobe_account(&mut self) {
        if self.catalog.page == Page::Skins {
            self.catalog.scroll = 0.0;
            self.wardrobe.open(
                &self.catalog.config.name,
                &self.catalog.config.microsoft_client_id,
            );
        }
    }

    /// Makes instance `id` the one in use (its folder is the game's).
    pub(super) fn select_instance(&mut self, id: u32) -> Result<()> {
        std::fs::create_dir_all(crate::instances::folder(&self.home, id))?;
        self.catalog.config.profile = id;
        self.catalog.config.save(&self.home)?;
        self.catalog.refresh(&self.home);
        self.refresh_worlds();
        Ok(())
    }
    fn edit_text(&mut self, output: horizon_client::osk::Output) {
        if self.catalog.editing == Some("instance_name") {
            use horizon_client::osk::Output;
            if let Some(edit) = self.view.instance_edit.as_mut() {
                match &output {
                    Output::Text(text)
                        if edit.name.chars().count() + text.chars().count()
                            <= crate::instances::NAME_LIMIT =>
                    {
                        edit.name.push_str(text)
                    }
                    Output::Backspace => {
                        edit.name.pop();
                    }
                    _ => {}
                }
            }
            if matches!(output, Output::Enter | Output::Close) {
                self.catalog.editing = None;
                crate::platform::hide();
            }
            return;
        }
        if self.login.edit(self.catalog.editing, &output) {
            if matches!(
                output,
                horizon_client::osk::Output::Enter | horizon_client::osk::Output::Close
            ) {
                self.catalog.editing = None;
                crate::platform::hide();
            }
        } else {
            self.catalog.edit(output, &self.home);
        }
    }
    pub(super) fn new(home: PathBuf) -> Self {
        // Upgrade existing installations that retained the old download cache.
        let cleanup_home = home.clone();
        std::thread::spawn(move || {
            if let Err(error) =
                horizon_client::official_install::cleanup_completed_downloads(&cleanup_home)
            {
                log::warn!("Could not clean completed Minecraft downloads: {error}");
            }
        });
        let mut catalog = Catalog::new(&home);
        horizon_client::android_runtime::graphics_startup::configure(home.clone());
        if horizon_client::android_runtime::graphics_startup::recover(&home) {
            catalog.config.renderer = crate::catalog::RendererMode::OpenGl;
            if let Err(error) = catalog.config.save(&home) {
                log::warn!("Could not save graphics recovery choice: {error}");
            }
            catalog.status = "Previous Vulkan startup was interrupted. Using OpenGL ES recovery; you can change Rendering mode in Settings.".into();
        }
        crate::configure_input_mode(catalog.config.input_host);
        crate::device_runtime::configure(catalog.config.device);
        text::choose(catalog.config.launcher_language);
        horizon_client::net::set_older_servers(!catalog.config.older_servers_off);
        let start_request = crate::engines::take_started(&home);
        let boot_error = crate::engines::take_boot_error(&home);
        // A request whose engine failed to load is not this engine's to open: the
        // launcher shows why instead of going straight into the game.
        let start_request = start_request.filter(|_| boot_error.is_none());
        let mut launcher = Self {
            start_request,
            wardrobe: wardrobe::Wardrobe::new(&home),
            home,
            controller_edges: Default::default(),
            controller_repeat: None,
            window: None,
            renderer: None,
            gui: None,
            audio: None,
            game: None,
            cursor: (0.0, 0.0),
            mouse_down: false,
            feedback: None,
            install: Arc::default(),
            worker: None,
            error: None,
            catalog,
            targets: vec![],
            list_targets: vec![],
            list_view: None,
            pending_focus: None,
            drag: None,
            chooser: None,
            exit_confirmation: false,
            login: login::Login::new(),
            session_worker: None,
            session_ready: false,
            prepared_session: None,
            focus: None,
            diagnostic_export: None,
            launch_target: LaunchTarget::Title,
            view: view::ViewState::default(),
            fling: None,
        };
        // Shown as a dialog until dismissed: a toast could expire before the
        // window is up.
        // A newer build of the app: the release lists, read in the background.
        if !launcher.catalog.config.skip_update_check {
            launcher.view.engine_store.check();
        }
        if let Some((id, reason)) = boot_error {
            launcher.view.transfer_error = Some((
                text::tf("Engine «{}» did not start", &[&id]),
                text::tf(
                    "The built-in engine started instead. Android said: {}",
                    &[&reason],
                ),
            ));
        }
        launcher
    }

    fn restore_launcher(&mut self, events: &ActiveEventLoop) {
        if !horizon_client::android_runtime::take_launcher_return() {
            return;
        }
        if let Some(mut game) = self.game.take() {
            game.exiting(events);
            drop(game);
        }
        crate::game::android_reset_bundled_mods();
        crate::document_store::set_game_active(false);
        crate::performance::stop();
        crate::platform::hide();
        horizon_client::auth::clear_current();
        self.session_ready = false;
        self.prepared_session = None;
        horizon_client::discord_presence::stop();
        horizon_client::ui_theme::reset_atlas();
        self.cursor = (0.0, 0.0);
        self.drag = None;
        self.chooser = None;
        self.exit_confirmation = false;
        self.focus = None;
        self.pending_focus = None;
        if let Err(error) = std::env::set_current_dir(&self.home) {
            self.fail(error);
            return;
        }
        horizon_client::android_runtime::clear_gamepads();
        self.controller_edges = Default::default();
        self.controller_repeat = None;
        self.catalog.config.input_host = horizon_client::android_runtime::input_mode();
        self.catalog.refresh(&self.home);
        self.login.refresh_status();
        crate::diagnostics::heartbeat("returned_to_launcher", || serde_json::json!({}));
        self.catalog.page = Page::Play;
        self.launch_target = LaunchTarget::Title;
        if let Ok(message) = std::fs::read_to_string(self.home.join("android-error.txt")) {
            self.error = Some(message);
        }
        if let Err(error) = self.open(events) {
            self.fail(error);
            events.exit();
        }
    }
    /// The launcher is full screen in landscape and keeps Android's status and
    /// navigation bars in portrait, laid out between them; the game follows the
    /// player's full-screen and aspect choices.
    pub(super) fn configure_screen(&self) {
        let game = self.game.is_some();
        let landscape = self
            .window
            .as_ref()
            .map(|w| {
                let size = w.outer_size();
                size.width > size.height
            })
            .or(self.view.landscape)
            .unwrap_or(false);
        let (immersive, status_bar) = if game {
            (self.catalog.config.screen.immersive, false)
        } else {
            (landscape, !landscape)
        };
        crate::native_ime::configure_screen(
            immersive,
            self.catalog.config.screen.avoid_cutout,
            status_bar,
        );
        winit::platform::android::set_native_content_aspect(if game {
            self.catalog.config.screen.aspect.ratio()
        } else {
            [0, 0]
        });
    }
    fn fail(&mut self, error: impl std::fmt::Display) {
        log::error!("{error}");
        crate::diagnostics::record_error(&error.to_string());
        self.error = Some(error.to_string());
        let _ = std::fs::write(self.home.join("android-error.txt"), error.to_string());
    }

    fn open(&mut self, events: &ActiveEventLoop) -> Result<()> {
        // The launcher keeps the orientation chosen in its settings, as the game
        // does: landscape unless the player picked another.
        crate::platform::set_orientation(self.catalog.config.orientation);
        self.configure_screen();
        crate::display::set_scale(self.catalog.config.render_scale);
        horizon_client::android_runtime::configure_gpu(false);
        if let Some(renderer) = &mut self.renderer {
            if let Some(size) = crate::display::prepare(self.window.as_ref().unwrap())? {
                renderer.resize(size[0], size[1]);
            }
            renderer.recreate_surface()?;
            let size = self.window.as_ref().unwrap().inner_size();
            renderer.resize(size.width, size.height);
            return Ok(());
        }
        let window = Arc::new(
            events.create_window(Window::default_attributes().with_title("Horizon Android"))?,
        );
        crate::display::prepare(&window)?;
        let gui = game::UiResources {
            atlas: HudAtlas::build(&self.home.join("installer"))?,
            panorama: Arc::new(horizon_client::gfx::panorama::Faces::colour([0; 4])),
        };
        self.renderer = Some(game::loading_renderer(
            window.clone(),
            false,
            Some(&gui.atlas),
            Some(&gui.panorama),
            None,
            if self.catalog.config.renderer == crate::catalog::RendererMode::OpenGl {
                horizon_client::graphics::GraphicsApi::OpenGl
            } else {
                horizon_client::graphics::GraphicsApi::Auto
            },
        )?);
        self.window = Some(window);
        self.gui = Some(gui);
        Ok(())
    }

    fn request_back(&mut self) {
        crate::platform::hide();
        if crate::transfers::running().is_some() {
            // Back stops a transfer; its dialog stays until the worker has cleaned up.
            crate::transfers::cancel();
            return;
        }
        if self.view.transfer_error.take().is_some() {
            return;
        }
        if self.exit_confirmation {
            self.exit_confirmation = false;
        } else if self.login.visible {
            self.login.hide();
        } else if self.catalog.editing == Some("instance_name") {
            self.catalog.editing = None;
            crate::platform::hide();
        } else if self.view.instance_edit.take().is_some() {
            // Back closes the instance sheet.
        } else if std::mem::take(&mut self.view.engine_confirm) {
            // And the engine question.
        } else if self.chooser.take().is_some() {
            // Back dismisses the current dialog, including the exit question.
        } else if self.catalog.editing.take().is_some() {
            // Back leaves the text field first.
        } else if self.catalog.page != Page::Play {
            // Then returns to Play; only Play asks before exiting.
            self.catalog.page = Page::Play;
            self.catalog.scroll = 0.0;
            self.catalog.status.clear();
        } else {
            self.exit_confirmation = true;
        }
        self.focus = None;
        self.pending_focus = self.exit_confirmation.then_some(pages::Action::Dismiss);
        self.targets.clear();
        self.list_targets.clear();
        self.drag = None;
        if let Err(error) = self.catalog.config.save(&self.home) {
            self.fail(error);
        }
    }

    fn action(&mut self, events: &ActiveEventLoop) -> Result<()> {
        if self.exit_confirmation
            || self.chooser.is_some()
            || self.worker.is_some()
            || self.session_worker.is_some()
        {
            return Ok(());
        }
        if self.login.pending() {
            return Ok(());
        }
        crate::diagnostics::heartbeat(
            "play_requested",
            || serde_json::json!({"renderer":self.catalog.config.renderer.label(),"resolution_percent":self.catalog.config.render_scale.percent()}),
        );
        horizon_client::android_runtime::configure_worker_limit(self.catalog.config.worker_limit);
        horizon_client::android_runtime::configure_cache_budget(
            self.catalog.config.cache_budget_mib,
        );
        crate::configure_input_mode(self.catalog.config.input_host);
        crate::device_runtime::configure(self.catalog.config.device);
        if horizon_client::official_install::ready(&self.home) {
            horizon_client::android_resources::repair_structure_transforms(
                &self.home.join("engine-data.zip"),
                &self
                    .home
                    .join(".local/horizon-client")
                    .join(horizon_core::VANILLA_VERSION),
            )?;
            crate::platform::hide();
            self.catalog.config.save(&self.home)?;
            // Migrate the earlier launcher preset once; subsequent edits belong to Key Binds.
            let keypad_path = self
                .catalog
                .config
                .game_home(&self.home)
                .join("horizon-keypad.json");
            if self.catalog.config.keypad_controls && !keypad_path.exists() {
                if let Some(dir) = keypad_path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                horizon_client::keypad::Settings {
                    enabled: true,
                    ..Default::default()
                }
                .save(&keypad_path)?;
            }
            self.catalog.config.keypad_controls = false;
            let game_home = self.catalog.config.game_home(&self.home);
            if !self.session_ready && crate::account::Accounts::load().selected().is_some() {
                let id = self.catalog.config.microsoft_client_id.clone();
                let wardrobe_home = self.home.join("wardrobe");
                let (sender, receiver) = mpsc::channel();
                self.session_worker = Some(receiver);
                self.catalog.status = "Preparing account session and appearance...".into();
                std::thread::spawn(move || {
                    let result = (|| {
                        let (account, secrets) = login::Login::session(&id)?
                            .ok_or("The selected account changed. Choose Play again.")?;
                        let appearance =
                            super::skins::cloud::fetch(&account, &secrets, &wardrobe_home)
                                .unwrap_or_default();
                        let file = crate::account::write_session_file(
                            &game_home.join("sessions"),
                            &account,
                            &secrets,
                        )?;
                        Ok(PreparedAccount {
                            key: account.key(),
                            file,
                            appearance,
                        })
                    })();
                    if let Err(unsent) = sender.send(result)
                        && let Ok(prepared) = unsent.0
                    {
                        let _ = std::fs::remove_file(prepared.file);
                    }
                });
                return Ok(());
            }
            self.session_ready = false;
            std::fs::create_dir_all(&game_home)?;
            if crate::account::Accounts::load().selected().is_none() {
                self.wardrobe
                    .prepare_game(&self.catalog.config.name, &game_home)?;
            }
            let extra_arguments = if crate::PUBLIC_BUILD {
                Vec::new()
            } else {
                match std::fs::read(self.home.join("android-launch.json")) {
                    Ok(bytes) => serde_json::from_slice::<Vec<String>>(&bytes)?,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                    Err(e) => return Err(e.into()),
                }
            };

            self.audio = None;
            self.error = None;
            let _ = std::fs::remove_file(self.home.join("android-error.txt"));
            if self.catalog.config.record_performance || self.catalog.config.device.gpu_profile {
                crate::performance::start(
                    &self.home,
                    serde_json::json!({
                        "renderer": self.catalog.config.renderer.label(),
                        "resolution_percent": self.catalog.config.render_scale.percent(),
                        "source_commit": option_env!("HORIZON_ANDROID_BUILD_COMMIT"),
                        "source_dirty": option_env!("HORIZON_ANDROID_BUILD_DIRTY"),
                        "performance_recording": self.catalog.config.record_performance,
                        "gpu_profiling": self.catalog.config.device.gpu_profile,
                    }),
                )?;
            }
            // The A press that launches the game must not accept its first menu too.
            horizon_client::android_runtime::clear_gamepads();
            self.controller_edges = Default::default();
            self.controller_repeat = None;
            let mut extra_arguments = extra_arguments;
            if let LaunchTarget::World(folder) = &self.launch_target {
                // The game opens an existing world by folder name (game.rs `--world`).
                extra_arguments.extend(["--world".into(), folder.clone()]);
            }
            let request = crate::game_launch::LaunchRequest {
                resources: self
                    .home
                    .join(".local/horizon-client")
                    .join(horizon_core::VANILLA_VERSION),
                game_home,
                session_file: self.prepared_session.take(),
                offline_name: crate::catalog::truncate_player_name(&self.catalog.config.name),
                server: if self.launch_target == LaunchTarget::Server {
                    self.catalog.config.server.clone()
                } else {
                    String::new()
                },
                control_socket: (!crate::PUBLIC_BUILD).then(|| self.home.join("control.json")),
                extra_arguments,
                open_gl: self.catalog.config.renderer == crate::catalog::RendererMode::OpenGl,
                gpu_batching: self.catalog.config.renderer == crate::catalog::RendererMode::Vulkan,
            };
            crate::document_store::set_game_active(true);
            // The game starts in the player's orientation (landscape by default,
            // whichever way the launcher was held) and screen mode. Android
            // stretches the last frame while it turns: make it a plain one, not
            // the portrait launcher squeezed sideways.
            self.draw_blank();
            crate::platform::set_orientation(self.catalog.config.orientation);
            crate::native_ime::configure_screen(
                self.catalog.config.screen.immersive,
                self.catalog.config.screen.avoid_cutout,
                false,
            );
            winit::platform::android::set_native_content_aspect(
                self.catalog.config.screen.aspect.ratio(),
            );
            let game = match crate::game_launch::start(request) {
                Ok(game) => game,
                Err(error) => {
                    crate::document_store::set_game_active(false);
                    crate::performance::stop();
                    return Err(error);
                }
            };
            // Android has one native window; release its surface before handing it over.
            self.renderer = None;
            self.window = None;
            self.gui = None;
            horizon_client::ui_theme::reset_atlas();
            self.game = Some(game);
            self.game.as_mut().unwrap().resumed(events);
        } else {
            if !self.home.join("engine-data.zip").is_file() {
                return Err(
                    "This APK is missing its engine data. Install a complete build.".into(),
                );
            }
            self.error = None;
            *self.install.lock().unwrap() = Default::default();
            let home = self.home.clone();
            let install = self.install.clone();
            let (sender, receiver) = mpsc::channel();
            self.worker = Some(receiver);
            std::thread::spawn(move || {
                let result =
                    horizon_client::official_install::install_with_progress(&home, |update| {
                        let mut current = install.lock().unwrap();
                        let done = if current.stage == update.stage {
                            current.done.max(update.done)
                        } else {
                            update.done
                        };
                        *current =
                            horizon_client::official_install::InstallProgress { done, ..update };
                    });
                let _ = sender.send(result.map_err(|e| e.to_string()));
            });
        }
        Ok(())
    }
}

impl ApplicationHandler for Launcher {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        self.configure_screen();
        if let Some(game) = &mut self.game {
            return game.resumed(events);
        }
        if let Err(error) = self.open(events) {
            self.fail(error);
            events.exit();
        }
    }
    fn suspended(&mut self, events: &ActiveEventLoop) {
        crate::diagnostics::heartbeat("suspended", || serde_json::json!({}));
        if let Some(game) = &mut self.game {
            return game.suspended(events);
        }
        self.drag = None;
        if let Some(renderer) = &mut self.renderer {
            renderer.suspend_surface();
        }
    }
    fn window_event(&mut self, events: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if crate::documents::handle_event(&event) {
            return;
        }
        if let WindowEvent::Resized(size) = &event {
            let _ = crate::native_ime::viewport_bottom();
            // Turning the phone switches the launcher between full screen
            // (landscape) and Android's bars (portrait).
            let landscape = size.width > size.height;
            if self.game.is_none() && self.view.landscape != Some(landscape) {
                self.view.landscape = Some(landscape);
                self.configure_screen();
            }
        }
        if matches!(event, WindowEvent::Focused(true)) {
            self.configure_screen();
        }
        if let Some(game) = &mut self.game {
            game.window_event(events, id, event);
            self.restore_launcher(events);
            return;
        }
        let result = match event {
            WindowEvent::CloseRequested => {
                self.request_back();
                Ok(())
            }
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
                Ok(())
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.focus = None;
                self.pending_focus = None;
                self.cursor = (position.x, position.y);
                Ok(())
            }
            WindowEvent::Touch(touch) => self.touch_event(events, touch),
            WindowEvent::Focused(false) => {
                crate::diagnostics::heartbeat("launcher_background", || serde_json::json!({}));
                horizon_client::android_runtime::clear_gamepads();
                self.controller_repeat = None;
                self.mouse_down = false;
                self.drag = None;
                Ok(())
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if state == ElementState::Pressed {
                    self.mouse_down = true;
                    Ok(())
                } else if std::mem::take(&mut self.mouse_down) {
                    self.activate(events)
                } else {
                    Ok(())
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.exit_confirmation {
                    return;
                }
                let y = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y * 42.0,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / self.ui_density(),
                };
                if let Some(chooser) = &mut self.chooser {
                    chooser.scroll = (chooser.scroll - y).max(0.0);
                } else {
                    self.catalog.scroll = (self.catalog.scroll - y).max(0.0);
                }
                Ok(())
            }
            WindowEvent::RedrawRequested => self.draw_pages(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if event.physical_key
                    == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape)
                    || event.logical_key
                        == winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrowserBack)
                {
                    self.type_key(event);
                    Ok(())
                } else if self.navigate(&event) {
                    Ok(())
                } else if self.catalog.editing.is_none()
                    && !event.repeat
                    && matches!(
                        event.logical_key,
                        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter)
                    )
                {
                    self.activate(events)
                } else {
                    self.type_key(event);
                    Ok(())
                }
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                self.catalog
                    .edit(horizon_client::osk::Output::Text(text), &self.home);
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(error);
        }
    }
    fn device_event(&mut self, events: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        if matches!(event, DeviceEvent::Removed) {
            self.mouse_down = false;
            self.drag = None;
        }
        if let Some(game) = &mut self.game {
            game.device_event(events, id, event);
        } else {
            horizon_client::android_runtime::gamepad_event(
                winit::platform::android::native_device_id(id),
                &event,
            );
        }
    }
    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        if let Some(game) = &mut self.game {
            game.about_to_wait(events);
            self.restore_launcher(events);
            return;
        }
        crate::diagnostics::heartbeat("launcher", || serde_json::json!({}));
        if let Some((result, delivery)) = self
            .diagnostic_export
            .as_ref()
            .and_then(|(r, delivery)| Some((r.try_recv().ok()?, *delivery)))
        {
            self.diagnostic_export = None;
            match result {
                Ok(path) => self.send_report(path, crate::transfers::Report::Diagnostics, delivery),
                Err(error) => self.toast(text::t("Could not prepare the report"), error, true),
            }
        }
        // An installed engine started for a launch: open what was asked, once the
        // launcher has its window and the game files are in place.
        if self.renderer.is_some()
            && self.start_request.is_some()
            && horizon_client::official_install::ready(&self.home)
            && let Some(request) = self.start_request.take()
        {
            let target = &request["target"];
            // The instance it was started for becomes the one in use.
            if let Some(id) = target["instance"]
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                && id != self.catalog.config.profile
            {
                self.catalog.config.profile = id;
                if let Err(error) = self.catalog.config.save(&self.home) {
                    log::warn!("Instance {id}: {error}");
                }
                self.catalog.refresh(&self.home);
                self.refresh_worlds();
            }
            self.launch_target = if let Some(world) = target["world"].as_str() {
                LaunchTarget::World(world.to_owned())
            } else if let Some(server) = target["server"].as_str() {
                self.catalog.config.server = server.to_owned();
                LaunchTarget::Server
            } else {
                LaunchTarget::Title
            };
            log::info!("Engine launch request: {target}");
            if let Err(error) = self.action(events) {
                self.fail(error);
            }
        }
        match self.view.engine_store.poll() {
            Some(crate::engine_store::Done::Installed(manifest)) => {
                self.refresh_engines();
                self.toast(
                    text::tf("«{}» installed", &[&manifest.name]),
                    text::t("Start it below, or choose it for an instance."),
                    false,
                );
            }
            Some(crate::engine_store::Done::Failed(error)) => self.toast(
                text::t("Could not install the engine"),
                text::t_dyn(&error),
                true,
            ),
            Some(crate::engine_store::Done::Checked) => {
                if !self.view.app_update_told
                    && let Some(update) = view::app_update(&self.view.engine_store)
                {
                    self.view.app_update_told = true;
                    let title = if update.id == "aurora-dev" {
                        text::t("A new dev build is out").to_owned()
                    } else {
                        text::tf("Horizon Aurora {} is out", &[&update.version])
                    };
                    self.toast(
                        title,
                        text::t("Settings › Engines: open its page to update."),
                        false,
                    );
                }
            }
            None => {}
        }
        while let Some(notice) = crate::transfers::poll() {
            if notice.worlds {
                self.refresh_worlds();
                self.refresh_instances();
            }
            if notice.error {
                self.view.toast = None;
                self.view.transfer_error = Some((notice.title, notice.detail));
            } else {
                self.toast(notice.title, notice.detail, false);
            }
        }
        crate::platform::poll_gamepad_connections();
        if let Err(error) = self.controller_frame(events) {
            self.fail(error);
        }
        if let (Some(window), Some(renderer)) = (&self.window, &mut self.renderer)
            && let Err(error) = crate::display::apply(window, renderer)
        {
            crate::display::set_scale(crate::display::RenderScale::default());
            self.fail(error);
        }
        if let Some(result) = self.worker.as_ref().and_then(|w| w.try_recv().ok()) {
            self.worker = None;
            if let Err(error) = result {
                let message = error.to_string();
                if message.contains("Dns Failed") || message.contains("lookup address") {
                    self.fail("Minecraft download interrupted: DNS could not resolve the download host. Check your connection, VPN or Private DNS, then retry Download. Completed files are kept.");
                } else {
                    self.fail(error);
                }
            }
        }
        for out in crate::platform::poll() {
            if matches!(out, horizon_client::osk::Output::Enter) {
                crate::platform::hide();
            }
            self.edit_text(out);
        }
        if let Some(audio) = &mut self.audio {
            audio.tick(false, &Default::default());
        }
        self.catalog.poll(&self.home);
        self.wardrobe.poll();
        if let Some(notice) = crate::documents::poll() {
            self.wardrobe.reload();
            self.catalog.status = notice.text;
            if notice.changed {
                self.catalog.refresh(&self.home);
                self.catalog.show_installed(notice.kind);
            }
        }
        for result in crate::gallery::poll() {
            self.catalog.status = result.unwrap_or_else(|e| format!("File export failed: {e}"));
        }
        if self.login.poll() {
            self.session_worker = None;
            self.prepared_session = None;
            self.session_ready = false;
            self.refresh_wardrobe_account();
        }
        if let Some(result) = self.session_worker.as_ref().and_then(|w| w.try_recv().ok()) {
            self.session_worker = None;
            match result {
                Ok(prepared) => {
                    // Only the current selection may publish the game handoff. A canceled
                    // worker downloads public cache images but cannot overwrite a new profile.
                    if crate::account::Accounts::load()
                        .selected()
                        .is_none_or(|a| a.key() != prepared.key)
                    {
                        let _ = std::fs::remove_file(prepared.file);
                        self.catalog.status =
                            "The selected account changed. Choose Play again.".into();
                    } else {
                        let root = self.home.join("wardrobe");
                        let cloud = prepared.appearance.skin.as_deref().map(|p| {
                            (
                                p,
                                prepared.appearance.slim,
                                prepared.appearance.cape.as_deref(),
                            )
                        });
                        let appearance =
                            super::skins::library::Library::load(&root).and_then(|library| {
                                library.prepare_game(
                                    &root,
                                    &prepared.key,
                                    &self.catalog.config.game_home(&self.home),
                                    cloud,
                                )
                            });
                        match appearance {
                            Ok(()) => {
                                self.prepared_session = Some(prepared.file);
                                self.session_ready = true;
                                if let Err(e) = self.action(events) {
                                    self.fail(e);
                                }
                            }
                            Err(error) => {
                                let _ = std::fs::remove_file(prepared.file);
                                self.fail(error);
                            }
                        }
                    }
                }
                Err(e) => self.fail(e),
            }
        }

        // A list sliding after a throw keeps moving at display rate; otherwise
        // ten frames a second are enough for the sky and progress.
        let mut sliding = false;
        if let Some(fling) = &mut self.fling {
            if let Some(distance) = fling.step() {
                let (scroll, maximum) = if fling.chooser {
                    let maximum = self.chooser.as_ref().map_or(0.0, |c| c.viewport()[2]);
                    (self.chooser.as_mut().map(|c| &mut c.scroll), maximum)
                } else {
                    (
                        Some(&mut self.catalog.scroll),
                        self.list_view.map_or(0.0, |v| v[2]),
                    )
                };
                if let Some(scroll) = scroll {
                    let next = (*scroll + distance).clamp(0.0, maximum);
                    // Hitting either end stops the slide.
                    sliding = next != *scroll && next > 0.0 && next < maximum;
                    *scroll = next;
                }
            }
            if !sliding {
                self.fling = None;
            }
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        events.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(if sliding { 16 } else { 100 }),
        ));
    }
    fn exiting(&mut self, events: &ActiveEventLoop) {
        if let Some(game) = &mut self.game {
            game.exiting(events);
        }
    }
}

impl Launcher {
    fn controller_frame(&mut self, events: &ActiveEventLoop) -> Result<()> {
        use horizon_client::gamepad::Button;
        use winit::keyboard::{Key, NamedKey};
        let mut state = horizon_client::android_runtime::gamepad_state();
        for (b, held) in [
            (Button::DPadLeft, state.left[0] < -0.55),
            (Button::DPadRight, state.left[0] > 0.55),
            (Button::DPadUp, state.left[1] > 0.55),
            (Button::DPadDown, state.left[1] < -0.55),
        ] {
            state.set(b, held || state.held(b));
        }
        let now = Instant::now();
        let mut pressed: Vec<_> = self
            .controller_edges
            .update(&state, 0.5)
            .into_iter()
            .filter_map(|(b, down)| down.then_some(b))
            .collect();
        if let Some((button, last, repeating)) = self.controller_repeat {
            if !state.held(button) {
                self.controller_repeat = None;
            } else if now.duration_since(last).as_secs_f32() >= if repeating { 0.09 } else { 0.35 }
            {
                pressed.push(button);
                self.controller_repeat = Some((button, now, true));
            }
        }
        if crate::documents::busy() {
            // The import overlay is modal: the controller drives its choices.
            for b in pressed {
                match b {
                    Button::DPadLeft | Button::DPadUp => crate::documents::focus_step(false),
                    Button::DPadRight | Button::DPadDown => crate::documents::focus_step(true),
                    Button::South => crate::documents::activate_focused(),
                    Button::East => crate::documents::cancel(),
                    _ => {}
                }
            }
            return Ok(());
        }
        for b in pressed {
            let direction = match b {
                Button::DPadUp => Some(NamedKey::ArrowUp),
                Button::DPadDown => Some(NamedKey::ArrowDown),
                Button::DPadLeft => Some(NamedKey::ArrowLeft),
                Button::DPadRight => Some(NamedKey::ArrowRight),
                _ => None,
            };
            if let Some(key) = direction {
                self.navigate_key(&Key::Named(key));
                if self.controller_repeat.is_none_or(|(old, _, _)| old != b) {
                    self.controller_repeat = Some((b, now, false));
                }
            } else if b == Button::South {
                self.activate(events)?;
            } else if b == Button::East {
                // The controller's Back follows the same steps as Android Back.
                self.request_back();
            }
        }
        Ok(())
    }
}
