#![forbid(unsafe_code)]
//! Native command palette and settings state (TUI-006, slice: app types).
//!
//! Pure state only: a searchable registry of real commands (each carries the
//! action the caller executes — never static labels), model/effort selection
//! that changes the next provider request and encodes to text so it survives
//! restart, independent main/child agent effort, and theme selection with a
//! no-color fallback plus a WCAG contrast check helper. No rendering, no IO,
//! no clock, no threads; permission checks stay with the real broker.
//!
//! Std only (`String`, `Vec`, `fmt`); compiles under `rustc --test` with no
//! external crates.
//!
//! Commit: base 5af7884. Evidence: card TUI-006 (T01 searchable palette
//! executes registered commands, T02 model/effort affects next request and
//! survives restart, T03 main/child effort independent, T04 theme contrast +
//! no-color fallback, T05 disabled commands explain constraints and cannot
//! bypass permissions); prior art `crates/cli/src/native_composer.rs`
//! (keymap enums, bounded buffers, `Display` explainers).

use std::fmt;

/// Maximum palette registry entries.
pub const MAX_COMMANDS: usize = 128;
/// Maximum matched results rendered per query.
pub const MAX_RESULTS: usize = 16;
/// Maximum search query length in chars (protects the frame).
pub const MAX_QUERY_CHARS: usize = 256;
/// Maximum model marker length in chars.
pub const MAX_MODEL_CHARS: usize = 256;
/// WCAG AA minimum contrast ratio for normal text.
pub const MIN_CONTRAST: f32 = 4.5;

/// The action a palette entry represents. The caller dispatches this against
/// the real engine — a palette entry is never a static label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandAction {
    OpenComposer,
    NextSession,
    ForkSession,
    OpenSettings,
    OpenModelPicker,
    Quit,
}

/// Why a command cannot run right now (T05: constraints are explained and
/// never bypassed — the palette refuses gated ids, the broker decides).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandGate {
    Available,
    /// Human-only authority — the real broker decides, never the palette.
    HumanApprovalRequired,
    /// Feature disabled in this build/runtime.
    Disabled { reason: String },
}

impl CommandGate {
    /// Whether [`Palette::execute`] will dispatch this command.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// Human-readable reason why the command cannot run (T05 explainer).
    /// Available commands report readiness; disabled ones carry the exact
    /// constraint so the UI can show it instead of failing silently.
    #[must_use]
    pub fn explain(&self) -> String {
        match self {
            Self::Available => "ready to run".to_string(),
            Self::HumanApprovalRequired => {
                "requires human approval; the permission broker must approve before this runs"
                    .to_string()
            }
            Self::Disabled { reason } => format!("unavailable: {reason}"),
        }
    }
}

impl fmt::Display for CommandGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.explain())
    }
}

/// A registered palette command.
#[derive(Clone, Debug)]
pub struct Command {
    pub id: &'static str,
    pub title: String,
    pub action: CommandAction,
    pub gate: CommandGate,
}

/// Effort level for main and child agents (independent settings, T03).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effort {
    Low,
    Medium,
    High,
}

impl Effort {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// Parses a persisted effort marker; `None` on unknown values.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

impl fmt::Display for Effort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Theme choice with an explicit no-color fallback (T04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
    /// No-color mode: render with default terminal colors, no ANSI palette.
    NoColor,
}

impl Theme {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::NoColor => "nocolor",
        }
    }

    /// Parses a persisted theme marker; `None` on unknown values.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "nocolor" => Some(Self::NoColor),
            _ => None,
        }
    }

    /// Whether the theme requires the terminal palette to provide contrast.
    /// `NoColor` always reads on any terminal.
    #[must_use]
    pub const fn requires_palette(self) -> bool {
        !matches!(self, Self::NoColor)
    }

    /// Default foreground for the theme.
    #[must_use]
    pub const fn foreground(self) -> Rgb {
        match self {
            Self::Dark => Rgb::new(0xE6, 0xE6, 0xE6),
            Self::Light => Rgb::new(0x1A, 0x1B, 0x1E),
            // ponytail: NoColor has no palette; marker grey documents the
            // default-terminal assumption. Upgrade to terminal-query when
            // the lane owns host detection.
            Self::NoColor => Rgb::new(0x80, 0x80, 0x80),
        }
    }

    /// Default background for the theme.
    #[must_use]
    pub const fn background(self) -> Rgb {
        match self {
            Self::Dark => Rgb::new(0x1A, 0x1B, 0x1E),
            Self::Light => Rgb::new(0xFA, 0xFA, 0xFA),
            Self::NoColor => Rgb::new(0x00, 0x00, 0x00),
        }
    }

    /// Whether the theme stays readable. `NoColor` trusts the user's own
    /// terminal contrast (always readable by construction); palette themes
    /// must meet [`MIN_CONTRAST`].
    #[must_use]
    pub fn is_readable(self) -> bool {
        match self {
            Self::NoColor => true,
            Self::Dark | Self::Light => meets_contrast(self.foreground(), self.background()),
        }
    }

    /// Explicit fallback when the terminal palette cannot provide contrast.
    #[must_use]
    pub const fn fallback_no_color() -> Self {
        Self::NoColor
    }
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 8-bit sRGB color for the contrast check helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// WCAG relative luminance of the color.
    #[must_use]
    pub fn luminance(self) -> f32 {
        fn linear(byte: u8) -> f32 {
            let c = f32::from(byte) / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }
}

/// WCAG contrast ratio between two colors (1.0 = identical, 21.0 = black/white).
#[must_use]
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f32 {
    let (hi, lo) = if a.luminance() >= b.luminance() {
        (a.luminance(), b.luminance())
    } else {
        (b.luminance(), a.luminance())
    };
    (hi + 0.05) / (lo + 0.05)
}

/// Whether a foreground/background pair meets [`MIN_CONTRAST`] (T04 helper).
#[must_use]
pub fn meets_contrast(fg: Rgb, bg: Rgb) -> bool {
    contrast_ratio(fg, bg) >= MIN_CONTRAST
}

/// Failures for model selection and persistence round trips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionError {
    /// Model marker is empty.
    ModelEmpty,
    /// Model marker exceeds [`MAX_MODEL_CHARS`].
    ModelTooLong { chars: usize, limit: usize },
    /// Persisted text is malformed; nothing was applied.
    BadFormat,
    /// Persisted text has an unknown version marker.
    UnknownVersion(String),
    /// Persisted text has an unknown enum value.
    UnknownValue { key: String, value: String },
}

impl fmt::Display for SelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelEmpty => write!(f, "model marker is empty; pick a model or clear it"),
            Self::ModelTooLong { chars, limit } => {
                write!(f, "model marker of {chars} chars exceeds {limit} char budget")
            }
            Self::BadFormat => write!(f, "saved selection is malformed; keeping current selection"),
            Self::UnknownVersion(v) => write!(f, "saved selection version {v:?} is not supported"),
            Self::UnknownValue { key, value } => {
                write!(f, "saved selection has unknown {key} value {value:?}")
            }
        }
    }
}

impl std::error::Error for SelectionError {}

/// Persisted selection state (encodes to text so it survives restart, T02).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub model: Option<String>,
    pub main_effort: Effort,
    pub child_effort: Effort,
    pub theme: Theme,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            model: None,
            main_effort: Effort::Medium,
            child_effort: Effort::Medium,
            theme: Theme::Dark,
        }
    }
}

/// Encoding version marker for [`Selection::encode`].
const SELECTION_VERSION: &str = "palette-selection-v1";

/// Escapes a model marker for one-line persistence (`\` and newline only).
fn escape_model(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out
}

/// Reverses [`escape_model`]; `None` on a dangling trailing backslash.
fn unescape_model(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                _ => return None,
            }
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

impl Selection {
    /// The model that applies to the next provider request (T02).
    #[must_use]
    pub fn next_request_model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// The effort applying to main-agent requests.
    #[must_use]
    pub fn next_request_effort(&self) -> Effort {
        self.main_effort
    }

    /// The effort applying to child-agent requests (independent, T03).
    #[must_use]
    pub fn child_request_effort(&self) -> Effort {
        self.child_effort
    }

    /// Select the model for the next provider request (T02 marker).
    pub fn set_model(&mut self, model: &str) -> Result<(), SelectionError> {
        if model.is_empty() {
            return Err(SelectionError::ModelEmpty);
        }
        let chars = model.chars().count();
        if chars > MAX_MODEL_CHARS {
            return Err(SelectionError::ModelTooLong {
                chars,
                limit: MAX_MODEL_CHARS,
            });
        }
        self.model = Some(model.to_string());
        Ok(())
    }

    /// Clear the model marker (provider default applies).
    pub fn clear_model(&mut self) {
        self.model = None;
    }

    /// Encode for restart persistence (T02). Line-based, std only:
    /// version, model (empty = none), main/child effort, theme.
    #[must_use]
    pub fn encode(&self) -> String {
        let model = self
            .model
            .as_deref()
            .map_or_else(String::new, escape_model);
        format!(
            "{SELECTION_VERSION}\nmodel={model}\nmain={}\nchild={}\ntheme={}\n",
            self.main_effort.as_str(),
            self.child_effort.as_str(),
            self.theme.as_str(),
        )
    }

    /// Decode persisted text. Strict: rejects malformed input and leaves
    /// the current selection untouched (caller keeps its state on `Err`).
    pub fn decode(raw: &str) -> Result<Self, SelectionError> {
        let mut lines = raw.lines();
        match lines.next() {
            Some(SELECTION_VERSION) => {}
            Some(other) => return Err(SelectionError::UnknownVersion(other.to_string())),
            None => return Err(SelectionError::BadFormat),
        }
        let mut model: Option<Option<String>> = None;
        let mut main: Option<Effort> = None;
        let mut child: Option<Effort> = None;
        let mut theme: Option<Theme> = None;
        for line in lines {
            let (key, value) = line.split_once('=').ok_or(SelectionError::BadFormat)?;
            match key {
                "model" => {
                    if model.is_some() {
                        return Err(SelectionError::BadFormat);
                    }
                    model = Some(if value.is_empty() {
                        None
                    } else {
                        let decoded =
                            unescape_model(value).ok_or(SelectionError::BadFormat)?;
                        if decoded.chars().count() > MAX_MODEL_CHARS {
                            return Err(SelectionError::ModelTooLong {
                                chars: decoded.chars().count(),
                                limit: MAX_MODEL_CHARS,
                            });
                        }
                        Some(decoded)
                    });
                }
                "main" => {
                    main = Some(Effort::parse(value).ok_or_else(|| {
                        SelectionError::UnknownValue {
                            key: key.to_string(),
                            value: value.to_string(),
                        }
                    })?);
                }
                "child" => {
                    child = Some(Effort::parse(value).ok_or_else(|| {
                        SelectionError::UnknownValue {
                            key: key.to_string(),
                            value: value.to_string(),
                        }
                    })?);
                }
                "theme" => {
                    theme = Some(Theme::parse(value).ok_or_else(|| {
                        SelectionError::UnknownValue {
                            key: key.to_string(),
                            value: value.to_string(),
                        }
                    })?);
                }
                _ => return Err(SelectionError::BadFormat),
            }
        }
        match (model, main, child, theme) {
            (Some(model), Some(main_effort), Some(child_effort), Some(theme)) => Ok(Self {
                model,
                main_effort,
                child_effort,
                theme,
            }),
            _ => Err(SelectionError::BadFormat),
        }
    }
}

/// The palette state: registry, query, selection.
#[derive(Debug, Default)]
pub struct Palette {
    commands: Vec<Command>,
    query: String,
    selection: Selection,
}

impl Palette {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a command. Returns `false` (entry dropped) past
    /// [`MAX_COMMANDS`]; nothing is evicted, order is stable.
    pub fn register(&mut self, command: Command) -> bool {
        if self.commands.len() >= MAX_COMMANDS {
            return false;
        }
        self.commands.push(command);
        true
    }

    /// Number of registered commands.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Current query.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Set the search query (bounded to [`MAX_QUERY_CHARS`] to protect the frame).
    pub fn set_query(&mut self, query: &str) {
        self.query = query.chars().take(MAX_QUERY_CHARS).collect();
    }

    /// Search results: case-insensitive substring match on title/id, capped
    /// at [`MAX_RESULTS`]. Unavailable commands are included (with their
    /// gate) so the UI can explain why they are disabled (T05).
    #[must_use]
    pub fn results(&self) -> Vec<&Command> {
        let needle = self.query.to_lowercase();
        self.commands
            .iter()
            .filter(|c| {
                needle.is_empty()
                    || c.title.to_lowercase().contains(&needle)
                    || c.id.to_lowercase().contains(&needle)
            })
            .take(MAX_RESULTS)
            .collect()
    }

    /// Resolve a command id to its executable action, refusing gated
    /// commands with their explanation (T01/T05). Unknown ids report a
    /// disabled gate — never an executable action.
    pub fn execute(&self, id: &str) -> Result<CommandAction, CommandGate> {
        let Some(command) = self.commands.iter().find(|c| c.id == id) else {
            return Err(CommandGate::Disabled {
                reason: "unknown command".to_string(),
            });
        };
        match &command.gate {
            CommandGate::Available => Ok(command.action.clone()),
            gate => Err(gate.clone()),
        }
    }

    /// Current persisted selection.
    #[must_use]
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    /// Mutable selection (model/effort/theme changes).
    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(
        id: &'static str,
        title: &str,
        action: CommandAction,
        gate: CommandGate,
    ) -> Command {
        Command {
            id,
            title: title.to_string(),
            action,
            gate,
        }
    }

    fn sample() -> Palette {
        let mut p = Palette::new();
        p.register(cmd(
            "session.next",
            "Next session",
            CommandAction::NextSession,
            CommandGate::Available,
        ));
        p.register(cmd(
            "app.quit",
            "Quit",
            CommandAction::Quit,
            CommandGate::Available,
        ));
        p.register(cmd(
            "composer.open",
            "Open composer",
            CommandAction::OpenComposer,
            CommandGate::Available,
        ));
        p
    }

    #[test]
    fn search_filters_by_title_and_id() {
        let mut p = sample();
        p.set_query("quit");
        let results = p.results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "app.quit");
        // Case-insensitive id match.
        p.set_query("SESSION");
        let results = p.results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "session.next");
        // Empty query lists everything registered.
        p.set_query("");
        assert_eq!(p.results().len(), 3);
        // No match, no results (never an error, never a fallback entry).
        p.set_query("zzz-no-such-command");
        assert!(p.results().is_empty());
    }

    #[test]
    fn execute_returns_registered_action_not_a_label() {
        let p = sample();
        assert_eq!(p.execute("composer.open"), Ok(CommandAction::OpenComposer));
        assert_eq!(p.execute("session.next"), Ok(CommandAction::NextSession));
        // Unknown ids are refused, never dispatched.
        assert!(p.execute("missing").is_err());
    }

    #[test]
    fn disabled_commands_explain_constraint_and_cannot_bypass() {
        let mut p = Palette::new();
        p.register(cmd(
            "remote.pair",
            "Pair device",
            CommandAction::OpenSettings,
            CommandGate::Disabled {
                reason: "remote mode is off".to_string(),
            },
        ));
        p.register(cmd(
            "billing.change",
            "Change billing",
            CommandAction::OpenSettings,
            CommandGate::HumanApprovalRequired,
        ));
        let err = p.execute("remote.pair").expect_err("gated command must not dispatch");
        assert_eq!(
            err,
            CommandGate::Disabled {
                reason: "remote mode is off".to_string()
            }
        );
        assert!(
            err.explain().contains("remote mode is off"),
            "explainer must carry the constraint, got: {err}"
        );
        let err = p
            .execute("billing.change")
            .expect_err("human-gated command must not dispatch");
        assert_eq!(err, CommandGate::HumanApprovalRequired);
        assert!(err.explain().contains("human approval"));
        // Disabled entries still surface in search so the UI can explain them.
        p.set_query("pair");
        let results = p.results();
        assert_eq!(results.len(), 1);
        assert!(!results[0].gate.is_available());
    }

    #[test]
    fn model_selection_persists_marker_through_encode() {
        let mut p = Palette::new();
        p.selection_mut().set_model("claude-sonnet-4").unwrap();
        p.selection_mut().main_effort = Effort::High;
        p.selection_mut().theme = Theme::Light;
        // Encode -> decode round trip (restart survival, T02).
        let saved = p.selection().encode();
        let restored = Selection::decode(&saved).unwrap();
        assert_eq!(restored.next_request_model(), Some("claude-sonnet-4"));
        assert_eq!(restored.next_request_effort(), Effort::High);
        assert_eq!(restored.theme, Theme::Light);
        assert_eq!(restored, *p.selection());
        // Tricky markers (separators, newlines, backslashes) round-trip.
        p.selection_mut().set_model("a;b\nc\\d=").unwrap();
        let saved = p.selection().encode();
        let restored = Selection::decode(&saved).unwrap();
        assert_eq!(restored.next_request_model(), Some("a;b\nc\\d="));
        // Empty marker rejected; overlong rejected; cleared model persists as none.
        assert_eq!(
            p.selection_mut().set_model(""),
            Err(SelectionError::ModelEmpty)
        );
        p.selection_mut().clear_model();
        assert_eq!(p.selection().next_request_model(), None);
        let restored = Selection::decode(&p.selection().encode()).unwrap();
        assert_eq!(restored.next_request_model(), None);
    }

    #[test]
    fn main_and_child_effort_stay_independent() {
        let mut p = Palette::new();
        p.selection_mut().main_effort = Effort::High;
        p.selection_mut().child_effort = Effort::Low;
        let sel = p.selection();
        assert_eq!(sel.next_request_effort(), Effort::High);
        assert_eq!(sel.child_request_effort(), Effort::Low);
        // Independence survives the restart round trip.
        let restored = Selection::decode(&sel.encode()).unwrap();
        assert_eq!(restored.next_request_effort(), Effort::High);
        assert_eq!(restored.child_request_effort(), Effort::Low);
    }

    #[test]
    fn no_color_fallback_stays_readable() {
        assert!(Theme::Dark.requires_palette());
        assert!(Theme::Light.requires_palette());
        assert!(!Theme::NoColor.requires_palette());
        assert!(Theme::NoColor.is_readable());
        assert!(Theme::Dark.is_readable());
        assert!(Theme::Light.is_readable());
        assert_eq!(Theme::fallback_no_color(), Theme::NoColor);
    }

    #[test]
    fn contrast_helper_accepts_readable_pairs_rejects_flat() {
        let black = Rgb::new(0, 0, 0);
        let white = Rgb::new(255, 255, 255);
        assert!(contrast_ratio(black, white) > 20.0);
        assert!(meets_contrast(black, white));
        assert!(!meets_contrast(black, black));
        assert!(!meets_contrast(Rgb::new(0x77, 0x77, 0x77), Rgb::new(0x88, 0x88, 0x88)));
    }

    #[test]
    fn registry_and_results_stay_bounded() {
        let mut p = Palette::new();
        for i in 0..(MAX_COMMANDS + 10) {
            let admitted = p.register(Command {
                id: "x",
                title: format!("Command {i}"),
                action: CommandAction::OpenComposer,
                gate: CommandGate::Available,
            });
            assert_eq!(admitted, i < MAX_COMMANDS);
        }
        assert_eq!(p.len(), MAX_COMMANDS);
        assert_eq!(p.results().len(), MAX_RESULTS);
        // Overlong queries are truncated, never unbounded.
        p.set_query(&"q".repeat(MAX_QUERY_CHARS + 50));
        assert_eq!(p.query().chars().count(), MAX_QUERY_CHARS);
    }

    #[test]
    fn decode_rejects_garbage_without_touching_state() {
        assert_eq!(Selection::decode(""), Err(SelectionError::BadFormat));
        assert!(Selection::decode("bogus-version\nmodel=\n").is_err());
        let good = Selection::default().encode();
        assert!(Selection::decode(&good).is_ok());
        assert_eq!(
            Selection::decode(&good.replace("main=medium", "main=ultra")),
            Err(SelectionError::UnknownValue {
                key: "main".to_string(),
                value: "ultra".to_string(),
            })
        );
        assert_eq!(
            Selection::decode(&good.replace("theme=dark", "theme=neon")),
            Err(SelectionError::UnknownValue {
                key: "theme".to_string(),
                value: "neon".to_string(),
            })
        );
        // Missing keys and unknown keys both fail strict.
        assert_eq!(
            Selection::decode("palette-selection-v1\nmodel=\nmain=low\n"),
            Err(SelectionError::BadFormat)
        );
    }
}
