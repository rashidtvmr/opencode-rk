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
//! Dialog parity (DIALOG-01..DIALOG-17) is modelled by the [`dialog`],
//! [`select`] and [`pickers`] submodules against the pinned upstream
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22`:
//! `packages/tui/src/ui/dialog.tsx:24-34,61-84,139-180,209-261,320-419,472-479`
//! (sizes, shell, footer band, focus restore, escape/ctrl+c, keyed render),
//! `ui/dialog-select.tsx:96-123,210-347,386-470,472-577,654,736-801,821-972`
//! (filtering, grouping, viewport, selection identity, keys, empty views,
//! row geometry, truncation, gutter),
//! `ui/select-controller.ts:1-12`, `ui/dialog-prompt.tsx:63-73,120-184`,
//! `ui/dialog-confirm.tsx:40-104`, `component/command-palette.tsx:14-74`,
//! `component/dialog-config.tsx:27-447`, `component/dialog-experiments.tsx:30-96`,
//! `component/dialog-model.tsx`, `model-options.tsx`,
//! `component/dialog-session-list.tsx:30-135,142-296`,
//! `component/dialog-mcp.tsx`, `component/dialog-integration.tsx`,
//! `component/dialog-theme-list.tsx`, `component/dialog-stash.tsx`,
//! `component/dialog-workspaces.tsx`, `component/dialog-workspace-file-changes.tsx`,
//! `component/dialog-open.tsx:55-128` and `util/locale.ts:45-84`. Each item
//! below repeats its exact `path:line` evidence. DIALOG-18/19 belong to the
//! diff-view (P15) and overlay/toast (P19) packages and are not modelled here.
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
    Disabled {
        reason: String,
    },
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
                write!(
                    f,
                    "model marker of {chars} chars exceeds {limit} char budget"
                )
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
        let model = self.model.as_deref().map_or_else(String::new, escape_model);
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
                        let decoded = unescape_model(value).ok_or(SelectionError::BadFormat)?;
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
                    main =
                        Some(
                            Effort::parse(value).ok_or_else(|| SelectionError::UnknownValue {
                                key: key.to_string(),
                                value: value.to_string(),
                            })?,
                        );
                }
                "child" => {
                    child =
                        Some(
                            Effort::parse(value).ok_or_else(|| SelectionError::UnknownValue {
                                key: key.to_string(),
                                value: value.to_string(),
                            })?,
                        );
                }
                "theme" => {
                    theme =
                        Some(
                            Theme::parse(value).ok_or_else(|| SelectionError::UnknownValue {
                                key: key.to_string(),
                                value: value.to_string(),
                            })?,
                        );
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

/// Shared dialog shell geometry and modal replacement stack.
///
/// Upstream evidence (pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`):
/// - `packages/tui/src/ui/dialog.tsx:24-34`: `DialogSize` "medium" 60,
///   "large" 88, "xlarge" 116; `DIALOG_INSET` 3; `DIALOG_LIST_INSET` 4.
/// - `dialog.tsx:61-84`: full-screen backdrop, `paddingTop` = height / 4
///   unless `centered`, panel width capped at terminal width - 2.
/// - `dialog.tsx:139-171`: `DialogLayout` gap 1, bottom padding 1, side
///   padding `inset`, body bottom padding per `footerGap`.
/// - `dialog.tsx:174-180`: `DialogBleed` negative `DIALOG_INSET` margins.
/// - `dialog.tsx:209-261`: footer is a band; `width < 60` stacks it in a
///   column; band inset is `DIALOG_INSET + inset - 1`.
/// - `dialog.tsx:320-343,399-419`: first dialog captures focus, `replace`
///   clears the stack, close only refocuses a still-mounted target.
/// - `dialog.tsx:472-479`: keyed render keeps one dialog entry, so reactive
///   updates do not remount it.
/// - `dialog.tsx:345-386`: Escape / Ctrl+C dispatch order (selection, then
///   focused editor text, then close).
pub mod dialog {
    /// `DIALOG_INSET` — shared dialog side padding (`dialog.tsx:26`).
    pub const DIALOG_INSET: u16 = 3;
    /// `DIALOG_LIST_INSET` — rows reserve one gutter cell (`dialog.tsx:28`).
    pub const DIALOG_LIST_INSET: u16 = 4;
    /// `DialogFooter` stacks its groups below this terminal width
    /// (`dialog.tsx:214`).
    pub const FOOTER_NARROW_WIDTH: u16 = 60;
    /// `DialogLayout` gap between header, body and footer (`dialog.tsx:157`).
    pub const LAYOUT_GAP: u16 = 1;
    /// Row title padding inside a select row (`dialog-select.tsx:944`).
    pub const ROW_TITLE_PADDING: u16 = 3;

    /// `DialogSize` (`dialog.tsx:24`).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum DialogSize {
        #[default]
        Medium,
        Large,
        XLarge,
    }

    impl DialogSize {
        /// Base dialog width in cells (`dialog.tsx:30-34`).
        #[must_use]
        pub const fn width(self) -> u16 {
            match self {
                Self::Medium => 60,
                Self::Large => 88,
                Self::XLarge => 116,
            }
        }
    }

    /// Panel width: the size width capped at `terminal - 2`
    /// (`dialog.tsx:80-81`). Terminals narrower than two cells yield 0.
    #[must_use]
    pub fn dialog_width(size: DialogSize, terminal_width: u16) -> u16 {
        size.width().min(terminal_width.saturating_sub(2))
    }

    /// Backdrop top padding: 0 when centered, otherwise the integer cell
    /// equivalent of `height / 4` (`dialog.tsx:64-67`). OpenTUI receives a
    /// fractional number and floors it when painting whole cells, so 59 rows
    /// start the panel at row 14 and 60 rows at row 15.
    #[must_use]
    pub fn dialog_top(terminal_height: u16, centered: bool) -> u16 {
        if centered {
            0
        } else {
            terminal_height / 4
        }
    }

    /// `dialogSelectContentWidth`: scroll padding, row padding, gutter,
    /// title padding and the separating gap (`dialog-select.tsx:125-128`).
    #[must_use]
    pub fn content_width(dialog_width: u16) -> u16 {
        dialog_width.saturating_sub(DIALOG_LIST_INSET * 2 + 4)
    }

    /// Horizontal padding of one select row. `marked` is
    /// `current || gutter`; new designs run rows edge to edge
    /// (`dialog-select.tsx:915-920`).
    #[must_use]
    pub fn row_padding(marked: bool, new_designs: bool) -> (u16, u16) {
        let left = if marked {
            DIALOG_LIST_INSET - 3
        } else {
            DIALOG_LIST_INSET - 1
        };
        let right = if new_designs {
            DIALOG_LIST_INSET
        } else {
            DIALOG_LIST_INSET - 1
        };
        (left, right)
    }

    /// Category heading and details use the legacy 1-cell scroll pad
    /// (`dialog-select.tsx:875,983`).
    #[must_use]
    pub fn list_body_inset(new_designs: bool) -> u16 {
        if new_designs {
            DIALOG_LIST_INSET
        } else {
            DIALOG_LIST_INSET - 1
        }
    }

    /// Body bottom padding under new designs: `footerGap - 1`, default gap 1
    /// (`dialog.tsx:159`). The select dialog passes 2, the prompt passes 2.
    #[must_use]
    pub fn body_bottom_padding(new_designs: bool, has_footer: bool, footer_gap: Option<u16>) -> u16 {
        if new_designs && has_footer {
            footer_gap.unwrap_or(1).saturating_sub(1)
        } else {
            0
        }
    }

    /// A raised footer band gives one padded cell back on each side
    /// (`dialog.tsx:227-228`).
    #[must_use]
    pub fn footer_band_inset(inset: u16) -> u16 {
        DIALOG_INSET + inset - 1
    }

    /// `DialogFooter` switches to a column when the terminal is narrower
    /// than 60 cells (`dialog.tsx:214`).
    #[must_use]
    pub const fn footer_is_narrow(terminal_width: u16) -> bool {
        terminal_width < FOOTER_NARROW_WIDTH
    }

    /// JS `Locale.truncate` (`util/locale.ts:45-48`): keep `len - 1`
    /// characters and append `…`. Upstream counts UTF-16 units; this helper
    /// counts scalar values, which is identical for the ASCII titles the
    /// dialogs use.
    #[must_use]
    pub fn truncate(text: &str, len: usize) -> String {
        let count = text.chars().count();
        if count <= len {
            return text.to_owned();
        }
        let mut out: String = text.chars().take(len.saturating_sub(1)).collect();
        out.push('…');
        out
    }

    /// JS `Locale.truncateLeft` (`util/locale.ts:71-74`).
    #[must_use]
    pub fn truncate_left(text: &str, len: usize) -> String {
        let count = text.chars().count();
        if count <= len {
            return text.to_owned();
        }
        let keep = len.saturating_sub(1);
        let tail: String = text.chars().skip(count - keep).collect();
        format!("…{tail}")
    }

    /// JS `Locale.truncateMiddle` (`util/locale.ts:76-84`).
    #[must_use]
    pub fn truncate_middle(text: &str, max: usize) -> String {
        let count = text.chars().count();
        if count <= max {
            return text.to_owned();
        }
        let budget = max.saturating_sub(1);
        let keep_start = (budget + 1) / 2;
        let keep_end = budget / 2;
        let start: String = text.chars().take(keep_start).collect();
        let end: String = text.chars().skip(count - keep_end).collect();
        format!("{start}…{end}")
    }

    /// Result of a modal key that the dialog stack consumed.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum KeyOutcome {
        /// A text selection was cleared; the dialog stays open.
        ClearedSelection,
        /// The focused editor held text and was cleared; the dialog stays.
        ClearedEditor,
        /// The dialog closed and the saved focus target may be restored.
        Closed,
        /// Nothing handled the key.
        Ignored,
    }

    /// Whether the panel's mouse-up must stop propagation
    /// (`dialog.tsx:72-79`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum PanelMouseUp {
        /// A selection release bubbles to the copy-on-select handler.
        Bubbles,
        /// No selection: consume the event so the backdrop stays inert.
        Stop,
    }

    /// One dialog entry. `replace` swaps the element and remounts once;
    /// reactive store updates (`set_size`, `set_centered`) must not.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct DialogEntry {
        pub id: String,
        pub size: DialogSize,
    }

    /// The single-dialog modal stack from `DialogProvider`.
    #[derive(Clone, Debug, Default)]
    pub struct DialogStack {
        entry: Option<DialogEntry>,
        mounts: u64,
        focus: Option<String>,
        centered: bool,
        dismiss: bool,
    }

    impl DialogStack {
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        #[must_use]
        pub fn is_open(&self) -> bool {
            self.entry.is_some()
        }

        #[must_use]
        pub fn entry(&self) -> Option<&DialogEntry> {
            self.entry.as_ref()
        }

        /// How many times the dialog element was mounted. Reactive updates
        /// leave it unchanged; `replace` increments it exactly once.
        #[must_use]
        pub fn mounts(&self) -> u64 {
            self.mounts
        }

        #[must_use]
        pub fn is_centered(&self) -> bool {
            self.centered
        }

        /// `dialog.replace(...)` (`dialog.tsx:400-419`): remember the current
        /// focus target only when the stack was empty, close every previous
        /// entry, and mount the replacement with the requested size.
        pub fn replace(&mut self, id: &str, size: DialogSize, focus_target: Option<&str>) {
            if self.entry.is_none() {
                self.focus = focus_target.map(str::to_owned);
            }
            self.entry = Some(DialogEntry {
                id: id.to_owned(),
                size,
            });
            self.centered = false;
            self.mounts += 1;
        }

        /// Reactive `setSize` — must not remount (`dialog.tsx:432-434`).
        pub fn set_size(&mut self, size: DialogSize) {
            if let Some(entry) = &mut self.entry {
                entry.size = size;
            }
        }

        /// Reactive `setCentered` (`dialog.tsx:435-437`).
        pub fn set_centered(&mut self, centered: bool) {
            self.centered = centered;
        }

        /// `dialog.clear()` (`dialog.tsx:389-399`). Returns the focus target
        /// only when it is still mounted; a destroyed renderable is dropped
        /// instead of being refocused (`dialog.tsx:326-343`).
        pub fn close(&mut self, surviving: impl Fn(&str) -> bool) -> Option<String> {
            self.entry = None;
            self.centered = false;
            let focus = self.focus.take();
            focus.filter(|id| surviving(id))
        }

        /// Escape (`dialog.tsx:348-363`): a live selection is cleared first;
        /// otherwise the dialog closes.
        pub fn escape(
            &mut self,
            has_selection: bool,
            surviving: impl Fn(&str) -> bool,
        ) -> KeyOutcome {
            if !self.is_open() {
                return KeyOutcome::Ignored;
            }
            if has_selection {
                return KeyOutcome::ClearedSelection;
            }
            self.close(surviving);
            KeyOutcome::Closed
        }

        /// Ctrl+C (`dialog.tsx:364-384`): selection, then focused editor text,
        /// then close.
        pub fn ctrl_c(
            &mut self,
            has_selection: bool,
            editor_text: Option<&str>,
            surviving: impl Fn(&str) -> bool,
        ) -> KeyOutcome {
            if !self.is_open() {
                return KeyOutcome::Ignored;
            }
            if has_selection {
                return KeyOutcome::ClearedSelection;
            }
            if editor_text.is_some_and(|text| !text.is_empty()) {
                return KeyOutcome::ClearedEditor;
            }
            self.close(surviving);
            KeyOutcome::Closed
        }

        /// Backdrop mouse-down records whether a selection exists; only then
        /// does the following release skip the close (`dialog.tsx:51-58`).
        pub fn backdrop_down(&mut self, has_selection: bool) {
            self.dismiss = has_selection;
        }

        /// Backdrop mouse-up; returns whether the dialog closed.
        pub fn backdrop_up(&mut self, surviving: impl Fn(&str) -> bool) -> bool {
            if self.dismiss {
                self.dismiss = false;
                return false;
            }
            self.close(surviving);
            true
        }

        /// Panel mouse-up (`dialog.tsx:72-79`): a selection release bubbles
        /// and must not close the dialog.
        pub fn panel_up(&mut self, selection_text: Option<&str>) -> PanelMouseUp {
            if selection_text.is_some_and(|text| !text.is_empty()) {
                return PanelMouseUp::Bubbles;
            }
            self.dismiss = false;
            PanelMouseUp::Stop
        }
    }
}

/// `DialogSelect` controller/state (pinned upstream
/// `packages/tui/src/ui/dialog-select.tsx`, `ui/select-controller.ts`,
/// `ui/dialog-prompt.tsx`, `ui/dialog-confirm.tsx`).
///
/// Pure state: no rendering, IO, clock or threads. Callers own the frame
/// painter, async fetches and permission broker; this module owns filtering,
/// grouping, selection identity, wrapping/page/section movement, viewport
/// budgeting, experiment gates, prompt/confirm keys and pointer routing.
pub mod select {
    use std::cmp::Ordering;

    /// `dialog.select.page_up` / `page_down` move by ten options
    /// (`dialog-select.tsx:494-509`).
    pub const PAGE_STEP: isize = 10;
    /// The list limit is `floor(terminalHeight / 2) - 6`
    /// (`dialog-select.tsx:262`).
    pub const VIEWPORT_DIVISOR: usize = 2;
    pub const VIEWPORT_HEADROOM: usize = 6;
    /// `DialogSelect` default `filterThreshold` is unset, i.e. 0; the command
    /// palette and settings override it with 0.7 (`command-palette.tsx:71`,
    /// `dialog-config.tsx:435`).
    pub const DEFAULT_FILTER_THRESHOLD: f32 = 0.0;

    /// Selection movement policy (`select-controller.ts:5-12`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum MovePolicy {
        Clamp,
        Wrap,
    }

    /// `reconcileSelection`: `max(0, min(count - 1, selected))`
    /// (`select-controller.ts:1-3`).
    #[must_use]
    pub fn reconcile_selection(selected: usize, count: usize) -> usize {
        if count == 0 {
            0
        } else {
            selected.min(count - 1)
        }
    }

    /// `moveSelection` (`select-controller.ts:5-12`).
    #[must_use]
    pub fn move_selection(
        selected: usize,
        count: usize,
        delta: isize,
        policy: MovePolicy,
    ) -> usize {
        if count == 0 {
            return 0;
        }
        let next = selected as isize + delta;
        match policy {
            MovePolicy::Clamp => {
                if next < 0 {
                    0
                } else {
                    reconcile_selection(next as usize, count)
                }
            }
            MovePolicy::Wrap => {
                if next < 0 {
                    count - 1
                } else if next >= count as isize {
                    0
                } else {
                    next as usize
                }
            }
        }
    }

    /// List limit `floor(terminalHeight / 2) - 6` (`dialog-select.tsx:262`).
    /// Upstream can compute a negative limit for absurdly short terminals;
    /// a negative row budget paints no rows, so this saturates at zero.
    #[must_use]
    pub const fn viewport_limit(terminal_height: usize) -> usize {
        (terminal_height / VIEWPORT_DIVISOR).saturating_sub(VIEWPORT_HEADROOM)
    }

    /// Which empty view renders (`dialog-select.tsx:736-753`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum EmptyState {
        None,
        /// `emptyView` / "No items available".
        Empty,
        /// `noMatchView` / "No results found".
        NoMatch,
    }

    /// One `DialogSelectOption` (`dialog-select.tsx:96-123`). `id` is the
    /// selection identity (`isDeepEqual` over `value` in upstream).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SelectOption {
        pub id: String,
        pub title: String,
        pub category: Option<String>,
        pub description: Option<String>,
        /// Supplemental fuzzy-search text (`searchText`).
        pub search_text: Option<String>,
        /// Right-aligned footer shown while the list is *grouped*
        /// (`footer`, `dialog-select.tsx:102,807`).
        pub footer: Option<String>,
        /// Right-aligned footer shown while the list is *flattened*, i.e. a
        /// search is active (`searchFooter`, `dialog-select.tsx:102,807`).
        pub search_footer: Option<String>,
        /// Extra rows rendered under the title (`details`).
        pub details: Vec<String>,
        /// Selected-detail region (`detail`), new-designs only.
        pub detail: Option<String>,
        /// Disabled options are removed before filtering
        /// (`dialog-select.tsx:211-217`).
        pub disabled: bool,
    }

    impl SelectOption {
        #[must_use]
        pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                title: title.into(),
                category: None,
                description: None,
                search_text: None,
                footer: None,
                search_footer: None,
                details: Vec::new(),
                detail: None,
                disabled: false,
            }
        }

        #[must_use]
        pub fn with_category(mut self, category: impl Into<String>) -> Self {
            self.category = Some(category.into());
            self
        }

        #[must_use]
        pub fn with_description(mut self, description: impl Into<String>) -> Self {
            self.description = Some(description.into());
            self
        }

        #[must_use]
        pub fn with_search_text(mut self, search_text: impl Into<String>) -> Self {
            self.search_text = Some(search_text.into());
            self
        }

        #[must_use]
        pub fn with_footer(mut self, footer: impl Into<String>) -> Self {
            self.footer = Some(footer.into());
            self
        }

        #[must_use]
        pub fn with_search_footer(mut self, search_footer: impl Into<String>) -> Self {
            self.search_footer = Some(search_footer.into());
            self
        }

        #[must_use]
        pub fn with_details(mut self, details: Vec<String>) -> Self {
            self.details = details;
            self
        }

        #[must_use]
        pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
            self.detail = Some(detail.into());
            self
        }

        #[must_use]
        pub fn disabled(mut self) -> Self {
            self.disabled = true;
            self
        }

        /// `description !== category` guard on the row (`dialog-select.tsx:813`).
        #[must_use]
        pub fn row_description(&self) -> Option<&str> {
            match &self.description {
                Some(description) if Some(description.as_str()) != self.category.as_deref() => {
                    Some(description)
                }
                _ => None,
            }
        }

        /// The row's right-aligned footer (`dialog-select.tsx:807`): while the
        /// list is flattened (a search is active) the option's own
        /// `searchFooter` wins, falling back to its category and then its
        /// grouped `footer`; grouped lists show only `footer`.
        #[must_use]
        pub fn row_footer(&self, flattened: bool) -> Option<&str> {
            if flattened {
                self.search_footer
                    .as_deref()
                    .or(self.category.as_deref())
                    .or(self.footer.as_deref())
            } else {
                self.footer.as_deref()
            }
        }
    }

    /// Independent experiment gates (`dialog-select.tsx:263-287,654,875-932`).
    /// No switch may enable another.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct SelectExperiment {
        /// Give the last row to a `…` indicator while more content remains.
        pub ellipsis_row: bool,
        /// Reserve the selected-detail region.
        pub detail_reservation: bool,
        /// Replace the title with the search input in the header row.
        pub search_in_title: bool,
        /// Use the `▪` current marker and `┃` active gutter.
        pub new_markers: bool,
    }

    impl SelectExperiment {
        #[must_use]
        pub const fn legacy() -> Self {
            Self {
                ellipsis_row: false,
                detail_reservation: false,
                search_in_title: false,
                new_markers: false,
            }
        }

        #[must_use]
        pub const fn new_designs() -> Self {
            Self {
                ellipsis_row: true,
                detail_reservation: true,
                search_in_title: true,
                new_markers: true,
            }
        }
    }

    /// Current-choice marker: `●` normally, `▪` with the marker experiment
    /// (`dialog-select.tsx:930-934`).
    #[must_use]
    pub const fn current_marker(experiment: SelectExperiment) -> &'static str {
        if experiment.new_markers {
            "▪"
        } else {
            "●"
        }
    }

    /// Active gutter cell: `┃` only with the marker experiment
    /// (`dialog-select.tsx:964-972`).
    #[must_use]
    pub const fn active_gutter(experiment: SelectExperiment, active: bool) -> &'static str {
        if experiment.new_markers && active {
            "┃"
        } else {
            " "
        }
    }

    /// Whether the search input replaces the title (`dialog-select.tsx:654`).
    #[must_use]
    pub const fn search_in_title(experiment: SelectExperiment, render_filter: bool) -> bool {
        experiment.search_in_title && render_filter
    }

    /// Case-insensitive fuzzy subsequence quality in `0.0..=1.0`, normalized
    /// per field so the palette's 0.7 threshold is comparable. Adjacent and
    /// word-boundary matches score higher. This models `fuzzysort` (not
    /// vendored) over the same three keys.
    fn fuzzy_quality(query: &[char], value: &str) -> Option<f32> {
        if query.is_empty() {
            return Some(1.0);
        }
        let haystack: Vec<char> = value.to_lowercase().chars().collect();
        let mut cursor = 0usize;
        let mut previous: Option<usize> = None;
        let mut score = 0.0f32;
        for &needle in query {
            let position = haystack.get(cursor..)?.iter().position(|c| *c == needle)? + cursor;
            score += 1.0;
            if position == 0 || !haystack[position - 1].is_alphanumeric() {
                score += 0.5;
            }
            if previous.is_some_and(|index| index + 1 == position) {
                score += 0.5;
            }
            previous = Some(position);
            cursor = position + 1;
        }
        let maximum = query.len() as f32 * 2.0;
        Some((score / maximum).clamp(0.0, 1.0))
    }

    /// A drill-down snapshot: filter text plus highlighted identity
    /// (`dialog-open.tsx:118-122`).
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct SelectSnapshot {
        pub filter: String,
        pub selected: Option<String>,
    }

    /// `DialogSelect` controller. Selection is stored as an identity and
    /// reconciled against the current flat list.
    #[derive(Clone, Debug)]
    pub struct SelectDialog {
        options: Vec<SelectOption>,
        filter: String,
        selected: usize,
        selected_id: Option<String>,
        current: Option<String>,
        focus_current: bool,
        preserve_selection: bool,
        locked: bool,
        flat: bool,
        custom_search: bool,
        render_filter: bool,
        skip_filter: bool,
        filter_threshold: f32,
        experiment: SelectExperiment,
    }

    impl Default for SelectDialog {
        fn default() -> Self {
            Self {
                options: Vec::new(),
                filter: String::new(),
                selected: 0,
                selected_id: None,
                current: None,
                focus_current: true,
                preserve_selection: false,
                locked: false,
                flat: false,
                custom_search: false,
                render_filter: true,
                skip_filter: false,
                filter_threshold: DEFAULT_FILTER_THRESHOLD,
                experiment: SelectExperiment::legacy(),
            }
        }
    }

    impl SelectDialog {
        #[must_use]
        pub fn new(options: Vec<SelectOption>) -> Self {
            let mut dialog = Self::default();
            dialog.set_options(options);
            dialog
        }

        #[must_use]
        pub fn with_experiment(mut self, experiment: SelectExperiment) -> Self {
            self.experiment = experiment;
            self
        }

        #[must_use]
        pub fn with_threshold(mut self, threshold: f32) -> Self {
            self.filter_threshold = threshold.clamp(0.0, 1.0);
            self
        }

        #[must_use]
        pub fn with_flat(mut self, flat: bool) -> Self {
            self.flat = flat;
            self
        }

        /// Custom `search` prop: the caller supplies already-filtered options
        /// and the list is always flattened (`dialog-select.tsx:211,233`).
        #[must_use]
        pub fn with_custom_search(mut self, custom: bool) -> Self {
            self.custom_search = custom;
            self
        }

        #[must_use]
        pub fn experiment(&self) -> SelectExperiment {
            self.experiment
        }

        #[must_use]
        pub fn options(&self) -> &[SelectOption] {
            &self.options
        }

        #[must_use]
        pub fn filter(&self) -> &str {
            &self.filter
        }

        #[must_use]
        pub fn selected_id(&self) -> Option<&str> {
            self.selected_id.as_deref()
        }

        #[must_use]
        pub fn is_locked(&self) -> bool {
            self.locked
        }

        pub fn set_locked(&mut self, locked: bool) {
            self.locked = locked;
        }

        /// Saved choice (`current`), marked but only auto-focused while
        /// `focus_current` is true (`dialog-select.tsx:169-185,311-319`).
        pub fn set_current(&mut self, id: Option<&str>) {
            self.current = id.map(str::to_owned);
        }

        pub fn set_focus_current(&mut self, focus: bool) {
            self.focus_current = focus;
        }

        pub fn set_preserve_selection(&mut self, preserve: bool) {
            self.preserve_selection = preserve;
        }

        /// Replace options and reconcile (the `props.options` effect).
        pub fn set_options(&mut self, options: Vec<SelectOption>) {
            self.options = options;
            self.reconcile();
        }

        /// Replace options without reconciling (an in-flight load).
        pub fn set_options_pending(&mut self, options: Vec<SelectOption>) {
            self.options = options;
        }

        fn reconcile(&mut self) {
            let ids: Vec<String> = self
                .filtered()
                .into_iter()
                .map(|option| option.id.clone())
                .collect();
            if ids.is_empty() {
                // Temporary empty results must not lose the intended selection.
                return;
            }
            // Upstream keeps the previous identity only when `current` is
            // known and focused, or when `preserveSelection` is set; with an
            // unknown current it clamps the row index instead
            // (`dialog-select.tsx:289-347`). A removed identity clamps too; it
            // never falls back to the `current` row.
            let follows_identity =
                self.preserve_selection || (self.current.is_some() && self.focus_current);
            if follows_identity {
                if let Some(selected) = &self.selected_id {
                    if let Some(index) = ids.iter().position(|id| id == selected) {
                        self.selected = index;
                        return;
                    }
                }
            }
            self.selected = reconcile_selection(self.selected, ids.len());
            self.selected_id = ids.get(self.selected).cloned();
        }

        /// Enabled options matching the filter, in display order
        /// (`dialog-select.tsx:210-231`).
        #[must_use]
        pub fn filtered(&self) -> Vec<&SelectOption> {
            let enabled = self.options.iter().filter(|option| !option.disabled);
            if self.custom_search || self.skip_filter || !self.render_filter {
                return enabled.collect();
            }
            let needle = self.filter.to_lowercase();
            if needle.is_empty() {
                return enabled.collect();
            }
            let query: Vec<char> = needle.chars().collect();
            let mut scored: Vec<(f32, &SelectOption)> = enabled
                .filter_map(|option| {
                    let title = fuzzy_quality(&query, &option.title);
                    let category = option
                        .category
                        .as_deref()
                        .and_then(|category| fuzzy_quality(&query, category));
                    let search = option
                        .search_text
                        .as_deref()
                        .and_then(|text| fuzzy_quality(&query, text));
                    // fuzzysort drops a key that does not match at all, so an
                    // option only survives when at least one key matched.
                    if title.is_none() && category.is_none() && search.is_none() {
                        return None;
                    }
                    // Title matches weigh twice, category and supplemental
                    // text once each (`dialog-select.tsx:220-227`).
                    let total = title.unwrap_or(0.0) * 2.0
                        + category.unwrap_or(0.0)
                        + search.unwrap_or(0.0);
                    (total >= self.filter_threshold).then_some((total, option))
                })
                .collect();
            // `sort_by` is stable, so equal scores keep source order.
            scored.sort_by(|left, right| {
                right.0.partial_cmp(&left.0).unwrap_or(Ordering::Equal)
            });
            scored.into_iter().map(|(_, option)| option).collect()
        }

        /// `flatten` (`dialog-select.tsx:233`): custom search always flattens;
        /// `flat` flattens only while a filter is present.
        #[must_use]
        pub fn flatten(&self) -> bool {
            self.custom_search || (self.flat && !self.filter.is_empty())
        }

        /// Category groups in insertion order; the empty category renders no
        /// heading (`dialog-select.tsx:235-244`).
        #[must_use]
        pub fn grouped(&self) -> Vec<(Option<&str>, Vec<&SelectOption>)> {
            let filtered = self.filtered();
            if self.flatten() {
                return if filtered.is_empty() {
                    Vec::new()
                } else {
                    vec![(None, filtered)]
                };
            }
            let mut groups: Vec<(Option<&str>, Vec<&SelectOption>)> = Vec::new();
            for option in filtered {
                let category = option.category.as_deref();
                if let Some((_, options)) = groups
                    .iter_mut()
                    .find(|(existing, _)| *existing == category)
                {
                    options.push(option);
                } else {
                    groups.push((category, vec![option]));
                }
            }
            groups
        }

        /// Flattened visible options in movement/Enter order.
        #[must_use]
        pub fn flat_list(&self) -> Vec<&SelectOption> {
            self.grouped()
                .into_iter()
                .flat_map(|(_, options)| options)
                .collect()
        }

        /// `rows` (`dialog-select.tsx:253-259`): one row per option plus its
        /// details, one heading for the first category, two for later ones.
        #[must_use]
        pub fn rows(&self) -> usize {
            let mut rows = 0usize;
            for (index, (category, options)) in self.grouped().into_iter().enumerate() {
                if category.is_some() {
                    rows += if index > 0 { 2 } else { 1 };
                }
                rows += options
                    .iter()
                    .map(|option| 1 + option.details.len())
                    .sum::<usize>();
            }
            rows
        }

        /// Viewport row budget at a terminal height (`dialog-select.tsx:262-273`).
        #[must_use]
        pub fn viewport_rows(&self, terminal_height: usize, more_below: bool) -> usize {
            let limit = viewport_limit(terminal_height);
            let rows = self.rows();
            let ellipsis = self.experiment.ellipsis_row && more_below && rows > limit;
            rows.min(limit).saturating_sub(if ellipsis { 1 } else { 0 })
        }

        /// Whether any option reserves a selected-detail region
        /// (`dialog-select.tsx:836`).
        #[must_use]
        pub fn detail_reserved(&self) -> bool {
            self.experiment.detail_reservation
                && self.options.iter().any(|option| option.detail.is_some())
        }

        #[must_use]
        pub fn selected_detail(&self) -> Option<&str> {
            self.selected_option()
                .and_then(|option| option.detail.as_deref())
        }

        #[must_use]
        pub fn empty_state(&self) -> EmptyState {
            if !self.flat_list().is_empty() {
                EmptyState::None
            } else if self.filter.is_empty() || !self.render_filter {
                EmptyState::Empty
            } else {
                EmptyState::NoMatch
            }
        }

        #[must_use]
        pub fn selected_index(&self) -> Option<usize> {
            let flat = self.flat_list();
            if flat.is_empty() {
                return None;
            }
            if let Some(selected) = &self.selected_id {
                if let Some(index) = flat.iter().position(|option| &option.id == selected) {
                    return Some(index);
                }
            }
            Some(reconcile_selection(self.selected, flat.len()))
        }

        #[must_use]
        pub fn selected_option(&self) -> Option<&SelectOption> {
            let index = self.selected_index()?;
            let flat = self.flat_list();
            flat.get(index).copied()
        }

        fn select_index(&mut self, index: usize) {
            let id = { self.flat_list().get(index).map(|option| option.id.clone()) };
            if let Some(id) = id {
                self.selected = index;
                self.selected_id = Some(id);
            }
        }

        /// Move the highlight; `locked` suppresses navigation.
        pub fn move_by(&mut self, delta: isize) -> bool {
            if self.locked {
                return false;
            }
            let count = self.flat_list().len();
            if count == 0 {
                self.selected_id = None;
                self.selected = 0;
                return false;
            }
            let current = self.selected_index().unwrap_or(0);
            let next = move_selection(current, count, delta, MovePolicy::Wrap);
            self.select_index(next);
            true
        }

        pub fn next(&mut self) -> bool {
            self.move_by(1)
        }

        pub fn previous(&mut self) -> bool {
            self.move_by(-1)
        }

        pub fn page_down(&mut self) -> bool {
            self.move_by(PAGE_STEP)
        }

        pub fn page_up(&mut self) -> bool {
            self.move_by(-PAGE_STEP)
        }

        pub fn home(&mut self) -> bool {
            if self.locked || self.flat_list().is_empty() {
                return false;
            }
            self.select_index(0);
            true
        }

        pub fn end(&mut self) -> bool {
            if self.locked {
                return false;
            }
            let count = self.flat_list().len();
            if count == 0 {
                return false;
            }
            self.select_index(count - 1);
            true
        }

        /// Alt+Up/Down section navigation (`dialog-select.tsx:411-418`):
        /// the next non-empty category's first option, wrapping; an unknown
        /// current section starts from `-1` exactly like upstream.
        pub fn move_section(&mut self, direction: i8) -> bool {
            if self.locked || direction == 0 {
                return false;
            }
            let current = self.selected_option().and_then(|o| o.category.clone());
            let target = {
                let groups = self.grouped();
                if groups.is_empty() {
                    None
                } else {
                    // Upstream starts from -1 when the selection's category is
                    // not one of the groups, so the first step lands on group 0.
                    let position = groups
                        .iter()
                        .position(|(category, _)| *category == current.as_deref())
                        .map_or(-1isize, |index| index as isize);
                    let count = groups.len() as isize;
                    let next = ((position + direction as isize + count) % count) as usize;
                    Some(
                        groups
                            .iter()
                            .take(next)
                            .map(|(_, options)| options.len())
                            .sum::<usize>(),
                    )
                }
            };
            let Some(index) = target else {
                return false;
            };
            self.select_index(index);
            true
        }

        /// Update the filter (`dialog-select.tsx:386-403`): a non-empty query
        /// selects the first result; clearing restores the current choice.
        pub fn set_filter(&mut self, filter: &str) -> bool {
            if self.locked || self.filter == filter {
                return false;
            }
            self.filter = filter.to_owned();
            if self.filter.is_empty() {
                if self.focus_current {
                    if let Some(current) = self.current.clone() {
                        if let Some(index) = {
                            self.flat_list()
                                .iter()
                                .position(|option| option.id == current)
                        } {
                            self.select_index(index);
                            return true;
                        }
                    }
                }
                // Without a `current` to restore, upstream leaves the row
                // index alone, so the highlight lands on whatever now
                // occupies that row (`dialog-select.tsx:386-403`).
                let id = self
                    .flat_list()
                    .get(self.selected)
                    .map(|option| option.id.clone());
                if let Some(id) = id {
                    self.selected_id = Some(id);
                } else {
                    self.reconcile();
                }
                return true;
            }
            if !self.flat_list().is_empty() {
                self.select_index(0);
            }
            true
        }

        pub fn push_filter(&mut self, ch: char) -> bool {
            if ch.is_control() {
                return false;
            }
            let mut next = self.filter.clone();
            next.push(ch);
            self.set_filter(&next)
        }

        /// Remove one character, never a partial code point.
        pub fn pop_filter(&mut self) -> bool {
            let Some((index, _)) = self.filter.char_indices().last() else {
                return false;
            };
            let mut next = self.filter.clone();
            next.truncate(index);
            self.set_filter(&next)
        }

        /// Enter: submit the active option; `locked` suppresses submission and
        /// an empty list has nothing to submit (`dialog-select.tsx:464-470`).
        #[must_use]
        pub fn submit(&self) -> Option<&SelectOption> {
            if self.locked {
                return None;
            }
            self.selected_option()
        }

        /// Pointer hover (`dialog-select.tsx:785-801`): ignored while locked
        /// and when the row is already chosen.
        pub fn pointer_move(&mut self, index: usize) -> bool {
            if self.locked || self.flat_list().get(index).is_none() {
                return false;
            }
            if self.selected_index() == Some(index) {
                return false;
            }
            self.select_index(index);
            true
        }

        /// Pointer press moves the highlight (never submits).
        pub fn pointer_down(&mut self, index: usize) -> bool {
            self.pointer_move(index)
        }

        /// Pointer release submits the row under the pointer.
        #[must_use]
        pub fn pointer_up(&self) -> Option<&SelectOption> {
            self.submit()
        }

        #[must_use]
        pub fn snapshot(&self) -> SelectSnapshot {
            SelectSnapshot {
                filter: self.filter.clone(),
                selected: self.selected_id.clone(),
            }
        }

        /// Restore a drill-down snapshot; a deleted identity reconciles
        /// instead of resurrecting (`dialog-open.tsx:118-128`).
        pub fn restore(&mut self, snapshot: SelectSnapshot) {
            self.filter = snapshot.filter;
            if let Some(selected) = snapshot.selected {
                if let Some(index) = {
                    self.flat_list()
                        .iter()
                        .position(|option| option.id == selected)
                } {
                    self.select_index(index);
                    return;
                }
            }
            self.reconcile();
        }
    }

    /// Drill-down/back stack used by the Open/workspace pickers
    /// (`dialog-open.tsx:103-122`); back restores query and identity.
    #[derive(Clone, Debug, Default)]
    pub struct DrillDown {
        stack: Vec<SelectSnapshot>,
    }

    impl DrillDown {
        pub fn push(&mut self, dialog: &SelectDialog) {
            self.stack.push(dialog.snapshot());
        }

        pub fn pop(&mut self, dialog: &mut SelectDialog) -> bool {
            let Some(snapshot) = self.stack.pop() else {
                return false;
            };
            dialog.restore(snapshot);
            true
        }

        #[must_use]
        pub fn depth(&self) -> usize {
            self.stack.len()
        }

        #[must_use]
        pub fn is_empty(&self) -> bool {
            self.stack.is_empty()
        }
    }

    /// Fences asynchronous option loads against intervening deletion or
    /// relocation (`dialog-open.tsx:70-84`): a stale generation is dropped,
    /// deleted ids stay deleted, and local moves win over a late payload.
    #[derive(Clone, Debug, Default)]
    pub struct LateLoadGuard {
        generation: u64,
        removed: std::collections::BTreeSet<String>,
        relocated: std::collections::BTreeMap<String, Option<String>>,
    }

    impl LateLoadGuard {
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        /// Read before starting a fetch; pass it to [`Self::apply`] later.
        #[must_use]
        pub fn generation(&self) -> u64 {
            self.generation
        }

        /// Start a newer load; its generation supersedes every response that
        /// was requested earlier (`dialog-open.tsx:55-84`).
        pub fn begin(&mut self) -> u64 {
            self.generation = self.generation.wrapping_add(1);
            self.generation
        }

        /// A deletion is folded into any in-flight response instead of
        /// invalidating it (`dialog-open.tsx:56-73`).
        pub fn remove(&mut self, id: &str) {
            self.removed.insert(id.to_owned());
            self.relocated.remove(id);
        }

        /// A local move is folded into any in-flight response instead of
        /// invalidating it (`dialog-open.tsx:56-73`).
        pub fn relocate(&mut self, id: &str, category: Option<&str>) {
            self.relocated.insert(id.to_owned(), category.map(str::to_owned));
        }

        #[must_use]
        pub fn is_removed(&self, id: &str) -> bool {
            self.removed.contains(id)
        }

        /// Apply a completed load only when it is still current.
        pub fn apply(
            &self,
            generation: u64,
            entries: Vec<SelectOption>,
        ) -> Option<Vec<SelectOption>> {
            if generation != self.generation {
                return None;
            }
            Some(
                entries
                    .into_iter()
                    .filter(|entry| !self.removed.contains(&entry.id))
                    .map(|mut entry| {
                        if let Some(category) = self.relocated.get(&entry.id) {
                            entry.category = category.clone();
                        }
                        entry
                    })
                    .collect(),
            )
        }
    }

    /// One extra action bound while the prompt input is focused
    /// (`ui/dialog-prompt.tsx:8-14,52-73`). `command` is the keymap command id;
    /// `command_title` defaults to `title` for help listings. The trigger
    /// callback is behavioral wiring and is not part of this display model.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct DialogPromptAction {
        pub title: String,
        pub command: String,
        pub command_title: Option<String>,
    }

    impl DialogPromptAction {
        #[must_use]
        pub fn new(title: impl Into<String>, command: impl Into<String>) -> Self {
            Self {
                title: title.into(),
                command: command.into(),
                command_title: None,
            }
        }

        /// `action.commandTitle ?? action.title` (`dialog-prompt.tsx:67`).
        #[must_use]
        pub fn command_title(&self) -> &str {
            self.command_title.as_deref().unwrap_or(&self.title)
        }
    }

    /// Upper bound on prompt extra actions, so a hostile caller cannot grow the
    /// prompt unbounded.
    pub const MAX_PROMPT_ACTIONS: usize = 16;

    /// `busyText ?? "Working…"` (`dialog-prompt.tsx:169`).
    pub const DEFAULT_PROMPT_BUSY_TEXT: &str = "Working…";

    /// One-row prompt state (`ui/dialog-prompt.tsx`). Busy suspends editing,
    /// blurs the input, blocks submission, extra actions and Escape cancel.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct PromptState {
        value: String,
        busy: bool,
        focused: bool,
        /// Spinner line shown while busy (`dialog-prompt.tsx:100-140,168-170`).
        busy_text: Option<String>,
        /// Extra actions bound while focused and not busy
        /// (`dialog-prompt.tsx:52-73`).
        actions: Vec<DialogPromptAction>,
    }

    impl PromptState {
        /// `dialog.prompt.submit` is laid out with `priority: 1` so it wins
        /// over the global textarea layer (`dialog-prompt.tsx:63-73`).
        pub const SUBMIT_PRIORITY: u8 = 1;

        #[must_use]
        pub fn open(value: &str) -> Self {
            Self {
                value: value.to_owned(),
                busy: false,
                focused: true,
                busy_text: None,
                actions: Vec::new(),
            }
        }

        #[must_use]
        pub fn value(&self) -> &str {
            &self.value
        }

        #[must_use]
        pub fn is_busy(&self) -> bool {
            self.busy
        }

        #[must_use]
        pub fn is_focused(&self) -> bool {
            self.focused
        }

        /// Raw `busyText`, or `None` (`dialog-prompt.tsx:169`).
        #[must_use]
        pub fn busy_text(&self) -> Option<&str> {
            self.busy_text.as_deref()
        }

        /// The line painted while busy, defaulting `busyText` to `"Working…"`
        /// (`dialog-prompt.tsx:168-170`); `None` when not busy.
        #[must_use]
        pub fn busy_line(&self) -> Option<&str> {
            self.busy
                .then(|| self.busy_text.as_deref().unwrap_or(DEFAULT_PROMPT_BUSY_TEXT))
        }

        pub fn set_busy_text(&mut self, text: Option<String>) {
            self.busy_text = text;
        }

        /// Register an extra action (`props.actions`); bounded by
        /// [`MAX_PROMPT_ACTIONS`].
        pub fn add_action(&mut self, action: DialogPromptAction) -> bool {
            if self.actions.len() >= MAX_PROMPT_ACTIONS {
                return false;
            }
            self.actions.push(action);
            true
        }

        /// Actions currently triggerable: none while busy
        /// (`dialog-prompt.tsx:52-56` binds them only when `!props.busy`).
        #[must_use]
        pub fn actions(&self) -> &[DialogPromptAction] {
            if self.busy {
                &[]
            } else {
                &self.actions
            }
        }

        pub fn set_busy(&mut self, busy: bool) {
            self.busy = busy;
            self.focused = !busy;
        }

        pub fn focus(&mut self) -> bool {
            if self.busy || self.focused {
                return false;
            }
            self.focused = true;
            true
        }

        pub fn push(&mut self, ch: char) -> bool {
            if self.busy || !self.focused || ch.is_control() {
                return false;
            }
            self.value.push(ch);
            true
        }

        pub fn backspace(&mut self) -> bool {
            if self.busy || !self.focused {
                return false;
            }
            self.value.pop().is_some()
        }

        /// Enter: disabled while busy (`dialog-prompt.tsx:43-46`).
        #[must_use]
        pub fn submit(&self) -> Option<&str> {
            if self.busy {
                return None;
            }
            Some(&self.value)
        }

        #[must_use]
        pub fn extra_actions_enabled(&self) -> bool {
            !self.busy
        }

        /// Escape cancel is suppressed while busy
        /// (`dialog-prompt.tsx:93-100`).
        #[must_use]
        pub fn escape(&self) -> bool {
            !self.busy
        }
    }

    /// Confirmation choice (`ui/dialog-confirm.tsx:24-26`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ConfirmChoice {
        Cancel,
        Confirm,
    }

    /// Confirmation key (`dialog-confirm.tsx:34-65`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ConfirmKey {
        Left,
        Right,
        Enter,
        Escape,
    }

    /// Result of one confirmation key.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ConfirmOutcome {
        /// Legacy Left/Right toggled the active button; no action yet.
        Toggle,
        Confirm,
        Cancel,
        Ignored,
    }

    /// `DialogConfirm` state: Confirm is selected first. Under new designs
    /// Enter always confirms and Escape cancels; the toggle keys do nothing.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ConfirmState {
        active: ConfirmChoice,
        new_designs: bool,
    }

    impl Default for ConfirmState {
        fn default() -> Self {
            Self::legacy()
        }
    }

    impl ConfirmState {
        #[must_use]
        pub const fn legacy() -> Self {
            Self {
                active: ConfirmChoice::Confirm,
                new_designs: false,
            }
        }

        #[must_use]
        pub const fn new_designs() -> Self {
            Self {
                active: ConfirmChoice::Confirm,
                new_designs: true,
            }
        }

        #[must_use]
        pub const fn active(&self) -> ConfirmChoice {
            self.active
        }

        pub fn key(&mut self, key: ConfirmKey) -> ConfirmOutcome {
            if self.new_designs {
                return match key {
                    ConfirmKey::Enter => ConfirmOutcome::Confirm,
                    ConfirmKey::Escape => ConfirmOutcome::Cancel,
                    ConfirmKey::Left | ConfirmKey::Right => ConfirmOutcome::Ignored,
                };
            }
            match key {
                ConfirmKey::Left | ConfirmKey::Right => {
                    self.active = match self.active {
                        ConfirmChoice::Confirm => ConfirmChoice::Cancel,
                        ConfirmChoice::Cancel => ConfirmChoice::Confirm,
                    };
                    ConfirmOutcome::Toggle
                }
                ConfirmKey::Enter => match self.active {
                    ConfirmChoice::Confirm => ConfirmOutcome::Confirm,
                    ConfirmChoice::Cancel => ConfirmOutcome::Cancel,
                },
                // Legacy has no escape binding in this layer; the provider's
                // dialog layer owns Escape.
                ConfirmKey::Escape => ConfirmOutcome::Ignored,
            }
        }

        /// Footer labels are lowercase; legacy buttons title-case them
        /// (`dialog-confirm.tsx:72-93`).
        #[must_use]
        pub const fn footer_label(choice: ConfirmChoice) -> &'static str {
            match choice {
                ConfirmChoice::Cancel => "cancel",
                ConfirmChoice::Confirm => "confirm",
            }
        }

        #[must_use]
        pub const fn button_label(choice: ConfirmChoice) -> &'static str {
            match choice {
                ConfirmChoice::Cancel => "Cancel",
                ConfirmChoice::Confirm => "Confirm",
            }
        }

        /// Legacy buttons pad one cell on each side (`dialog-confirm.tsx:86-88`).
        #[must_use]
        pub const fn button_side_padding() -> u16 {
            1
        }
    }
}

/// Picker dialog state shared by the model, session, MCP, integration, theme,
/// stash and workspace dialogs.
///
/// Upstream evidence (pin `450e7b9`): `dialog-model.tsx:100-202`,
/// `dialog-model-free.tsx:26-51`, `model-options.tsx:1-40`,
/// `dialog-session-list.tsx:39-122,142-193,252-296`, `dialog-mcp.tsx:14-135`,
/// `dialog-integration.tsx:118-150,290-336`, `dialog-theme-list.tsx:14-65`,
/// `dialog-stash.tsx:39-85`, `dialog-workspaces.tsx:181-245`,
/// `dialog-workspace-file-changes.tsx:62-105`.
pub mod pickers {
    /// Model picker ordering and favorite priority
    /// (`dialog-model.tsx:100-202`, `model-options.tsx:7-40`).
    pub type ModelKey = (String, String);

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ModelCandidate {
        pub provider: String,
        pub id: String,
        pub title: String,
        pub free: bool,
        pub deprecated: bool,
    }

    impl ModelCandidate {
        #[must_use]
        pub fn key(&self) -> ModelKey {
            (self.provider.clone(), self.id.clone())
        }
    }

    fn is_free_group(model: &ModelCandidate, new_designs: bool, restricted: bool) -> bool {
        new_designs && !restricted && model.provider == "opencode" && model.free
    }

    /// `sortModelOptions` (`model-options.tsx:22-40`): provider groups
    /// (`opencode-go`, then `opencode`), provider name, free models first.
    fn sort_models(models: &mut [ModelCandidate]) {
        models.sort_by(|a, b| {
            let group = (b.provider == "opencode-go").cmp(&(a.provider == "opencode-go"));
            if group != std::cmp::Ordering::Equal {
                return group;
            }
            let group = (b.provider == "opencode").cmp(&(a.provider == "opencode"));
            if group != std::cmp::Ordering::Equal {
                return group;
            }
            let name = a.provider.cmp(&b.provider);
            if name != std::cmp::Ordering::Equal {
                return name;
            }
            b.free.cmp(&a.free)
        });
    }

    /// Full picker order for an empty query: favorites, recents (minus
    /// favorites), remaining models, free group last
    /// (`dialog-model.tsx:132-201`).
    #[must_use]
    pub fn order_models(
        models: &[ModelCandidate],
        favorites: &[ModelKey],
        recents: &[ModelKey],
        new_designs: bool,
        restricted_to_provider: bool,
    ) -> Vec<ModelCandidate> {
        let available: Vec<ModelCandidate> = models
            .iter()
            .filter(|model| !model.deprecated)
            .cloned()
            .collect();
        let pinned = |key: &ModelKey| {
            available
                .iter()
                .find(|model| &model.key() == key)
                .cloned()
        };
        let mut ordered: Vec<ModelCandidate> = favorites.iter().filter_map(pinned).collect();
        ordered.extend(
            recents
                .iter()
                .filter(|key| !favorites.contains(key))
                .filter_map(pinned),
        );
        let mut rest: Vec<ModelCandidate> = available
            .iter()
            .filter(|model| !favorites.contains(&model.key()))
            .filter(|model| !recents.contains(&model.key()))
            .cloned()
            .collect();
        sort_models(&mut rest);
        let (free, paid): (Vec<_>, Vec<_>) = rest
            .into_iter()
            .partition(|model| is_free_group(model, new_designs, restricted_to_provider));
        ordered.extend(paid);
        ordered.extend(free);
        ordered
    }

    /// Filtered picker results: fuzzy title matches with favorites hoisted
    /// (`dialog-model.tsx:182-194`, `model-options.tsx:7-13`).
    #[must_use]
    pub fn search_models<'a>(
        query: &str,
        models: &'a [ModelCandidate],
        favorites: &[ModelKey],
    ) -> Vec<&'a ModelCandidate> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }
        let matched: Vec<&ModelCandidate> = models
            .iter()
            .filter(|model| !model.deprecated)
            .filter(|model| model.title.to_lowercase().contains(&needle))
            .collect();
        let mut hoisted: Vec<&ModelCandidate> = matched
            .iter()
            .copied()
            .filter(|model| favorites.contains(&model.key()))
            .collect();
        hoisted.extend(
            matched
                .into_iter()
                .filter(|model| !favorites.contains(&model.key())),
        );
        hoisted
    }

    /// The session list debounces its server search by 150 ms
    /// (`dialog-session-list.tsx:41`).
    pub const SESSION_SEARCH_DEBOUNCE_MS: u64 = 150;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SessionRow {
        pub id: String,
        pub title: String,
        pub parent: bool,
        pub updated_date: String,
        pub pinned: bool,
        pub directory: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum SessionSearchState {
        /// Query and debounced query disagree, or a load is in flight.
        Searching,
        Error,
        Empty,
    }

    /// Session picker state (`dialog-session-list.tsx:39-122,142-193`).
    #[derive(Clone, Debug)]
    pub struct SessionPicker {
        filter: String,
        debounced: String,
        loading: bool,
        error: bool,
        all_projects: bool,
        current_directory: String,
        today: String,
        to_delete: Option<String>,
        result: Option<(String, bool)>,
    }

    impl SessionPicker {
        /// `allProjects` starts from the persisted `tabs.scope !== "cwd"`
        /// preference (`dialog-session-list.tsx:43-46`).
        #[must_use]
        pub fn new(scope_cwd: bool, current_directory: &str, today: &str) -> Self {
            Self {
                filter: String::new(),
                debounced: String::new(),
                loading: true,
                error: false,
                all_projects: !scope_cwd,
                current_directory: current_directory.to_owned(),
                today: today.to_owned(),
                to_delete: None,
                result: None,
            }
        }

        #[must_use]
        pub fn all_projects(&self) -> bool {
            self.all_projects
        }

        pub fn toggle_scope(&mut self) {
            self.all_projects = !self.all_projects;
            self.loading = true;
        }

        #[must_use]
        pub fn filter(&self) -> &str {
            &self.filter
        }

        #[must_use]
        pub fn debounced(&self) -> &str {
            &self.debounced
        }

        /// Typing updates the visible filter immediately; the returned value
        /// says whether the debounce timer must be (re)started.
        pub fn set_filter(&mut self, filter: &str) -> bool {
            if self.filter == filter {
                return false;
            }
            self.filter = filter.to_owned();
            self.loading = true;
            self.error = false;
            self.to_delete = None;
            true
        }

        /// Debounce elapsed: promote the filter to the query actually sent.
        pub fn debounce_elapsed(&mut self) -> bool {
            let query = self.filter.trim().to_owned();
            if query == self.debounced {
                return false;
            }
            self.debounced = query;
            true
        }

        /// Apply a completed search. A response for another query, another
        /// scope, or a failed request degrades to the local list
        /// (`dialog-session-list.tsx:102-110`).
        pub fn apply_result(
            &mut self,
            query: &str,
            all_projects: bool,
            error: bool,
            server: Vec<SessionRow>,
            local: &[SessionRow],
        ) -> Vec<SessionRow> {
            self.loading = false;
            self.error = error;
            let query = query.trim().to_owned();
            if error || query != self.debounced || all_projects != self.all_projects {
                self.result = None;
                return self.local_rows(local);
            }
            self.result = Some((query.clone(), all_projects));
            let needle = query.to_lowercase();
            server
                .into_iter()
                .filter(|session| !session.parent)
                .filter(|session| {
                    needle.is_empty() || session.title.to_lowercase().contains(&needle)
                })
                .collect()
        }

        fn local_rows(&self, local: &[SessionRow]) -> Vec<SessionRow> {
            let needle = self.filter.trim().to_lowercase();
            local
                .iter()
                .filter(|session| !session.parent)
                .filter(|session| {
                    self.all_projects || session.directory == self.current_directory
                })
                .filter(|session| {
                    needle.is_empty() || session.title.to_lowercase().contains(&needle)
                })
                .cloned()
                .collect()
        }

        /// `searchState` (`dialog-session-list.tsx:111-122`): the visible
        /// message distinguishes loading, failure and empty.
        #[must_use]
        pub fn search_state(&self) -> SessionSearchState {
            let query = self.filter.trim();
            if query != self.debounced || self.loading {
                return SessionSearchState::Searching;
            }
            if self.error {
                return SessionSearchState::Error;
            }
            SessionSearchState::Empty
        }

        /// The exact empty/no-match text (`dialog-session-list.tsx:116-121`).
        #[must_use]
        pub fn search_message(&self) -> &'static str {
            let query = !self.filter.trim().is_empty();
            match self.search_state() {
                SessionSearchState::Searching => {
                    if query {
                        "Searching sessions…"
                    } else {
                        "Loading sessions…"
                    }
                }
                SessionSearchState::Error => {
                    if query {
                        "Could not search sessions. Change the search to try again."
                    } else {
                        "Could not load sessions."
                    }
                }
                SessionSearchState::Empty => {
                    if query {
                        "No sessions found"
                    } else {
                        "No sessions available"
                    }
                }
            }
        }

        /// First delete press arms, the second deletes; moving clears
        /// (`dialog-session-list.tsx:163-170,247-291`). Returns `true` when
        /// the caller must perform the deletion.
        pub fn arm_delete(&mut self, id: &str) -> bool {
            if self.to_delete.as_deref() == Some(id) {
                self.to_delete = None;
                return true;
            }
            self.to_delete = Some(id.to_owned());
            false
        }

        pub fn on_move(&mut self) {
            self.to_delete = None;
        }

        #[must_use]
        pub fn is_armed(&self, id: &str) -> bool {
            self.to_delete.as_deref() == Some(id)
        }

        /// Armed rows replace their title with the confirmation prompt.
        #[must_use]
        pub fn row_title(&self, row: &SessionRow, delete_key: &str) -> String {
            if self.is_armed(&row.id) {
                format!("Press {delete_key} again to confirm")
            } else {
                row.title.clone()
            }
        }

        /// Grouping: pinned first, then today, then the update date
        /// (`dialog-session-list.tsx:142-192`).
        #[must_use]
        pub fn group_of(&self, row: &SessionRow) -> String {
            if row.pinned && !row.parent {
                "Pinned".to_owned()
            } else if row.updated_date == self.today {
                "Today".to_owned()
            } else {
                row.updated_date.clone()
            }
        }
    }

    /// MCP server states (`dialog-mcp.tsx:15-33,84-107`).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum McpStatus {
        Pending,
        Connected,
        Failed { error: String },
        NeedsAuth {
            error: String,
            integration: Option<String>,
        },
        Disabled,
    }

    impl McpStatus {
        #[must_use]
        pub fn label(&self) -> &'static str {
            match self {
                Self::Pending => "Connecting …",
                Self::Connected => "Connected ✓",
                Self::Failed { .. } => "Failed !",
                Self::NeedsAuth { .. } => "Sign in required →",
                Self::Disabled => "Disabled ○",
            }
        }

        /// Footer action title; `connect` for pending/disabled.
        #[must_use]
        pub fn toggle_title(&self) -> &'static str {
            match self {
                Self::Connected => "disconnect",
                Self::Failed { .. } => "retry",
                Self::NeedsAuth { .. } => "sign in",
                Self::Pending | Self::Disabled => "connect",
            }
        }

        #[must_use]
        pub fn error(&self) -> Option<&str> {
            match self {
                Self::Failed { error } => Some(error),
                Self::NeedsAuth { error, .. } => Some(error),
                Self::Pending | Self::Connected | Self::Disabled => None,
            }
        }
    }

    /// Enter on an MCP row (`dialog-mcp.tsx:94-108`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum McpEnter {
        SignIn,
        ErrorDetails,
        Ignore,
    }

    #[must_use]
    pub fn mcp_enter(status: &McpStatus) -> McpEnter {
        match status {
            McpStatus::NeedsAuth {
                integration: Some(_),
                ..
            } => McpEnter::SignIn,
            McpStatus::Failed { .. } | McpStatus::NeedsAuth { .. } => McpEnter::ErrorDetails,
            McpStatus::Pending | McpStatus::Connected | McpStatus::Disabled => McpEnter::Ignore,
        }
    }

    /// Connection method shapes (`dialog-integration.tsx:58-81,316-336`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ConnectMethod {
        Key,
        Command,
        External,
        OAuth,
    }

    impl ConnectMethod {
        /// Progress copy shown while the method runs
        /// (`dialog-integration.tsx:388,439`).
        #[must_use]
        pub const fn progress_message(self) -> &'static str {
            match self {
                Self::Command => "Starting command…",
                Self::Key => "Connecting…",
                Self::External | Self::OAuth => "Connecting…",
            }
        }
    }

    /// One integration flow (`dialog-integration.tsx:118-336`).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum IntegrationPhase {
        /// Provider has no account yet; connect is disabled.
        EnvironmentOnly,
        /// Accounts list with an "Add account" entry.
        Accounts,
        /// Method chooser; a single method opens it directly.
        Methods,
        /// Form for method credentials; `None` answer returns to the caller.
        Form,
        Progress(&'static str),
        Connected,
        /// Cancelled flows close the dialog.
        Cancelled,
        Failed,
    }

    impl IntegrationPhase {
        #[must_use]
        pub fn entry(methods: usize, accounts: usize) -> Self {
            if methods == 0 {
                return Self::EnvironmentOnly;
            }
            if methods == 1 {
                return Self::Form;
            }
            if accounts > 0 {
                return Self::Accounts;
            }
            Self::Methods
        }

        pub fn start(&mut self, method: ConnectMethod) {
            *self = Self::Progress(method.progress_message());
        }

        /// A cancelled form leaves the current dialog untouched.
        pub fn form_answer(&mut self, answered: bool) {
            if !answered {
                return;
            }
            *self = Self::Connected;
        }

        /// Cancel closes the flow (`CANCELLED` in upstream).
        pub fn cancel(&mut self) {
            *self = Self::Cancelled;
        }

        pub fn fail(&mut self) {
            *self = Self::Failed;
        }

        #[must_use]
        pub fn is_closed(&self) -> bool {
            matches!(self, Self::Cancelled | Self::Failed)
        }
    }

    /// Theme list preview (`dialog-theme-list.tsx:14-65`): moving previews,
    /// closing without confirming reverts, selecting confirms.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ThemePreview {
        initial: String,
        current: String,
        confirmed: bool,
    }

    impl ThemePreview {
        #[must_use]
        pub fn open(initial: &str) -> Self {
            Self {
                initial: initial.to_owned(),
                current: initial.to_owned(),
                confirmed: false,
            }
        }

        #[must_use]
        pub fn current(&self) -> &str {
            &self.current
        }

        /// `onMove`: preview the highlighted theme.
        pub fn move_to(&mut self, theme: &str) {
            self.current = theme.to_owned();
        }

        /// `onSelect`: keep the previewed theme.
        pub fn select(&mut self) -> String {
            self.confirmed = true;
            self.current.clone()
        }

        /// `onCleanup` without confirmation: revert.
        pub fn cancel(&mut self) -> String {
            if !self.confirmed {
                self.current = self.initial.clone();
            }
            self.current.clone()
        }

        /// Filter cleared reverts to the initial theme; a non-empty filter
        /// previews its first result (`dialog-theme-list.tsx:57-64`).
        pub fn on_filter(&mut self, query: &str, first: Option<&str>) {
            if query.is_empty() {
                self.current = self.initial.clone();
                return;
            }
            if let Some(theme) = first {
                self.current = theme.to_owned();
            }
        }
    }

    /// Armed destructive action shared by the stash and workspace dialogs
    /// (`dialog-stash.tsx:39-85`, `dialog-workspaces.tsx:181-245`).
    #[derive(Clone, Debug, Default)]
    pub struct ArmedDelete {
        armed: Option<String>,
    }

    impl ArmedDelete {
        /// First press arms, second press performs. Returns `true` when the
        /// caller must delete now.
        pub fn press(&mut self, id: &str) -> bool {
            if self.armed.as_deref() == Some(id) {
                self.armed = None;
                return true;
            }
            self.armed = Some(id.to_owned());
            false
        }

        pub fn clear(&mut self) {
            self.armed = None;
        }

        #[must_use]
        pub fn armed(&self) -> Option<&str> {
            self.armed.as_deref()
        }

        #[must_use]
        pub fn title(&self, id: &str, title: &str, delete_key: &str) -> String {
            if self.armed.as_deref() == Some(id) {
                format!("Press {delete_key} again to confirm")
            } else {
                title.to_owned()
            }
        }
    }

    /// Workspace file-change confirmation (`dialog-workspace-file-changes.tsx:62-105`).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct FileChangesConfirm {
        pub confirm_title: String,
        pub confirm_key: String,
    }

    impl Default for FileChangesConfirm {
        fn default() -> Self {
            Self {
                confirm_title: "confirm".to_owned(),
                confirm_key: "enter".to_owned(),
            }
        }
    }

    impl FileChangesConfirm {
        #[must_use]
        pub fn delete() -> Self {
            Self {
                confirm_title: "delete".to_owned(),
                confirm_key: "d".to_owned(),
            }
        }

        /// Resolve a key press: `Some(true)` confirms, `Some(false)` cancels,
        /// `None` ignores. `return` stands in for an `enter` binding because
        /// the keymap names the physical key.
        #[must_use]
        pub fn resolve(&self, key: &str) -> Option<bool> {
            if key == "esc" {
                return Some(false);
            }
            if key == self.confirm_key || (self.confirm_key == "enter" && key == "return") {
                return Some(true);
            }
            None
        }
    }
}

#[cfg(test)]
mod dialog_parity_tests {
    use super::dialog::*;
    use super::pickers::*;
    use super::select::*;

    fn options() -> Vec<SelectOption> {
        vec![
            SelectOption::new("model", "Switch model")
                .with_category("Agent")
                .with_search_text("provider list"),
            SelectOption::new("session", "Switch session")
                .with_category("Session")
                .with_search_text("resume conversation"),
            SelectOption::new("new", "New session").with_category("Session"),
            SelectOption::new("rename", "Rename session")
                .with_category("Session")
                .disabled(),
            SelectOption::new("exit", "Exit the app").with_category("System"),
        ]
    }

    // DIALOG-05 / DIALOG-06: disabled rows are removed before filtering and
    // the row budget counts headings, gaps and details
    // (dialog-select.tsx:211-259).
    #[test]
    fn disabled_options_are_removed_before_filtering_and_rows_count_gaps() {
        let dialog = SelectDialog::new(options());
        let ids: Vec<_> = dialog.filtered().iter().map(|o| o.id.as_str()).collect();
        assert!(!ids.contains(&"rename"));
        // 4 option rows + headings Agent/Session/System with two gaps = 9.
        assert_eq!(dialog.rows(), 9);
        let mut filtered = dialog.clone();
        filtered.set_filter("rename");
        assert!(filtered.flat_list().is_empty());
        assert_eq!(filtered.empty_state(), EmptyState::NoMatch);
    }

    // DIALOG-06: title matches weigh twice, category/searchText still match
    // (dialog-select.tsx:220-229).
    #[test]
    fn search_weights_title_twice_and_uses_search_text() {
        let dialog = SelectDialog::new(options());
        let mut search = dialog.clone();
        search.set_filter("session");
        let ids: Vec<_> = search.flat_list().iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"session") && ids.contains(&"new"));
        assert!(!ids.contains(&"exit"));
        // A title match outweighs a category-only match.
        let mut weighted = SelectDialog::new(vec![
            SelectOption::new("by-title", "Alpha").with_category("Zed"),
            SelectOption::new("by-category", "Gamma").with_category("Alpha"),
        ]);
        weighted.set_filter("alpha");
        assert_eq!(weighted.selected_id(), Some("by-title"));
        let mut supplemental = dialog.clone();
        supplemental.set_filter("resume conversation");
        assert_eq!(
            supplemental.selected_id(),
            Some("session"),
            "searchText participates in matching"
        );
        // A 0.7 threshold still admits a strong title match but not an
        // unrelated query (command-palette.tsx:71).
        let mut threshold = SelectDialog::new(options()).with_threshold(0.7);
        threshold.set_filter("swit");
        assert_eq!(threshold.selected_id(), Some("model"));
        threshold.set_filter("swx");
        assert!(threshold.flat_list().is_empty());
    }

    // DIALOG-08: movement wraps, page moves ten, home/end clamp
    // (dialog-select.tsx:483-527, select-controller.ts:5-12).
    #[test]
    fn movement_wraps_and_page_moves_ten_options() {
        let many: Vec<_> = (0..25)
            .map(|index| SelectOption::new(format!("row-{index}"), format!("Row {index}")))
            .collect();
        let mut dialog = SelectDialog::new(many);
        assert_eq!(dialog.selected_id(), Some("row-0"));
        dialog.previous();
        assert_eq!(dialog.selected_id(), Some("row-24"));
        dialog.next();
        assert_eq!(dialog.selected_id(), Some("row-0"));
        assert!(dialog.page_down());
        assert_eq!(dialog.selected_id(), Some("row-10"));
        dialog.page_down();
        assert_eq!(dialog.selected_id(), Some("row-20"));
        dialog.page_down();
        assert_eq!(dialog.selected_id(), Some("row-0"));
        dialog.page_up();
        assert_eq!(dialog.selected_id(), Some("row-24"));
        assert!(dialog.home());
        assert_eq!(dialog.selected_id(), Some("row-0"));
        assert!(dialog.end());
        assert_eq!(dialog.selected_id(), Some("row-24"));
    }

    // DIALOG-08: Alt+Up/Down moves between categories and wraps
    // (dialog-select.tsx:411-418).
    #[test]
    fn section_navigation_wraps_between_categories() {
        let mut dialog = SelectDialog::new(options());
        assert!(dialog.move_section(1));
        assert_eq!(dialog.selected_id(), Some("session"));
        assert!(dialog.move_section(1));
        assert_eq!(dialog.selected_id(), Some("exit"));
        assert!(dialog.move_section(1));
        assert_eq!(dialog.selected_id(), Some("model"));
        assert!(dialog.move_section(-1));
        assert_eq!(dialog.selected_id(), Some("exit"));
    }

    // DIALOG-08: `locked` suppresses navigation, filtering, submission and
    // pointer input (dialog-select.tsx:405,464,660,785-801).
    #[test]
    fn locked_suppresses_navigation_filtering_pointer_and_submission() {
        let mut dialog = SelectDialog::new(options());
        dialog.set_locked(true);
        assert!(!dialog.next());
        assert!(!dialog.page_down());
        assert!(!dialog.home());
        assert!(!dialog.end());
        assert!(!dialog.move_section(1));
        assert!(!dialog.push_filter('s'));
        assert!(!dialog.pointer_move(2));
        assert!(!dialog.pointer_down(2));
        assert!(dialog.submit().is_none());
        assert_eq!(dialog.selected_id(), Some("model"));
        dialog.set_locked(false);
        assert!(dialog.push_filter('s'));
        assert!(dialog.submit().is_some());
        dialog.set_filter("");
        assert!(dialog.pointer_move(1));
        assert_eq!(dialog.selected_id(), Some("session"));
    }

    // DIALOG-05: the two empty views are distinct (dialog-select.tsx:736-753).
    #[test]
    fn empty_and_no_match_views_are_distinct() {
        let empty = SelectDialog::new(Vec::new());
        assert_eq!(empty.empty_state(), EmptyState::Empty);
        let mut dialog = SelectDialog::new(options());
        assert_eq!(dialog.empty_state(), EmptyState::None);
        dialog.set_filter("zzz");
        assert_eq!(dialog.empty_state(), EmptyState::NoMatch);
        let all_disabled = SelectDialog::new(vec![
            SelectOption::new("a", "A").disabled(),
            SelectOption::new("b", "B").disabled(),
        ]);
        assert_eq!(all_disabled.empty_state(), EmptyState::Empty);
    }

    // DIALOG-06: grouping keeps insertion order; `flat` flattens only while
    // a filter is present and a custom search always flattens
    // (dialog-select.tsx:233-244).
    #[test]
    fn grouping_keeps_order_and_flattens_only_for_flat_or_custom_search() {
        let mut dialog = SelectDialog::new(options());
        let categories: Vec<_> = dialog
            .grouped()
            .iter()
            .map(|(category, _)| category.map(str::to_owned))
            .collect();
        assert_eq!(
            categories,
            vec![
                Some("Agent".to_owned()),
                Some("Session".to_owned()),
                Some("System".to_owned())
            ]
        );
        dialog.set_filter("s");
        assert!(!dialog.flatten(), "flat is a prop, off by default");
        assert_eq!(dialog.grouped().len(), 3);

        let mut flat = SelectDialog::new(options()).with_flat(true);
        flat.set_filter("s");
        assert!(flat.flatten());
        assert_eq!(flat.grouped().len(), 1);
        assert_eq!(flat.grouped()[0].0, None);
        flat.set_filter("");
        assert!(!flat.flatten());

        let custom = SelectDialog::new(options()).with_custom_search(true);
        assert_eq!(custom.grouped().len(), 1);
        assert_eq!(custom.grouped()[0].0, None);
    }

    // DIALOG-05: details add rows and category headings add gaps
    // (dialog-select.tsx:253-259,821).
    #[test]
    fn rows_account_for_details_and_category_gaps() {
        let dialog = SelectDialog::new(vec![
            SelectOption::new("a", "A")
                .with_category("One")
                .with_details(vec!["detail 1".into(), "detail 2".into()]),
            SelectOption::new("b", "B").with_category("Two"),
        ]);
        // heading(1) + row(1) + details(2) + gap/heading(2) + row(1)
        assert_eq!(dialog.rows(), 7);
        let flat = SelectDialog::new(vec![SelectOption::new("a", "A")
            .with_details(vec!["detail".into()])])
        .with_flat(true);
        assert_eq!(flat.rows(), 2);
    }

    // DIALOG-07: selection identity survives reorder while `current` is
    // known, and option removal clamps instead of resurrecting
    // (dialog-select.tsx:289-347).
    #[test]
    fn selection_identity_survives_reorder_and_removal() {
        let mut dialog = SelectDialog::new(options());
        dialog.set_current(Some("model"));
        dialog.next();
        assert_eq!(dialog.selected_id(), Some("session"));
        let mut reordered = dialog.options().to_vec();
        reordered.reverse();
        dialog.set_options(reordered);
        assert_eq!(dialog.selected_id(), Some("session"));
        assert_eq!(
            dialog.selected_index(),
            dialog
                .flat_list()
                .iter()
                .position(|option| option.id == "session")
        );
        let survivors: Vec<_> = dialog
            .options()
            .iter()
            .filter(|option| option.id != "session")
            .cloned()
            .collect();
        dialog.set_options(survivors);
        assert_ne!(dialog.selected_id(), Some("session"));
        assert!(dialog.selected_id().is_some());
    }

    // DIALOG-07: without a `current` prop upstream clamps the row index
    // instead of following identity (dialog-select.tsx:294-303).
    #[test]
    fn options_without_current_keep_the_row_index() {
        let mut dialog = SelectDialog::new(options());
        dialog.next();
        assert_eq!(dialog.selected_id(), Some("session"));
        let mut reordered = dialog.options().to_vec();
        reordered.reverse();
        // Reversed flat order: exit, new, session, model (rename disabled).
        dialog.set_options(reordered);
        assert_eq!(dialog.selected_id(), Some("new"));
    }

    // DIALOG-07: a temporary empty load keeps the intended selection.
    #[test]
    fn temporary_empty_results_keep_the_intended_selection() {
        let mut dialog = SelectDialog::new(options());
        dialog.set_current(Some("model"));
        dialog.set_filter("session");
        let intended = dialog.selected_id().expect("a match is highlighted").to_owned();
        dialog.set_options_pending(Vec::new());
        assert!(dialog.flat_list().is_empty());
        dialog.set_options(options());
        assert_eq!(dialog.selected_id(), Some(intended.as_str()));
    }

    // DIALOG-07: clearing the filter restores the current choice
    // (dialog-select.tsx:386-402).
    #[test]
    fn clearing_search_restores_current_choice() {
        let mut dialog = SelectDialog::new(options());
        dialog.set_current(Some("exit"));
        dialog.next();
        assert_eq!(dialog.selected_id(), Some("session"));
        dialog.set_filter("s");
        assert_ne!(dialog.selected_id(), Some("exit"));
        dialog.set_filter("");
        assert_eq!(dialog.selected_id(), Some("exit"));
        // focusCurrent=false keeps the row index instead of restoring current
        // (`dialog-select.tsx:386-403`).
        dialog.set_focus_current(false);
        dialog.set_filter("s");
        let row_index = dialog.selected_index().expect("a row is highlighted");
        dialog.set_filter("");
        assert_eq!(dialog.selected_index(), Some(row_index));
        assert_ne!(
            dialog.selected_id(),
            Some("exit"),
            "current is not restored while focusCurrent is false"
        );
    }

    // DIALOG-14: drill-down restores query and selection; back cannot
    // resurrect a deleted identity (dialog-open.tsx:103-128).
    #[test]
    fn drill_down_restores_query_and_selection_and_cannot_resurrect_deleted() {
        let mut dialog = SelectDialog::new(options());
        dialog.set_filter("session");
        let intended = dialog.selected_id().expect("a match is highlighted").to_owned();
        let mut stack = DrillDown::default();
        stack.push(&dialog);
        assert_eq!(stack.depth(), 1);
        dialog.set_filter("exit");
        assert_eq!(dialog.selected_id(), Some("exit"));
        assert!(stack.pop(&mut dialog));
        assert_eq!(dialog.filter(), "session");
        assert_eq!(dialog.selected_id(), Some(intended.as_str()));
        assert!(!stack.pop(&mut dialog));
        assert!(stack.is_empty());
    }

    #[test]
    fn drill_down_back_reconciles_a_deleted_identity() {
        let mut dialog = SelectDialog::new(options());
        dialog.next();
        assert_eq!(dialog.selected_id(), Some("session"));
        let mut stack = DrillDown::default();
        stack.push(&dialog);
        let survivors: Vec<_> = dialog
            .options()
            .iter()
            .filter(|option| option.id != "session")
            .cloned()
            .collect();
        dialog.set_options(survivors);
        assert!(stack.pop(&mut dialog));
        assert_eq!(dialog.filter(), "");
        assert_ne!(dialog.selected_id(), Some("session"));
        assert!(dialog.selected_id().is_some());
    }

    // DIALOG-14: a late load cannot resurrect a deletion or revert a move
    // (dialog-open.tsx:70-84).
    #[test]
    fn late_loads_cannot_resurrect_or_revert_local_changes() {
        let mut guard = LateLoadGuard::new();
        let pending = guard.generation();
        let mut entries = options();
        guard.remove("exit");
        // The deletion observed in flight is folded into the response; it
        // does not discard the response (`dialog-open.tsx:56-73`).
        let applied = guard
            .apply(pending, entries.clone())
            .expect("the in-flight response still applies");
        assert!(applied.iter().all(|option| option.id != "exit"));
        // A newer load supersedes the response of an older one.
        let older = guard.begin();
        let newer = guard.begin();
        assert!(guard.apply(older, entries.clone()).is_none());
        assert!(guard.apply(newer, entries.clone()).is_some());
        let current = guard.generation();
        guard.relocate("model", Some("Changed"));
        let entries = vec![SelectOption::new("model", "Switch model").with_category("Agent")];
        let applied = guard.apply(current, entries).expect("current generation");
        assert_eq!(applied[0].category.as_deref(), Some("Changed"));
    }

    // DIALOG-09: experiment switches are independently gated
    // (dialog-select.tsx:263-287,654,836,930-932,964-972).
    #[test]
    fn experiments_are_independently_gated() {
        let options = vec![SelectOption::new("a", "A").with_detail("facts")];
        let legacy = SelectDialog::new(options.clone());
        assert_eq!(current_marker(legacy.experiment()), "●");
        assert_eq!(active_gutter(legacy.experiment(), true), " ");
        assert!(!legacy.detail_reserved());
        assert!(!search_in_title(legacy.experiment(), true));
        assert_eq!(legacy.viewport_rows(40, true), 1);

        let markers_only = SelectDialog::new(options.clone()).with_experiment(SelectExperiment {
            new_markers: true,
            ..SelectExperiment::legacy()
        });
        assert_eq!(current_marker(markers_only.experiment()), "▪");
        assert_eq!(active_gutter(markers_only.experiment(), true), "┃");
        assert!(!markers_only.detail_reserved());
        assert!(!markers_only.experiment().ellipsis_row);

        let detail_only = SelectDialog::new(options.clone()).with_experiment(SelectExperiment {
            detail_reservation: true,
            ..SelectExperiment::legacy()
        });
        assert!(detail_only.detail_reserved());
        assert_eq!(detail_only.selected_detail(), Some("facts"));
        assert_eq!(current_marker(detail_only.experiment()), "●");

        let title_only = SelectDialog::new(options).with_experiment(SelectExperiment {
            search_in_title: true,
            ..SelectExperiment::legacy()
        });
        assert!(search_in_title(title_only.experiment(), true));
        assert!(!search_in_title(title_only.experiment(), false));
        assert!(!title_only.experiment().ellipsis_row);
    }

    // DIALOG-05: viewport budget matches floor(height/2)-6 and the
    // new-designs ellipsis row (dialog-select.tsx:262-277).
    #[test]
    fn viewport_budget_matches_pinned_height_formula() {
        assert_eq!(viewport_limit(23), 5);
        assert_eq!(viewport_limit(24), 6);
        assert_eq!(viewport_limit(25), 6);
        assert_eq!(viewport_limit(40), 14);
        assert_eq!(viewport_limit(0), 0);
        let many: Vec<_> = (0..30)
            .map(|index| SelectOption::new(format!("r{index}"), format!("R{index}")))
            .collect();
        let dialog = SelectDialog::new(many);
        assert_eq!(dialog.rows(), 30);
        assert_eq!(dialog.viewport_rows(24, true), 6);
        let design = SelectDialog::new(dialog.options().to_vec())
            .with_experiment(SelectExperiment::new_designs());
        assert_eq!(design.viewport_rows(24, true), 5);
        assert_eq!(design.viewport_rows(24, false), 6);
        // The "…" row takes the last row of the limit
        // (`dialog-select.tsx:262-273`).
        assert_eq!(design.viewport_rows(40, true), 13);
        assert_eq!(design.viewport_rows(40, false), 14);
    }

    // DIALOG-01: medium/large/xlarge widths, terminal-2 cap and quarter
    // height placement (dialog.tsx:30-34,61-84).
    #[test]
    fn dialog_geometry_matches_pinned_widths_and_placement() {
        assert_eq!(dialog_width(DialogSize::Medium, 120), 60);
        assert_eq!(dialog_width(DialogSize::Large, 120), 88);
        assert_eq!(dialog_width(DialogSize::XLarge, 120), 116);
        assert_eq!(dialog_width(DialogSize::Medium, 59), 57);
        assert_eq!(dialog_width(DialogSize::Large, 59), 57);
        assert_eq!(dialog_width(DialogSize::XLarge, 59), 57);
        assert_eq!(dialog_width(DialogSize::Medium, 1), 0);
        assert_eq!(dialog_top(59, false), 14);
        assert_eq!(dialog_top(60, false), 15);
        assert_eq!(dialog_top(24, false), 6);
        assert_eq!(dialog_top(59, true), 0);
    }

    // DIALOG-02: shared shell gaps, list row padding, footer band and the
    // 59/60-column responsive boundary (dialog.tsx:139-261,
    // dialog-select.tsx:125-128,915-920).
    #[test]
    fn dialog_shell_gaps_and_row_padding_match() {
        assert_eq!(content_width(60), 48);
        assert_eq!(content_width(57), 45);
        assert_eq!(row_padding(false, false), (3, 3));
        assert_eq!(row_padding(true, false), (1, 3));
        assert_eq!(row_padding(false, true), (3, 4));
        assert_eq!(row_padding(true, true), (1, 4));
        assert_eq!(list_body_inset(false), 3);
        assert_eq!(list_body_inset(true), 4);
        assert_eq!(body_bottom_padding(true, true, None), 0);
        assert_eq!(body_bottom_padding(true, true, Some(2)), 1);
        assert_eq!(body_bottom_padding(false, true, Some(2)), 0);
        assert_eq!(body_bottom_padding(true, false, None), 0);
        assert_eq!(footer_band_inset(0), 2);
        assert_eq!(footer_band_inset(DIALOG_LIST_INSET - DIALOG_INSET), 3);
        assert!(footer_is_narrow(59));
        assert!(!footer_is_narrow(60));
        assert_eq!(LAYOUT_GAP, 1);
        assert_eq!(ROW_TITLE_PADDING, 3);
    }

    // DIALOG-07: truncation helpers used by the row title and details
    // (dialog-select.tsx:946-951,987; util/locale.ts:45-84).
    #[test]
    fn row_titles_truncate_like_overlay_locale_helpers() {
        assert_eq!(truncate("Switch model", 61), "Switch model");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate_left("abcdef", 4), "…def");
        assert_eq!(truncate_middle("abcdefgh", 5), "ab…gh");
        let option = SelectOption::new("a", "A")
            .with_category("Session")
            .with_description("Session");
        assert_eq!(option.row_description(), None);
        let other = SelectOption::new("a", "A")
            .with_category("Session")
            .with_description("other");
        assert_eq!(other.row_description(), Some("other"));
    }

    // DIALOG-03: reactive updates do not remount; close only refocuses a
    // surviving target (dialog.tsx:320-343,399-419,472-479).
    #[test]
    fn dialog_stack_does_not_remount_on_reactive_updates() {
        let mut stack = DialogStack::new();
        stack.replace("commands", DialogSize::Medium, Some("editor"));
        assert!(stack.is_open());
        assert_eq!(stack.mounts(), 1);
        stack.set_size(DialogSize::Large);
        stack.set_centered(true);
        assert_eq!(stack.mounts(), 1);
        assert_eq!(stack.entry().map(|entry| entry.size), Some(DialogSize::Large));
        assert!(stack.is_centered());
        stack.replace("commands", DialogSize::Large, Some("editor"));
        assert_eq!(stack.mounts(), 2, "explicit replace mounts once");
        assert_eq!(stack.close(|_| true), Some("editor".to_owned()));
        assert!(!stack.is_open());
    }

    #[test]
    fn dialog_close_drops_a_destroyed_focus_target() {
        let mut stack = DialogStack::new();
        stack.replace("commands", DialogSize::Medium, Some("editor"));
        assert_eq!(stack.close(|_| false), None);
    }

    // DIALOG-04: Escape/Ctrl+C order — selection, focused editor, close
    // (dialog.tsx:345-386).
    #[test]
    fn escape_and_ctrl_c_dispatch_in_source_order() {
        let mut stack = DialogStack::new();
        stack.replace("commands", DialogSize::Medium, None);
        assert_eq!(stack.escape(true, |_| false), KeyOutcome::ClearedSelection);
        assert!(stack.is_open());
        assert_eq!(stack.escape(false, |_| true), KeyOutcome::Closed);

        stack.replace("commands", DialogSize::Medium, None);
        assert_eq!(
            stack.ctrl_c(true, Some("draft"), |_| true),
            KeyOutcome::ClearedSelection
        );
        assert_eq!(
            stack.ctrl_c(false, Some("draft"), |_| true),
            KeyOutcome::ClearedEditor
        );
        assert!(stack.is_open());
        assert_eq!(
            stack.ctrl_c(false, Some(""), |_| true),
            KeyOutcome::Closed
        );
    }

    // DIALOG-04: backdrop dismissal and panel release semantics
    // (dialog.tsx:51-79).
    #[test]
    fn backdrop_and_panel_release_follow_selection_state() {
        let mut stack = DialogStack::new();
        stack.replace("commands", DialogSize::Medium, None);
        stack.backdrop_down(true);
        assert!(!stack.backdrop_up(|_| true));
        assert!(stack.is_open());
        assert!(stack.backdrop_up(|_| true));
        assert!(!stack.is_open());

        let mut stack = DialogStack::new();
        stack.replace("commands", DialogSize::Medium, None);
        assert_eq!(stack.panel_up(Some("copied")), PanelMouseUp::Bubbles);
        assert!(stack.is_open());
        assert_eq!(stack.panel_up(Some("")), PanelMouseUp::Stop);
        assert!(stack.is_open());
    }

    // DIALOG-10: busy prompt suspends editing, submission and cancel, and
    // the submit layer has priority 1 (dialog-prompt.tsx:43-103).
    #[test]
    fn prompt_busy_suspends_editing_submission_and_cancel() {
        let mut prompt = PromptState::open("start");
        assert!(prompt.is_focused());
        assert!(prompt.push('!'));
        assert_eq!(prompt.submit(), Some("start!"));
        prompt.set_busy(true);
        assert!(!prompt.is_focused());
        assert!(!prompt.push('x'));
        assert!(!prompt.backspace());
        assert!(!prompt.focus());
        assert_eq!(prompt.submit(), None);
        assert!(!prompt.extra_actions_enabled());
        assert!(!prompt.escape());
        prompt.set_busy(false);
        assert!(prompt.is_focused());
        assert!(prompt.escape());
        assert_eq!(prompt.submit(), Some("start!"));
        assert_eq!(PromptState::SUBMIT_PRIORITY, 1);
    }

    // DIALOG-10: extra actions and the busy line
    // (dialog-prompt.tsx:8-14,52-73,100-140,168-170).
    #[test]
    fn prompt_actions_and_busy_text_follow_upstream() {
        let mut prompt = PromptState::open("q");
        // Not busy: the busy line is absent.
        assert_eq!(prompt.busy_line(), None);
        // Registered actions are triggerable while idle.
        assert!(prompt.add_action(DialogPromptAction::new("rewrite", "prompt.rewrite")));
        let mut titled = DialogPromptAction::new("toggle", "prompt.toggle");
        titled.command_title = Some("Toggle value".into());
        assert!(prompt.add_action(titled));
        assert_eq!(prompt.actions().len(), 2);
        assert_eq!(prompt.actions()[0].command_title(), "rewrite", "falls back to title");
        assert_eq!(prompt.actions()[1].command_title(), "Toggle value");
        // Busy: actions vanish and the default busy line shows.
        prompt.set_busy(true);
        assert!(prompt.actions().is_empty(), "actions are disabled while busy");
        assert_eq!(prompt.busy_line(), Some("Working…"));
        prompt.set_busy_text(Some("Summoning".into()));
        assert_eq!(prompt.busy_text(), Some("Summoning"));
        assert_eq!(prompt.busy_line(), Some("Summoning"));
        // Idle again: the custom text is retained but no line is painted.
        prompt.set_busy(false);
        assert_eq!(prompt.actions().len(), 2);
        assert_eq!(prompt.busy_line(), None);
        // The action cap is enforced.
        let mut capped = PromptState::open("");
        for i in 0..MAX_PROMPT_ACTIONS {
            assert!(capped.add_action(DialogPromptAction::new(format!("a{i}"), "c")));
        }
        assert!(!capped.add_action(DialogPromptAction::new("overflow", "c")));
        assert_eq!(capped.actions().len(), MAX_PROMPT_ACTIONS);
    }

    // DIALOG-10: confirmation legacy and new-design key variants
    // (dialog-confirm.tsx:28-99).
    #[test]
    fn confirmation_legacy_and_new_design_key_variants() {
        let mut legacy = ConfirmState::legacy();
        assert_eq!(legacy.active(), ConfirmChoice::Confirm);
        assert_eq!(legacy.key(ConfirmKey::Left), ConfirmOutcome::Toggle);
        assert_eq!(legacy.active(), ConfirmChoice::Cancel);
        assert_eq!(legacy.key(ConfirmKey::Enter), ConfirmOutcome::Cancel);
        assert_eq!(legacy.key(ConfirmKey::Right), ConfirmOutcome::Toggle);
        assert_eq!(legacy.key(ConfirmKey::Enter), ConfirmOutcome::Confirm);
        assert_eq!(legacy.key(ConfirmKey::Escape), ConfirmOutcome::Ignored);
        assert_eq!(ConfirmState::button_side_padding(), 1);
        assert_eq!(ConfirmState::footer_label(ConfirmChoice::Confirm), "confirm");
        assert_eq!(ConfirmState::button_label(ConfirmChoice::Confirm), "Confirm");

        let mut design = ConfirmState::new_designs();
        assert_eq!(design.active(), ConfirmChoice::Confirm);
        assert_eq!(design.key(ConfirmKey::Left), ConfirmOutcome::Ignored);
        assert_eq!(design.key(ConfirmKey::Right), ConfirmOutcome::Ignored);
        assert_eq!(design.active(), ConfirmChoice::Confirm);
        assert_eq!(design.key(ConfirmKey::Escape), ConfirmOutcome::Cancel);
        assert_eq!(design.key(ConfirmKey::Enter), ConfirmOutcome::Confirm);
    }

    // DIALOG-12: favorites, recents, deprecated filtering and the free group
    // (dialog-model.tsx:100-202, model-options.tsx:7-40).
    #[test]
    fn model_picker_orders_favorites_recents_then_free_and_hides_deprecated() {
        let models = vec![
            ModelCandidate {
                provider: "anthropic".into(),
                id: "claude".into(),
                title: "Claude".into(),
                free: false,
                deprecated: false,
            },
            ModelCandidate {
                provider: "anthropic".into(),
                id: "legacy".into(),
                title: "Legacy".into(),
                free: false,
                deprecated: true,
            },
            ModelCandidate {
                provider: "opencode".into(),
                id: "free-a".into(),
                title: "Free A".into(),
                free: true,
                deprecated: false,
            },
            ModelCandidate {
                provider: "opencode".into(),
                id: "paid-a".into(),
                title: "Paid A".into(),
                free: false,
                deprecated: false,
            },
        ];
        let favorites = vec![("anthropic".to_string(), "claude".to_string())];
        let recents = vec![("opencode".to_string(), "paid-a".to_string())];
        let ordered = order_models(&models, &favorites, &recents, true, false);
        let ids: Vec<_> = ordered.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids[0], "claude");
        assert_eq!(ids[1], "paid-a");
        assert_eq!(*ids.last().unwrap(), "free-a", "free group renders last");
        assert!(!ids.contains(&"legacy"));

        // Without the new designs the free row has no separate group.
        let ordered = order_models(&models, &[], &[], false, false);
        assert_eq!(ordered[0].id, "free-a");
        // A provider-restricted picker never creates the free group.
        let ordered = order_models(&models, &[], &[], true, true);
        assert_eq!(ordered[0].id, "free-a");
    }

    // DIALOG-12: search hoists favorites and drops deprecated models
    // (dialog-model.tsx:182-194).
    #[test]
    fn model_search_prioritizes_favorites() {
        let models = vec![
            ModelCandidate {
                provider: "a".into(),
                id: "one".into(),
                title: "Claude Sonnet".into(),
                free: false,
                deprecated: false,
            },
            ModelCandidate {
                provider: "a".into(),
                id: "two".into(),
                title: "Claude Haiku".into(),
                free: false,
                deprecated: false,
            },
            ModelCandidate {
                provider: "a".into(),
                id: "old".into(),
                title: "Claude Legacy".into(),
                free: false,
                deprecated: true,
            },
        ];
        let results = search_models(
            "claude",
            &models,
            &[("a".to_string(), "two".to_string())],
        );
        let ids: Vec<_> = results.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, vec!["two", "one"]);
    }

    // DIALOG-13: debounce, stale responses, scope persistence, search copy,
    // grouping and armed deletion (dialog-session-list.tsx:39-122,142-296).
    #[test]
    fn session_picker_debounces_and_ignores_stale_responses() {
        let row = |id: &str, title: &str, directory: &str| SessionRow {
            id: id.into(),
            title: title.into(),
            parent: false,
            updated_date: "2026-10-30".into(),
            pinned: false,
            directory: directory.into(),
        };
        let local = vec![row("a", "Alpha", "/work"), row("b", "Beta", "/other")];
        let server = vec![row("s", "Server one", "/work")];

        let mut picker = SessionPicker::new(true, "/work", "2026-10-30");
        assert_eq!(SESSION_SEARCH_DEBOUNCE_MS, 150);
        assert!(!picker.all_projects(), "cwd scope persists from config");
        assert_eq!(picker.search_message(), "Loading sessions…");
        assert!(picker.set_filter("one"));
        assert_eq!(picker.search_state(), SessionSearchState::Searching);
        assert!(picker.debounce_elapsed());
        assert_eq!(picker.debounced(), "one");
        // A response for the old (empty) query is stale and degrades to the
        // local list, which has no row matching "one".
        let rows = picker.apply_result("", false, false, server.clone(), &local);
        assert_eq!(rows.len(), 0, "stale response is dropped");
        let rows = picker.apply_result("one", false, false, server.clone(), &local);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "s");

        let rows = picker.apply_result("one", true, false, server.clone(), &local);
        assert!(rows.is_empty(), "scope mismatch is stale");
        let rows = picker.apply_result("one", false, true, server.clone(), &local);
        assert!(rows.is_empty(), "error degrades to the filtered local list");
        assert_eq!(
            picker.search_message(),
            "Could not search sessions. Change the search to try again."
        );

        let mut picker = SessionPicker::new(true, "/work", "2026-10-30");
        // A failed request degrades to the local list, which is scoped to the
        // current directory while `allProjects` is off
        // (`dialog-session-list.tsx:102-110`).
        let rows = picker.apply_result("", false, true, Vec::new(), &local);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "a", "cwd scope drops other directories");
        picker.toggle_scope();
        assert!(picker.all_projects());
        let rows = picker.apply_result("", true, true, Vec::new(), &local);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn session_picker_arms_deletion_and_groups_rows() {
        let mut picker = SessionPicker::new(false, "/work", "2026-10-30");
        let row = SessionRow {
            id: "a".into(),
            title: "Alpha".into(),
            parent: false,
            updated_date: "2026-10-30".into(),
            pinned: false,
            directory: "/work".into(),
        };
        assert!(!picker.arm_delete("a"));
        assert!(picker.is_armed("a"));
        assert_eq!(picker.row_title(&row, "ctrl+d"), "Press ctrl+d again to confirm");
        picker.on_move();
        assert!(!picker.is_armed("a"));
        assert_eq!(picker.row_title(&row, "ctrl+d"), "Alpha");
        assert!(!picker.arm_delete("a"));
        assert!(picker.arm_delete("a"), "second press deletes");
        assert!(!picker.is_armed("a"));
        assert_eq!(picker.group_of(&row), "Today");
        let pinned = SessionRow {
            pinned: true,
            updated_date: "2026-10-01".into(),
            ..row.clone()
        };
        assert_eq!(picker.group_of(&pinned), "Pinned");
        let older = SessionRow {
            updated_date: "2026-10-01".into(),
            ..row
        };
        assert_eq!(picker.group_of(&older), "2026-10-01");
    }

    // DIALOG-15: MCP pending/error/auth states stay distinct
    // (dialog-mcp.tsx:14-33,84-124).
    #[test]
    fn mcp_states_have_distinct_labels_actions_and_errors() {
        let states = [
            McpStatus::Pending,
            McpStatus::Connected,
            McpStatus::Failed {
                error: "boom".into(),
            },
            McpStatus::NeedsAuth {
                error: "login".into(),
                integration: None,
            },
            McpStatus::NeedsAuth {
                error: "login".into(),
                integration: Some("openai".into()),
            },
            McpStatus::Disabled,
        ];
        let labels: Vec<_> = states.iter().map(McpStatus::label).collect();
        assert_eq!(
            labels,
            vec![
                "Connecting …",
                "Connected ✓",
                "Failed !",
                "Sign in required →",
                "Sign in required →",
                "Disabled ○"
            ]
        );
        assert_eq!(McpStatus::Connected.toggle_title(), "disconnect");
        assert_eq!(
            McpStatus::Failed {
                error: String::new()
            }
            .toggle_title(),
            "retry"
        );
        assert_eq!(states[3].toggle_title(), "sign in");
        assert_eq!(McpStatus::Pending.toggle_title(), "connect");
        assert_eq!(mcp_enter(&states[3]), McpEnter::ErrorDetails);
        assert_eq!(mcp_enter(&states[4]), McpEnter::SignIn);
        assert_eq!(mcp_enter(&states[2]), McpEnter::ErrorDetails);
        assert_eq!(mcp_enter(&states[1]), McpEnter::Ignore);
        assert_eq!(states[2].error(), Some("boom"));
        assert_eq!(states[1].error(), None);
    }

    // DIALOG-15: integration method/account/form/progress/cancel phases
    // (dialog-integration.tsx:118-150,290-336,388-439).
    #[test]
    fn integration_phases_cover_accounts_methods_progress_and_cancel() {
        assert_eq!(
            IntegrationPhase::entry(0, 0),
            IntegrationPhase::EnvironmentOnly
        );
        assert_eq!(IntegrationPhase::entry(1, 0), IntegrationPhase::Form);
        assert_eq!(IntegrationPhase::entry(2, 0), IntegrationPhase::Methods);
        assert_eq!(IntegrationPhase::entry(2, 1), IntegrationPhase::Accounts);

        let mut phase = IntegrationPhase::Methods;
        phase.start(ConnectMethod::Command);
        assert_eq!(phase, IntegrationPhase::Progress("Starting command…"));
        let mut phase = IntegrationPhase::Form;
        phase.form_answer(false);
        assert_eq!(phase, IntegrationPhase::Form, "cancelled form keeps dialog");
        phase.form_answer(true);
        assert_eq!(phase, IntegrationPhase::Connected);
        phase.cancel();
        assert_eq!(phase, IntegrationPhase::Cancelled);
        assert!(phase.is_closed());
        let mut phase = IntegrationPhase::Progress("Connecting…");
        phase.fail();
        assert!(phase.is_closed());
        assert_eq!(ConnectMethod::External.progress_message(), "Connecting…");
    }

    // DIALOG-16: theme preview reverts on cancel, keeps on select and
    // resets when the filter clears (dialog-theme-list.tsx:14-65).
    #[test]
    fn theme_preview_reverts_without_confirmation() {
        let mut preview = ThemePreview::open("opencode");
        preview.move_to("light");
        assert_eq!(preview.current(), "light");
        assert_eq!(preview.cancel(), "opencode");
        preview.move_to("light");
        assert_eq!(preview.select(), "light");
        assert_eq!(preview.cancel(), "light", "confirmed preview is kept");
        preview.on_filter("", Some("dark"));
        assert_eq!(preview.current(), "opencode");
        let first = String::from("light");
        preview.on_filter("li", Some(&first));
        assert_eq!(preview.current(), "light");
        preview.on_filter("zzz", None);
        assert_eq!(preview.current(), "light", "no result keeps the preview");
    }

    // DIALOG-17: stash/workspace armed deletion and file-change confirmation
    // (dialog-stash.tsx:39-85, dialog-workspaces.tsx:181-245,
    // dialog-workspace-file-changes.tsx:62-105).
    #[test]
    fn destructive_pickers_require_a_second_press() {
        let mut armed = ArmedDelete::default();
        assert!(!armed.press("stash-1"));
        assert_eq!(
            armed.title("stash-1", "old prompt", "ctrl+d"),
            "Press ctrl+d again to confirm"
        );
        assert_eq!(armed.title("stash-2", "other", "ctrl+d"), "other");
        armed.clear();
        assert!(!armed.press("stash-1"));
        assert!(armed.press("stash-1"));
        assert_eq!(armed.armed(), None);

        let default = FileChangesConfirm::default();
        assert_eq!(default.resolve("return"), Some(true));
        assert_eq!(default.resolve("enter"), Some(true));
        assert_eq!(default.resolve("esc"), Some(false));
        assert_eq!(default.resolve("d"), None);
        let delete = FileChangesConfirm::delete();
        assert_eq!(delete.confirm_title, "delete");
        assert_eq!(delete.resolve("d"), Some(true));
        assert_eq!(delete.resolve("return"), None);
        assert_eq!(delete.resolve("esc"), Some(false));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(id: &'static str, title: &str, action: CommandAction, gate: CommandGate) -> Command {
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
        let err = p
            .execute("remote.pair")
            .expect_err("gated command must not dispatch");
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
        assert!(!meets_contrast(
            Rgb::new(0x77, 0x77, 0x77),
            Rgb::new(0x88, 0x88, 0x88)
        ));
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
