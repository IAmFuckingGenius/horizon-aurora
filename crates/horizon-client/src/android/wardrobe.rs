//! Thin native view over the shared account and wardrobe APIs.
use super::{
    kit::{self, Icon, Kit, Style, col},
    login::Login,
    pages::Action,
    text::{t, t_dyn, tf},
};
use crate::account::{Accounts, Provider};
use crate::android_host::skins::{
    self, Model, cloud,
    library::{Kind, Library},
    preview,
};
use horizon_client::{hud::rgba, ui_theme::Face};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

pub(super) enum CloudAction {
    Upload,
    Cape(usize),
}
pub(super) struct Wardrobe {
    pub root: PathBuf,
    default_skin: PathBuf,
    library: Library,
    key: String,
    provider: Option<Provider>,
    pub capes: bool,
    pub camera: preview::Camera,
    cache: cloud::Cache,
    worker: Option<mpsc::Receiver<(String, Result<(cloud::Cache, bool), String>)>>,
    bitmap: Vec<([f32; 4], u32)>,
    rect: [f32; 4],
    fingers: std::collections::BTreeMap<u64, [f32; 2]>,
    note: String,
    damaged: bool,
}
impl Wardrobe {
    pub fn new(home: &Path) -> Self {
        let root = home.join("wardrobe");
        let loaded = Library::load(&root);
        let damaged = loaded.is_err();
        let note = loaded.as_ref().err().cloned().unwrap_or_default();
        let mut value = Self {
            root,
            default_skin: home
                .join(".local/horizon-client")
                .join(horizon_core::VANILLA_VERSION)
                .join("assets/minecraft/textures/entity/player/wide/steve.png"),
            library: loaded.unwrap_or_default(),
            key: String::new(),
            provider: None,
            capes: false,
            camera: Default::default(),
            cache: Default::default(),
            worker: None,
            bitmap: vec![],
            rect: [0.0; 4],
            fingers: Default::default(),
            note,
            damaged,
        };
        value.preview();
        value
    }
    pub fn open(&mut self, offline: &str, client_id: &str) {
        let account = Accounts::load().selected().cloned();
        let key = account
            .as_ref()
            .map(|a| a.key())
            .unwrap_or_else(|| format!("offline:{}", offline.to_ascii_lowercase()));
        if self.key != key {
            // A previous provider's in-flight result must not delay this profile's cloud load.
            self.worker = None;
            self.fingers.clear();
            self.key = key;
            self.provider = account.map(|a| a.provider);
            self.cache = Default::default();
        }
        self.reload();
        if self.provider.is_some() && self.worker.is_none() {
            self.cloud(client_id, None);
        }
        self.preview();
    }
    pub fn reload(&mut self) {
        match Library::load(&self.root) {
            Ok(value) => {
                self.library = value;
                self.damaged = false;
                self.preview();
            }
            Err(error) => {
                self.damaged = true;
                self.note = error;
            }
        }
    }
    pub fn poll(&mut self) {
        if let Some((key, result)) = self.worker.as_ref().and_then(|r| r.try_recv().ok()) {
            self.worker = None;
            if key != self.key {
                return;
            }
            match result {
                Ok((cache, activated_cape)) => {
                    let selection = self.library.selections.entry(self.key.clone()).or_default();
                    if selection.skin.is_none() {
                        selection.model = if cache.slim {
                            Model::Slim
                        } else {
                            Model::Classic
                        };
                    }
                    if activated_cape {
                        selection.cape = None;
                        selection.hide_cape = false;
                        if let Err(error) = self.library.save(&self.root) {
                            self.note = error;
                            self.preview();
                            return;
                        }
                    }
                    self.cache = cache;
                    self.note =
                        "Cloud profile synchronized. Local overrides are kept separate.".into();
                    self.preview();
                }
                Err(error) => self.note = error,
            }
        }
    }
    fn kind(&self) -> Kind {
        if self.capes { Kind::Cape } else { Kind::Skin }
    }
    fn save(&mut self) {
        if let Err(error) = self.library.save(&self.root) {
            self.note = error;
        }
        self.preview();
    }
    pub fn select(&mut self, index: usize) {
        if self.damaged || crate::documents::busy() {
            return;
        }
        if let Some(entry) = self.library.entries.get(index).cloned() {
            self.note = match self
                .library
                .select(&self.root, &self.key, &entry.id, entry.kind)
            {
                Ok(()) => "Applied locally. The account's cloud profile is unchanged.".into(),
                Err(error) => error,
            };
            self.preview();
        }
    }
    pub fn import(&mut self) {
        if self.damaged {
            return;
        }
        self.note = match crate::documents::begin_appearance(self.root.clone(), self.kind()) {
            Ok(()) => "Choose a PNG image in Android Files.".into(),
            Err(error) => error,
        };
    }
    pub fn use_cloud(&mut self) {
        if self.damaged || crate::documents::busy() {
            return;
        }
        let selection = self.library.selections.entry(self.key.clone()).or_default();
        if self.capes {
            selection.cape = None;
            selection.hide_cape = false;
        } else {
            selection.skin = None;
        }
        self.note = if self.provider.is_some() {
            "Using the account's cloud appearance. No cloud changes were made."
        } else {
            "Local override cleared. Using the default appearance."
        }
        .into();
        self.save();
    }
    pub fn model(&mut self) {
        if self.damaged || crate::documents::busy() {
            return;
        }
        let selection = self.library.selections.entry(self.key.clone()).or_default();
        selection.model = if selection.model == Model::Classic {
            Model::Slim
        } else {
            Model::Classic
        };
        self.save();
    }
    pub fn hide_cape(&mut self) {
        if self.damaged || crate::documents::busy() {
            return;
        }
        let selection = self.library.selections.entry(self.key.clone()).or_default();
        selection.hide_cape = !selection.hide_cape;
        self.save();
    }
    pub fn cloud(&mut self, client_id: &str, action: Option<CloudAction>) {
        if self.worker.is_some() {
            return;
        }
        let Some(provider) = self.provider else {
            self.note = "Offline appearance is local to this installation.".into();
            return;
        };
        if provider == Provider::Ely && action.is_some() {
            self.note =
                "Ely.by cloud skins are managed at ely.by. Custom capes can be applied locally."
                    .into();
            if let Err(error) = crate::platform::open_login_url("https://ely.by/skins") {
                self.note = error;
            }
            return;
        }
        let key = self.key.clone();
        let root = self.root.clone();
        let id = client_id.to_owned();
        let selection = self
            .library
            .selections
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let cape = match &action {
            Some(CloudAction::Cape(index)) => {
                self.cache.snapshot.capes.get(*index).map(|c| c.id.clone())
            }
            _ => None,
        };
        if matches!(action, Some(CloudAction::Upload)) && selection.skin.is_none() {
            self.note = "Select a local skin before uploading to your Microsoft profile.".into();
            return;
        }
        if matches!(action, Some(CloudAction::Cape(_))) && cape.is_none() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        self.worker = Some(receiver);
        self.note = "Contacting account service...".into();
        std::thread::spawn(move || {
            let result = (|| {
                let (account, secrets) = Login::session(&id)?
                    .ok_or("Sign in before synchronizing your cloud appearance.")?;
                if account.key() != key {
                    return Err("The selected account changed. Open the gallery again.".into());
                }
                match action {
                    Some(CloudAction::Upload) => {
                        let path = Library::image(
                            &root,
                            selection.skin.as_deref().ok_or("Select a skin first.")?,
                        )?;
                        cloud::upload_skin(
                            &account,
                            &secrets,
                            &skins::read_file(&path)?,
                            selection.model,
                        )?;
                    }
                    Some(CloudAction::Cape(_)) => {
                        cloud::activate_cape(&account, &secrets, cape.as_deref())?
                    }
                    None => {}
                }
                cloud::fetch(&account, &secrets, &root)
                    .map(|cache| (cache, matches!(action, Some(CloudAction::Cape(_)))))
            })();
            let _ = sender.send((key, result));
        });
    }
    pub fn prepare_game(&mut self, offline: &str, game: &Path) -> Result<(), String> {
        let key = Accounts::load()
            .selected()
            .map(|a| a.key())
            .unwrap_or_else(|| format!("offline:{}", offline.to_ascii_lowercase()));
        let cloud = if key == self.key {
            self.cache
                .skin
                .as_deref()
                .map(|p| (p, self.cache.slim, self.cache.cape.as_deref()))
        } else {
            None
        };
        Library::load(&self.root)?.prepare_game(&self.root, &key, game, cloud)
    }
    pub fn preview(&mut self) {
        let selection = self
            .library
            .selections
            .get(&self.key)
            .cloned()
            .unwrap_or_default();
        let path = |id: &Option<String>| {
            id.as_deref()
                .and_then(|id| Library::image(&self.root, id).ok())
        };
        let skin = path(&selection.skin)
            .or_else(|| self.cache.skin.clone())
            .and_then(|p| skins::read_file(&p).ok())
            .and_then(|b| skins::read_skin(&b).ok())
            .or_else(|| {
                let bytes = skins::read_file(&self.default_skin).ok()?;
                skins::read_skin(&bytes).ok()
            })
            .unwrap_or_else(preview::mannequin);
        let cape = if selection.hide_cape {
            None
        } else {
            path(&selection.cape)
                .or_else(|| self.cache.cape.clone())
                .and_then(|p| skins::read_file(&p).ok())
                .and_then(|b| skins::read_cape(&b).ok())
        };
        let model = selection.model;
        let image = preview::render(&skin, cape.as_ref(), model, self.camera, [160, 200]);
        self.bitmap.clear();
        for y in 0..image.height() {
            let mut x = 0;
            while x < image.width() {
                let color = image.get_pixel(x, y).0;
                let start = x;
                x += 1;
                while x < image.width() && image.get_pixel(x, y).0 == color {
                    x += 1;
                }
                if color[3] > 0 {
                    self.bitmap.push((
                        [start as f32, y as f32, (x - start) as f32, 1.0],
                        rgba(color[0], color[1], color[2], color[3]),
                    ));
                }
            }
        }
    }
    pub fn gesture(&mut self, touch: &winit::event::Touch, density: f32) -> bool {
        use winit::event::TouchPhase;
        let point = [
            touch.location.x as f32 / density,
            touch.location.y as f32 / density,
        ];
        if touch.phase == TouchPhase::Started {
            let [x, y, w, h] = self.rect;
            if point[0] >= x && point[0] < x + w && point[1] >= y && point[1] < y + h {
                self.fingers.insert(touch.id, point);
                return true;
            }
            return false;
        }
        let Some(old) = self.fingers.get(&touch.id).copied() else {
            return false;
        };
        if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            self.fingers.remove(&touch.id);
            return true;
        }
        if let Some(other) = self
            .fingers
            .iter()
            .find_map(|(id, p)| (*id != touch.id).then_some(*p))
        {
            let distance =
                |p: [f32; 2]| ((p[0] - other[0]).powi(2) + (p[1] - other[1]).powi(2)).sqrt();
            let before = distance(old);
            if before > 1.0 {
                self.camera.zoom(distance(point) / before);
            }
        } else {
            // The model follows the finger: a swipe left turns it left.
            self.camera
                .rotate((old[0] - point[0]) * 0.7, (point[1] - old[1]) * 0.5);
        }
        self.fingers.insert(touch.id, point);
        self.preview();
        true
    }
    /// The page's rows under the Skins/Capes switch, in order.
    fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        if self.capes && self.provider.is_some() {
            rows.extend((0..self.cache.snapshot.capes.len()).map(Row::Cloud));
        }
        rows.extend(
            self.library
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.kind == self.kind())
                .map(|(i, _)| Row::Local(i)),
        );
        if self.provider.is_some() {
            rows.push(Row::Button(
                "Use the account's own appearance",
                "user",
                Action::WardrobeUseCloud,
            ));
            rows.push(Row::Button(
                "Refresh from the account",
                "reload",
                Action::WardrobeCloud,
            ));
            if !self.capes {
                rows.push(Row::Button(
                    if self.provider == Some(Provider::Microsoft) {
                        "Upload the chosen skin to Microsoft"
                    } else {
                        "Change the Ely.by skin on the website"
                    },
                    "external-link",
                    Action::WardrobeUpload,
                ));
            }
        } else {
            rows.push(Row::Button(
                "Back to the default look",
                "reload",
                Action::WardrobeUseCloud,
            ));
        }
        if self.capes {
            rows.push(Row::Button(
                "Hide or show the cape",
                "close",
                Action::WardrobeHideCape,
            ));
        }
        rows
    }
    /// Who the appearance belongs to, for the page's subtitle: the account, or
    /// `offline` (the offline name as the player typed it).
    pub fn owner(&self, offline: &str) -> String {
        let account = Accounts::load().selected().map(|a| a.name.clone());
        match (self.provider, account) {
            (Some(Provider::Microsoft), Some(name)) => format!("{name} · Microsoft"),
            (Some(Provider::Ely), Some(name)) => format!("{name} · Ely.by"),
            _ => tf("{} · offline", &[offline.trim()]),
        }
    }
    /// Whether the gallery shows nothing of the current kind.
    pub fn empty(&self) -> bool {
        let local = self.library.entries.iter().any(|e| e.kind == self.kind());
        let cloud = self.capes && self.provider.is_some() && !self.cache.snapshot.capes.is_empty();
        !local && !cloud
    }
    /// The 3D preview in `r` with its turn, zoom and model buttons under it.
    pub fn draw_preview(&mut self, k: &mut Kit, r: [f32; 4]) {
        let [px, py, pw, ph] = r;
        let ph = ph - 112.0;
        self.rect = [px, py, pw, ph];
        kit::plate(k.ui, [px, py, pw, ph], 1, col::SIDE, col::PLATE_RING);
        let scale = ((pw - 8.0) / 160.0).min((ph - 8.0) / 200.0).max(0.1);
        let ox = px + (pw - 160.0 * scale) * 0.5;
        let oy = py + (ph - 200.0 * scale) * 0.5;
        for ([x, y, w, h], color) in &self.bitmap {
            kit::fill(
                k.ui,
                ox + x * scale,
                oy + y * scale,
                w * scale,
                h * scale,
                *color,
            );
        }
        let cy = py + ph + 8.0;
        let bw = (pw - 24.0) / 4.0;
        for (i, (icon, label, action)) in [
            (
                Some(Icon::Sprite(kit::CHEVRON_LEFT)),
                "",
                Action::WardrobeRotate(-1),
            ),
            (
                Some(Icon::Pixel("chevron-right")),
                "",
                Action::WardrobeRotate(1),
            ),
            (None, "−", Action::WardrobeZoom(false)),
            (None, "+", Action::WardrobeZoom(true)),
        ]
        .into_iter()
        .enumerate()
        {
            k.button(
                [px + i as f32 * (bw + 8.0), cy, bw, 48.0],
                Style::Secondary,
                icon,
                label,
                Some(action),
            );
        }
        let model = self
            .library
            .selections
            .get(&self.key)
            .map(|s| s.model)
            .unwrap_or_default();
        k.button(
            [px, cy + 56.0, pw, 48.0],
            Style::Secondary,
            Some(Icon::Pixel("user")),
            if model == Model::Slim {
                t("Model: Alex (slim arms)")
            } else {
                t("Model: Steve (classic)")
            },
            Some(Action::WardrobeModel),
        );
    }
    /// The Skins/Capes switch, import, and the list in `r`; the list scrolls by
    /// `scroll`. Returns the list's rectangle and its content height.
    pub fn draw_list(
        &mut self,
        k: &mut Kit,
        r: [f32; 4],
        scroll: f32,
        list_targets: &mut Vec<([f32; 4], Action)>,
    ) -> ([f32; 4], f32) {
        let [x, y, w, h] = r;
        let import = t("Import PNG");
        let import_w = k.button_width(Style::Secondary, Some(Icon::Pixel("plus")), import);
        let seg_w = (w - import_w - 8.0).max(160.0);
        k.segmented(
            [x, y, seg_w, 48.0],
            &[
                (
                    Some("user"),
                    t("Skins"),
                    Action::WardrobeKind(false),
                    !self.capes,
                ),
                (
                    Some("image"),
                    t("Capes"),
                    Action::WardrobeKind(true),
                    self.capes,
                ),
            ],
            true,
        );
        let import_r = if seg_w + 8.0 + import_w <= w {
            [x + seg_w + 8.0, y, import_w, 48.0]
        } else {
            [x, y + 56.0, w, 48.0]
        };
        k.button(
            import_r,
            Style::Secondary,
            Some(Icon::Pixel("plus")),
            import,
            (!self.damaged).then_some(Action::WardrobeImport),
        );
        let mut top = import_r[1] + 56.0;
        let note = t_dyn(&self.note);
        let note_lines = if note.is_empty() {
            Vec::new()
        } else {
            k.wrap(Face::Body, 12.0, &note, w)
        };
        let note_h = note_lines.len() as f32 * 16.0;
        let bottom = y + h - if note_h > 0.0 { note_h + 8.0 } else { 0.0 };
        for (i, line) in note_lines.iter().enumerate() {
            k.text(
                Face::Body,
                12.0,
                16.0,
                x,
                bottom + 8.0 + i as f32 * 16.0,
                col::MUTED,
                line,
            );
        }
        if bottom - top < 64.0 {
            top = bottom - 64.0;
        }
        let panel = [x, top, w, bottom - top];
        k.panel(panel, false);
        let (lx, ly, lw, lh) = (x + 6.0, top + 6.0, w - 12.0, panel[3] - 12.0);
        let selection = self
            .library
            .selections
            .get(&self.key)
            .cloned()
            .unwrap_or_default();
        let selected = if self.capes {
            selection.cape.clone()
        } else {
            selection.skin.clone()
        };
        let rows = self.rows();
        let content = rows.len() as f32 * 60.0;
        let clip = k.mark();
        let mut ry = ly - scroll;
        for row in rows {
            let r = [lx + 6.0, ry + 6.0, lw - 12.0, 48.0];
            ry += 60.0;
            let (title, detail, on, action, icon) = match row {
                Row::Cloud(i) => {
                    let cape = &self.cache.snapshot.capes[i];
                    (
                        cape.title.clone(),
                        if cape.active {
                            t("On the account").to_owned()
                        } else {
                            t("On the account · tap to wear it").to_owned()
                        },
                        cape.active,
                        Action::WardrobeActivateCape(i),
                        "globe",
                    )
                }
                Row::Local(i) => {
                    let entry = &self.library.entries[i];
                    (
                        entry.title.clone(),
                        t("On this phone").to_owned(),
                        selected.as_deref() == Some(entry.id.as_str()),
                        Action::WardrobeSelect(i),
                        "image",
                    )
                }
                Row::Button(label, icon, action) => {
                    list_targets.push((r, action));
                    if r[1] + 48.0 > ly && r[1] < ly + lh {
                        k.button(
                            r,
                            Style::Ghost,
                            Some(Icon::Pixel(icon)),
                            t(label),
                            Some(action),
                        );
                    }
                    continue;
                }
            };
            list_targets.push((r, action));
            if r[1] + 48.0 <= ly || r[1] >= ly + lh {
                continue;
            }
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
            k.ui.icon(icon, r[0] + 12.0, r[1] + 12.0, 24.0, col::INK_SOFT);
            let tw = r[2] - 48.0 - if on { 40.0 } else { 12.0 };
            k.text_fit(
                Face::BodySemiBold,
                15.0,
                18.0,
                r[0] + 48.0,
                r[1] + 6.0,
                tw,
                col::INK,
                &title,
            );
            k.text_fit(
                Face::Mono,
                11.0,
                14.0,
                r[0] + 48.0,
                r[1] + 27.0,
                tw,
                col::MUTED,
                &detail,
            );
            if on {
                k.ui.icon("check", r[0] + r[2] - 32.0, r[1] + 12.0, 24.0, col::SUCCESS);
            }
            k.hit(r, action);
        }
        if self.empty() && ry + 8.0 < ly + lh {
            let hint = if self.capes {
                t(
                    "No capes on this phone yet. Add a cape PNG (64×32) with the button above; it shows only for you.",
                )
            } else {
                t(
                    "No skins on this phone yet. Add a skin PNG (64×64) with the button above, then tap it to wear it.",
                )
            };
            k.paragraph(
                Face::BodyMedium,
                13.0,
                18.0,
                lx + 8.0,
                ry + 8.0,
                lw - 16.0,
                6,
                col::MUTED,
                hint,
            );
        }
        k.clip(clip, [lx, ly, lw, lh]);
        ([lx, ly, lw, lh], content + 12.0)
    }
}

/// One line of the wardrobe list.
enum Row {
    /// A cape on the account (Microsoft's capes).
    Cloud(usize),
    /// An image in the phone's gallery.
    Local(usize),
    Button(&'static str, &'static str, Action),
}
