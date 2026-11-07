use super::kit::Kit;
use super::*;
use horizon_client::osk::Output;
use winit::{
    event::{KeyEvent, Touch},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Action {
    Page(Page),
    Play,
    ShaderSource(crate::shader_catalog::Origin),
    ModSource(crate::shader_catalog::Origin),
    Field(&'static str),
    FinishNameEdit,
    Install(usize),
    Import(horizon_client::content_files::Kind),
    OpenFolder(horizon_client::content_files::Kind),
    ToggleLocal(usize),
    DeleteLocal(usize),
    ConfirmDeleteLocal,
    ConfirmExit,
    Next,
    Previous,
    Profile,
    RecordPerformance,
    Account,
    SignIn,
    ElySignIn,
    MicrosoftAccount,
    OfflineAccount,
    SubmitEly,
    SavedAccounts,
    SavedPage(bool),
    SelectAccount(usize),
    SelectOffline,
    WardrobeKind(bool),
    WardrobeSelect(usize),
    WardrobeImport,
    WardrobeCloud,
    WardrobeUpload,
    WardrobeActivateCape(usize),
    WardrobeUseCloud,
    WardrobeModel,
    WardrobeRotate(i8),
    WardrobeZoom(bool),
    WardrobeHideCape,
    DeviceSignIn,
    CopyLoginCode,
    CancelLogin,
    ReviewApp,
    Repository,
    /// The Instances page and its sheet.
    NewInstance,
    SelectInstance(u32),
    PlayInstance(u32),
    EditInstance(u32),
    /// The sheet's engine: `None` the APK's own, else an index of the installed ones.
    InstanceEngine(Option<usize>),
    SaveInstance,
    DeleteInstance,
    CloseInstanceEdit,
    /// Settings › Display: the launcher's language.
    LauncherLanguage(crate::catalog::LauncherLanguage),
    /// Settings › Engines: Import asks first; then the file picker.
    ImportEngine,
    ConfirmEngineImport,
    CancelEngineImport,
    LaunchEngine(usize),
    DeleteEngine(usize),
    BuiltInEngine,
    /// Read the lists of engines in Aurora's releases.
    CheckEngines,
    /// Download and install an offered engine.
    DownloadEngine(usize),
    /// Open the release page of the newer build of the app.
    OpenAppUpdate,
    /// Read the release lists when the launcher opens, or not.
    UpdateCheck,
    SignOut,
    LoginLink,
    CloseLogin,
    Renderer,
    RenderScale,
    ChooseRenderScale(crate::display::RenderScale),
    Orientation,
    ChooseInputHost(crate::catalog::InputHost),
    ChooseOrientation(crate::catalog::Orientation),
    ChooseRenderer(crate::catalog::RendererMode),
    ChooseProfile(u32),
    Dismiss,
    Immersive,
    Cutout,
    Aspect,
    ChooseAspect(crate::display::Aspect),
    DisplayHz,
    ChooseDisplayHz(u16),
    CacheBudget,
    ChooseCacheBudget(u32),
    ToggleBundled(&'static str),
    WorldFiles,
    GameFiles,
    WorkerLimit,
    ChooseWorkerLimit(u32),
    PerformanceHints,
    GpuProfile,
    /// Add-ons: the catalogue (false) or what is installed (true).
    AddonView(bool),
    /// Play straight into the quick-join server.
    JoinServer,
    /// Play straight into the most recent world.
    ContinueWorld,
    /// Play straight into this world of the Worlds page.
    OpenWorld(usize),
    ClearDownloadCache,
    CancelInstall,
    ClearSearch,
    /// Scroll Settings to this section.
    SettingsSection(u8),
    /// Portrait Home: the last-world card on or off.
    HomeLastWorld,
    /// Portrait Home: the profile picker on or off.
    HomeProfile,
    /// Worlds: save this world as a ZIP where the player picks.
    ExportWorld(usize),
    /// Worlds: unpack a world ZIP the player picks.
    ImportWorld,
    CancelTransfer,
    /// Close the dialog of a failed transfer.
    DismissTransfer,
    /// Settings › Diagnostics: the ways to take a report out.
    ReportMenu(crate::transfers::Report),
    SendReport(crate::transfers::Report, crate::transfers::Delivery),
}

impl Launcher {
    /// The same spatial focus algorithm as the game's controller menus.
    pub(super) fn navigate(&mut self, key: &KeyEvent) -> bool {
        self.navigate_key(&key.logical_key)
    }
    pub(super) fn navigate_key(&mut self, key: &winit::keyboard::Key) -> bool {
        use horizon_client::gamepad::{self, Button};
        use winit::keyboard::{Key, NamedKey};
        if self.pending_focus.is_some() {
            return true;
        }
        if self.catalog.editing.is_some() || self.targets.is_empty() {
            return false;
        }
        let button = match key {
            Key::Named(NamedKey::ArrowUp) => Button::DPadUp,
            Key::Named(NamedKey::ArrowDown) => Button::DPadDown,
            Key::Named(NamedKey::ArrowLeft) => Button::DPadLeft,
            Key::Named(NamedKey::ArrowRight) => Button::DPadRight,
            Key::Named(NamedKey::Tab) => {
                self.focus = Some(self.focus.map_or(0, |i| (i + 1) % self.targets.len()));
                return true;
            }
            _ => return false,
        };
        if matches!(button, Button::DPadUp | Button::DPadDown)
            && let Some((_, action)) = self.focus.and_then(|i| self.targets.get(i))
            && let Some(index) = self.list_targets.iter().position(|(_, a)| a == action)
        {
            let rects: Vec<_> = self.list_targets.iter().map(|(r, _)| *r).collect();
            if let Some(next) = gamepad::focus_step_strict(rects[index], button, &rects)
                && let Some(view) = self.list_view
            {
                let (rect, action) = self.list_targets[next];
                let scroll = if let Some(chooser) = &mut self.chooser {
                    &mut chooser.scroll
                } else {
                    &mut self.catalog.scroll
                };
                *scroll = super::gesture::reveal(*scroll, rect, view);
                self.pending_focus = Some(action);
                self.focus = None;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                return true;
            }
        }
        self.focus = Some(
            if let Some(index) = self.focus.filter(|i| *i < self.targets.len()) {
                let rects: Vec<_> = self.targets.iter().map(|(rect, _)| *rect).collect();
                gamepad::focus_step_rect(rects[index], button, &rects).unwrap_or(index)
            } else {
                0
            },
        );
        true
    }

    /// One frame of the page colour alone, presented before the screen turns.
    pub(super) fn draw_blank(&mut self) {
        let (Some(r), Some(gui)) = (&mut self.renderer, &self.gui) else {
            return;
        };
        if r.suspended {
            return;
        }
        let mut b = HudBatch::new([r.config.width, r.config.height]);
        let (w, h) = (b.width, b.height);
        b.fill(&gui.atlas, 0.0, 0.0, w, h, super::kit::col::PAGE);
        if let Err(error) = r.render(&game::loading_frame(&b)) {
            log::warn!("Launcher transition frame: {error}");
        }
    }
    /// Physical pixels per dp: Android's `DisplayMetrics.density`.
    pub(super) fn density(&self) -> f32 {
        self.window
            .as_ref()
            .map_or(2.0, |w| w.scale_factor() as f32)
    }
    pub(super) fn draw_pages(&mut self) -> Result<()> {
        // The screens call back into `self`; hold the renderer and atlas aside.
        let Some(mut renderer) = self.renderer.take() else {
            return Ok(());
        };
        let Some(gui) = self.gui.take() else {
            self.renderer = Some(renderer);
            return Ok(());
        };
        let result = self.draw_frame(&mut renderer, &gui);
        self.renderer = Some(renderer);
        self.gui = Some(gui);
        result
    }
    fn draw_frame(&mut self, r: &mut Renderer, gui: &game::UiResources) -> Result<()> {
        let density = self.density();
        if r.suspended {
            return Ok(());
        }
        self.targets.clear();
        self.list_targets.clear();
        self.list_view = None;
        let mut b = HudBatch::new([r.config.width, r.config.height]);
        let mut ui = Ui::launcher(&gui.atlas, &mut b, (0.0, 0.0), density);
        // OEM navigation bars can remain visible even in fullscreen (keypad phones).
        // Inset the launcher layout only; keep surface and input coordinates intact.
        if let Some(window) = &self.window {
            use winit::platform::android::WindowExtAndroid;
            let rect = window.content_rect();
            let input = winit::platform::android::native_input_scale();
            if rect.bottom > 0 {
                ui.h = ui.h.min(rect.bottom as f32 * input[1] as f32 / ui.density);
            }
            if rect.right > 0 {
                ui.w = ui.w.min(rect.right as f32 * input[0] as f32 / ui.density);
            }
        }
        if let Some(bottom) = crate::native_ime::viewport_bottom() {
            ui.h = ui.h.min(bottom as f32 / ui.density);
        }
        let press =
            (self.mouse_down || self.drag.as_ref().is_some_and(|drag| !drag.moved)).then(|| {
                [
                    self.cursor.0 as f32 / ui.density,
                    self.cursor.1 as f32 / ui.density,
                ]
            });
        let flash = self
            .feedback
            .filter(|(_, time)| time.elapsed() < std::time::Duration::from_millis(160))
            .map(|(action, _)| action);
        let mut targets = std::mem::take(&mut self.targets);
        {
            let mut k = Kit {
                ui: &mut ui,
                targets: &mut targets,
                press,
                flash,
            };
            self.draw_view(&mut k);
            if self.view.instance_edit.is_some() {
                self.list_targets.clear();
                self.list_view = None;
                k.targets.clear();
                self.draw_instance_edit(&mut k);
            }
            if self.view.engine_confirm {
                self.list_targets.clear();
                self.list_view = None;
                k.targets.clear();
                self.draw_engine_confirm(&mut k);
            }
            if self.login.visible {
                self.list_targets.clear();
                self.list_view = None;
                k.targets.clear();
                self.login
                    .draw(&mut k, &self.catalog.config.name, self.catalog.editing);
            }
            if let Some(chooser) = &mut self.chooser {
                k.targets.clear();
                chooser.draw(&mut k);
                self.list_targets = chooser.navigation();
                self.list_view = Some(chooser.viewport());
            }
            if self.exit_confirmation {
                k.targets.clear();
                self.list_targets.clear();
                self.list_view = None;
                self.draw_exit(&mut k);
            }
            if crate::transfers::running().is_some() || self.view.transfer_error.is_some() {
                k.targets.clear();
                self.list_targets.clear();
                self.list_view = None;
                self.draw_transfer(&mut k);
            }
        }
        self.targets = targets;
        if let Some(action) = self.pending_focus.take() {
            self.focus = self.targets.iter().position(|(_, a)| *a == action);
        }
        if let Some(index) = self.focus
            && let Some((rect, action)) = self.targets.get(index)
        {
            let steps = if matches!(action, Action::Page(_) | Action::SettingsSection(_)) {
                2
            } else {
                1
            };
            super::kit::focus(&mut ui, *rect, steps);
            let [x, y, w, h] = *rect;
            self.cursor = (
                f64::from((x + w / 2.0) * ui.density),
                f64::from((y + h / 2.0) * ui.density),
            );
        }
        crate::documents::draw_ui(&mut ui);
        ui.finish();

        if !r.render(&game::loading_frame(&b))? && !b.patches.is_empty() {
            // A suspended/lost surface skips the frame, including its glyph
            // uploads. Rebuild them next time instead of caching invisible text.
            horizon_client::ui_theme::reset_atlas();
        }
        Ok(())
    }
    pub(super) fn activate(&mut self, events: &ActiveEventLoop) -> Result<()> {
        if self.session_worker.is_some() && self.chooser.is_none() && !self.exit_confirmation {
            return Ok(());
        }
        let Some(r) = &self.renderer else {
            return Ok(());
        };
        let mut b = HudBatch::new([r.config.width, r.config.height]);
        let density = self.density();
        let ui = Ui::launcher(
            &self.gui.as_ref().unwrap().atlas,
            &mut b,
            (0.0, 0.0),
            density,
        );
        let p = self.focus.and_then(|index| self.targets.get(index)).map_or(
            [
                self.cursor.0 as f32 / ui.density,
                self.cursor.1 as f32 / ui.density,
            ],
            |([x, y, w, h], _)| [x + w / 2.0, y + h / 2.0],
        );
        let action = self
            .targets
            .iter()
            .find(|([x, y, w, h], _)| {
                (*x..*x + *w).contains(&p[0]) && (*y..*y + *h).contains(&p[1])
            })
            .map(|(_, a)| *a);
        // The import overlay takes its own input.
        if crate::documents::busy() {
            return Ok(());
        }
        // A transfer's dialog is modal and stays usable during a download.
        match action {
            Some(Action::CancelTransfer) => {
                crate::transfers::cancel();
                return Ok(());
            }
            Some(Action::DismissTransfer) => {
                self.view.transfer_error = None;
                return Ok(());
            }
            _ if crate::transfers::running().is_some() || self.view.transfer_error.is_some() => {
                return Ok(());
            }
            _ => {}
        }
        const WAIT: &str = "Please wait: the current download is still running";
        if self.catalog.busy() {
            // Say why the tap does nothing instead of ignoring it silently.
            if action.is_some() {
                self.feedback = action.map(|action| (action, std::time::Instant::now()));
                self.error = Some(WAIT.into());
            }
            return Ok(());
        }
        if self.error.as_deref() == Some(WAIT) {
            self.error = None;
        }
        self.feedback = action.map(|action| (action, std::time::Instant::now()));
        self.focus = None;
        if action.is_some() && horizon_client::official_install::ready(&self.home) {
            let audio = self.audio.get_or_insert_with(|| {
                let mut audio = horizon_client::audio::Audio::new(
                    &self.home.join(".local/horizon-client/26.3"),
                );
                let settings = horizon_client::settings::Settings::load(
                    &self
                        .catalog
                        .config
                        .game_home(&self.home)
                        .join("options.txt"),
                );
                audio.master = settings.volumes[0];
                let mut volumes = settings.volumes;
                volumes[0] = 1.0;
                horizon_client::settings::set_source_volumes(volumes);
                audio
            });
            audio.play_button_click();
        }
        match action {
            Some(Action::ConfirmExit) => {
                events.exit();
            }
            Some(Action::Immersive) => {
                self.catalog.config.screen.immersive ^= true;
                self.catalog.config.save(&self.home)?;
                self.configure_screen();
            }
            Some(Action::Cutout) => {
                self.catalog.config.screen.avoid_cutout ^= true;
                self.catalog.config.save(&self.home)?;
                self.configure_screen();
            }
            Some(Action::Aspect) => self.open_chooser(chooser::Kind::Aspect),
            Some(Action::ChooseAspect(aspect)) => {
                self.catalog.config.screen.aspect = aspect;
                self.catalog.config.save(&self.home)?;
                self.configure_screen();
                self.chooser = None;
            }
            Some(Action::CacheBudget) => self.open_chooser(chooser::Kind::CacheBudget),
            Some(Action::ChooseCacheBudget(mib)) => {
                self.catalog.config.cache_budget_mib = mib.min(1024);
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                self.catalog.status = "Disposable cache budget saved. Applies on next Play; not a total RAM or VRAM limit.".into();
            }
            Some(Action::ToggleBundled(id)) => {
                let game = self.catalog.config.game_home(&self.home);
                let mut list = horizon_client::mod_list::ModList::load(&game);
                let enabled = list.enabled.contains(id) && !list.disabled.contains(id);
                if enabled {
                    list.enabled.remove(id);
                    list.disabled.insert(id.into());
                } else {
                    list.disabled.remove(id);
                    list.enabled.insert(id.into());
                }
                std::fs::create_dir_all(&game)?;
                std::fs::write(game.join(horizon_client::mod_list::FILE), list.serialize())?;
                self.catalog.refresh(&self.home);
                self.catalog.status = format!(
                    "Bundled {id} {}. Applies on Play; settings are in Options > Mods.",
                    if enabled { "disabled" } else { "enabled" }
                );
            }
            Some(Action::GameFiles) => {
                self.catalog.status = if crate::platform::open_documents(crate::document_store::ROOT) {
                    "Game files opened. Worlds, mods, packs, shaders and reports are available here."
                } else { "Open system Files > Horizon Android." }.into();
            }
            Some(Action::WorldFiles) => {
                let id = crate::document_store::folder_id(self.catalog.config.profile, "worlds");
                self.catalog.status = if crate::platform::open_documents(&id) {
                    "Copy a complete world folder to back up or import. Finish copying before Play; never replace an existing world silently."
                } else { "Open system Files > Horizon Android > Worlds. Copy complete world folders while the game is closed." }.into();
            }
            Some(Action::WorkerLimit) => self.open_chooser(chooser::Kind::WorkerLimit),
            Some(Action::ChooseWorkerLimit(limit)) => {
                self.catalog.config.worker_limit = limit.min(64);
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                self.catalog.status = "Chunk worker ceiling saved. Applies on the next Play; other engine threads are unchanged.".into();
            }
            Some(Action::DisplayHz) => self.open_chooser(chooser::Kind::DisplayHz),
            Some(Action::ChooseDisplayHz(hz)) => {
                self.catalog.config.device.display_hz = hz;
                self.catalog.config.device.sanitize();
                self.catalog.config.save(&self.home)?;
                crate::device_runtime::configure(self.catalog.config.device);
                self.chooser = None;
            }
            Some(Action::PerformanceHints) => {
                self.catalog.config.device.performance_hints ^= true;
                self.catalog.status="ADPF gives Android CPU frame-work hints on supported devices. It does not cap FPS or guarantee a speedup.".into();
                self.catalog.config.save(&self.home)?;
                crate::device_runtime::configure(self.catalog.config.device);
            }
            Some(Action::GpuProfile) => {
                self.catalog.config.device.gpu_profile ^= true;
                self.catalog.status="Detailed GPU profiling records diagnostics and has overhead. Reports remain on this device.".into();
                self.catalog.config.save(&self.home)?;
                crate::device_runtime::configure(self.catalog.config.device);
            }

            Some(Action::ModSource(origin)) => {
                self.catalog.config.mod_source = origin;
                self.catalog.config.save(&self.home)?;
                self.catalog.offset = 0;
                self.catalog.browse(&self.home);
            }
            Some(Action::ShaderSource(origin)) => {
                self.catalog.config.shader_source = origin;
                self.catalog.config.save(&self.home)?;
                self.catalog.offset = 0;
                self.catalog.browse(&self.home);
            }
            Some(Action::OpenFolder(kind)) => {
                crate::platform::hide();
                let folder = match kind {
                    horizon_client::content_files::Kind::Mods => "mods",
                    horizon_client::content_files::Kind::Packs => "packs",
                    horizon_client::content_files::Kind::Shaders => "shaders",
                };
                let id = crate::document_store::folder_id(self.catalog.config.profile, folder);
                self.catalog.status = if crate::platform::open_documents(&id) {
                    "Opened in Files. Return here after copying files.".into()
                } else {
                    "Open the system Files app and choose Horizon Android.".into()
                };
            }
            Some(Action::Import(kind)) => {
                crate::platform::hide();
                let home = self.catalog.config.game_home(&self.home);
                if let Err(e) = crate::documents::begin(kind, home) {
                    self.catalog.status = e;
                }
            }
            Some(Action::ToggleLocal(i)) => self.catalog.toggle_local(&self.home, i),
            Some(Action::DeleteLocal(i)) => {
                self.catalog.pending_delete = self.catalog.local.get(i).cloned();
                if let Some(item) = &self.catalog.pending_delete {
                    self.chooser = Some(super::chooser::Chooser::confirm_delete(item.file.clone()));
                }
            }
            Some(Action::ConfirmDeleteLocal) => {
                self.catalog.remove_local(&self.home);
                self.chooser = None;
            }
            Some(Action::NewInstance) => {
                self.chooser = None;
                self.refresh_engines();
                self.view.instance_edit = Some(super::view::InstanceEdit {
                    id: None,
                    name: String::new(),
                    engine: None,
                    delete_armed: None,
                });
            }
            Some(Action::EditInstance(id)) => {
                self.refresh_engines();
                let instance = crate::instances::get(&self.home, id);
                self.view.instance_edit = Some(super::view::InstanceEdit {
                    id: Some(id),
                    name: super::view::instance_name(&instance),
                    engine: instance.engine,
                    delete_armed: None,
                });
            }
            Some(Action::InstanceEngine(index)) => {
                let engine = index
                    .and_then(|i| self.view.engines.get(i))
                    .map(|e| e.manifest.id.clone());
                if let Some(edit) = self.view.instance_edit.as_mut() {
                    edit.engine = engine;
                }
            }
            Some(Action::CloseInstanceEdit) => {
                self.view.instance_edit = None;
                self.catalog.editing = None;
                crate::platform::hide();
            }
            Some(Action::SaveInstance) => {
                self.catalog.editing = None;
                crate::platform::hide();
                if let Some(edit) = self.view.instance_edit.take() {
                    let name = crate::instances::clean_name(&edit.name);
                    let saved = match edit.id {
                        None => crate::instances::create(
                            &self.home,
                            &name,
                            edit.engine,
                            Some(self.catalog.config.profile),
                        )
                        .map(|i| i.id),
                        Some(id) => {
                            let mut instance = crate::instances::get(&self.home, id);
                            // The default name stays the default (it follows the language).
                            if name != super::view::instance_name(&instance) {
                                instance.name = name;
                            }
                            instance.engine = edit.engine;
                            crate::instances::save(&self.home, &instance).map(|()| id)
                        }
                    };
                    match saved {
                        Ok(id) => {
                            if edit.id.is_none() {
                                self.select_instance(id)?;
                            }
                            self.refresh_instances();
                        }
                        Err(error) => self.toast(
                            super::text::t("Could not save the instance"),
                            super::text::t_dyn(&error),
                            true,
                        ),
                    }
                }
            }
            Some(Action::DeleteInstance) => {
                let Some(edit) = self.view.instance_edit.as_mut() else {
                    return Ok(());
                };
                let armed = edit
                    .delete_armed
                    .is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(5));
                if !armed {
                    edit.delete_armed = Some(std::time::Instant::now());
                    return Ok(());
                }
                let Some(id) = edit.id else {
                    return Ok(());
                };
                let name = edit.name.clone();
                self.view.instance_edit = None;
                match crate::instances::delete(&self.home, id) {
                    Ok(()) => {
                        if self.catalog.config.profile == id {
                            self.select_instance(0)?;
                        }
                        self.toast(
                            super::text::tf("«{}» deleted", &[&name]),
                            String::new(),
                            false,
                        );
                    }
                    Err(error) => self.toast(
                        super::text::t("Could not delete the instance"),
                        super::text::t_dyn(&error),
                        true,
                    ),
                }
                self.refresh_instances();
            }
            Some(Action::SelectInstance(id)) => self.select_instance(id)?,
            Some(Action::PlayInstance(id)) => {
                self.select_instance(id)?;
                self.launch_target = LaunchTarget::Title;
                self.play(events)?;
            }
            Some(Action::LauncherLanguage(choice)) => {
                self.catalog.config.launcher_language = choice;
                self.catalog.config.save(&self.home)?;
                super::text::choose(choice);
                // Toasts and statuses already shown stay in the old language.
                self.view.toast = None;
            }
            Some(Action::ImportEngine) => self.view.engine_confirm = true,
            Some(Action::CancelEngineImport) => self.view.engine_confirm = false,
            Some(Action::ConfirmEngineImport) => {
                self.view.engine_confirm = false;
                if let Err(error) = crate::transfers::import_engine(self.home.clone()) {
                    self.toast(super::text::t("Could not install the engine"), error, true);
                }
            }
            Some(Action::LaunchEngine(index)) => {
                if let Some(engine) = self.view.engines.get(index).cloned() {
                    let target = serde_json::json!({
                        "open": "title",
                        "instance": self.catalog.config.profile,
                    });
                    self.restart_on(Some(&engine.manifest.id), Some(target));
                }
            }
            Some(Action::BuiltInEngine) => self.restart_on(None, None),
            Some(Action::CheckEngines) => self.view.engine_store.check(),
            Some(Action::OpenAppUpdate) => {
                if let Some(channel) = crate::engines::channel(crate::backend::VERSION) {
                    crate::platform::open_url(crate::engines::release_page(channel));
                }
            }
            Some(Action::UpdateCheck) => {
                self.catalog.config.skip_update_check ^= true;
                self.catalog.config.save(&self.home)?;
                if !self.catalog.config.skip_update_check {
                    self.view.engine_store.check();
                }
            }
            Some(Action::DownloadEngine(index)) => {
                if let Some(offer) = self.view.engine_store.offers.get(index).cloned() {
                    self.view.engine_store.download(self.home.clone(), offer);
                }
            }
            Some(Action::DeleteEngine(index)) => {
                let confirmed = self.view.engine_delete.is_some_and(|(at, when)| {
                    at == index && when.elapsed() < std::time::Duration::from_secs(5)
                });
                if !confirmed {
                    self.view.engine_delete = Some((index, std::time::Instant::now()));
                } else if let Some(engine) = self.view.engines.get(index).cloned() {
                    self.view.engine_delete = None;
                    if crate::engines::running().as_deref() == Some(engine.manifest.id.as_str()) {
                        self.toast(
                            super::text::t("This engine is running"),
                            super::text::t("Switch to the built-in engine first, then delete it."),
                            true,
                        );
                    } else {
                        let removed = crate::engines::remove(&self.home, &engine.manifest.id)
                            .and_then(|()| {
                                crate::instances::forget_engine(&self.home, &engine.manifest.id)
                            });
                        match removed {
                            Ok(()) => self.toast(
                                super::text::tf("«{}» deleted", &[&engine.manifest.name]),
                                String::new(),
                                false,
                            ),
                            Err(error) => self.toast(
                                super::text::t("Could not delete the engine"),
                                error,
                                true,
                            ),
                        }
                        self.refresh_engines();
                    }
                }
            }
            Some(Action::Repository) => {
                self.catalog.status = if crate::platform::open_repository() {
                    "Repository opened in browser."
                } else {
                    "No browser available to open the repository."
                }
                .into();
            }
            Some(Action::Account) => {
                self.catalog.editing = None;
                crate::platform::hide();
                self.login.providers = true;
                self.login.offline = false;
                self.login.ely = false;
                self.login.choosing = false;
                self.login.visible = true;
            }
            Some(Action::OfflineAccount | Action::MicrosoftAccount) => {
                self.login.providers = false;
                self.login.offline = matches!(action, Some(Action::OfflineAccount));
                self.login.ely = false;
                self.login.choosing = false;
                self.login.visible = true;
            }
            Some(Action::SavedAccounts) => {
                self.login.providers = false;
                self.login.refresh_status();
                self.login.choosing = true;
                self.login.ely = false;
            }
            Some(Action::SavedPage(next)) => {
                if next && self.login.account_offset + 5 < self.login.saved_accounts.len() {
                    self.login.account_offset += 5;
                } else if !next {
                    self.login.account_offset = self.login.account_offset.saturating_sub(5);
                }
            }
            Some(Action::SelectAccount(index)) => {
                let mut accounts = crate::account::Accounts::load();
                if let Some(account) = self.login.saved_accounts.get(index)
                    && accounts.accounts.iter().any(|a| a.key() == account.key())
                {
                    accounts.selected = Some(account.key());
                    accounts.save()?;
                }
                self.session_ready = false;
                self.prepared_session = None;
                self.session_worker = None;
                self.login.refresh_status();
                if self.catalog.page == Page::Skins {
                    self.wardrobe.open(
                        &self.catalog.config.name,
                        &self.catalog.config.microsoft_client_id,
                    );
                }
            }
            Some(Action::SelectOffline) => {
                self.catalog.config.save(&self.home)?;
                self.catalog.editing = None;
                crate::platform::hide();
                self.login.hide();
                let mut accounts = crate::account::Accounts::load();
                accounts.selected = None;
                accounts.save()?;
                self.session_ready = false;
                self.prepared_session = None;
                self.session_worker = None;
                self.login.choosing = false;
                self.login.refresh_status();
                if self.catalog.page == Page::Skins {
                    self.wardrobe.open(
                        &self.catalog.config.name,
                        &self.catalog.config.microsoft_client_id,
                    );
                }
            }
            Some(Action::ElySignIn) => {
                self.login.providers = false;
                self.login.close();
                self.login.ely = true;
                self.login.visible = true;
            }
            Some(Action::SubmitEly) => {
                self.catalog.editing = None;
                crate::platform::hide();
                self.session_ready = false;
                self.prepared_session = None;
                self.session_worker = None;
                if !self.login.pending() {
                    self.login.start_ely();
                }
            }
            Some(Action::CloseLogin) => {
                self.login.hide();
                crate::platform::hide();
                self.catalog.editing = None;
                self.refresh_wardrobe_account();
            }
            Some(Action::DeviceSignIn) => {
                self.session_ready = false;
                self.prepared_session = None;
                self.session_worker = None;
                self.login
                    .start_device(&self.catalog.config.microsoft_client_id);
            }
            Some(Action::CopyLoginCode) => self.login.copy_code(),
            Some(Action::CancelLogin) => {
                self.login.close();
                self.login.visible = true;
            }
            Some(Action::ReviewApp) => self.login.review_app(),
            Some(Action::SignIn) => {
                self.session_ready = false;
                self.prepared_session = None;
                self.session_worker = None;
                self.login.start(&self.catalog.config.microsoft_client_id);
            }
            Some(Action::SignOut) => {
                self.session_ready = false;
                self.prepared_session = None;
                self.session_worker = None;
                self.login.sign_out();
                self.refresh_wardrobe_account();
            }
            Some(Action::LoginLink) => self.login.open_link(),
            Some(Action::Page(page)) => {
                if page == Page::Worlds {
                    self.refresh_worlds();
                }
                // The Add-ons rail item returns to the kind last shown; the kind
                // segments on the page itself switch directly.
                let addons = |p: Page| matches!(p, Page::Mods | Page::Packs | Page::Shaders);
                let page = if page == Page::Mods && !addons(self.catalog.page) {
                    self.view.last_addon
                } else {
                    page
                };
                if addons(page) {
                    self.view.last_addon = page;
                }
                if page == Page::Skins {
                    self.wardrobe.open(
                        &self.catalog.config.name,
                        &self.catalog.config.microsoft_client_id,
                    );
                }
                self.catalog.editing = None;
                if crate::platform::active() {
                    crate::platform::hide();
                }
                self.catalog.page = page;
                self.catalog.scroll = 0.0;
                self.error = None;
                // A previous tab's search result must not linger under this one.
                self.catalog.status.clear();
                self.catalog.offset = 0;
                self.catalog.browse(&self.home);
            }
            Some(Action::Play) => {
                self.launch_target = LaunchTarget::Title;
                self.play(events)?;
            }
            Some(Action::WardrobeKind(cape)) => {
                self.wardrobe.capes = cape;
                self.catalog.scroll = 0.0;
            }
            Some(Action::WardrobeSelect(index)) => self.wardrobe.select(index),
            Some(Action::WardrobeImport) => self.wardrobe.import(),
            Some(Action::WardrobeCloud) => self
                .wardrobe
                .cloud(&self.catalog.config.microsoft_client_id, None),
            Some(Action::WardrobeUpload) => self.wardrobe.cloud(
                &self.catalog.config.microsoft_client_id,
                Some(wardrobe::CloudAction::Upload),
            ),
            Some(Action::WardrobeActivateCape(index)) => self.wardrobe.cloud(
                &self.catalog.config.microsoft_client_id,
                Some(wardrobe::CloudAction::Cape(index)),
            ),
            Some(Action::WardrobeUseCloud) => self.wardrobe.use_cloud(),
            Some(Action::WardrobeModel) => self.wardrobe.model(),
            Some(Action::WardrobeRotate(direction)) => {
                // As a swipe: ‹ turns the model left.
                self.wardrobe.camera.rotate(-(direction as f32) * 25.0, 0.0);
                self.wardrobe.preview();
            }
            Some(Action::WardrobeZoom(inward)) => {
                self.wardrobe
                    .camera
                    .zoom(if inward { 1.15 } else { 1.0 / 1.15 });
                self.wardrobe.preview();
            }
            Some(Action::WardrobeHideCape) => self.wardrobe.hide_cape(),
            Some(Action::JoinServer) => {
                crate::platform::hide();
                self.catalog.editing = None;
                if !self.catalog.config.server.trim().is_empty() {
                    self.launch_target = LaunchTarget::Server;
                    self.toast(
                        super::text::tf("Connecting to {}…", &[self.catalog.config.server.trim()]),
                        String::new(),
                        false,
                    );
                    self.play(events)?;
                }
            }
            Some(Action::ContinueWorld) => {
                if let Some(world) = self.view.worlds.first() {
                    self.launch_target = LaunchTarget::World(world.folder.clone());
                    self.play(events)?;
                }
            }
            Some(Action::OpenWorld(index)) => {
                if let Some(world) = self.view.worlds.get(index) {
                    self.launch_target = LaunchTarget::World(world.folder.clone());
                    self.play(events)?;
                }
            }
            Some(Action::AddonView(installed)) => {
                self.view.addons_installed = installed;
                self.catalog.scroll = 0.0;
                if installed {
                    self.catalog.refresh(&self.home);
                } else {
                    self.catalog.browse(&self.home);
                }
            }
            Some(Action::ClearSearch) => {
                self.catalog.query.clear();
                self.catalog.offset = 0;
                self.catalog.search(&self.home);
            }
            Some(Action::ClearDownloadCache) => {
                use super::text::t;
                if self.worker.is_some() {
                    self.toast(t("Wait for the download to finish"), String::new(), true);
                } else {
                    match horizon_client::official_install::cleanup_download_cache(&self.home) {
                        Ok(()) => self.toast(t("Download cache cleared"), String::new(), false),
                        Err(error) => {
                            self.toast(t("Could not clear the cache"), error.to_string(), true)
                        }
                    }
                }
            }
            Some(Action::CancelInstall) => {
                horizon_client::official_install::cancel();
                self.toast(
                    super::text::t("Download cancelled"),
                    super::text::t("Downloaded files kept"),
                    false,
                );
            }
            Some(Action::HomeLastWorld) => {
                self.catalog.config.home_last_world ^= true;
                self.catalog.config.save(&self.home)?;
            }
            Some(Action::HomeProfile) => {
                self.catalog.config.home_hide_profile ^= true;
                self.catalog.config.save(&self.home)?;
            }
            Some(Action::SettingsSection(index)) => {
                if let Some(top) = self.view.settings_tops.get(usize::from(index)) {
                    self.catalog.scroll = *top;
                }
            }
            Some(Action::FinishNameEdit) => {
                if self.catalog.editing == Some("name") {
                    self.refresh_wardrobe_account();
                }
                self.catalog.editing = None;
                crate::platform::hide();
                self.catalog.config.save(&self.home)?;
            }
            Some(Action::Field("instance_name")) => {
                if let Some(edit) = &self.view.instance_edit {
                    self.catalog.editing = Some("instance_name");
                    crate::platform::show(&edit.name);
                }
            }
            Some(Action::Field(name)) => {
                if let Some(value) = self.login.field(name) {
                    self.catalog.editing = Some(name);
                    if name == "ely_password" || name == "ely_otp" {
                        crate::platform::show_secret(value);
                    } else {
                        crate::platform::show(value);
                    }
                    return Ok(());
                }
                if name == "render_scale" {
                    self.catalog.render_scale_text =
                        self.catalog.config.render_scale.percent().to_string();
                    self.chooser = None;
                }
                self.catalog.editing = Some(name);
                let initial = match name {
                    "name" => &self.catalog.config.name,
                    "server" => &self.catalog.config.server,
                    "render_scale" => &self.catalog.render_scale_text,
                    "client_id" => &self.catalog.config.microsoft_client_id,
                    _ => &self.catalog.query,
                };
                crate::platform::show(initial);
            }
            Some(Action::Install(i)) => self.catalog.install(&self.home, i),
            Some(Action::Next) => {
                self.catalog.offset += 12;
                self.catalog.search(&self.home);
            }
            Some(Action::Previous) => {
                self.catalog.offset = self.catalog.offset.saturating_sub(12);
                self.catalog.search(&self.home);
            }
            Some(Action::Profile) => self.open_chooser(chooser::Kind::Profile),
            Some(Action::Renderer) => self.open_chooser(chooser::Kind::Renderer),
            Some(Action::RenderScale) => self.open_chooser(chooser::Kind::RenderScale),
            Some(Action::ChooseRenderScale(scale)) => {
                self.catalog.config.render_scale = scale;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                crate::display::set_scale(scale);
            }
            Some(Action::Orientation) => self.open_chooser(chooser::Kind::Orientation),
            Some(Action::ChooseInputHost(mode)) => {
                self.catalog.config.input_host = mode;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                crate::configure_input_mode(mode);
            }
            Some(Action::ChooseOrientation(mode)) => {
                self.catalog.config.orientation = mode;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                // The launcher turns now; the game starts the same way.
                crate::platform::set_orientation(mode);
            }
            Some(Action::ChooseRenderer(mode)) => {
                self.catalog.config.renderer = mode;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
            }
            Some(Action::ChooseProfile(profile)) => {
                self.select_instance(profile)?;
                self.chooser = None;
            }
            Some(Action::Dismiss) => {
                self.exit_confirmation = false;
                self.chooser = None;
                if self.session_worker.is_none() {
                    self.session_ready = false;
                    self.prepared_session = None;
                    self.session_worker = None;
                }
            }
            Some(Action::ExportWorld(index)) => {
                if let Some(world) = self.view.worlds.get(index) {
                    let dir = self
                        .catalog
                        .config
                        .game_home(&self.home)
                        .join("saves")
                        .join(&world.folder);
                    if let Err(error) = crate::transfers::export_world(dir, &world.name) {
                        self.toast(super::text::t("Could not export the world"), error, true);
                    }
                }
            }
            Some(Action::ImportWorld) => {
                let saves = self.catalog.config.game_home(&self.home).join("saves");
                if let Err(error) = crate::transfers::import_world(saves) {
                    self.toast(super::text::t("Could not import the world"), error, true);
                }
            }
            Some(Action::CancelTransfer | Action::DismissTransfer) => {}
            Some(Action::ReportMenu(report)) => {
                self.open_chooser(chooser::Kind::Report(report));
            }
            Some(Action::SendReport(report, delivery)) => {
                self.chooser = None;
                self.prepare_report(report, delivery);
            }
            Some(Action::RecordPerformance) => {
                self.catalog.config.record_performance = !self.catalog.config.record_performance;
                if !self.catalog.config.record_performance {
                    crate::performance::stop();
                }
                self.catalog.config.save(&self.home)?;
                self.catalog.status = if self.catalog.config.record_performance {
                    "Recording starts on Play. Detailed timings add overhead. Return here to save the report."
                } else { "Performance recording off." }.into();
            }
            None => {}
        }
        Ok(())
    }
    /// Writes the report's file, then saves, shares or shows it.
    fn prepare_report(
        &mut self,
        report: crate::transfers::Report,
        delivery: crate::transfers::Delivery,
    ) {
        use super::text::{t, t_dyn};
        use crate::transfers::{Delivery, Report};
        match report {
            Report::Diagnostics if self.diagnostic_export.is_some() => {}
            Report::Diagnostics => {
                let home = self.home.clone();
                let (sender, receiver) = mpsc::channel();
                self.diagnostic_export = Some((receiver, delivery));
                self.toast(
                    t("Preparing the report"),
                    t("Nothing is uploaded. Review the ZIP before you send it."),
                    false,
                );
                std::thread::spawn(move || {
                    let _ = sender.send(crate::diagnostics::export(&home));
                });
            }
            Report::Performance if delivery == Delivery::Files => {
                self.catalog.status = match crate::performance::export(&self.home) {
                    Ok(()) => "Reports opened in Files. Copy or share a report from there.".into(),
                    Err(e) => e,
                };
            }
            Report::Performance => match crate::performance::latest(&self.home) {
                Ok(path) => self.send_report(path, report, delivery),
                Err(error) => self.toast(t("No recording yet"), t_dyn(&error), true),
            },
            Report::LaunchLog => match crate::diagnostics::launch_log(&self.home) {
                Ok(path) => self.send_report(path, report, delivery),
                Err(error) => self.toast(t("Could not write the launch log"), error, true),
            },
        }
    }
    /// Hands a written report file to the picker, the share sheet or Files.
    pub(super) fn send_report(
        &mut self,
        path: std::path::PathBuf,
        report: crate::transfers::Report,
        delivery: crate::transfers::Delivery,
    ) {
        use super::text::{t, tf};
        use crate::transfers::Delivery;
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        match delivery {
            Delivery::Save => {
                if let Err(error) = crate::transfers::save_report(path, report) {
                    self.toast(t("Could not save the file"), error, true);
                }
            }
            Delivery::Share => {
                if let Err(error) = crate::transfers::share_report(&path, report) {
                    self.toast(t("Could not share the file"), error, true);
                }
            }
            Delivery::Files => {
                if crate::platform::open_documents("reports") {
                    self.toast(
                        t("Opened in Files"),
                        tf("«{}» is in Horizon Android › Reports.", &[&name]),
                        false,
                    );
                } else {
                    self.toast(
                        t("Open the Files app"),
                        tf("«{}» is in Horizon Android › Reports.", &[&name]),
                        false,
                    );
                }
            }
        }
    }
    pub(super) fn ui_density(&self) -> f32 {
        let (Some(r), Some(gui)) = (&self.renderer, &self.gui) else {
            return 1.0;
        };
        let mut batch = HudBatch::new([r.config.width, r.config.height]);
        Ui::launcher(&gui.atlas, &mut batch, (0.0, 0.0), self.density()).density
    }

    pub(super) fn touch_event(&mut self, events: &ActiveEventLoop, t: Touch) -> Result<()> {
        if self.catalog.page == Page::Skins
            && !self.login.visible
            && self.wardrobe.gesture(&t, self.ui_density())
        {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return Ok(());
        }
        self.focus = None;
        self.pending_focus = None;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        self.cursor = (t.location.x, t.location.y);
        match t.phase {
            TouchPhase::Started => {
                // A touch catches a sliding list.
                self.fling = None;
                self.drag = Some(super::gesture::Drag::new(t.location.x, t.location.y));
            }
            TouchPhase::Moved => {
                let density = self.ui_density() as f64;
                if let Some(drag) = &mut self.drag {
                    let delta = drag.update(t.location.x, t.location.y, density);
                    if drag.moved {
                        if let Some(chooser) = &mut self.chooser {
                            chooser.scroll = (chooser.scroll + delta).max(0.0);
                        } else if self
                            .list_view
                            .is_some_and(|view| drag.origin_y() / density >= view[0] as f64)
                        {
                            self.catalog.scroll = (self.catalog.scroll + delta).max(0.0);
                        }
                    }
                }
            }
            TouchPhase::Ended => {
                if let Some(drag) = self.drag.take() {
                    if !drag.moved {
                        self.activate(events)?;
                    } else if let Some(velocity) = drag.release_velocity() {
                        let chooser = self.chooser.is_some();
                        let list = self.list_view.is_some_and(|view| {
                            drag.origin_y() / f64::from(self.ui_density()) >= f64::from(view[0])
                        });
                        if chooser || list {
                            self.fling = Some(super::gesture::Fling::new(velocity, chooser));
                        }
                    }
                }
            }
            TouchPhase::Cancelled => {
                self.drag = None;
            }
        }
        Ok(())
    }
    pub(super) fn type_key(&mut self, key: KeyEvent) {
        if key.physical_key == PhysicalKey::Code(KeyCode::Escape)
            || key.logical_key
                == winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrowserBack)
        {
            if !key.repeat {
                self.request_back();
            }
            return;
        }
        if self.catalog.editing.is_none() {
            return;
        }
        let out = match key.physical_key {
            PhysicalKey::Code(KeyCode::Backspace) => Some(Output::Backspace),
            PhysicalKey::Code(KeyCode::Enter) => {
                crate::platform::hide();
                Some(Output::Enter)
            }
            _ => key.text.map(|s| Output::Text(s.into())),
        };
        if let Some(out) = out {
            self.edit_text(out);
        }
    }
}
