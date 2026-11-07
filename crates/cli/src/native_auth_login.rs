#![forbid(unsafe_code)]
//! Provider OAuth / device-code login surface for the native CLI.
//!
//! Upstream evidence (pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`): provider
//! authentication is a first-class dialog — `packages/tui/src/component/
//! dialog-provider.tsx` lists providers, and the OAuth branches render the
//! verification URL plus user code while a device-token poll is in flight
//! (`packages/tui/src/provider/auth.ts`).  The native client had **no** login
//! route at all: `persisted_auth` could read and write an OAuth credential, and
//! `providers::openai_device_auth` could run the RFC 8628 flow, but nothing in
//! the CLI ever started either.  This module is the missing surface.
//!
//! Two halves, deliberately separated:
//!
//! * [`LoginSurface`] is pure state and layout.  It performs no IO, never holds
//!   an access or refresh token, and is fully unit-tested.
//! * [`LoginRun`] owns the network flow on a background thread with its own
//!   current-thread runtime, so the event loop never blocks on auth.  It reports
//!   back through a bounded channel polled by the caller.
//!
//! Secrets never enter the surface: the credential is handed straight to
//! `providers::persisted_auth::save_oauth_credential` by the caller.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

/// Maximum providers the picker renders.
pub const MAX_PROVIDERS: usize = 8;

/// How a provider mints an OAuth credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginKind {
    /// RFC 8628 device authorization: display a code, poll for consent.
    DeviceCode,
    /// Hosted browser callback (the caller opens the URL).
    Browser,
}

impl LoginKind {
    /// Short, user-facing method label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DeviceCode => "device code",
            Self::Browser => "browser",
        }
    }
}

/// One provider the login dialog can start a flow for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginProvider {
    /// Persisted auth key (`auth.json` entry name).
    pub id: &'static str,
    /// Rendered row label.
    pub label: &'static str,
    pub kind: LoginKind,
}

/// Providers with a wired native flow.
///
/// Only OpenAI has a complete device-authorization transport in this build
/// (`providers::openai_device_auth`).  Anthropic's Claude Code flow is a hosted
/// browser consent that still needs a human-supplied callback; it is listed so
/// the picker is honest about the method, and [`LoginSurface::begin`] refuses it
/// with an explicit reason instead of pretending to poll.
pub const PROVIDERS: [LoginProvider; 2] = [
    LoginProvider {
        id: "openai",
        label: "OpenAI \u{b7} ChatGPT",
        kind: LoginKind::DeviceCode,
    },
    LoginProvider {
        id: "anthropic",
        label: "Anthropic \u{b7} Claude",
        kind: LoginKind::Browser,
    },
];

/// Where the login dialog currently is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoginPhase {
    /// Provider picker.
    Choosing,
    /// `begin_device_authorization` is in flight.
    Starting,
    /// Consent is pending; the user has a code to enter in a browser.
    AwaitingConsent { user_code: String, url: String },
    /// The authorization code is being exchanged for tokens.
    Exchanging,
    /// Credential persisted.
    Succeeded { account: Option<String> },
    /// Flow failed with a user-facing reason.
    Failed { message: String },
    /// User dismissed the dialog.
    Cancelled,
}

impl LoginPhase {
    /// Whether the flow reached a state the caller must not poll out of.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded { .. } | Self::Failed { .. } | Self::Cancelled)
    }
}

/// The login dialog's pure state.
#[derive(Clone, Debug)]
pub struct LoginSurface {
    open: bool,
    phase: LoginPhase,
    selected: usize,
    provider: LoginProvider,
}

impl Default for LoginSurface {
    fn default() -> Self {
        Self {
            open: false,
            phase: LoginPhase::Choosing,
            selected: 0,
            provider: PROVIDERS[0].clone(),
        }
    }
}

impl LoginSurface {
    /// Fresh surface (closed).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Open on the provider picker.
    pub fn open(&mut self) {
        self.open = true;
        self.phase = LoginPhase::Choosing;
        self.selected = 0;
        self.provider = PROVIDERS[0].clone();
    }

    /// Close and forget any in-flight presentation.
    pub fn close(&mut self) {
        self.open = false;
        self.phase = LoginPhase::Cancelled;
    }

    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    #[must_use]
    pub fn phase(&self) -> &LoginPhase {
        &self.phase
    }

    /// The provider the picker has highlighted.
    #[must_use]
    pub fn provider(&self) -> &LoginProvider {
        &self.provider
    }

    /// Highlighted row index.
    #[must_use]
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// Move the highlight down, wrapping.
    pub fn select_next(&mut self) -> bool {
        if self.phase != LoginPhase::Choosing {
            return false;
        }
        let before = self.selected;
        self.selected = (self.selected + 1) % PROVIDERS.len().min(MAX_PROVIDERS);
        self.provider = PROVIDERS[self.selected].clone();
        self.selected != before
    }

    /// Move the highlight up, wrapping.
    pub fn select_prev(&mut self) -> bool {
        if self.phase != LoginPhase::Choosing {
            return false;
        }
        let count = PROVIDERS.len().min(MAX_PROVIDERS);
        let before = self.selected;
        self.selected = (self.selected + count - 1) % count;
        self.provider = PROVIDERS[self.selected].clone();
        self.selected != before
    }

    /// Commit the highlighted provider and move to [`LoginPhase::Starting`].
    ///
    /// Returns the provider id the caller must start a flow for.  A provider
    /// whose flow is not wired in this build fails closed with a reason rather
    /// than leaving the dialog spinning.
    pub fn begin(&mut self) -> Result<&'static str, String> {
        if !self.open {
            return Err("login dialog is not open".to_owned());
        }
        if self.phase != LoginPhase::Choosing {
            return Err("login already started".to_owned());
        }
        if self.provider.kind != LoginKind::DeviceCode {
            let message = format!(
                "{} needs a browser consent this build cannot receive yet",
                self.provider.label
            );
            self.phase = LoginPhase::Failed {
                message: message.clone(),
            };
            return Err(message);
        }
        self.phase = LoginPhase::Starting;
        Ok(self.provider.id)
    }

    /// The device authorization request succeeded.
    pub fn began_authorization(&mut self, user_code: String, url: String) -> bool {
        if self.phase != LoginPhase::Starting {
            return false;
        }
        self.phase = LoginPhase::AwaitingConsent { user_code, url };
        true
    }

    /// The poll succeeded and the code is being exchanged.
    pub fn exchanging(&mut self) -> bool {
        if !matches!(self.phase, LoginPhase::AwaitingConsent { .. }) {
            return false;
        }
        self.phase = LoginPhase::Exchanging;
        true
    }

    /// The credential was persisted.
    pub fn succeeded(&mut self, account: Option<String>) -> bool {
        if self.phase.is_terminal() {
            return false;
        }
        self.phase = LoginPhase::Succeeded { account };
        true
    }

    /// The flow failed.
    pub fn failed(&mut self, message: impl Into<String>) -> bool {
        if self.phase.is_terminal() {
            return false;
        }
        self.phase = LoginPhase::Failed {
            message: message.into(),
        };
        true
    }

    /// The title row.
    #[must_use]
    pub fn title(&self) -> String {
        match &self.phase {
            LoginPhase::Choosing | LoginPhase::Starting | LoginPhase::AwaitingConsent { .. } => {
                format!("Log in with {}", self.provider.label)
            }
            LoginPhase::Exchanging => "Finishing sign-in".to_owned(),
            LoginPhase::Succeeded { .. } => "Signed in".to_owned(),
            LoginPhase::Failed { .. } => "Sign-in failed".to_owned(),
            LoginPhase::Cancelled => "Sign-in cancelled".to_owned(),
        }
    }

    /// The footer hint row for the current phase.
    #[must_use]
    pub const fn hint(&self) -> &'static str {
        match &self.phase {
            LoginPhase::Choosing => "enter continue \u{b7} \u{2191}\u{2193} select \u{b7} esc cancel",
            LoginPhase::Starting | LoginPhase::AwaitingConsent { .. } | LoginPhase::Exchanging => {
                "waiting for approval \u{b7} esc cancel"
            }
            LoginPhase::Succeeded { .. } | LoginPhase::Failed { .. } | LoginPhase::Cancelled => {
                "enter close"
            }
        }
    }

    /// Body rows, already clipped to `width`.
    ///
    /// Rendered as plain lines so the caller can reuse the existing modal paint
    /// path; no row ever contains a token.
    #[must_use]
    pub fn body(&self, width: u16) -> Vec<String> {
        let rows: Vec<String> = match &self.phase {
            LoginPhase::Choosing => PROVIDERS
                .iter()
                .take(MAX_PROVIDERS)
                .enumerate()
                .map(|(index, provider)| {
                    let marker = if index == self.selected { '\u{203a}' } else { ' ' };
                    format!("{marker} {} ({})", provider.label, provider.kind.label())
                })
                .collect(),
            LoginPhase::Starting => vec!["Requesting a device code \u{2026}".to_owned()],
            LoginPhase::AwaitingConsent { user_code, url } => vec![
                "Open this URL in a browser:".to_owned(),
                url.clone(),
                String::new(),
                "and enter this code:".to_owned(),
                user_code.clone(),
                String::new(),
                "Waiting for approval \u{2026}".to_owned(),
            ],
            LoginPhase::Exchanging => vec!["Exchanging authorization code \u{2026}".to_owned()],
            LoginPhase::Succeeded { account } => {
                let mut rows = vec!["Credentials saved \u{2713}".to_owned()];
                if let Some(account) = account {
                    rows.push(format!("Account: {account}"));
                }
                rows
            }
            LoginPhase::Failed { message } => vec![message.clone()],
            LoginPhase::Cancelled => vec!["Sign-in cancelled".to_owned()],
        };
        rows.into_iter().map(|row| clip(&row, width)).collect()
    }
}

/// Truncate to `width` scalars with an ellipsis, matching the dialog convention.
fn clip(value: &str, width: u16) -> String {
    let width = usize::from(width);
    if value.chars().count() <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut out: String = value.chars().take(width.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// What a background [`LoginRun`] reports to the event loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoginEvent {
    /// Device authorization started; show `user_code` and `url`.
    Authorization { user_code: String, url: String },
    /// The credential was persisted to `auth.json`. No access or refresh token
    /// ever crosses this channel, so an event log cannot leak a secret.
    Saved { account: Option<String> },
    /// The flow failed with a user-facing reason.
    Failed(String),
}

/// A device-authorization flow running off the event loop.
pub struct LoginRun {
    rx: Receiver<LoginEvent>,
    cancel: tokio::sync::watch::Sender<bool>,
}

impl std::fmt::Debug for LoginRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginRun").finish_non_exhaustive()
    }
}

impl LoginRun {
    /// Start the OpenAI device-authorization flow on a background thread.
    ///
    /// The thread owns a current-thread runtime; the event loop only polls and
    /// may cancel at any point.
    pub fn start(provider: &str) -> Result<Self, String> {
        if provider != "openai" {
            return Err(format!("no device-code flow for provider {provider}"));
        }
        let provider = provider.to_owned();
        let (tx, rx) = mpsc::channel();
        let (cancel, cancel_rx) = tokio::sync::watch::channel(false);
        std::thread::Builder::new()
            .name("oc2-auth-login".to_owned())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(_) => {
                        let _ = tx.send(LoginEvent::Failed("auth runtime unavailable".to_owned()));
                        return;
                    }
                };
                runtime.block_on(run_device_flow(&provider, &tx, cancel_rx));
            })
            .map_err(|_| "unable to start auth worker".to_owned())?;
        Ok(Self { rx, cancel })
    }

    /// Non-blocking poll for the next event.
    pub fn try_poll(&mut self) -> Option<LoginEvent> {
        match self.rx.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(LoginEvent::Failed(
                "auth worker stopped unexpectedly".to_owned(),
            )),
        }
    }

    /// Ask the worker to stop; it reports no further credential.
    pub fn cancel(&self) {
        let _ = self.cancel.send(true);
    }
}

async fn run_device_flow(
    provider: &str,
    tx: &mpsc::Sender<LoginEvent>,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) {
    use opencode_rk_providers::openai_device_auth as device;

    let transport = match device::AuthTransport::new() {
        Ok(transport) => transport,
        Err(_) => {
            let _ = tx.send(LoginEvent::Failed("could not create auth transport".to_owned()));
            return;
        }
    };
    let endpoints = device::DeviceEndpoints::default();
    let authorization = match device::begin_device_authorization(&transport, &endpoints, &mut cancel).await {
        Ok(authorization) => authorization,
        Err(error) => {
            let _ = tx.send(LoginEvent::Failed(describe(error)));
            return;
        }
    };
    let url = authorization.verification_url.clone();
    let user_code = authorization.user_code.clone();
    if tx
        .send(LoginEvent::Authorization { user_code, url })
        .is_err()
    {
        return;
    }
    match device::authorize_device(&transport, &endpoints, &authorization, &mut cancel).await {
        Ok(credential) => {
            // Persist before reporting success: the dialog only claims
            // "signed in" once `auth.json` really holds the credential.
            let account = credential.account_id.clone();
            let saved = opencode_rk_providers::persisted_auth::save_oauth_credential(
                provider.to_owned(),
                credential.access,
                credential.refresh,
                credential.expires_at_ms,
                credential.account_id,
            )
            .await;
            match saved {
                Ok(()) => {
                    let _ = tx.send(LoginEvent::Saved { account });
                }
                Err(error) => {
                    let _ = tx.send(LoginEvent::Failed(format!(
                        "could not save credential: {error}"
                    )));
                }
            }
        }
        Err(error) => {
            let _ = tx.send(LoginEvent::Failed(describe(error)));
        }
    }
}

/// User-facing text for a device-auth failure.
fn describe(error: opencode_rk_providers::openai_device_auth::OpenAiDeviceAuthError) -> String {
    use opencode_rk_providers::openai_device_auth::OpenAiDeviceAuthError as E;
    match error {
        E::Request => "the provider could not be reached".to_owned(),
        E::InvalidResponse => "the provider returned an unexpected response".to_owned(),
        E::BodyTooLarge => "the provider response was too large".to_owned(),
        E::Denied => "authorization was denied".to_owned(),
        E::Expired => "the device code expired; start again".to_owned(),
        E::Cancelled => "sign-in cancelled".to_owned(),
        E::PollLimit => "timed out waiting for approval; start again".to_owned(),
        E::EndpointPolicy => "the auth endpoint policy rejected this build".to_owned(),
        E::InvalidExpiry => "the provider returned an invalid expiry".to_owned(),
    }
}

/// Human-readable countdown label for the consent phase.
///
/// Upstream shows the remaining validity of the device code; the surface keeps
/// it derived rather than stored so a redraw never drifts from the deadline.
#[must_use]
pub fn remaining_label(remaining: Duration) -> String {
    let seconds = remaining.as_secs();
    if seconds == 0 {
        return "expired".to_owned();
    }
    let minutes = seconds / 60;
    if minutes == 0 {
        format!("{seconds}s")
    } else {
        format!("{minutes}m {:02}s", seconds % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface() -> LoginSurface {
        let mut surface = LoginSurface::new();
        surface.open();
        surface
    }

    #[test]
    fn opening_starts_on_the_picker_with_every_provider_listed() {
        let surface = surface();
        assert!(surface.is_open());
        assert_eq!(surface.phase(), &LoginPhase::Choosing);
        let body = surface.body(80);
        assert_eq!(body.len(), PROVIDERS.len());
        assert!(body[0].starts_with('\u{203a}'), "first row is highlighted: {body:?}");
        assert!(body[1].starts_with(' '), "other rows are not: {body:?}");
        assert!(body[0].contains("OpenAI"), "label is rendered: {body:?}");
        assert!(body[0].contains("device code"), "method is rendered: {body:?}");
    }

    #[test]
    fn selection_wraps_both_ways_and_moves_the_marker() {
        let mut surface = surface();
        assert!(surface.select_next());
        assert_eq!(surface.selected(), 1);
        assert_eq!(surface.provider().id, "anthropic");
        assert!(surface.body(80)[1].starts_with('\u{203a}'));
        assert!(!surface.body(80)[0].starts_with('\u{203a}'));

        assert!(surface.select_next(), "wraps to the first row");
        assert_eq!(surface.selected(), 0);
        assert!(surface.select_prev(), "wraps to the last row");
        assert_eq!(surface.selected(), PROVIDERS.len() - 1);
    }

    #[test]
    fn begin_refuses_a_flow_that_is_not_wired_instead_of_hanging() {
        let mut surface = surface();
        surface.select_next();
        let error = surface.begin().expect_err("browser flow is refused");
        assert!(error.contains("browser consent"), "reason is explicit: {error}");
        assert!(matches!(surface.phase(), LoginPhase::Failed { .. }));
        assert!(surface.phase().is_terminal());
        // A refused flow cannot be re-entered.
        assert!(surface.begin().is_err());
    }

    #[test]
    fn the_device_flow_shows_the_code_and_url_and_never_a_token() {
        let mut surface = surface();
        assert_eq!(surface.begin(), Ok("openai"));
        assert_eq!(surface.phase(), &LoginPhase::Starting);
        assert!(surface
            .began_authorization("ABCD-1234".to_owned(), "https://auth.example/device".to_owned()));
        let body = surface.body(80);
        assert!(body.iter().any(|row| row == "ABCD-1234"), "code is shown verbatim: {body:?}");
        assert!(body
            .iter()
            .any(|row| row == "https://auth.example/device"), "url is shown: {body:?}");

        assert!(surface.exchanging());
        assert_eq!(surface.phase(), &LoginPhase::Exchanging);
        assert!(surface.succeeded(Some("acct-1".to_owned())));
        let body = surface.body(80);
        assert!(body[0].contains("Credentials saved"));
        assert!(body.iter().any(|row| row == "Account: acct-1"));
        assert!(surface.phase().is_terminal());
    }

    #[test]
    fn phases_reject_out_of_order_transitions() {
        let mut surface = surface();
        // Not started yet.
        assert!(!surface.began_authorization("c".into(), "u".into()));
        assert!(!surface.exchanging());
        assert!(surface.begin().is_ok());
        // Already starting.
        assert_eq!(surface.begin(), Err("login already started".to_owned()));
        assert!(!surface.exchanging(), "cannot exchange before consent");
        assert!(surface.began_authorization("c".into(), "u".into()));
        assert!(surface.failed("nope"));
        assert!(!surface.succeeded(None), "terminal phase cannot be overwritten");
        assert!(!surface.failed("again"));
    }

    #[test]
    fn closing_reports_cancelled_and_is_terminal() {
        let mut surface = surface();
        surface.close();
        assert!(!surface.is_open());
        assert_eq!(surface.phase(), &LoginPhase::Cancelled);
        assert!(surface.phase().is_terminal());
        assert_eq!(surface.body(20), vec!["Sign-in cancelled"]);
    }

    #[test]
    fn wide_codes_and_long_errors_are_clipped_to_the_dialog_width() {
        let mut pending = surface();
        pending.begin().unwrap();
        pending.began_authorization(
            "ABCD-1234-EFGH-5678-IJKL".to_owned(),
            "https://auth.openai.com/codex/device?with=a-very-long-query".to_owned(),
        );
        for row in pending.body(10) {
            assert!(row.chars().count() <= 10, "row exceeds width: {row:?}");
        }
        let mut failed = surface();
        failed.begin().unwrap();
        failed.failed("x".repeat(200));
        assert_eq!(failed.body(12)[0].chars().count(), 12);
    }

    #[test]
    fn hints_document_only_bindings_the_surface_handles() {
        let mut surface = surface();
        assert!(surface.hint().contains("enter"));
        assert!(surface.hint().contains("esc"));
        surface.begin().unwrap();
        surface.began_authorization("c".into(), "u".into());
        assert_eq!(surface.hint(), "waiting for approval \u{b7} esc cancel");
        surface.succeeded(None);
        assert_eq!(surface.hint(), "enter close");
    }

    #[test]
    fn remaining_label_is_derived_and_never_negative() {
        assert_eq!(remaining_label(Duration::from_secs(0)), "expired");
        assert_eq!(remaining_label(Duration::from_secs(45)), "45s");
        assert_eq!(remaining_label(Duration::from_secs(125)), "2m 05s");
    }

    #[test]
    fn an_unsupported_provider_never_spawns_a_worker() {
        assert!(LoginRun::start("anthropic").is_err());
        assert!(LoginRun::start("nope").is_err());
    }
}
