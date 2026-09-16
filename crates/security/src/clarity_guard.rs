//! Terse-mode auto-clarity guard (TOOL-023 / CV-SLICE-02).
//!
//! Fail-closed, default-on guard in front of the terse renderer: any set
//! kind flag forces [`Mode::Full`]. Guarded output renders in complete
//! sentences in the caller's language with no self-reference to compression
//! or style. Pure and synchronous: no I/O, no clock, no network, no env,
//! no global state. Time O(n) in response bytes, O(1) extra state beyond
//! the returned string; the caller-supplied repeat history is bounded.
#![forbid(unsafe_code)]

/// Substrings that must never appear in [`Mode::Full`] output
/// (case-insensitive). Guard bypass if present. Seed list; the integrator
/// may only extend it, never weaken it.
pub const BANNED: &[&str] = &["terse", "compressed", "caveman", "mode active"];

/// Bound on caller-supplied repeat history (`RepeatCtx` retains nothing itself).
pub const REPEAT_HISTORY: usize = 3;

/// Render mode selected by [`select_mode`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Terse,
    Full,
}

/// Kind flags for one response. Any flag true forces [`Mode::Full`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Flags {
    pub security: bool,
    pub irreversible: bool,
    pub ordered_steps: bool,
    pub repeat: bool,
}

impl Flags {
    #[must_use]
    pub fn any(&self) -> bool {
        self.security || self.irreversible || self.ordered_steps || self.repeat
    }
}

/// Caller-supplied bounded history of the last [`REPEAT_HISTORY`] question
/// hashes. The guard retains no transcript itself.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepeatCtx {
    hashes: [u64; REPEAT_HISTORY],
    len: usize,
}

impl RepeatCtx {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one question hash, evicting the oldest beyond the bound.
    pub fn push(&mut self, hash: u64) {
        if self.len < REPEAT_HISTORY {
            self.hashes[self.len] = hash;
            self.len += 1;
        } else {
            self.hashes.copy_within(1.., 0);
            self.hashes[REPEAT_HISTORY - 1] = hash;
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn contains(&self, hash: u64) -> bool {
        self.hashes[..self.len].contains(&hash)
    }
}

/// BCP-47 style language tag of the user's dominant language.
/// Full output renders in this language and never falls back to another.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LangTag(String);

impl LangTag {
    #[must_use]
    pub fn new(tag: &str) -> Self {
        Self(tag.trim().to_ascii_lowercase())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Primary subtag (`"vi"` for `"vi"`, `"vi-VN"`, `"vi_VN"`).
    #[must_use]
    pub fn primary(&self) -> &str {
        self.0.split(['-', '_']).next().unwrap_or("").trim()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl Default for LangTag {
    fn default() -> Self {
        Self::new("en")
    }
}

/// Full render context for one response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderCtx {
    pub kind_flags: Flags,
    pub lang: LangTag,
    pub history: RepeatCtx,
}

impl RenderCtx {
    #[must_use]
    pub fn new(
        security: bool,
        irreversible: bool,
        ordered_steps: bool,
        repeat: bool,
        lang: &str,
    ) -> Self {
        Self {
            kind_flags: Flags {
                security,
                irreversible,
                ordered_steps,
                repeat,
            },
            lang: LangTag::new(lang),
            history: RepeatCtx::new(),
        }
    }

    #[must_use]
    pub fn with_history(mut self, history: RepeatCtx) -> Self {
        self.history = history;
        self
    }
}

/// Guard configuration. Default-on: the guard is locked on and there is no
/// setting that disables it while keeping terse output; either opt-out
/// forces [`Mode::Full`] for every response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuardConfig {
    /// Guard locked on. Setting `false` cannot weaken the guard: it forces
    /// `Full` for every response instead of re-enabling terse output.
    pub clarity_guard: bool,
    /// Setting `false` disables terse output entirely (every response `Full`).
    pub terse_enabled: bool,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self {
            clarity_guard: true,
            terse_enabled: true,
        }
    }
}

impl GuardConfig {
    /// Opt out of terse output entirely: every response renders full.
    /// The guard itself stays on.
    #[must_use]
    pub fn terse_opt_out() -> Self {
        Self {
            clarity_guard: true,
            terse_enabled: false,
        }
    }
}

/// Fail-closed mode selection: any flag true yields [`Mode::Full`]; a
/// missing language tag also yields `Full` (never terse on doubt).
#[must_use]
pub fn select_mode(ctx: &RenderCtx) -> Mode {
    if ctx.kind_flags.any() {
        Mode::Full
    } else if ctx.lang.is_empty() {
        Mode::Full
    } else {
        Mode::Terse
    }
}

/// Mode selection under an explicit [`GuardConfig`]. Either opt-out forces
/// `Full`; a disabled guard cannot re-enable terse output.
#[must_use]
pub fn select_mode_cfg(ctx: &RenderCtx, cfg: &GuardConfig) -> Mode {
    if !cfg.clarity_guard || !cfg.terse_enabled {
        Mode::Full
    } else {
        select_mode(ctx)
    }
}

/// Render `body` under the default (guard-on, terse-allowed) config.
#[must_use]
pub fn render(ctx: &RenderCtx, body: &str) -> String {
    render_with_config(ctx, body, &GuardConfig::default())
}

/// Render `body` under an explicit [`GuardConfig`].
#[must_use]
pub fn render_with_config(ctx: &RenderCtx, body: &str, cfg: &GuardConfig) -> String {
    match select_mode_cfg(ctx, cfg) {
        Mode::Full => render_full(ctx, body),
        Mode::Terse => render_terse(body),
    }
}

/// Case-insensitive banned self-reference scan. `true` means guard bypass.
#[must_use]
pub fn contains_banned(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    BANNED.iter().any(|b| lower.contains(b))
}

fn render_full(ctx: &RenderCtx, body: &str) -> String {
    match ctx.lang.primary() {
        "vi" => render_full_vi(&ctx.kind_flags, body),
        "en" => render_full_en(&ctx.kind_flags, body),
        // No fallback to another language: unknown tags get fail-closed
        // structural full rendering built only from the caller's own body.
        _ => render_full_generic(&ctx.kind_flags, body),
    }
}

fn render_full_en(flags: &Flags, body: &str) -> String {
    if flags.ordered_steps {
        let lead = if flags.security {
            "Security review requires the following steps in order."
        } else {
            "Complete the following steps in order."
        };
        let mut out = String::from(lead);
        for (i, step) in steps_of(body, "Review the request.").iter().enumerate() {
            out.push('\n');
            out.push_str(&format!("{}. {}", i + 1, step));
        }
        if flags.irreversible {
            out.push('\n');
            out.push_str("Confirm completion before continuing.");
        }
        out
    } else if flags.irreversible {
        let noun = salient_noun(body);
        format!(
            "Please confirm before continuing.\nThis action requires confirmation and affects {noun}.\nConfirm that you want to proceed with {noun}."
        )
    } else if flags.security {
        let subject = first_line(body).unwrap_or("The request");
        format!(
            "{}. This is a security warning and requires careful review. Do not proceed until the behavior is confirmed.",
            ensure_terminated(subject)
        )
    } else {
        // Repeat or any other fail-closed Full: body as complete sentences
        // plus an explicit review closing.
        let base = sentences_of(body, "Review this response.");
        format!("{base} Review this response carefully before acting on it.")
    }
}

fn render_full_vi(flags: &Flags, body: &str) -> String {
    if flags.ordered_steps {
        let mut out = String::from("Hoan thanh cac buoc sau theo thu tu.");
        for (i, step) in steps_of(body, "Xem xet yeu cau.").iter().enumerate() {
            out.push('\n');
            out.push_str(&format!("{}. {}", i + 1, step));
        }
        if flags.irreversible {
            out.push('\n');
            out.push_str("Hay xac nhan hoan thanh truoc khi tiep tuc.");
        }
        out
    } else if flags.irreversible {
        let noun = salient_noun(body);
        format!(
            "Vui long xac nhan truoc khi tiep tuc.\nHanh dong nay lien quan den {noun} va can xac nhan.\nHay xac nhan ban muon thuc hien voi {noun}."
        )
    } else if flags.security {
        let subject = first_line(body).unwrap_or("Yeu cau");
        format!(
            "{}. Day la canh bao bao mat va can duoc xem xet ky. Khong tiep tuc cho den khi hanh vi duoc xac nhan.",
            ensure_terminated(subject)
        )
    } else {
        let base = sentences_of(body, "Xem xet phan hoi nay.");
        format!("{base} Hay xac nhan noi dung nay truoc khi hanh dong.")
    }
}

/// Fail-closed rendering for unknown language tags: numbered sentences when
/// ordered, otherwise complete sentences, composed only from the caller's
/// body plus language-neutral structure. Never borrows another language.
fn render_full_generic(flags: &Flags, body: &str) -> String {
    if flags.ordered_steps {
        let numbered: Vec<String> = steps_of(body, "Review the request.")
            .iter()
            .enumerate()
            .map(|(i, step)| format!("{}. {}", i + 1, step))
            .collect();
        numbered.join("\n")
    } else {
        sentences_of(body, "Review the request.")
    }
}

/// Compressed form for terse-eligible responses: single-line whitespace
/// collapse. Never applied to flagged content.
fn render_terse(body: &str) -> String {
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn first_line(body: &str) -> Option<&str> {
    body.lines().map(str::trim).find(|l| !l.is_empty())
}

/// Salient action noun: last alphanumeric token of the body, else "request".
fn salient_noun(body: &str) -> String {
    body.split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|t| !t.is_empty())
        .next_back()
        .unwrap_or("request")
        .to_owned()
}

fn ensure_terminated(text: &str) -> String {
    let t = text.trim();
    if t.ends_with(['.', '!', '?']) {
        t.to_owned()
    } else {
        format!("{t}.")
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Body lines as complete numbered-step sentences.
fn steps_of(body: &str, fallback: &str) -> Vec<String> {
    let lines: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return vec![ensure_terminated(fallback)];
    }
    lines
        .iter()
        .map(|l| ensure_terminated(&capitalize(l)))
        .collect()
}

/// Body as complete sentences on one line.
fn sentences_of(body: &str, fallback: &str) -> String {
    let lines: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return ensure_terminated(fallback);
    }
    lines
        .iter()
        .map(|l| ensure_terminated(&capitalize(l)))
        .collect::<Vec<_>>()
        .join(" ")
}
