//! Native account panel; authentication is shared with the desktop launcher.
use super::{
    kit::{self, Icon, Kit, Style, col},
    pages::Action,
    text::{t, t_dyn, tf},
};
use crate::account::{self, Account, Accounts, Provider, Secrets, SignIn, SignInState};
use horizon_client::ui_theme::Face;

pub(super) struct Login {
    pub choosing: bool,
    pub providers: bool,
    pub offline: bool,
    pub saved_accounts: Vec<Account>,
    pub account_offset: usize,
    pub ely: bool,
    pub username: String,
    pub password: String,
    pub otp: String,
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
            choosing: false,
            providers: false,
            offline: false,
            saved_accounts: vec![],
            account_offset: 0,
            ely: false,
            username: String::new(),
            password: String::new(),
            otp: String::new(),
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
        let accounts = Accounts::load();
        let account = accounts.selected().cloned();
        self.saved_accounts = accounts.accounts;
        self.profile = account.as_ref().map(|a| a.name.clone());
        self.signed_in = account.is_some_and(|a| {
            account::store()
                .get(&a.key())
                .ok()
                .flatten()
                .is_some_and(|s| serde_json::from_str::<Secrets>(&s).is_ok())
        });
    }
    pub fn pending(&self) -> bool {
        self.task.is_some()
    }
    pub fn hide(&mut self) {
        self.password.clear();
        self.otp.clear();
        self.visible = false;
    }
    pub fn close(&mut self) {
        self.password.clear();
        self.otp.clear();
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
        self.ely = false;
        self.choosing = false;
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
    /// Reports completion so views can switch their provider-scoped selection,
    /// including when the account panel was closed during browser sign-in.
    pub fn poll(&mut self) -> bool {
        let mut completed = false;
        if let Some(task) = &self.task {
            self.state = Some(task.state());
            if let Some(SignInState::Done(result)) = &self.state {
                self.message = match result {
                    Ok(a) => format!("Signed in as {}", a.name),
                    Err(e) => e.clone(),
                };
                self.task = None;
                self.refresh_status();
                completed = true;
            }
        }
        if self.auto_open_device && matches!(self.state, Some(SignInState::Code(_))) {
            self.auto_open_device = false;
            self.open_link();
        }
        completed
    }
    pub fn start_ely(&mut self) {
        if let Some(task) = self.task.take() {
            task.cancel();
        }
        self.message.clear();
        self.task = Some(SignIn::start_ely(
            self.username.clone(),
            std::mem::take(&mut self.password),
            std::mem::take(&mut self.otp),
        ));
        self.state = Some(SignInState::Starting);
    }
    pub fn field(&self, name: &str) -> Option<&str> {
        match name {
            "ely_username" => Some(&self.username),
            "ely_password" => Some(&self.password),
            "ely_otp" => Some(&self.otp),
            _ => None,
        }
    }
    pub fn edit(&mut self, field: Option<&str>, output: &horizon_client::osk::Output) -> bool {
        let value = match field {
            Some("ely_username") => &mut self.username,
            Some("ely_password") => &mut self.password,
            Some("ely_otp") => &mut self.otp,
            _ => return false,
        };
        match output {
            horizon_client::osk::Output::Text(text) if value.len() + text.len() <= 1024 => {
                value.push_str(text);
            }
            horizon_client::osk::Output::Backspace => {
                value.pop();
            }
            _ => {}
        }
        true
    }
    pub fn sign_out(&mut self) {
        self.close();
        self.visible = true;
        let mut accounts = Accounts::load();
        if let Some(a) = accounts.selected().cloned() {
            self.message = match accounts
                .remove_when_idle(account::store(), &a.key())
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
        account::session::selected_session(id.trim())
    }
    /// The account sheet over a scrim: the ways to play (offline, Microsoft, Ely.by,
    /// saved accounts), then the chosen one's own page.
    pub fn draw(&self, k: &mut Kit, offline_name: &str, editing: Option<&str>) {
        let (w, h) = (k.ui.w, k.ui.h);
        kit::fill(k.ui, 0.0, 0.0, w, h, col::SCRIM);
        let sw = w.min(640.0);
        let wide = sw >= 560.0;
        // Each page's height, so short pages don't leave an empty sheet.
        let want: f32 = if self.providers {
            if wide { 300.0 } else { 392.0 }
        } else if self.choosing {
            if wide { 316.0 } else { 470.0 }
        } else if self.ely {
            if wide { 360.0 } else { 470.0 }
        } else if self.offline {
            300.0
        } else {
            430.0
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
        let title = if self.providers {
            t("Account")
        } else if self.choosing {
            t("Saved accounts")
        } else if self.ely {
            t("Ely.by account")
        } else if self.offline {
            t("Play without signing in")
        } else {
            t("Microsoft account")
        };
        // Every page but the first has a way back to it.
        let mut tx = x + 16.0;
        if !self.providers {
            k.button(
                [x + 8.0, y + 22.0, 48.0, 48.0],
                Style::Ghost,
                Some(Icon::Pixel("arrow-left")),
                "",
                Some(Action::Account),
            );
            tx = x + 60.0;
        }
        k.text_fit(
            Face::BodyBold,
            20.0,
            24.0,
            tx,
            y + 34.0,
            x + sw - 72.0 - tx,
            col::INK,
            title,
        );
        k.button(
            [x + sw - 16.0 - 48.0, y + 22.0, 48.0, 48.0],
            Style::Ghost,
            Some(Icon::Pixel("close")),
            "",
            Some(Action::CloseLogin),
        );
        let body = [x + 16.0, y + 80.0, sw - 32.0, sh - 88.0];
        if self.providers {
            self.draw_providers(k, body, offline_name, wide);
        } else if self.choosing {
            self.draw_saved(k, body, offline_name, wide);
        } else if self.ely {
            self.draw_ely(k, body, editing, wide);
        } else if self.offline {
            self.draw_offline(k, body, offline_name, editing == Some("name"));
        } else {
            self.draw_microsoft(k, body);
        }
    }

    /// Who plays now, and the four ways to change it.
    fn draw_providers(&self, k: &mut Kit, [x, y, w, _]: [f32; 4], offline: &str, wide: bool) {
        let current = match (self.profile.as_deref(), self.signed_in) {
            (Some(name), true) => {
                let provider = self
                    .saved_accounts
                    .iter()
                    .find(|a| a.name == name)
                    .map_or("Microsoft", |a| provider_name(a.provider));
                tf("Now playing as {} ({})", &[name, provider])
            }
            _ => tf("Now playing as {} (offline)", &[&display_name(offline)]),
        };
        k.paragraph(
            Face::BodyMedium,
            13.0,
            18.0,
            x,
            y,
            w,
            2,
            col::INK_SOFT,
            &current,
        );
        let saved = self.saved_accounts.len().to_string();
        let rows: [(&'static str, &str, String, Action); 4] = [
            (
                "user",
                t("Without signing in"),
                t("Any name. Singleplayer and servers without licence checks.").to_owned(),
                Action::OfflineAccount,
            ),
            (
                "monitor",
                "Microsoft",
                t("A licensed Minecraft account. Needed for most public servers.").to_owned(),
                Action::MicrosoftAccount,
            ),
            (
                "globe",
                "Ely.by",
                t("A free account with skins, for servers that use Ely.by.").to_owned(),
                Action::ElySignIn,
            ),
            (
                "archive",
                t("Saved accounts"),
                tf("Switch between accounts signed in before: {}.", &[&saved]),
                Action::SavedAccounts,
            ),
        ];
        let cols = if wide { 2 } else { 1 };
        let cw = (w - 8.0 * (cols - 1) as f32) / cols as f32;
        let top = y + 44.0;
        for (i, (icon, title, detail, action)) in rows.into_iter().enumerate() {
            let rx = x + (i % cols) as f32 * (cw + 8.0);
            let ry = top + (i / cols) as f32 * 84.0;
            option_card(k, [rx, ry, cw, 76.0], icon, title, &detail, action);
        }
    }

    /// The accounts signed in before, a page of them at a time, and offline play.
    fn draw_saved(&self, k: &mut Kit, [x, y, w, h]: [f32; 4], offline: &str, wide: bool) {
        let cols = if wide { 2 } else { 1 };
        let cw = (w - 8.0 * (cols - 1) as f32) / cols as f32;
        let selected = Accounts::load().selected().map(Account::key);
        let mut items: Vec<(String, String, Action, bool)> = vec![(
            display_name(offline),
            t("Offline").to_owned(),
            Action::SelectOffline,
            !self.signed_in,
        )];
        for (index, account) in self
            .saved_accounts
            .iter()
            .enumerate()
            .skip(self.account_offset)
            .take(5)
        {
            items.push((
                account.name.clone(),
                provider_name(account.provider).to_owned(),
                Action::SelectAccount(index),
                selected.as_deref() == Some(&account.key()),
            ));
        }
        for (i, (name, provider, action, on)) in items.iter().enumerate() {
            let rx = x + (i % cols) as f32 * (cw + 8.0);
            let ry = y + (i / cols) as f32 * 56.0;
            if ry + 48.0 > y + h - 56.0 {
                break;
            }
            account_row(k, [rx, ry, cw, 48.0], name, provider, *on, *action);
        }
        if self.saved_accounts.is_empty() {
            let ry = y + (items.len().div_ceil(cols)) as f32 * 56.0 + 4.0;
            k.paragraph(
                Face::BodyMedium,
                13.0,
                18.0,
                x,
                ry,
                w,
                3,
                col::MUTED,
                t("No saved accounts yet. Sign in with Microsoft or Ely.by and the account stays here."),
            );
        }
        let pages = self.saved_accounts.len() > 5;
        if pages {
            let py = y + h - 48.0;
            let half = (w - 8.0) / 2.0;
            k.button(
                [x, py, half, 48.0],
                Style::Secondary,
                Some(Icon::Sprite(kit::CHEVRON_LEFT)),
                t("Previous"),
                (self.account_offset > 0).then_some(Action::SavedPage(false)),
            );
            k.button(
                [x + half + 8.0, py, half, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("chevron-right")),
                t("Next"),
                (self.account_offset + 5 < self.saved_accounts.len())
                    .then_some(Action::SavedPage(true)),
            );
        }
    }

    /// Ely.by: name, password and a two-factor code, sent only to Ely.by.
    fn draw_ely(&self, k: &mut Kit, [x, y, w, h]: [f32; 4], editing: Option<&str>, wide: bool) {
        let fields = [
            (
                t("Username or email"),
                "ely_username",
                self.username.as_str(),
                false,
            ),
            (
                t("Password (not saved)"),
                "ely_password",
                self.password.as_str(),
                true,
            ),
            (
                t("Two-factor code, if you set one up"),
                "ely_otp",
                self.otp.as_str(),
                true,
            ),
        ];
        let typing = editing.filter(|f| f.starts_with("ely_"));
        let mut cy = y;
        if typing.is_none() {
            cy += k.paragraph(
                Face::BodyMedium,
                13.0,
                18.0,
                x,
                cy,
                w,
                3,
                col::INK_SOFT,
                t("Your password goes only to authserver.ely.by and is not stored on the phone."),
            ) + 10.0;
        }
        // While a field is typed into, only it shows: the keyboard covers the rest.
        let shown: Vec<_> = fields
            .iter()
            .filter(|f| typing.is_none_or(|name| name == f.1))
            .collect();
        let half = (w - 8.0) / 2.0;
        for (i, (label, name, value, secret)) in shown.iter().enumerate() {
            // Wide sheets put password and code side by side under the name.
            let (fx, fy, fw) = if wide && typing.is_none() && i > 0 {
                (x + (i - 1) as f32 * (half + 8.0), cy, half)
            } else {
                (x, cy, w)
            };
            k.text(Face::Body, 12.0, 17.0, fx, fy, col::MUTED, label);
            let display = if *secret {
                "•".repeat(value.chars().count().min(40))
            } else {
                (*value).to_owned()
            };
            let active = editing == Some(*name);
            let caret = k.field(
                [fx, fy + 20.0, fw, 48.0],
                None,
                &display,
                "",
                Action::Field(name),
                active,
            );
            caret_blink(k, active, caret, fy + 20.0);
            if !(wide && typing.is_none() && i == 1) {
                cy += 76.0;
            }
        }
        if typing.is_some() {
            let done_w = k.button_width(Style::Secondary, Some(Icon::Pixel("check")), t("Done"));
            k.button(
                [x, cy, done_w, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("check")),
                t("Done"),
                Some(Action::FinishNameEdit),
            );
            return;
        }
        let ready = !self.username.trim().is_empty() && !self.password.is_empty();
        if self.pending() {
            spinner(k, x, cy + 17.0);
            k.text(
                Face::BodyMedium,
                13.0,
                18.0,
                x + 20.0,
                cy + 14.0,
                col::MUTED,
                t("Signing in…"),
            );
        } else {
            let label = t("Sign in to Ely.by");
            let bw = k
                .button_width(Style::Primary, None, label)
                .max(w.min(280.0));
            k.button(
                [x, cy, bw, 48.0],
                if ready {
                    Style::Primary
                } else {
                    Style::Disabled
                },
                None,
                label,
                ready.then_some(Action::SubmitEly),
            );
        }
        cy += 56.0;
        if !self.message.is_empty() && cy < y + h - 16.0 {
            let lines = ((y + h - cy) / 17.0).max(1.0) as usize;
            k.paragraph(
                Face::Body,
                12.0,
                17.0,
                x,
                cy,
                w,
                lines,
                col::MUTED,
                &t_dyn(&self.message),
            );
        }
    }

    /// Offline play: the in-game name, then use it.
    fn draw_offline(&self, k: &mut Kit, [x, y, w, _]: [f32; 4], name: &str, editing: bool) {
        let mut cy = y;
        if !editing {
            cy += k.paragraph(
                Face::BodyMedium,
                13.0,
                18.0,
                x,
                cy,
                w,
                3,
                col::INK_SOFT,
                t("Singleplayer worlds and servers without licence checks. The name is up to 16 letters, digits or _."),
            ) + 10.0;
        }
        k.text(Face::Body, 12.0, 17.0, x, cy, col::MUTED, t("In-game name"));
        cy += 20.0;
        let caret = k.field(
            [x, cy, w, 48.0],
            None,
            name,
            t("Name"),
            Action::Field("name"),
            editing,
        );
        caret_blink(k, editing, caret, cy);
        cy += 56.0;
        if editing {
            let done_w = k.button_width(Style::Secondary, Some(Icon::Pixel("check")), t("Done"));
            k.button(
                [x, cy, done_w, 48.0],
                Style::Secondary,
                Some(Icon::Pixel("check")),
                t("Done"),
                Some(Action::FinishNameEdit),
            );
            return;
        }
        let label = t("Play offline");
        let bw = k.button_width(Style::Primary, Some(Icon::Pixel("check")), label);
        let ready = !name.trim().is_empty();
        k.button(
            [x, cy, bw.max(w.min(280.0)), 48.0],
            if ready {
                Style::Primary
            } else {
                Style::Disabled
            },
            Some(Icon::Pixel("check")),
            label,
            ready.then_some(Action::SelectOffline),
        );
    }

    /// Microsoft: the browser or a code on another device.
    fn draw_microsoft(&self, k: &mut Kit, [lx, ly, column, height]: [f32; 4]) {
        let bottom = ly + height;
        let mut cy = ly;
        let code = match &self.state {
            Some(SignInState::Code(code)) => Some(code.user_code.clone()),
            _ => None,
        };
        let microsoft_signed_in = self.signed_in
            && self.saved_accounts.iter().any(|a| {
                Some(&a.name) == self.profile.as_ref() && a.provider == Provider::Microsoft
            });
        if microsoft_signed_in && code.is_none() && !self.pending() {
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
                &tf("Signed in as {}. The session renews by itself.", &[&name]),
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
            let half = (column - 8.0) / 2.0;
            let side = half >= 240.0;
            microsoft(
                k,
                [lx, cy, if side { half } else { column }, 48.0],
                t("Sign in with the browser"),
                Action::SignIn,
            );
            let (gx, gy, gw) = if side {
                (lx + half + 8.0, cy, half)
            } else {
                (lx, cy + 52.0, column)
            };
            k.button(
                [gx, gy, gw, 48.0],
                Style::Ghost,
                None,
                t("Use a code on another device"),
                Some(Action::DeviceSignIn),
            );
            cy = gy + 52.0;
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
        if !self.message.is_empty() && cy < bottom - 12.0 {
            let lines = ((bottom - cy) / 16.0).max(1.0) as usize;
            k.paragraph(
                Face::Body,
                12.0,
                16.0,
                lx,
                cy,
                column,
                lines.min(4),
                col::MUTED,
                &t_dyn(&self.message),
            );
        }
    }
}

fn provider_name(provider: Provider) -> &'static str {
    if provider == Provider::Ely {
        "Ely.by"
    } else {
        "Microsoft"
    }
}
/// The offline name as shown: "Player" while none is set.
fn display_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        t("Player").to_owned()
    } else {
        name.to_owned()
    }
}
/// The field's caret, blinking twice a second while it is typed into.
fn caret_blink(k: &mut Kit, editing: bool, caret: f32, field_y: f32) {
    let on = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .is_ok_and(|t| t.as_millis() % 1000 < 500);
    if editing && on {
        kit::fill(k.ui, caret + 1.0, field_y + 14.0, 2.0, 20.0, col::INK);
    }
}
/// A tappable card: icon, title and a line or two of what it is.
fn option_card(
    k: &mut Kit,
    r: [f32; 4],
    icon: &'static str,
    title: &str,
    detail: &str,
    action: Action,
) {
    let [x, y, w, h] = r;
    let pressed = k.pressed(r, Some(action));
    kit::fill(
        k.ui,
        x,
        y,
        w,
        h,
        if pressed {
            col::CONTROL_PRESSED
        } else {
            col::CONTROL
        },
    );
    kit::frame(k.ui, r, col::CONTROL_RING);
    k.ui.icon(icon, x + 12.0, y + 14.0, 24.0, col::ACCENT_HI);
    k.text_fit(
        Face::BodyBold,
        15.0,
        19.0,
        x + 48.0,
        y + 10.0,
        w - 60.0,
        col::INK,
        title,
    );
    k.paragraph(
        Face::Body,
        12.0,
        16.0,
        x + 48.0,
        y + 31.0,
        w - 60.0,
        2,
        col::MUTED,
        detail,
    );
    k.hit(r, action);
}
/// One account: its head, name and provider; the one in use is ticked.
fn account_row(k: &mut Kit, r: [f32; 4], name: &str, provider: &str, on: bool, action: Action) {
    let [x, y, w, h] = r;
    let pressed = k.pressed(r, Some(action));
    kit::fill(
        k.ui,
        x,
        y,
        w,
        h,
        if pressed {
            col::CONTROL_PRESSED
        } else {
            col::CONTROL
        },
    );
    kit::frame(k.ui, r, if on { col::ACCENT } else { col::CONTROL_RING });
    k.ui.icon("user", x + 12.0, y + 12.0, 24.0, col::INK_SOFT);
    let right = if on { 40.0 } else { 12.0 };
    k.text_fit(
        Face::BodySemiBold,
        15.0,
        18.0,
        x + 48.0,
        y + 6.0,
        w - 48.0 - right,
        col::INK,
        name,
    );
    k.text_fit(
        Face::Mono,
        11.0,
        14.0,
        x + 48.0,
        y + 27.0,
        w - 48.0 - right,
        col::MUTED,
        provider,
    );
    if on {
        k.ui.icon("check", x + w - 32.0, y + 12.0, 24.0, col::SUCCESS);
    }
    k.hit(r, action);
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
