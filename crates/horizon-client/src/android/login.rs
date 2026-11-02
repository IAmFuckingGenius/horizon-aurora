//! Native account panel; authentication is shared with the desktop launcher.
use super::{
    kit::{self, Icon, Kit, Style, col},
    pages::Action,
    text::t,
};
use crate::account::{self, Account, Accounts, Secrets, SignIn, SignInState};
use horizon_client::ui_theme::Face;

pub(super) struct Login {
    pub visible: bool,
    pub signed_in: bool,
    pub profile: Option<String>,
    task: Option<SignIn>,
    auto_open_device: bool,
    state: Option<SignInState>,
    message: String,
}
impl Login {
    pub fn new() -> Self {
        let mut login = Self {
            visible: false,
            signed_in: false,
            profile: None,
            task: None,
            auto_open_device: false,
            state: None,
            message: String::new(),
        };
        login.refresh_status();
        login
    }
    pub(super) fn refresh_status(&mut self) {
        let account = Accounts::load().selected().cloned();
        self.profile = account.as_ref().map(|a| a.name.clone());
        self.signed_in = account.is_some_and(|a| {
            account::store()
                .get(&a.uuid)
                .ok()
                .flatten()
                .is_some_and(|s| serde_json::from_str::<Secrets>(&s).is_ok())
        });
    }
    pub fn pending(&self) -> bool {
        self.task.is_some()
    }
    pub fn hide(&mut self) {
        self.visible = false;
    }
    pub fn close(&mut self) {
        self.auto_open_device = false;
        if let Some(task) = self.task.take() {
            task.cancel();
        }
        self.state = None;
        self.visible = false;
    }
    pub fn start(&mut self, id: &str) {
        self.start_device(id);
        self.auto_open_device = true;
    }
    pub fn start_device(&mut self, id: &str) {
        self.close();
        self.visible = true;
        let id = id.trim();
        if !account::legacy_client(id)
            && (id.len() != 36
                || !id.chars().enumerate().all(|(i, c)| {
                    if [8, 13, 18, 23].contains(&i) {
                        c == '-'
                    } else {
                        c.is_ascii_hexdigit()
                    }
                }))
        {
            self.message =
                "Register our Microsoft public client application first, then enter its client ID."
                    .into();
            return;
        }
        self.message.clear();
        self.task = Some(SignIn::start_with_client_id(Some(id.into())));
        self.state = Some(SignInState::Starting);
    }
    pub fn poll(&mut self) {
        if let Some(task) = &self.task {
            self.state = Some(task.state());
            if let Some(SignInState::Done(result)) = &self.state {
                self.message = match result {
                    Ok(a) => format!("Signed in as {}", a.name),
                    Err(e) => e.clone(),
                };
                self.task = None;
                self.refresh_status();
            }
        }
        if self.auto_open_device && matches!(self.state, Some(SignInState::Code(_))) {
            self.auto_open_device = false;
            self.open_link();
        }
    }
    pub fn sign_out(&mut self) {
        self.close();
        self.visible = true;
        let mut accounts = Accounts::load();
        if let Some(a) = accounts.selected().cloned() {
            self.message = match accounts
                .remove(account::store(), &a.uuid)
                .and_then(|_| accounts.save())
            {
                Ok(()) => "Signed out. Offline play is available.".into(),
                Err(e) => e,
            };
        }
        self.refresh_status();
    }
    pub fn copy_code(&mut self) {
        if let Some(SignInState::Code(code)) = &self.state {
            self.message = if crate::platform::copy_login_code(&code.user_code) {
                "Code copied. Paste it on the Microsoft page.".into()
            } else {
                "Could not copy the code. Try again.".into()
            };
        }
    }
    pub fn open_link(&mut self) {
        if let Some(SignInState::Code(code)) = &self.state {
            let mut url = url::Url::parse(&code.verification_uri).ok();
            if let Some(uri) = &mut url {
                uri.query_pairs_mut().append_pair("otc", &code.user_code);
            }
            let url = url
                .map(String::from)
                .unwrap_or_else(|| code.verification_uri.clone());
            self.copy_code();
            if let Err(e) = crate::platform::open_login_url(&url) {
                self.message = e;
            }
        }
    }
    pub fn review_app(&mut self) {
        if let Err(e) = crate::platform::open_login_url("https://aka.ms/mce-reviewappid") {
            self.message = e;
        }
    }
    pub fn session(id: &str) -> Result<Option<(Account, Secrets)>, String> {
        let mut accounts = Accounts::load();
        let Some(a) = accounts.selected().cloned() else {
            return Ok(None);
        };
        let secrets = account::store()
            .get(&a.uuid)?
            .and_then(|s| serde_json::from_str(&s).ok())
            .ok_or("Session ended. Sign in again, or sign out to play offline.")?;
        let (a, secrets) = account::fresh(&account::Endpoints::live(), id.trim(), &a, secrets)?;
        accounts.add(account::store(), a.clone(), &secrets)?;
        accounts.save()?;
        Ok(Some((a, secrets)))
    }
    /// The account sheet: Microsoft sign-in on the left, offline play on the right
    /// (stacked on narrow screens), over a scrim.
    pub fn draw(&self, k: &mut Kit, offline_name: &str, editing_name: bool) {
        let (w, h) = (k.ui.w, k.ui.h);
        kit::fill(k.ui, 0.0, 0.0, w, h, col::SCRIM);
        let sw = w.min(640.0);
        let two = sw >= 560.0;
        let sh = (if two { 336.0 } else { 520.0f32 }).min(h - 12.0);
        let (x, y) = ((w - sw) / 2.0, h - sh);
        k.panel([x, y, sw, sh + 8.0], false);
        kit::fill(
            k.ui,
            x + (sw - 40.0) / 2.0,
            y + 18.0,
            40.0,
            4.0,
            col::CONTROL_RING,
        );
        k.text(
            Face::BodyBold,
            20.0,
            24.0,
            x + 16.0,
            y + 40.0,
            col::INK,
            t("Account"),
        );
        k.button(
            [x + sw - 16.0 - 48.0, y + 28.0, 48.0, 48.0],
            Style::Ghost,
            Some(Icon::Pixel("close")),
            "",
            Some(Action::CloseLogin),
        );
        let column = if two {
            (sw - 32.0 - 24.0) / 2.0
        } else {
            sw - 32.0
        };
        let (lx, ly) = (x + 16.0, y + 80.0);
        // Left: Microsoft.
        k.text(
            Face::BodyBold,
            16.0,
            19.0,
            lx,
            ly,
            col::INK,
            t("Sign in with Microsoft"),
        );
        let mut cy = ly + 27.0;
        let code = match &self.state {
            Some(SignInState::Code(code)) => Some(code.user_code.clone()),
            _ => None,
        };
        if self.signed_in {
            let name = self.profile.clone().unwrap_or_default();
            cy += k.paragraph(
                Face::BodyMedium,
                13.0,
                18.85,
                lx,
                cy,
                column,
                3,
                col::INK_SOFT,
                &super::text::tf("Signed in as {}. The session renews by itself.", &[&name]),
            ) + 8.0;
            let switch_w = k.button_width(
                Style::Secondary,
                Some(Icon::Pixel("user")),
                t("Switch account"),
            );
            k.button(
                [lx, cy, switch_w.min(column), 48.0],
                Style::Secondary,
                Some(Icon::Pixel("user")),
                t("Switch account"),
                Some(Action::SignIn),
            );
            cy += 56.0;
            let out_w = k.button_width(Style::Danger, None, t("Sign out"));
            k.button(
                [lx, cy, out_w, 48.0],
                Style::Danger,
                None,
                t("Sign out"),
                Some(Action::SignOut),
            );
            cy += 56.0;
        } else if let Some(code) = code {
            cy += k.paragraph(
                Face::BodyMedium,
                13.0,
                18.85,
                lx,
                cy,
                column,
                3,
                col::INK_SOFT,
                t("Needed for servers that check the licence. On another device open microsoft.com/link and enter the code:"),
            ) + 8.0;
            let copy_w = k.button_width(Style::Secondary, Some(Icon::Sprite(kit::COPY)), t("Copy"));
            let box_w = column - copy_w - 8.0;
            kit::fill(k.ui, lx, cy, box_w, 48.0, col::SIDE);
            kit::frame(k.ui, [lx, cy, box_w, 48.0], col::PLATE_RING);
            let cw = k.measure(Face::Mono, 18.0, &code);
            k.text(
                Face::Mono,
                18.0,
                24.0,
                lx + (box_w - cw) / 2.0,
                cy + 12.0,
                col::INK,
                &code,
            );
            k.button(
                [lx + box_w + 8.0, cy, copy_w, 48.0],
                Style::Secondary,
                Some(Icon::Sprite(kit::COPY)),
                t("Copy"),
                Some(Action::CopyLoginCode),
            );
            cy += 56.0;
            microsoft(
                k,
                [lx, cy, column, 48.0],
                t("Open the link"),
                Action::LoginLink,
            );
            cy += 56.0;
            spinner(k, lx, cy + 3.0);
            k.text_fit(
                Face::BodyMedium,
                13.0,
                18.85,
                lx + 20.0,
                cy,
                column - 20.0,
                col::MUTED,
                t("Waiting for confirmation…"),
            );
            cy += 24.0;
        } else if self.pending() {
            spinner(k, lx, cy + 3.0);
            k.text_fit(
                Face::BodyMedium,
                13.0,
                18.85,
                lx + 20.0,
                cy,
                column - 20.0,
                col::MUTED,
                t("Requesting a sign-in code…"),
            );
            cy += 28.0;
        } else {
            cy += k.paragraph(
                Face::BodyMedium,
                13.0,
                18.85,
                lx,
                cy,
                column,
                3,
                col::INK_SOFT,
                t("Needed for servers that check the licence. You type your password on Microsoft's own page."),
            ) + 8.0;
            microsoft(
                k,
                [lx, cy, column, 48.0],
                t("Sign in with the browser"),
                Action::SignIn,
            );
            cy += 52.0;
            k.button(
                [lx, cy, column, 48.0],
                Style::Ghost,
                None,
                t("Use a code on another device"),
                Some(Action::DeviceSignIn),
            );
            cy += 52.0;
        }
        if self.pending() {
            k.button(
                [lx, cy, column, 48.0],
                Style::Ghost,
                None,
                t("Cancel sign-in"),
                Some(Action::CancelLogin),
            );
            cy += 52.0;
        }
        if self.message.contains("not approved by Mojang") {
            k.button(
                [lx, cy, column, 48.0],
                Style::Ghost,
                None,
                t("Request Minecraft API approval"),
                Some(Action::ReviewApp),
            );
            cy += 52.0;
        }
        if !self.message.is_empty() && cy < y + sh - 20.0 {
            let lines = ((y + sh - 8.0 - cy) / 16.0).max(1.0) as usize;
            cy += k.paragraph(
                Face::Body,
                12.0,
                16.0,
                lx,
                cy,
                column,
                lines.min(4),
                col::MUTED,
                &self.message,
            );
        }
        // Right: offline play.
        let (rx, ry) = if two {
            (lx + column + 24.0, ly)
        } else {
            (lx, cy + 14.0)
        };
        if ry + 120.0 > y + sh {
            return;
        }
        k.text(
            Face::BodyBold,
            16.0,
            19.0,
            rx,
            ry,
            col::INK,
            t("Without signing in"),
        );
        let mut oy = ry + 27.0;
        oy += k.paragraph(
            Face::BodyMedium,
            13.0,
            18.85,
            rx,
            oy,
            column,
            2,
            col::INK_SOFT,
            t("Singleplayer worlds and servers without licence checks."),
        ) + 8.0;
        k.text(
            Face::Body,
            12.0,
            17.4,
            rx,
            oy,
            col::MUTED,
            t("In-game name"),
        );
        oy += 22.0;
        let caret = k.field(
            [rx, oy, column, 48.0],
            None,
            offline_name,
            t("Name"),
            Action::Field("name"),
            editing_name,
        );
        let blink = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .is_ok_and(|t| t.as_millis() % 1000 < 500);
        if editing_name && blink {
            kit::fill(k.ui, caret + 1.0, oy + 14.0, 2.0, 20.0, col::INK);
        }
        oy += 56.0;
        if editing_name {
            let save_w =
                k.button_width(Style::Secondary, Some(Icon::Pixel("check")), t("Save name"));
            k.button(
                [rx, oy, save_w, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("check")),
                t("Save name"),
                Some(Action::FinishNameEdit),
            );
        }
    }
}

/// Microsoft's sign-in button: its four-square logo on dark grey.
fn microsoft(k: &mut Kit, r: [f32; 4], label: &str, action: Action) {
    use horizon_client::ui_theme::color::hex;
    let [x, y, w, h] = r;
    let pressed = k.pressed(r, Some(action));
    kit::fill(
        k.ui,
        x,
        y,
        w,
        h,
        hex(if pressed { 0x3d3d3d } else { 0x2f2f2f }, 255),
    );
    let lw = k.measure(Face::SemiBold, 15.0, label);
    let cx = x + (w - 20.0 - 12.0 - lw) / 2.0;
    for (dx, dy, rgb) in [
        (0.0, 0.0, 0xf25022),
        (11.0, 0.0, 0x7fba00),
        (0.0, 11.0, 0x00a4ef),
        (11.0, 11.0, 0xffb900),
    ] {
        kit::fill(
            k.ui,
            cx + dx,
            y + (h - 20.0) / 2.0 + dy,
            9.0,
            9.0,
            hex(rgb, 255),
        );
    }
    k.text(
        Face::SemiBold,
        15.0,
        18.0,
        cx + 32.0,
        y + (h - 18.0) / 2.0,
        hex(0xffffff, 255),
        label,
    );
    k.hit(r, action);
}

/// Two squares turning a quarter every 250 ms.
fn spinner(k: &mut Kit, x: f32, y: f32) {
    let step = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |t| t.as_millis() / 250)
        % 4) as usize;
    let spots = [(0.0, 0.0), (6.0, 0.0), (6.0, 6.0), (0.0, 6.0)];
    let (ax, ay) = spots[step];
    let (bx, by) = spots[(step + 2) % 4];
    kit::fill(k.ui, x + ax, y + ay, 6.0, 6.0, col::ACCENT_HI);
    kit::fill(k.ui, x + bx, y + by, 6.0, 6.0, col::CONTROL_RING);
}
impl Drop for Login {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.cancel();
        }
    }
}
