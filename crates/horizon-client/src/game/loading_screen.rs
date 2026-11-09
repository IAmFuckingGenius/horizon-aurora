//! The loading screen while the resources load (`Loading`), the window's app before the game
//! takes over (`App`), and the checks a start makes first: missing or damaged prepared files,
//! an unclean last exit, a fullscreen the window system could not show.
use super::*;

pub(crate) fn exit_app(events: &dyn event_loop::EventLoopCtl) {
    #[cfg(target_os = "android")]
    horizon_client::android_runtime::return_to_launcher();
    #[cfg(not(target_os = "android"))]
    events.exit();
    #[cfg(target_os = "android")]
    let _ = events;
}
/// The window system ended the event loop with a failure (on Linux, a compositor dropping the
/// connection after a protocol error) while fullscreen was on: the next start opens windowed, so a
/// fullscreen the system can't show doesn't crash every launch (#2904).
pub(super) fn leave_fullscreen_after(
    result: &std::result::Result<(), EventLoopError>,
    settings: &mut Settings,
) {
    if matches!(result, Err(EventLoopError::ExitFailure(_))) && settings.fullscreen {
        settings.fullscreen = false;
        settings.save(&Game::options_path());
        log::warn!(
            "the window system closed the game in fullscreen; the next start opens windowed"
        );
    }
}

pub(super) fn run_event_loop<'a>(mut game: Game<'a>, resources: &'a Arc<Resources>) -> Result<()> {
    if game.options.headless {
        // No display to open winit's loop on: the game's own loop.
        game.run_headless();
        // narrator::start ran for it too (options.txt's narrator setting).
        narrator::stop();
        write_report(&game)?;
        return game.error.take().map_or(Ok(()), Err);
    }
    let mut split = split::Split::new(game, resources);
    let result = new_event_loop()?.run_app(&mut split);
    game = split.host;
    narrator::stop();
    leave_fullscreen_after(&result, &mut game.settings);
    result?;
    #[cfg(feature = "discord")]
    horizon_client::discord_presence::stop();
    write_report(&game)?;
    if let Some(e) = game.error.take() {
        return Err(e);
    }
    Ok(())
}

/// The loading screen's state while the worker loads the resources.
pub(super) struct Loading {
    pub(super) window: Option<Arc<Window>>,
    pub(super) renderer: Option<Renderer>,
    /// The boot screen's atlas, until the pack's GUI atlas replaces it.
    pub(super) boot: horizon_client::hud::HudAtlas,
    pub(super) gui: Option<UiResources>,
    pub(super) settings: Settings,
    pub(super) fullscreen: horizon_client::window::FullscreenState,
    pub(super) video_modes: horizon_client::window::VideoModeList,
    pub(super) options: Option<Options>,
    pub(super) vanilla_root: PathBuf,
    pub(super) pack_error: Option<String>,
    pub(super) started: Instant,
    pub(super) panorama_rotation: horizon_client::gfx::panorama::Rotation,
    pub(super) progress: Progress,
    pub(super) receiver: Receiver<LoaderMsg>,
    pub(super) loaded: Option<std::result::Result<Loaded, String>>,
    pub(super) renderer_job: Option<JoinHandle<std::result::Result<Renderer, String>>>,
    /// GLES preparation exclusively owns the context; keep pumping native events without drawing.
    pub(super) renderer_job_gl: bool,
    pub(super) backgrounded: bool,
    pub(super) surface_lost: bool,
    pub(super) cancel_renderer: bool,
    pub(super) prepared_renderer: Option<Renderer>,
    /// The loader's `Done` arrived: its channel closing afterwards is expected.
    pub(super) delivered: bool,
    pub(super) handle: Option<JoinHandle<()>>,
    pub(super) shown: bool,
    /// When the menu bundle arrived (`set_ui`), and whether a title frame with
    /// it was presented since: the world renderer waits for that frame
    /// (`ready_for_handoff`).
    pub(super) ui_at: Option<Instant>,
    pub(super) title_drawn: bool,
    pub(super) menu: startup_menu::StartupMenu,
    pub(super) error: Option<Box<dyn std::error::Error>>,
}
/// How long the handoff waits for the first title frame before building the
/// world renderer anyway (a hidden window may never be asked to redraw).
pub(super) const TITLE_FRAME_WAIT: Duration = Duration::from_secs(1);
impl Loading {
    /// Keep native events flowing while the GPU worker creates world resources.
    /// GLES has exclusive context access; Vulkan can keep drawing the loading screen.
    /// The worker never owns a window surface.
    fn prepare_renderer(&mut self) -> Result<()> {
        if self.menu.reload_packs {
            return Ok(());
        }
        if let Some(job) = &self.renderer_job {
            if !job.is_finished() {
                return Ok(());
            }
            let job = self.renderer_job.take().unwrap();
            self.prepared_renderer = Some(job.join().map_err(|_| "renderer worker panicked")??);
            if self.renderer_job_gl
                && !self.surface_lost
                && let Some(previous) = &mut self.renderer
            {
                // The old surface was released before the worker took the GL context.
                // activate_replacement will attach the new renderer to the current window.
                previous.suspended = false;
            }
            self.renderer_job_gl = false;
            return Ok(());
        }
        if !cfg!(target_os = "android") || self.prepared_renderer.is_some() {
            return Ok(());
        }
        let Some(Ok(loaded)) = &mut self.loaded else {
            return Ok(());
        };
        let Some(previous) = &self.renderer else {
            return Ok(());
        };
        let options = self.options.as_ref().ok_or("loading already finished")?;
        let gl = previous.info.backend == wgpu::Backend::Gl;
        if options.offscreen || (!gl && previous.info.backend != wgpu::Backend::Vulkan) {
            return Ok(());
        }
        if gl && (!self.menu.needs_world_renderer() || !self.menu.entry_presented) {
            return Ok(());
        }
        let gui = self.gui.as_ref().ok_or("missing menu bundle")?;
        let window = self.window.clone().ok_or("window closed")?;
        let context = previous.background_context();
        let assets = loaded.pack.render.clone();
        let forcefield = loaded.pack.forcefield.clone();
        let panorama = gui.panorama.clone();
        let hud = (
            gui.atlas.pixels().into_owned(),
            gui.atlas.width,
            gui.atlas.height,
        );
        let glint = gui.atlas.glint.clone();
        let glint_armor = gui.atlas.glint_armor.clone();
        let prepared = loaded.renderer_cpu.take();
        let api = rendering_api(&self.settings);
        let translucent_batching = self.settings.horizon.translucent_batching;
        let graphics = self.settings.horizon.clone();
        let display = previous.display_reading();
        let shader_pack = shader_packs::picked_pack_shape(&self.settings);
        #[cfg(target_os = "android")]
        if gl {
            // No render, resize or surface operation may contend with a long driver compile.
            // Keep the last presented loading frame while native events continue to be drained.
            self.renderer.as_mut().unwrap().suspend_surface();
        }
        self.renderer_job_gl = gl;
        self.renderer_job = Some(
            std::thread::Builder::new()
                .name("Renderer preparation".into())
                .spawn(move || {
                    let started = Instant::now();
                    let sprite_pixels = assets.pixels.get();
                    let result = pollster::block_on(Renderer::new_with_context(
                        window,
                        true,
                        &sprite_pixels,
                        assets.sprite_size,
                        assets.sprite_minimum_size,
                        assets.texture_layers,
                        &assets.sprite_mips,
                        &assets.animations,
                        (&hud.0, hud.1, hud.2),
                        (&glint.0, glint.1, glint.2),
                        (&glint_armor.0, glint_armor.1, glint_armor.2),
                        (&forcefield.2, forcefield.0),
                        panorama,
                        prepared,
                        Some(context),
                        api,
                        translucent_batching,
                        shader_pack.draws_flat,
                        &graphics,
                        shader_pack.frame(),
                        display,
                    ))
                    .map_err(|error| error.to_string());
                    log::info!(
                        "Horizon: background renderer prepared in {} ms",
                        started.elapsed().as_millis()
                    );
                    result
                })?,
        );
        Ok(())
    }
    /// The world renderer is built on the main thread, so building it before
    /// the title is on the screen holds the title back by that build. Wait for
    /// one title frame first, unless a world is waiting to open.
    pub(super) fn ready_for_handoff(&self) -> bool {
        self.title_drawn
            || self.menu.pending.is_some()
            || self.ui_at.is_none_or(|at| at.elapsed() >= TITLE_FRAME_WAIT)
    }
    pub(super) fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.surface_lost = false;
        if self.renderer_job_gl && self.renderer_job.is_some() {
            return;
        }
        #[cfg(target_os = "android")]
        if let Some(renderer) = &mut self.renderer {
            if let Err(error) = renderer.recreate_surface() {
                self.error = Some(error);
                crate::game::exit_app(event_loop);
            } else if let Some(window) = &self.window {
                let size = window.inner_size();
                renderer.resize(size.width, size.height);
            }
            return;
        }
        if self.window.is_some() {
            return;
        }
        // The Window constructor: created fullscreen when options.txt or --fullscreen says so.
        let fullscreen =
            self.settings.fullscreen || self.options.as_ref().is_some_and(|o| o.fullscreen);
        let size = self
            .options
            .as_ref()
            .map_or([1280, 720], |o| o.window_size)
            .map(f64::from);
        self.fullscreen = horizon_client::window::FullscreenState::new(fullscreen);
        let attributes = horizon_client::window::with_horizon_icon(
            Window::default_attributes().with_title("Horizon"),
        );
        // Wayland names the window by its app id, which the horizon.desktop entry matches.
        #[cfg(target_os = "linux")]
        let attributes = winit::platform::wayland::WindowAttributesExtWayland::with_name(
            attributes, "horizon", "horizon",
        );
        // `--offscreen` renders into a texture: its window would only show white.
        let window_step = startup::step("window.create");
        let window = match event_loop.create_window(
            attributes
                .with_inner_size(winit::dpi::LogicalSize::new(size[0], size[1]))
                .with_fullscreen(fullscreen.then_some(winit::window::Fullscreen::Borderless(None)))
                .with_visible(!self.options.as_ref().is_some_and(|o| o.offscreen)),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(e.into());
                crate::game::exit_app(event_loop);
                return;
            }
        };
        horizon_client::window::settle_created_fullscreen(&window, fullscreen);
        drop(window_step);
        horizon_client::telemetry::note_display(&window);
        // The renderer takes seconds to build and winit's window sits white
        // until the first frame presents, so paint the boot screen on it and
        // keep repainting it as the console's lines arrive; the wgpu frames
        // redraw the same design when they start. GDI is unsafe:
        // horizon-threads holds it (this crate forbids unsafe code).
        #[cfg(target_os = "windows")]
        let boot_paint_stop = hwnd(&window).map(|hwnd| {
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let repaint_stop = stop.clone();
            std::thread::spawn(move || {
                while !repaint_stop.load(std::sync::atomic::Ordering::Relaxed) {
                    horizon_threads::boot_paint::paint(
                        hwnd,
                        horizon_theme::fonts::GEIST_SEMIBOLD,
                        horizon_theme::fonts::GEIST_MONO,
                        &horizon_client::logging::recent_lines(),
                    );
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
            });
            stop
        });
        let offscreen = self.options.as_ref().is_some_and(|o| o.offscreen);
        let boot_step = startup::step("renderer.boot");
        match loading_renderer(
            window.clone(),
            offscreen,
            Some(&self.boot),
            None,
            None,
            rendering_api(&self.settings),
        ) {
            Ok(r) => self.renderer = Some(r),
            Err(e) => {
                #[cfg(target_os = "windows")]
                if let Some(stop) = &boot_paint_stop {
                    stop.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                self.error = Some(e);
                crate::game::exit_app(event_loop);
                return;
            }
        }
        drop(boot_step);
        // The wgpu frames take the window over: stop the boot paint thread
        // before the first one presents.
        #[cfg(target_os = "windows")]
        if let Some(stop) = &boot_paint_stop {
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        horizon_client::hud::GUI_SCALE.store(self.settings.gui_scale, Ordering::Relaxed);
        self.window = Some(window);
    }
    pub(super) fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        mut event: WindowEvent,
    ) {
        if let WindowEvent::Occluded(backgrounded) = &event {
            self.backgrounded = *backgrounded;
        }
        if self.renderer_job_gl && self.renderer_job.is_some() {
            if matches!(event, WindowEvent::CloseRequested)
                || matches!(&event, WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed
                        && matches!(event.logical_key, winit::keyboard::Key::Named(
                            winit::keyboard::NamedKey::Escape | winit::keyboard::NamedKey::BrowserBack)))
            {
                self.cancel_renderer = true;
            }
            return;
        }
        if self.window.as_ref().is_none_or(|w| w.id() != id) {
            return;
        }
        follow_keymap(&mut event);
        if let Err(error) = self.menu_event(event_loop, event) {
            self.error = Some(error);
            crate::game::exit_app(event_loop);
        }
    }
    /// The menu bundle arrived: rebuild the loading screen with the real atlas
    /// and title panorama.
    pub(super) fn set_ui(&mut self, gui: UiResources, root: PathBuf) {
        log::info!(
            "Horizon: loading the title artwork after {} ms",
            self.started.elapsed().as_millis()
        );
        self.menu.set_root(root, &self.settings);
        self.gui = Some(gui);
        self.title_drawn = false;
        horizon_client::ui_theme::reset_atlas();
        self.apply_menu_settings();
        let Some(window) = self.window.clone() else {
            return;
        };
        let offscreen = self.options.as_ref().is_some_and(|o| o.offscreen);
        if let Some(renderer) = &mut self.renderer {
            renderer.release_surface();
        }
        let bundle = self.gui.as_ref().expect("the menu bundle was just set");
        let _step = startup::step("renderer.title_artwork");
        match loading_renderer(
            window,
            offscreen,
            Some(&bundle.atlas),
            Some(&bundle.panorama),
            self.renderer.as_ref(),
            rendering_api(&self.settings),
        ) {
            Ok(r) => {
                horizon_client::hud::GUI_SCALE.store(self.settings.gui_scale, Ordering::Relaxed);
                self.renderer = Some(r);
            }
            Err(e) => self.error = Some(e),
        }
        // The title frame's wait starts once its renderer is built.
        self.ui_at = Some(Instant::now());
    }
    pub(super) fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer_job_gl && self.renderer_job.is_some() {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(50),
            ));
            return;
        }
        #[cfg(target_os = "android")]
        self.menu_android_keyboard();
        #[cfg(target_os = "android")]
        if let Err(error) = self.menu_gamepad_frame(event_loop) {
            self.error = Some(error);
        }
        #[cfg(target_os = "android")]
        self.tick_menu_audio();
        // The loop may run once more after exit(): keep the first error, the
        // one that ended loading.
        if self.error.is_some() {
            event_loop.exit();
            return;
        }
        if horizon_threads::stop_requests() > 0 {
            // No world is open yet: nothing to save.
            crate::game::exit_app(event_loop);
            return;
        }
        if let Err(error) = self.poll_menu_resources() {
            self.error = Some(error);
            crate::game::exit_app(event_loop);
            return;
        }
        self.poll_menu_realms(event_loop);
        if let Some(w) = &self.window {
            if self.options.as_ref().is_some_and(|o| o.offscreen) {
                self.draw();
            } else {
                w.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::Poll);
    }
    pub(super) fn draw(&mut self) {
        let Some(size) = self
            .renderer
            .as_ref()
            .map(|r| [r.config.width, r.config.height])
        else {
            return;
        };
        let now = Instant::now();
        let frame_ms = self
            .menu
            .last_frame
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f32() * 1000.0);
        #[cfg(target_os = "android")]
        crate::diagnostics::heartbeat(
            "game_resource_loading",
            || serde_json::json!({"gpu":self.renderer.as_ref().map(|r| &r.info.name)}),
        );
        let mut batch = HudBatch::new(size);
        self.draw_menu(&mut batch);
        horizon_client::widgets::publish_frame();
        self.menu.frame_drawn((batch.width, batch.height));
        let mut frame = loading_frame(&batch);
        frame.yaw = self.panorama_rotation.update(
            self.started.elapsed().as_secs_f32(),
            self.settings.file.double("panoramaScrollSpeed") as f32,
        );
        if let Some(renderer) = &mut self.renderer {
            match renderer.render(&frame) {
                Ok(true) => {
                    self.menu.entry_presented = self.menu.needs_world_renderer();
                    if self.gui.is_some() && self.menu.pending.is_none() {
                        self.title_drawn = true;
                    }
                    if self.gui.is_some() && !self.menu.needs_world_renderer() {
                        self.menu.perf.title_shown();
                    }
                }
                Ok(_) => {}
                Err(e) => log::warn!("Horizon: loading screen frame failed: {e}"),
            }
            if let Some(frame_ms) = frame_ms {
                let cpu = now
                    .elapsed()
                    .saturating_sub(renderer.last_wait)
                    .as_secs_f32()
                    * 1000.0;
                self.menu
                    .frame_graph
                    .record(frame_ms, cpu, renderer.gpu_times.last().copied());
                renderer.gpu_times.clear();
            }
        }
        if !self.shown {
            self.shown = true;
            startup::mark("loading_screen.shown");
            log::info!(
                "Horizon: loading screen shown after {} ms",
                self.started.elapsed().as_millis()
            );
        }
    }
    pub(super) fn finish(&mut self, loaded: Loaded) -> Result<Option<session::Session>> {
        let handoff_at = Instant::now();
        let mut options = self.options.take().ok_or("loading already finished")?;
        options.resources = loaded.root.clone();
        // StartupMenu owns world selection until it can hand it to Game.
        options.world = None;
        options.server = None;
        let gui = self.gui.take().ok_or("missing menu bundle")?;
        let window = self.window.clone().ok_or("window closed")?;
        let renderer_at = Instant::now();
        let renderer = if let Some(mut renderer) = self.prepared_renderer.take() {
            renderer
                .activate_replacement(self.renderer.as_mut().ok_or("missing loading renderer")?)?;
            // Glyphs are still being drawn into the loading atlas until this
            // handoff; invalidate only now, after the new GPU atlas is ready.
            gui.atlas.font.reset_cache();
            renderer.hud.sprite_animations = gui
                .atlas
                .sprite_animations
                .iter()
                .map(|(rect, animation)| (*rect, animation.clone(), (usize::MAX, 0, 0)))
                .collect();
            renderer
        } else {
            // Keep the loader's device and swap in the real textures in one step.
            if let Some(renderer) = &mut self.renderer {
                renderer.release_surface();
            }
            startup::time("renderer.world", || {
                build_renderer(
                    window.clone(),
                    options.offscreen,
                    &loaded.pack,
                    &gui,
                    self.renderer.as_ref(),
                    loaded.renderer_cpu,
                    rendering_api(&self.settings),
                    self.settings.horizon.translucent_batching,
                    &self.settings.horizon,
                    shader_packs::picked_pack_shape(&self.settings),
                    self.renderer
                        .as_ref()
                        .map(|r| r.display_reading())
                        .unwrap_or_default(),
                    (
                        self.settings.file.int("textureFiltering"),
                        self.settings.file.int("maxAnisotropyBit"),
                    ),
                )
            })?
        };
        log::info!(
            "Horizon: world renderer ready after {} ms",
            self.started.elapsed().as_millis()
        );
        let renderer_ms = renderer_at.elapsed().as_millis();
        // Audio is created on the main thread, after the core is ready.
        let audio = if !options.execution_role.is_client() {
            Audio::silent(&options.resources)
        } else {
            #[cfg(target_os = "android")]
            {
                self.menu.audio.take().unwrap_or_else(|| {
                    Audio::with_device(&options.resources, &self.settings.sound_device)
                })
            }
            #[cfg(not(target_os = "android"))]
            Audio::with_device(&options.resources, &self.settings.sound_device)
        };
        let pack_error = loaded.pack_error.or_else(|| self.pack_error.take());
        // The session drops the games before releasing their borrowed resources.
        let game_at = Instant::now();
        let session = session::Session::try_new(loaded.resources, |res| -> Result<_> {
            let game_step = startup::step("handoff.start_game");
            let Some(mut game) = start_game(
                res,
                loaded.pack,
                gui,
                options,
                pack_error,
                audio,
                self.vanilla_root.clone(),
                self.started,
                loaded.rest_loader,
                Some(self.settings.clone()),
                loaded.game_cpu,
                loaded.entity_models,
            )?
            else {
                return Ok(None);
            };
            drop(game_step);
            let game_ms = game_at.elapsed().as_millis();
            game.window = Some(window);
            game.renderer = Some(renderer);
            game.world_renderer_ready = true;
            game.fullscreen = self.fullscreen;
            game.panorama_rotation = self.panorama_rotation;
            self.menu.handoff(&mut game);
            game.apply_settings();
            game.previous = Instant::now();
            // Minecraft.onGameLoadFinished: a start that made it this far is clean.
            game.settings.file.set(
                "startedCleanly",
                horizon_client::options_file::V::Bool(true),
            );
            game.settings.save(&Game::options_path());
            log::info!(
                "Horizon: startup handoff {} ms (renderer {} ms, start_game {} ms)",
                handoff_at.elapsed().as_millis(),
                renderer_ms,
                game_ms
            );
            Ok(Some(split::Split::new(game, res)))
        })?;
        Ok(session.has_game().then_some(session))
    }
}

/// The application: the loading screen until the resources are ready, then
/// the game itself, on the same event loop and window.
pub(super) enum App {
    Loading(Box<Loading>),
    Ready(Box<session::Session>),
}
impl ApplicationHandler for App {
    #[cfg(target_os = "android")]
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        match self {
            App::Ready(game) => game.with_game(|game| game.suspended(event_loop)),
            App::Loading(loading) => {
                loading.surface_lost = true;
                if !loading.renderer_job_gl
                    && let Some(renderer) = &mut loading.renderer
                {
                    renderer.suspend_surface();
                }
            }
        }
    }
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        match self {
            App::Loading(loading) => loading.resumed(event_loop),
            App::Ready(game) => game.with_game(|game| game.resumed(event_loop)),
        }
    }
    #[cfg(target_os = "android")]
    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        if let App::Ready(game) = self {
            game.with_game(|game| game.memory_warning(event_loop));
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match self {
            App::Loading(loading) => loading.window_event(event_loop, id, event),
            App::Ready(game) => game.with_game(|game| game.window_event(event_loop, id, event)),
        }
    }
    fn device_event(&mut self, event_loop: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        match self {
            App::Ready(game) => game.with_game(|game| game.device_event(event_loop, id, event)),
            App::Loading(_loading) =>
            {
                #[cfg(target_os = "android")]
                if _loading.settings.controller.enabled {
                    horizon_client::android_runtime::gamepad_event(
                        winit::platform::android::native_device_id(id),
                        &event,
                    );
                }
            }
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        enum Step {
            Waiting,
            Ready(Box<session::Session>),
            Done,
            Failed(Box<dyn std::error::Error>),
        }
        #[cfg(target_os = "android")]
        if matches!(self, App::Loading(loading) if loading.surface_lost || loading.backgrounded) {
            // A ready worker must not recreate a surface while Android owns
            // no native window. Its messages stay queued until resumed.
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let step = match self {
            App::Ready(game) => {
                game.with_game(|game| game.about_to_wait(event_loop));
                return;
            }
            App::Loading(loading) => {
                loading.about_to_wait(event_loop);
                if loading.error.is_some() {
                    crate::game::exit_app(event_loop);
                    return;
                }
                if let Err(error) = loading.prepare_renderer() {
                    loading.error = Some(error);
                    crate::game::exit_app(event_loop);
                    return;
                }
                if loading.cancel_renderer && loading.renderer_job.is_none() {
                    crate::game::exit_app(event_loop);
                    return;
                }
                // Preserve the presented-frame boundary before preparing world-only GLES work.
                let defer_gles_world = cfg!(target_os = "android")
                    && loading
                        .renderer
                        .as_ref()
                        .is_some_and(|r| r.info.backend == wgpu::Backend::Gl)
                    && (!loading.menu.needs_world_renderer() || !loading.menu.entry_presented);
                match if loading.menu.reload_packs
                    || loading.renderer_job.is_some()
                    || defer_gles_world
                    || !loading.ready_for_handoff()
                {
                    None
                } else {
                    loading.loaded.take()
                } {
                    None => Step::Waiting,
                    Some(Err(e)) => Step::Failed(e.into()),
                    Some(Ok(loaded)) => match loading.finish(loaded) {
                        Ok(Some(game)) => Step::Ready(Box::new(game)),
                        Ok(None) => Step::Done,
                        Err(e) => Step::Failed(e),
                    },
                }
            }
        };
        match step {
            Step::Waiting => {}
            Step::Ready(game) => *self = App::Ready(game),
            Step::Done => crate::game::exit_app(event_loop),
            Step::Failed(e) => {
                if let App::Loading(loading) = self {
                    loading.error = Some(e);
                }
                crate::game::exit_app(event_loop);
            }
        }
    }
    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        if let App::Ready(game) = self {
            game.with_game(|game| game.exiting(event_loop));
        }
        #[cfg(target_os = "android")]
        {
            let report = |error: Option<&(dyn std::error::Error + 'static)>| {
                if let Some(error) = error {
                    log::error!("Horizon: {error}");
                    let home = crate::backend::game_home();
                    crate::diagnostics::record_error(&error.to_string());
                    horizon_client::crash::report_error(&home, "client", error);
                    let _ = std::fs::write(home.join("android-error.txt"), error.to_string());
                }
            };
            match self {
                App::Ready(game) => game.with_game(|game| report(game.host.error.as_deref())),
                App::Loading(loading) => report(loading.error.as_deref()),
            }
        }
    }
}

/// Every launch reads the prepared game files (`horizon-prepare`'s output, whose
/// manifest is `horizon-resources.json`): without them, say where they were
/// looked for and how to get them, before a window or a loader starts.
pub(super) fn missing_resources(root: &std::path::Path) -> Option<String> {
    let shown = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    if !root.join("horizon-resources.json").is_file() {
        return Some(format!(
            "Horizon's prepared game files are missing: no horizon-resources.json in {}\n\
             Horizon is started from its launcher: download it at {} (Download). It needs your \
             own Minecraft: Java Edition; the launcher signs you in and prepares the game files.\n\
             Developers: horizon-prepare --prepare, or pass --resources <folder>",
            shown.display(),
            horizon_client::no_resources::DOWNLOAD_URL
        ));
    }
    damaged_prepared(root).map(|problem| {
        format!(
            "Horizon's prepared game files are damaged: {problem} in {}\n\
             Prepare them again with the launcher, or: horizon-prepare --prepare",
            shown.display()
        )
    })
}

/// Which top-level prepared file a killed preparation or an interrupted copy
/// left unusable. The loader parses these before it can start; naming one
/// beats serde's bare "expected value at line 1 column 1".
pub(super) fn damaged_prepared(root: &std::path::Path) -> Option<String> {
    let manifest = root.join("horizon-resources.json");
    let bytes = std::fs::read(&manifest).ok()?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Some("horizon-resources.json is empty".into());
    }
    if serde_json::from_slice::<serde_json::Value>(&bytes).is_err() {
        return Some("horizon-resources.json is not valid JSON".into());
    }
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        if path.extension() != Some(std::ffi::OsStr::new("json")) {
            continue;
        }
        if std::fs::metadata(&path).is_ok_and(|meta| meta.len() == 0) {
            return Some(format!("{} is empty", entry.file_name().to_string_lossy()));
        }
    }
    None
}

/// `Minecraft.<init>`: `startedCleanly` left false means the previous start
/// died before `onGameLoadFinished`. Vanilla then leaves fullscreen
/// ("resetting fullscreen mode") so a start that died on the display mode can
/// come back; the resource packs are rolled back only when their load fails
/// (`rollbackResourcePacks`), not here. True when the last start was unclean.
pub(super) fn unclean_start_recovery(settings: &mut Settings) -> bool {
    // Vanilla's field defaults true: only an explicit `false` is an unclean
    // start; an options file that never had the key is not one.
    let unclean = matches!(
        settings.file.get("startedCleanly"),
        Some(horizon_client::options_file::V::Bool(false))
    );
    if unclean {
        settings.fullscreen = false;
        settings.file.fullscreen_resolution = None;
    }
    unclean
}
