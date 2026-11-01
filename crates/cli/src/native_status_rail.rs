#![forbid(unsafe_code)]
// This module is the tested model that the renderer integration package will
// consume; until that wiring lands every item here is unreachable from the
// non-test build, so dead-code lints would only report the pending hookup.
#![allow(dead_code)]
//! Native prompt footer rail, prompt scanner and bottom-stack precedence
//! (package P09: CP-11..CP-16, CP-19..CP-22).
//!
//! Upstream evidence, pinned at
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22`. The upstream tree stores these
//! files under `packages/tui/src/` (the package brief's
//! `packages/opencode/src/cli/cmd/tui/` path is stale for this pin):
//! - `packages/tui/src/feature-plugins/prompt/footer.tsx:14-132` — footer
//!   plugin: live subagent/shell counts, usage/cost status list, the
//!   `promptFooterLayout` fitting function and the disabled/hover styling.
//! - `packages/tui/src/feature-plugins/prompt/footer.tsx:146-156`
//!   (`promptFooterLayout`) — reserve, fit and drop order.
//! - `packages/tui/src/component/prompt/index.tsx:1900-1983` — the footer row
//!   and its slot order: `prompt.footer` (host default: `prompt.footer.status`
//!   then `prompt.footer.file`) with the plugin's `append` contribution last.
//! - `packages/tui/src/plugin/render.tsx:130-160` — `Slot` paints
//!   `prepend`, host children, `append` in that order, which is why the
//!   built-in footer plugin lands to the right of the location/file slots.
//! - `packages/tui/src/component/prompt/index.tsx:1911-1965` — the exclusive
//!   status-slot switch (running scanner + interrupt, worktree move progress,
//!   pending new worktree, idle location).
//! - `packages/tui/src/component/prompt/index.tsx:109-146`
//!   (`PromptInterruptStatus`) and `:500-531` (`session.interrupt`) —
//!   interrupt arming, 5000 ms expiry, 0.22 s flash, interceptions.
//! - `packages/tui/src/component/prompt/index.tsx:1609-1613` — `showDetails =
//!   store.interrupt === 0 || dimensions().width >= 80`.
//! - `packages/tui/src/ui/spinner.ts:246-329` — eight-cell knight-rider
//!   scanner (`createFrames`, `getScannerState`, `calculateColorIndex`).
//! - `packages/tui/src/component/one-cell-spinner.tsx:64-75` and
//!   `packages/tui/src/ui/one-cell-motion.ts:13-32,111` — the one-cell
//!   spinner frame cycle and its disabled-animation `still` glyph.
//! - `packages/tui/src/ui/file-path.tsx:31-129` (`truncateFilePath`) and
//!   `packages/tui/src/util/path-format.ts:3-10` (`abbreviateHome`) — location
//!   abbreviation and truncation.
//! - `packages/tui/src/util/locale.ts:89-97` (`truncateMiddle`) and
//!   `packages/tui/src/component/prompt/index.tsx:242-257` — editor-file label
//!   composition and its `max(12, min(48, width / 3))` budget.
//! - `packages/tui/src/routes/session/index.tsx:1590-1658` — bottom-stack
//!   precedence (composer, permission, form, missing location, prompt) and the
//!   queue dock gate.
//! - `packages/tui/src/routes/session/index.tsx:2485-2513`
//!   (`QueuedPromptDock`) — dock dimensions, hover background, label.
//!
//! Scope honesty: this module is pure text/state. It renders no colors, owns
//! no terminal and performs no IO; it chooses span text, span order and the
//! semantic role the view maps onto the active theme. Nothing is inferred
//! from timers or wall-clock: every dynamic value (elapsed animation time,
//! interrupt state, counts, usage strings) is a caller-supplied fact.

use crate::native_status::live_counts_label;

/// Upstream `footer.tsx:59`: `Math.max(0, dimensions().width - 8)` — the
/// fitting width is the terminal width minus the composer's horizontal
/// padding (2 columns each side, doubled for the border column).
pub const RAIL_TERMINAL_INSET: usize = 8;
/// Upstream `footer.tsx:149`: the reserve for the right-hand side is
/// `Math.min(28, Math.floor(width / 2))`.
pub const LAYOUT_RESERVE_MAX: usize = 28;
/// Upstream `footer.tsx:57`: with an empty usage/cost list the shortcut block
/// is gated on `dimensions().width >= 44`, not on measured text width.
pub const SHORTCUTS_MIN_WIDTH: usize = 44;
/// Upstream `index.tsx:1912`: reduced-animation scanner replacement.
pub const SCANNER_STATIC: &str = "[⋯]";
/// Upstream `one-cell-spinner.tsx:66`: disabled-animation fallback glyph.
pub const ONE_CELL_STILL: &str = "▪";
/// Upstream `index.tsx:1612`: details survive an armed interrupt only at or
/// above this terminal width.
pub const INTERRUPT_DETAILS_MIN_WIDTH: usize = 80;
/// Upstream `index.tsx:520`: an armed interrupt resets after 5000 ms.
pub const INTERRUPT_EXPIRY_MS: u64 = 5000;
/// Upstream `index.tsx:119`: arming flash tween duration (0.22 s).
pub const INTERRUPT_FLASH_MS: u64 = 220;
/// Upstream `index.tsx:126`: the flash jumps to 0.75 and fades to zero.
pub const INTERRUPT_FLASH_PEAK: f32 = 0.75;

/// Eight-cell scanner cell count (`spinner.ts:273` default `width ?? 8`).
pub const SCANNER_WIDTH: usize = 8;
/// `spinner.ts:275` default `holdStart ?? 30`.
pub const SCANNER_HOLD_START_FRAMES: usize = 30;
/// `spinner.ts:276` default `holdEnd ?? 9`.
pub const SCANNER_HOLD_END_FRAMES: usize = 9;
/// `spinner.ts:257` default `trailSteps ?? 6` (via `deriveTrailColors`).
pub const SCANNER_TRAIL_STEPS: usize = 6;
/// `index.tsx:1916`: `<spinner ... interval={40} />`.
pub const SCANNER_INTERVAL_MS: u64 = 40;
/// `spinner.ts:306`: forward + hold end + backward + hold start.
pub const SCANNER_TOTAL_FRAMES: usize =
    SCANNER_WIDTH + SCANNER_HOLD_END_FRAMES + (SCANNER_WIDTH - 1) + SCANNER_HOLD_START_FRAMES;
/// `spinner.ts:324`: active cell glyph in `blocks` style.
pub const SCANNER_ACTIVE_GLYPH: char = '■';
/// `spinner.ts:324`: inactive cell glyph in `blocks` style.
pub const SCANNER_INACTIVE_GLYPH: char = '⬝';
/// `one-cell-motion.ts:111` (`SEED_MONO`) frame list and interval.
pub const ONE_CELL_MONO_FRAMES: [&str; 4] = ["-", "\\", "|", "/"];
pub const ONE_CELL_MONO_INTERVAL_MS: u64 = 120;
/// `index.tsx:1648` / `locale.ts:89-91`: editor label budget bounds.
pub const EDITOR_LABEL_MIN_WIDTH: usize = 12;
pub const EDITOR_LABEL_MAX_WIDTH: usize = 48;

// ---------------------------------------------------------------------------
// Footer fitting (CP-19)
// ---------------------------------------------------------------------------

/// Which footer blocks survive fitting at one terminal width
/// (`footer.tsx:146-156`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FooterLayout {
    pub usage: bool,
    pub shortcuts: bool,
}

/// The layout width handed to `promptFooterLayout` (`footer.tsx:59`).
#[must_use]
pub fn layout_width(terminal_width: usize) -> usize {
    terminal_width.saturating_sub(RAIL_TERMINAL_INSET)
}

/// `footer.tsx:149`: `width - Math.min(28, Math.floor(width / 2))`, computed
/// on the already-inset layout width.
#[must_use]
pub fn layout_available_width(terminal_width: usize) -> usize {
    let width = layout_width(terminal_width);
    width.saturating_sub(LAYOUT_RESERVE_MAX.min(width / 2))
}

/// `footer.tsx:55-63,146-156`: the footer fitting model.
///
/// With no usage status the shortcut block is shown purely at
/// `terminal_width >= 44`. Otherwise the joined usage list and the joined
/// shortcut list compete for `available`: both, then usage alone, then
/// shortcuts alone, then neither. Note the upstream rule is that usage always
/// outranks shortcuts when only one can survive.
#[must_use]
pub fn prompt_footer_layout(
    terminal_width: usize,
    status: &[String],
    command_shortcut: Option<&str>,
) -> FooterLayout {
    if status.is_empty() {
        return FooterLayout {
            usage: false,
            shortcuts: terminal_width >= SHORTCUTS_MIN_WIDTH,
        };
    }
    let usage = status.join(" · ");
    let shortcuts = command_shortcut
        .map(|command| format!("{command} commands"))
        .unwrap_or_default();
    let available = layout_available_width(terminal_width);
    let usage_fits = !usage.is_empty() && cell_width(&usage) <= available;
    let shortcuts_fit = !shortcuts.is_empty() && cell_width(&shortcuts) <= available;
    if usage_fits
        && shortcuts_fit
        && cell_width(&format!("{usage} · {shortcuts}")) <= available
    {
        return FooterLayout {
            usage: true,
            shortcuts: true,
        };
    }
    if usage_fits {
        return FooterLayout {
            usage: true,
            shortcuts: false,
        };
    }
    if !usage.is_empty() {
        // Usage exists but does not fit: upstream falls through to
        // `{ usage: false, shortcuts: false }`; the shortcut-only branch is
        // reached only when there is no usage list at all.
        return FooterLayout {
            usage: false,
            shortcuts: false,
        };
    }
    FooterLayout {
        usage: false,
        shortcuts: shortcuts_fit,
    }
}

/// `index.tsx:1609-1613`: `store.interrupt === 0 || dimensions().width >= 80`.
#[must_use]
pub fn show_details(interrupt_armed: bool, terminal_width: usize) -> bool {
    !interrupt_armed || terminal_width >= INTERRUPT_DETAILS_MIN_WIDTH
}

// ---------------------------------------------------------------------------
// Interrupt arming (CP-13)
// ---------------------------------------------------------------------------

/// Why an interrupt press did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptIgnore {
    /// `session.interrupt` is `enabled: status() === "running"`
    /// (`index.tsx:502`), so a keymap dispatch while idle is a no-op.
    NotRunning,
    /// `index.tsx:515`: an open autocomplete consumes the key.
    AutocompleteOpen,
    /// `index.tsx:516`: only the focused composer arms an interrupt.
    InputUnfocused,
    /// `index.tsx:521`: no session to interrupt.
    NoSession,
}

/// Outcome of one `session.interrupt` press (`index.tsx:500-531`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptOutcome {
    /// `index.tsx:518-520`: shell mode only leaves shell mode.
    ExitShellMode,
    /// The press was intercepted before touching the arm state.
    Ignored(InterruptIgnore),
    /// First press inside the window: armed.
    Armed(u8),
    /// Second press inside the window: the caller must interrupt the session
    /// (`resume: true`) and the arm resets to zero.
    Triggered,
}

/// Interrupt arming state (`index.tsx:500-531`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InterruptArm {
    count: u8,
    deadline_ms: Option<u64>,
    armed_at_ms: Option<u64>,
}

impl InterruptArm {
    #[must_use]
    pub fn count(self) -> u8 {
        self.count
    }

    #[must_use]
    pub fn is_armed(self) -> bool {
        self.count > 0
    }

    /// Expiry instant of the current arm.
    #[must_use]
    pub fn deadline_ms(self) -> Option<u64> {
        self.deadline_ms
    }

    /// Instant the arm was (re)raised, for the 0.22 s flash.
    #[must_use]
    pub fn armed_at_ms(self) -> Option<u64> {
        self.armed_at_ms
    }

    /// `index.tsx:520`: `setTimeout(() => setStore("interrupt", 0), 5000)`.
    /// The timer fires at or after the deadline, so the arm is gone at
    /// `now >= deadline` and still armed at `deadline - 1`.
    pub fn expire_at(&mut self, now_ms: u64) {
        if self.deadline_ms.is_some_and(|deadline| now_ms >= deadline) {
            self.count = 0;
            self.deadline_ms = None;
            self.armed_at_ms = None;
        }
    }

    /// One interrupt press with the live view facts, in upstream order:
    /// enablement, autocomplete, focus, shell mode, session id.
    pub fn press(
        &mut self,
        now_ms: u64,
        running: bool,
        autocomplete_open: bool,
        input_focused: bool,
        shell_mode: bool,
        has_session: bool,
    ) -> InterruptOutcome {
        self.expire_at(now_ms);
        if !running {
            return InterruptOutcome::Ignored(InterruptIgnore::NotRunning);
        }
        if autocomplete_open {
            return InterruptOutcome::Ignored(InterruptIgnore::AutocompleteOpen);
        }
        if !input_focused {
            return InterruptOutcome::Ignored(InterruptIgnore::InputUnfocused);
        }
        if shell_mode {
            return InterruptOutcome::ExitShellMode;
        }
        if !has_session {
            return InterruptOutcome::Ignored(InterruptIgnore::NoSession);
        }
        self.count = self.count.saturating_add(1);
        if self.count >= 2 {
            self.count = 0;
            self.deadline_ms = None;
            self.armed_at_ms = None;
            return InterruptOutcome::Triggered;
        }
        self.deadline_ms = Some(now_ms.saturating_add(INTERRUPT_EXPIRY_MS));
        self.armed_at_ms = Some(now_ms);
        InterruptOutcome::Armed(self.count)
    }
}

/// `index.tsx:117-136`: arming jumps the flash to 0.75 and tweens it to zero
/// over 220 ms; with animations disabled the level stays at zero, so the armed
/// color is the plain warning color. `tween` defaults to the `smoothstep` ease
/// (`ui/animation.ts:32,46-47,177-183`), so `level = 0.75 * (1 - smoothstep(p))`
/// with `p = elapsed / 220`; a linear ramp would diverge from the painted color.
#[must_use]
pub fn interrupt_flash_level(animations: bool, elapsed_since_arm_ms: u64) -> f32 {
    if !animations || elapsed_since_arm_ms >= INTERRUPT_FLASH_MS {
        return 0.0;
    }
    let progress = elapsed_since_arm_ms as f32 / INTERRUPT_FLASH_MS as f32;
    let eased = smoothstep(progress);
    INTERRUPT_FLASH_PEAK * (1.0 - eased)
}

/// `ui/animation.ts:32`: `tween`'s default ease. `progress * progress * (3 - 2 * progress)`.
#[must_use]
fn smoothstep(progress: f32) -> f32 {
    progress * progress * (3.0 - 2.0 * progress)
}

// ---------------------------------------------------------------------------
// Eight-cell prompt scanner (CP-12)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ScannerState {
    position: i32,
    holding: bool,
    hold_progress: i32,
    forward: bool,
}

/// `spinner.ts:25-83` (`getScannerState`) for the `bidirectional` direction
/// and the prompt defaults (`holdStart` 30, `holdEnd` 9).
fn scanner_state(frame_index: usize) -> ScannerState {
    let frame = frame_index % SCANNER_TOTAL_FRAMES;
    let forward_frames = SCANNER_WIDTH;
    let hold_end_frames = SCANNER_HOLD_END_FRAMES;
    let backward_frames = SCANNER_WIDTH - 1;
    if frame < forward_frames {
        ScannerState {
            position: frame as i32,
            holding: false,
            hold_progress: 0,
            forward: true,
        }
    } else if frame < forward_frames + hold_end_frames {
        ScannerState {
            position: SCANNER_WIDTH as i32 - 1,
            holding: true,
            hold_progress: (frame - forward_frames) as i32,
            forward: true,
        }
    } else if frame < forward_frames + hold_end_frames + backward_frames {
        let backward_index = frame - forward_frames - hold_end_frames;
        ScannerState {
            position: SCANNER_WIDTH as i32 - 2 - backward_index as i32,
            holding: false,
            hold_progress: 0,
            forward: false,
        }
    } else {
        ScannerState {
            position: 0,
            holding: true,
            hold_progress: (frame - forward_frames - hold_end_frames - backward_frames) as i32,
            forward: false,
        }
    }
}

/// `spinner.ts:106-139` (`calculateColorIndex`). `-1` means "inactive".
fn scanner_color_index(frame_index: usize, char_index: usize) -> i32 {
    let state = scanner_state(frame_index);
    let distance = if state.forward {
        state.position - char_index as i32
    } else {
        char_index as i32 - state.position
    };
    if state.holding {
        return distance + state.hold_progress;
    }
    if distance > 0 && (distance as usize) < SCANNER_TRAIL_STEPS {
        return distance;
    }
    if distance == 0 {
        return 0;
    }
    -1
}

/// One scanner frame (`spinner.ts:309-326`) in `blocks` style
/// (`index.tsx:1658-1664`).
#[must_use]
pub fn scanner_frame(frame_index: usize) -> String {
    (0..SCANNER_WIDTH)
        .map(|char_index| {
            let index = scanner_color_index(frame_index, char_index);
            if (0..SCANNER_TRAIL_STEPS as i32).contains(&index) {
                SCANNER_ACTIVE_GLYPH
            } else {
                SCANNER_INACTIVE_GLYPH
            }
        })
        .collect()
}

/// All 54 scanner frames (forward 8, hold end 9, backward 7, hold start 30).
#[must_use]
pub fn scanner_frames() -> Vec<String> {
    (0..SCANNER_TOTAL_FRAMES).map(scanner_frame).collect()
}

/// Frame selected by elapsed animation time: the prompt `<spinner>` advances
/// one frame per 40 ms and wraps (`index.tsx:1916`, `spinner.ts:306`).
#[must_use]
pub fn scanner_frame_index(elapsed_ms: u64) -> usize {
    ((elapsed_ms / SCANNER_INTERVAL_MS) as usize) % SCANNER_TOTAL_FRAMES
}

/// Scanner frame at an elapsed time.
#[must_use]
pub fn scanner_frame_at(elapsed_ms: u64) -> String {
    scanner_frame(scanner_frame_index(elapsed_ms))
}

/// `index.tsx:1912-1917`: `config.animations ?? true`; when animations are
/// disabled the rail renders the static `[⋯]` marker instead of the scanner.
#[must_use]
pub fn scanner_render(animations: bool, elapsed_ms: u64) -> String {
    if animations {
        scanner_frame_at(elapsed_ms)
    } else {
        SCANNER_STATIC.to_owned()
    }
}

// ---------------------------------------------------------------------------
// One-cell spinner (mini footer sibling of the scanner)
// ---------------------------------------------------------------------------

/// `one-cell-motion.ts:13-32` (`oneCellFrame`) without intro/pace/once: the
/// frame advances every `interval` and wraps.
#[must_use]
pub fn one_cell_frame(elapsed_ms: u64) -> &'static str {
    let count = ONE_CELL_MONO_FRAMES.len() as u64;
    ONE_CELL_MONO_FRAMES[((elapsed_ms / ONE_CELL_MONO_INTERVAL_MS) % count) as usize]
}

/// `one-cell-spinner.tsx:64-75`: animations disabled renders the caller's
/// `still` text, defaulting to U+25AA.
#[must_use]
pub fn one_cell_render(animations: bool, elapsed_ms: u64, still: Option<&'static str>) -> &'static str {
    if animations {
        one_cell_frame(elapsed_ms)
    } else {
        still.unwrap_or(ONE_CELL_STILL)
    }
}

// ---------------------------------------------------------------------------
// Status slot branches (CP-11)
// ---------------------------------------------------------------------------

/// Composer mode (`index.tsx:1609-1613`); orthogonal to the session status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptMode {
    Normal,
    Shell,
}

/// Exclusive content of `prompt.footer.status` (`index.tsx:1911-1965`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusSlot {
    /// `status() === "running"`.
    Running,
    /// `move.progress()`.
    MoveProgress,
    /// `move.pendingNew()`.
    PendingNew,
    /// Idle: the working-directory label.
    Location,
}

/// Ground-truth projection of the footer's live facts.
#[derive(Clone, Copy, Debug)]
pub struct RailFacts<'a> {
    /// Full terminal width (`useTerminalDimensions`).
    pub terminal_width: usize,
    /// Measured width of the `prompt.footer.status` box, used for the location
    /// truncation budget (`index.tsx:1644-1649,1906-1909`).
    pub location_width: usize,
    pub mode: PromptMode,
    /// `data.session.status(sessionID) == "running"`.
    pub running: bool,
    /// `move.progress()` plus its 1..=3 trailing dots.
    pub move_progress: Option<(&'a str, usize)>,
    /// `move.pendingNew()`.
    pub pending_new: bool,
    pub interrupt: InterruptArm,
    /// `config.animations ?? true`.
    pub animations: bool,
    /// Monotonic caller clock, used only for the arming flash.
    pub now_ms: u64,
    /// Elapsed scanner time.
    pub elapsed_ms: u64,
    /// Whether the composer owns input (`useInteractivity`).
    pub interactive: bool,
    pub subagents: usize,
    pub shells: usize,
    pub usage: &'a [String],
    pub command_shortcut: Option<&'a str>,
    pub agent_shortcut: Option<&'a str>,
    pub location: Option<&'a str>,
    pub editor: Option<EditorContext<'a>>,
    pub live_hovered: bool,
    pub location_hovered: bool,
}

impl<'a> RailFacts<'a> {
    #[must_use]
    pub fn new(terminal_width: usize) -> Self {
        Self {
            terminal_width,
            location_width: terminal_width,
            mode: PromptMode::Normal,
            running: false,
            move_progress: None,
            pending_new: false,
            interrupt: InterruptArm::default(),
            animations: true,
            now_ms: 0,
            elapsed_ms: 0,
            interactive: true,
            subagents: 0,
            shells: 0,
            usage: &[],
            command_shortcut: None,
            agent_shortcut: None,
            location: None,
            editor: None,
            live_hovered: false,
            location_hovered: false,
        }
    }

    /// The `<Switch>` order of `index.tsx:1911-1965`: running, move progress,
    /// pending new worktree, then the idle location. Exactly one branch owns
    /// the status slot, so a running session always replaces the directory.
    #[must_use]
    pub fn status_slot(&self) -> StatusSlot {
        if self.running {
            StatusSlot::Running
        } else if self.move_progress.is_some() {
            StatusSlot::MoveProgress
        } else if self.pending_new {
            StatusSlot::PendingNew
        } else {
            StatusSlot::Location
        }
    }

    /// `index.tsx:139-145`: `esc ` plus the armed/unarmed label.
    #[must_use]
    pub fn interrupt_label(&self) -> (&'static str, &'static str) {
        if self.interrupt.is_armed() {
            ("esc ", "again to interrupt")
        } else {
            ("esc ", "interrupt")
        }
    }
}

// ---------------------------------------------------------------------------
// Rail spans (CP-11, CP-14..CP-16, CP-19, CP-20)
// ---------------------------------------------------------------------------

/// Owning slot of one rail span, in paint order.
///
/// `plugin/render.tsx:143-156` renders host children (`prompt.footer.status`,
/// then `prompt.footer.file`) before the plugin's `append` contribution
/// (`opencode.prompt.footer`), so the footer plugin is the rightmost block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailSlot {
    Status,
    File,
    Footer,
}

/// Rail segments in paint order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailSegment {
    Spinner,
    Interrupt,
    MoveProgress,
    PendingNew,
    Location,
    EditorFile,
    /// Live subagent/shell counts (footer plugin).
    Live,
    /// Usage/cost list with its ` · ` separator.
    Usage,
    /// `agents` hint (only when the live/status branch is empty).
    Agents,
    /// `commands` hint.
    Commands,
    /// `esc shell` / `esc exit shell mode` (shell mode replaces the plugin).
    ShellHint,
}

impl RailSegment {
    #[must_use]
    pub fn slot(self) -> RailSlot {
        match self {
            RailSegment::Spinner
            | RailSegment::Interrupt
            | RailSegment::MoveProgress
            | RailSegment::PendingNew
            | RailSegment::Location => RailSlot::Status,
            RailSegment::EditorFile => RailSlot::File,
            RailSegment::Live
            | RailSegment::Usage
            | RailSegment::Agents
            | RailSegment::Commands
            | RailSegment::ShellHint => RailSlot::Footer,
        }
    }
}

/// Semantic paint role; the view maps these onto the active theme
/// (`footer.tsx:79-119`, `index.tsx:1915-1975`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailRole {
    TextBase,
    TextMuted,
    /// `text.action.primary.disabled` with `TextAttributes.DIM`.
    TextDisabled,
    /// `theme.border.base` / agent color: scanner cells and worktree accents.
    Accent,
    /// `text.feedback.warning.base` while the interrupt is armed.
    Warning,
    /// The same warning tinted toward the flash color during the 0.22 s
    /// arming tween.
    WarningFlash,
}

impl RailRole {
    /// The `agents`/`commands` hint roles (`footer.tsx:100-119`): a focused
    /// composer renders base + muted, otherwise both halves are disabled.
    #[must_use]
    fn hint(interactive: bool, label: bool) -> RailRole {
        match (interactive, label) {
            (true, false) => RailRole::TextBase,
            (true, true) => RailRole::TextMuted,
            (false, _) => RailRole::TextDisabled,
        }
    }
}

/// One painted span of the rail. `text` is the exact cell content; consecutive
/// spans in the same slot concatenate without implicit separators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailSpan {
    pub segment: RailSegment,
    pub text: String,
    pub role: RailRole,
}

impl RailSpan {
    fn new(segment: RailSegment, text: impl Into<String>, role: RailRole) -> Self {
        Self {
            segment,
            text: text.into(),
            role,
        }
    }

    #[must_use]
    pub fn slot(&self) -> RailSlot {
        self.segment.slot()
    }
}

/// The rail in paint order: status slot, file slot, footer plugin.
#[must_use]
pub fn rail_spans(facts: &RailFacts<'_>) -> Vec<RailSpan> {
    let mut spans = Vec::new();

    // `prompt.footer.status` (`index.tsx:1901-1967`).
    match facts.status_slot() {
        StatusSlot::Running => {
            // `<box marginLeft={1}>` around the scanner, row `gap={1}` before
            // the interrupt text: leading margin and gap are folded into the
            // adjacent span text so the concatenation is cell-exact.
            let (role, text) = if facts.animations {
                (RailRole::Accent, scanner_frame_at(facts.elapsed_ms))
            } else {
                (RailRole::TextMuted, SCANNER_STATIC.to_owned())
            };
            spans.push(RailSpan::new(
                RailSegment::Spinner,
                format!(" {text}"),
                role,
            ));
            let (esc, label) = facts.interrupt_label();
            let (esc_role, label_role) = if facts.interrupt.is_armed() {
                let level = facts
                    .interrupt
                    .armed_at_ms()
                    .map_or(0.0, |armed| {
                        interrupt_flash_level(facts.animations, facts.now_ms.saturating_sub(armed))
                    });
                let role = if level > 0.0 {
                    RailRole::WarningFlash
                } else {
                    RailRole::Warning
                };
                (role, role)
            } else {
                (RailRole::TextBase, RailRole::TextMuted)
            };
            spans.push(RailSpan::new(
                RailSegment::Interrupt,
                format!(" {esc}"),
                esc_role,
            ));
            spans.push(RailSpan::new(RailSegment::Interrupt, label, label_role));
        }
        StatusSlot::MoveProgress => {
            let (progress, dots) = facts.move_progress.unwrap_or(("", 1));
            spans.push(RailSpan::new(
                RailSegment::MoveProgress,
                format!("   {}", move_progress_label(progress, dots)),
                RailRole::Accent,
            ));
        }
        StatusSlot::PendingNew => {
            spans.push(RailSpan::new(
                RailSegment::PendingNew,
                "   (new worktree)",
                RailRole::Accent,
            ));
        }
        StatusSlot::Location => {
            if let Some(location) = facts.location {
                spans.push(RailSpan::new(
                    RailSegment::Location,
                    truncate_path(location, facts.location_width),
                    if facts.location_hovered {
                        RailRole::TextBase
                    } else {
                        RailRole::TextMuted
                    },
                ));
            }
        }
    }

    // `prompt.footer.file` (`index.tsx:1968-1981`).
    if let Some(editor) = facts.editor {
        if editor.state.renders() {
            if let Some(label) = editor_label_display(editor.path, editor.ranges, facts.terminal_width) {
                spans.push(RailSpan::new(
                    RailSegment::EditorFile,
                    label,
                    if editor.state == EditorState::Pending {
                        RailRole::Accent
                    } else {
                        RailRole::TextMuted
                    },
                ));
            }
        }
    }

    // `opencode.prompt.footer` (`footer.tsx:65-131`).
    if facts.mode == PromptMode::Shell {
        // `footer.tsx:122-129`: shell mode replaces the entire plugin block.
        spans.push(RailSpan::new(RailSegment::ShellHint, "esc ", RailRole::TextBase));
        spans.push(RailSpan::new(
            RailSegment::ShellHint,
            shell_mode_hint(facts.terminal_width),
            RailRole::TextMuted,
        ));
        return spans;
    }

    let details = show_details(facts.interrupt.is_armed(), facts.terminal_width);
    let layout = prompt_footer_layout(facts.terminal_width, facts.usage, facts.command_shortcut);
    let live = live_counts_label(facts.subagents, facts.shells);
    let has_status = !facts.usage.is_empty();

    if live.is_some() || has_status {
        if let Some(counts) = live.as_ref() {
            // `footer.tsx:82-84`: the `session.child.first` shortcut keeps its
            // own base color even while the counts are muted.
            if let Some(key) = facts.agent_shortcut {
                spans.push(RailSpan::new(
                    RailSegment::Live,
                    format!("{key} "),
                    RailRole::TextBase,
                ));
            }
            spans.push(RailSpan::new(
                RailSegment::Live,
                counts.clone(),
                if facts.live_hovered {
                    RailRole::TextBase
                } else {
                    RailRole::TextMuted
                },
            ));
        }
        if details && layout.usage && has_status {
            let usage = facts.usage.join(" · ");
            let text = if live.is_some() {
                format!(" · {usage}")
            } else {
                usage
            };
            spans.push(RailSpan::new(RailSegment::Usage, text, RailRole::TextMuted));
        }
    } else if details && layout.shortcuts {
        // `footer.tsx:99-108`: the `agents` hint exists only when the live
        // branch is empty.
        if let Some(key) = facts.agent_shortcut {
            spans.push(RailSpan::new(
                RailSegment::Agents,
                format!("{key} "),
                RailRole::hint(facts.interactive, false),
            ));
            spans.push(RailSpan::new(
                RailSegment::Agents,
                "agents",
                RailRole::hint(facts.interactive, true),
            ));
        }
    }
    // `footer.tsx:110-120`: the `commands` hint is a sibling of the switch,
    // so a live row still carries it when everything fits.
    if details && layout.shortcuts {
        if let Some(key) = facts.command_shortcut {
            spans.push(RailSpan::new(
                RailSegment::Commands,
                format!("{key} "),
                RailRole::hint(facts.interactive, false),
            ));
            spans.push(RailSpan::new(
                RailSegment::Commands,
                "commands",
                RailRole::hint(facts.interactive, true),
            ));
        }
    }
    spans
}

/// Concatenated rail text for one slot.
#[must_use]
pub fn slot_text(spans: &[RailSpan], slot: RailSlot) -> String {
    spans
        .iter()
        .filter(|span| span.slot() == slot)
        .map(|span| span.text.as_str())
        .collect()
}

/// `footer.tsx:126`: `dimensions().width < 44 ? "shell" : "exit shell mode"`.
#[must_use]
pub fn shell_mode_hint(terminal_width: usize) -> &'static str {
    if terminal_width < SHORTCUTS_MIN_WIDTH {
        "shell"
    } else {
        "exit shell mode"
    }
}

/// `index.tsx:1927-1936` / `prompt/move.tsx`: progress text plus
/// `".".repeat(creatingDots)`, dots cycling 1..=3.
#[must_use]
pub fn move_progress_label(progress: &str, creating_dots: usize) -> String {
    format!("{progress}{}", ".".repeat(creating_dots.clamp(1, 3)))
}

// ---------------------------------------------------------------------------
// Location label and truncation (CP-14)
// ---------------------------------------------------------------------------

/// `path-format.ts:3-10` (`abbreviateHome`, POSIX branch): `~` for the home
/// directory itself, `~/relative` inside it, unchanged otherwise.
#[must_use]
pub fn abbreviate_home(directory: &str, home: &str) -> String {
    if home.is_empty() {
        return directory.to_owned();
    }
    let home = if home == "/" {
        "/"
    } else {
        home.trim_end_matches('/')
    };
    if directory == home {
        return "~".to_owned();
    }
    let relative = if home == "/" {
        directory.strip_prefix('/')
    } else {
        directory.strip_prefix(home).and_then(|rest| rest.strip_prefix('/'))
    };
    match relative {
        Some(rest) if !rest.is_empty() => format!("~/{rest}"),
        _ => directory.to_owned(),
    }
}

/// `index.tsx:1636-1643`: home-abbreviated directory plus `:branch` when the
/// location has a current branch.
#[must_use]
pub fn location_label(directory: &str, home: &str, branch: Option<&str>) -> String {
    let directory = abbreviate_home(directory, home);
    match branch.filter(|branch| !branch.is_empty()) {
        Some(branch) => format!("{directory}:{branch}"),
        None => directory,
    }
}

/// The working-directory action menu (`ui/working-directory-actions.tsx:16-58`):
/// `Copy path`, `Open folder`, and `Workspaces` only when a move handler
/// exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocationMenu<'a> {
    pub directory: &'a str,
    pub can_move: bool,
}

/// `working-directory-actions.tsx:16-19`: a terminal text selection suppresses
/// the click, as does a location with no directory.
#[must_use]
pub fn location_menu<'a>(
    directory: Option<&'a str>,
    has_selection: bool,
    can_move: bool,
) -> Option<LocationMenu<'a>> {
    if has_selection {
        return None;
    }
    directory.map(|directory| LocationMenu {
        directory,
        can_move,
    })
}

/// `file-path.tsx:31-76` (`truncateFilePath`, POSIX branch): keep the root,
/// one `…` path segment, the basename with its extension, then as many
/// trailing parent segments as fit.
#[must_use]
pub fn truncate_path(value: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if cell_width(value) <= max_width {
        return value.to_owned();
    }
    let root = if value.starts_with('/') { "/" } else { "" };
    let source = &value[root.len()..];
    let segments: Vec<&str> = source.split('/').filter(|part| !part.is_empty()).collect();
    let Some(basename) = segments.last().copied() else {
        return take_start(value, max_width);
    };
    if segments.len() < 2 {
        let root_width = cell_width(root);
        if root_width >= max_width {
            return take_start(root, max_width);
        }
        return format!(
            "{root}{}",
            truncate_basename(basename, max_width - root_width)
        );
    }
    let prefix = format!("{root}…/");
    let prefix_width = cell_width(&prefix);
    let basename_budget = max_width.saturating_sub(prefix_width);
    if basename_budget == 0 {
        return take_start(&prefix, max_width);
    }
    let compact = truncate_basename(basename, basename_budget);
    if compact != basename {
        return format!("{prefix}{compact}");
    }
    // `file-path.tsx:66-75`: walk parent segments from the basename outward,
    // eliding the last one that does not fit whole.
    let mut selected: Vec<String> = vec![basename.to_owned()];
    let separator_width = cell_width("/");
    let mut width = prefix_width + cell_width(basename);
    for index in (0..segments.len() - 1).rev() {
        let segment = segments[index];
        let next = cell_width(segment) + separator_width;
        if width + next > max_width {
            let available = max_width.saturating_sub(width + separator_width);
            if available > 1 {
                selected.insert(0, format!("{}…", take_start(segment, available - 1)));
            }
            break;
        }
        selected.insert(0, segment.to_owned());
        width += next;
    }
    format!("{prefix}{}", selected.join("/"))
}

/// `file-path.tsx:78-90` (`truncateBasename`): keep the extension, truncate
/// the stem from the middle.
fn truncate_basename(value: &str, max_width: usize) -> String {
    if cell_width(value) <= max_width {
        return value.to_owned();
    }
    if max_width <= 1 {
        return take_start("…", max_width);
    }
    let dot = value.rfind('.').filter(|dot| *dot > 0);
    let extension = dot.map_or("", |dot| &value[dot..]);
    let extension_width = cell_width(extension);
    if extension_width >= max_width {
        return format!("…{}", take_end(extension, max_width.saturating_sub(1)));
    }
    let stem = dot.map_or(value, |dot| &value[..dot]);
    format!(
        "{}…{}",
        take_start(stem, max_width - extension_width - 1),
        extension
    )
}

// ---------------------------------------------------------------------------
// Editor-file label (CP-15)
// ---------------------------------------------------------------------------

/// `index.tsx:235-239` / `getEditorRangeLabel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

impl EditorRange {
    /// `hasEditorRangeSelection` (`index.tsx:148-153`).
    #[must_use]
    pub fn has_selection(self) -> bool {
        self.start_line != self.end_line || self.start_character != self.end_character
    }
}

/// Editor attach state (`editor.labelState()`, consumed at
/// `index.tsx:1969-1975`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorState {
    /// No editor selection: the file slot renders nothing.
    None,
    /// Received, not yet attached: accent color.
    Pending,
    /// Attached: muted color.
    Attached,
}

impl EditorState {
    #[must_use]
    pub fn renders(self) -> bool {
        !matches!(self, EditorState::None)
    }
}

/// An editor context fact: path, ranges and attach state.
#[derive(Clone, Copy, Debug)]
pub struct EditorContext<'a> {
    pub path: &'a str,
    pub ranges: &'a [EditorRange],
    pub state: EditorState,
}

/// `index.tsx:246-250`: basename, except `index.*` files which keep their
/// parent directory for disambiguation.
#[must_use]
pub fn editor_file_name(path: &str) -> String {
    let path = path.trim_end_matches('/');
    let (parent, filename) = match path.rsplit_once('/') {
        Some((parent, filename)) => (parent, filename),
        None => ("", path),
    };
    let index_file = filename
        .strip_prefix("index.")
        .is_some_and(|rest| !rest.is_empty() && !rest.contains('.') && !rest.contains('/'));
    if index_file {
        if let Some(directory) = parent.rsplit('/').next().filter(|part| !part.is_empty()) {
            return format!("{directory}/{filename}");
        }
    }
    filename.to_owned()
}

/// `index.tsx:240-245`: first selected range (else the first range) as
/// `#line` or `#start-end`, plus ` +N` when more than one range exists.
#[must_use]
pub fn editor_selection_label(ranges: &[EditorRange]) -> Option<String> {
    if ranges.is_empty() {
        return None;
    }
    let first = ranges
        .iter()
        .find(|range| range.has_selection())
        .unwrap_or(&ranges[0]);
    let label = if first.has_selection() {
        if first.start_line == first.end_line {
            Some(format!("#{}", first.start_line))
        } else {
            Some(format!("#{}-{}", first.start_line, first.end_line))
        }
    } else {
        None
    };
    let extra = (ranges.len() > 1).then(|| format!("+{}", ranges.len() - 1));
    match (label, extra) {
        (Some(label), Some(extra)) => Some(format!("{label} {extra}")),
        (Some(label), None) => Some(label),
        (None, Some(extra)) => Some(extra),
        (None, None) => None,
    }
}

/// `index.tsx:242-257`: file label plus selection label, truncated middle at
/// `max(12, min(48, floor(width / 3)))`.
#[must_use]
pub fn editor_label_display(path: &str, ranges: &[EditorRange], terminal_width: usize) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let mut label = editor_file_name(path);
    if let Some(selection) = editor_selection_label(ranges) {
        label.push_str(&selection);
    }
    Some(truncate_middle(&label, editor_label_width(terminal_width)))
}

/// `index.tsx:251`: `Math.max(12, Math.min(48, Math.floor(width / 3)))`.
#[must_use]
pub fn editor_label_width(terminal_width: usize) -> usize {
    (terminal_width / 3).clamp(EDITOR_LABEL_MIN_WIDTH, EDITOR_LABEL_MAX_WIDTH)
}

/// `locale.ts:89-97` (`truncateMiddle`): JS string length semantics (UTF-16
/// code units), `ceil((n-1)/2)` head, `…`, `floor((n-1)/2)` tail. Callers
/// guarantee `max_width >= 12`; a non-positive budget renders nothing.
#[must_use]
pub fn truncate_middle(value: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if utf16_len(value) <= max_width {
        return value.to_owned();
    }
    let keep_start = max_width / 2;
    let keep_end = max_width.saturating_sub(1) / 2;
    format!(
        "{}…{}",
        utf16_prefix(value, keep_start),
        utf16_suffix(value, keep_end)
    )
}

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

/// Longest prefix whose UTF-16 length fits `units` (cuts on a char boundary).
fn utf16_prefix(value: &str, units: usize) -> &str {
    let mut used = 0;
    for (index, ch) in value.char_indices() {
        let next = ch.len_utf16();
        if used + next > units {
            return &value[..index];
        }
        used += next;
    }
    value
}

/// Longest suffix whose UTF-16 length fits `units` (cuts on a char boundary).
fn utf16_suffix(value: &str, units: usize) -> &str {
    let mut used = 0;
    for (index, ch) in value.char_indices().rev() {
        let next = ch.len_utf16();
        if used + next > units {
            return &value[index + ch.len_utf8()..];
        }
        used += next;
    }
    value
}

// ---------------------------------------------------------------------------
// Bottom stack: precedence and queue dock (CP-21, CP-22)
// ---------------------------------------------------------------------------

/// Live facts for the bottom of the session screen
/// (`routes/session/index.tsx:1594-1655`).
#[derive(Clone, Copy, Debug, Default)]
pub struct BottomFacts {
    /// Raw `composer.open` signal.
    pub composer_open: bool,
    /// `session().parentID` is set (a child session).
    pub child_session: bool,
    pub forms: usize,
    pub permissions: usize,
    /// `currentLocation.error` targets this session's directory.
    pub location_missing: bool,
    /// `!disabled()`: the route still owns input.
    pub enabled: bool,
}

/// Which box owns the bottom slot. Exactly one prompt-like surface renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BottomSlot {
    /// The composer itself (`routes/session/index.tsx:1620-1626`).
    Composer,
    /// `PermissionPrompt` for the first pending request.
    Permission,
    /// `FormPrompt` for the first form.
    Form,
    /// `SessionLocationMissing`.
    LocationMissing,
    /// The normal prompt.
    Prompt,
    /// Nothing renders (a disabled route with no other surface).
    Empty,
}

/// `routes/session/index.tsx:1621-1655`: composer (open, or a child session
/// with no forms), then permission, form, missing location, prompt.
#[must_use]
pub fn bottom_slot(facts: &BottomFacts) -> BottomSlot {
    if facts.composer_open || (facts.child_session && facts.forms == 0) {
        return BottomSlot::Composer;
    }
    if facts.permissions > 0 {
        return BottomSlot::Permission;
    }
    if facts.forms > 0 {
        return BottomSlot::Form;
    }
    if facts.location_missing {
        return BottomSlot::LocationMissing;
    }
    if facts.enabled {
        return BottomSlot::Prompt;
    }
    BottomSlot::Empty
}

/// `queuedPrompts` block height: content row plus `paddingTop`/`paddingBottom`
/// (`routes/session/index.tsx:2496-2505`); the left border adds no rows.
pub const QUEUE_DOCK_HEIGHT: usize = 3;
/// `routes/session/index.tsx:2494`: `marginBottom={1}`.
pub const QUEUE_DOCK_MARGIN_BOTTOM: usize = 1;
pub const QUEUE_DOCK_PADDING_TOP: usize = 1;
pub const QUEUE_DOCK_PADDING_BOTTOM: usize = 1;
pub const QUEUE_DOCK_PADDING_LEFT: usize = 2;
pub const QUEUE_DOCK_PADDING_RIGHT: usize = 1;

/// The queue dock (`routes/session/index.tsx:2485-2513`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueDock {
    /// `{n} queued`, plus ` · {first prompt}` when the first text is non-empty.
    pub label: String,
    /// Pointer hover swaps the background to `theme.decrease(raised.base)`;
    /// unhovered is `theme.background.raised.base`.
    pub hovered: bool,
}

impl QueueDock {
    /// Columns left for the label: the row width minus the left border and
    /// the horizontal paddings.
    #[must_use]
    pub fn text_width(&self, terminal_width: usize) -> usize {
        terminal_width.saturating_sub(
            1 + QUEUE_DOCK_PADDING_LEFT + QUEUE_DOCK_PADDING_RIGHT,
        )
    }
}

/// `routes/session/index.tsx:1593-1595`: the dock renders only while the
/// composer is closed, the route is enabled and at least one prompt is queued.
#[must_use]
pub fn queue_dock(facts: &BottomFacts, prompts: &[&str], hovered: bool) -> Option<QueueDock> {
    if facts.composer_open || !facts.enabled || prompts.is_empty() {
        return None;
    }
    let mut label = format!("{} queued", prompts.len());
    let next = prompts[0].replace('\n', " ");
    if !next.is_empty() {
        label.push_str(" · ");
        label.push_str(&next);
    }
    Some(QueueDock { label, hovered })
}

/// Closing the composer (`routes/session/index.tsx:1621-1626`): a child
/// session navigates to its parent, otherwise the composer just closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposerClose<'a> {
    NavigateToParent(&'a str),
    Close,
}

#[must_use]
pub fn composer_close(parent: Option<&str>) -> ComposerClose<'_> {
    match parent {
        Some(parent) if !parent.is_empty() => ComposerClose::NavigateToParent(parent),
        _ => ComposerClose::Close,
    }
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

/// Display width in terminal cells, measured with the view's bounded,
/// escape-stripping cluster segmentation (`native_view_layout`). Upstream
/// uses `stringWidth` (`util/string-width.ts`).
fn cell_width(text: &str) -> usize {
    crate::native_view_layout::visit_unpadded_cells(text, 4096, |_, _| {})
}

fn graphemes(text: &str) -> Vec<&str> {
    use unicode_segmentation::UnicodeSegmentation;
    UnicodeSegmentation::graphemes(text, true).collect()
}

fn take_start(value: &str, max_width: usize) -> String {
    take(value, max_width, false)
}

fn take_end(value: &str, max_width: usize) -> String {
    take(value, max_width, true)
}

fn take(value: &str, max_width: usize, reverse: bool) -> String {
    let mut parts = graphemes(value);
    if reverse {
        parts.reverse();
    }
    let mut selected = Vec::new();
    let mut width = 0;
    for part in parts {
        let next = cell_width(part);
        if width + next > max_width {
            break;
        }
        selected.push(part);
        width += next;
    }
    if reverse {
        selected.reverse();
    }
    selected.concat()
}

#[cfg(test)]
mod native_status_rail_tests {
    use super::*;

    fn usage() -> Vec<String> {
        vec!["1.2K (3%)".to_owned()]
    }

    fn segments(spans: &[RailSpan]) -> Vec<RailSegment> {
        spans.iter().map(|span| span.segment).collect()
    }

    fn texts(spans: &[RailSpan]) -> Vec<&str> {
        spans.iter().map(|span| span.text.as_str()).collect()
    }

    fn span_of(spans: &[RailSpan], segment: RailSegment) -> &RailSpan {
        spans
            .iter()
            .find(|span| span.segment == segment)
            .unwrap_or_else(|| panic!("missing {segment:?} in {:?}", texts(spans)))
    }

    // -- CP-11: running replaces idle directory -----------------------------

    #[test]
    fn cp11_running_replaces_idle_directory_exclusively() {
        let mut facts = RailFacts::new(80);
        facts.location = Some("~/project:main");
        assert_eq!(facts.status_slot(), StatusSlot::Location);
        let idle = rail_spans(&facts);
        assert_eq!(segments(&idle), vec![RailSegment::Location]);
        assert_eq!(idle[0].text, "~/project:main");

        // Running: the scanner + interrupt own the slot; the directory is gone.
        facts.running = true;
        facts.elapsed_ms = 40;
        assert_eq!(facts.status_slot(), StatusSlot::Running);
        let running = rail_spans(&facts);
        assert_eq!(
            segments(&running),
            vec![
                RailSegment::Spinner,
                RailSegment::Interrupt,
                RailSegment::Interrupt
            ]
        );
        assert!(!texts(&running).contains(&"~/project:main"));
        // Exact geometry: marginLeft 1, eight scanner cells, row gap 1,
        // `esc ` then the unarmed label.
        assert_eq!(running[0].text, " ■■⬝⬝⬝⬝⬝⬝");
        assert_eq!(running[1].text, " esc ");
        assert_eq!(running[2].text, "interrupt");
        assert_eq!(
            slot_text(&running, RailSlot::Status),
            " ■■⬝⬝⬝⬝⬝⬝ esc interrupt"
        );

        // Idle again: the directory returns and the running spans are gone.
        facts.running = false;
        let idle = rail_spans(&facts);
        assert_eq!(segments(&idle), vec![RailSegment::Location]);
        assert_eq!(idle[0].text, "~/project:main");
    }

    #[test]
    fn cp11_move_and_pending_new_replace_the_location() {
        let mut facts = RailFacts::new(80);
        facts.location = Some("~/project");
        facts.move_progress = Some(("Submitting prompt", 2));
        assert_eq!(facts.status_slot(), StatusSlot::MoveProgress);
        let spans = rail_spans(&facts);
        assert_eq!(segments(&spans), vec![RailSegment::MoveProgress]);
        // Upstream `index.tsx:1929-1937`: `paddingLeft={3}` then the progress
        // text plus its 1..=3 dots.
        assert_eq!(spans[0].text, "   Submitting prompt..");
        assert_eq!(spans[0].role, RailRole::Accent);
        assert!(!texts(&spans).contains(&"~/project"));

        facts.move_progress = None;
        facts.pending_new = true;
        assert_eq!(facts.status_slot(), StatusSlot::PendingNew);
        let spans = rail_spans(&facts);
        assert_eq!(texts(&spans), vec!["   (new worktree)"]);
        assert_eq!(spans[0].role, RailRole::Accent);
    }

    #[test]
    fn cp11_status_slot_switch_order_is_running_then_move_then_pending() {
        // `index.tsx:1911-1965`: first matching `<Match>` wins.
        let mut facts = RailFacts::new(80);
        facts.pending_new = true;
        facts.move_progress = Some(("Creating worktree", 1));
        facts.running = true;
        assert_eq!(facts.status_slot(), StatusSlot::Running);
        facts.running = false;
        assert_eq!(facts.status_slot(), StatusSlot::MoveProgress);
        facts.move_progress = None;
        assert_eq!(facts.status_slot(), StatusSlot::PendingNew);
        facts.pending_new = false;
        assert_eq!(facts.status_slot(), StatusSlot::Location);
    }

    // -- CP-12: eight-cell scanner and reduced animation --------------------

    #[test]
    fn cp12_scanner_geometry_matches_upstream_defaults() {
        assert_eq!(SCANNER_WIDTH, 8);
        assert_eq!(SCANNER_HOLD_START_FRAMES, 30);
        assert_eq!(SCANNER_HOLD_END_FRAMES, 9);
        assert_eq!(SCANNER_TRAIL_STEPS, 6);
        assert_eq!(SCANNER_TOTAL_FRAMES, 54);
        assert_eq!(SCANNER_INTERVAL_MS, 40);
        assert_eq!(SCANNER_ACTIVE_GLYPH, '■');
        assert_eq!(SCANNER_INACTIVE_GLYPH, '⬝');
        let frames = scanner_frames();
        assert_eq!(frames.len(), 54);
        assert!(frames.iter().all(|frame| frame.chars().count() == 8));
        // Frame 53 is the last hold-at-start frame; one 40 ms step wraps.
        assert_eq!(scanner_frame_index(53 * 40), 53);
        assert_eq!(scanner_frame_at(54 * 40), scanner_frame_at(0));
    }

    #[test]
    fn cp12_scanner_fixture_frames_are_cell_exact() {
        // Fixtures prompt-scanner-{0,40,320,680,1200,2120}: frame = elapsed / 40.
        assert_eq!(scanner_frame_at(0), "■⬝⬝⬝⬝⬝⬝⬝");
        assert_eq!(scanner_frame_at(40), "■■⬝⬝⬝⬝⬝⬝");
        assert_eq!(scanner_frame_at(320), "⬝⬝■■■■■■");
        assert_eq!(scanner_frame_at(680), "⬝⬝⬝⬝⬝⬝■■");
        // 1200 ms = frame 30 and 2120 ms = frame 53: both inside holds that
        // have already faded past the six-step trail.
        assert_eq!(scanner_frame_at(1200), "⬝⬝⬝⬝⬝⬝⬝⬝");
        assert_eq!(scanner_frame_at(2120), "⬝⬝⬝⬝⬝⬝⬝⬝");
    }

    #[test]
    fn cp12_scanner_holds_fade_one_trail_step_per_frame() {
        // Hold at end (frames 8..16): the lead stays at cell 7 through
        // progress 5, then the whole trail has faded.
        for frame in 8..14 {
            assert_eq!(scanner_frame(frame).chars().last(), Some('■'), "{frame}");
        }
        assert_eq!(scanner_frame(13), "⬝⬝⬝⬝⬝⬝⬝■");
        assert_eq!(scanner_frame(14), "⬝⬝⬝⬝⬝⬝⬝⬝");
        // Backward pass starts at frame 17 with the lead at cell 6.
        assert_eq!(scanner_frame(17), "⬝⬝⬝⬝⬝⬝■■");
        assert_eq!(scanner_frame(23), "■■■■■■⬝⬝");
        // Hold at start (frames 24..53) fades from a full trail to blank.
        assert_eq!(scanner_frame(24), "■■■■■■⬝⬝");
        assert_eq!(scanner_frame(25), "■■■■■⬝⬝⬝");
        assert_eq!(scanner_frame(29), "■⬝⬝⬝⬝⬝⬝⬝");
        assert_eq!(scanner_frame(30), "⬝⬝⬝⬝⬝⬝⬝⬝");
        assert_eq!(scanner_frame(53), "⬝⬝⬝⬝⬝⬝⬝⬝");
    }

    #[test]
    fn cp12_reduced_animation_mode_renders_the_static_marker() {
        // `index.tsx:1912-1917`: `config.animations ?? true`; the fallback text
        // is `[⋯]` in `theme.text.muted`.
        assert_eq!(scanner_render(false, 0), SCANNER_STATIC);
        assert_eq!(scanner_render(false, 21_000), SCANNER_STATIC);
        assert_eq!(scanner_render(true, 0), "■⬝⬝⬝⬝⬝⬝⬝");

        let mut facts = RailFacts::new(80);
        facts.running = true;
        facts.animations = false;
        facts.elapsed_ms = 21_000;
        let spans = rail_spans(&facts);
        assert_eq!(spans[0].text, " [⋯]");
        assert_eq!(spans[0].role, RailRole::TextMuted);
    }

    #[test]
    fn cp12_one_cell_spinner_cycles_and_falls_back() {
        // `one-cell-motion.ts:111` SEED_MONO: frames - \ | /, interval 120.
        assert_eq!(
            (0..5).map(|step| one_cell_frame(step * 120)).collect::<Vec<_>>(),
            vec!["-", "\\", "|", "/", "-"]
        );
        // Sub-interval time keeps the current frame.
        assert_eq!(one_cell_frame(119), "-");
        assert_eq!(one_cell_frame(240), "|");
        // `one-cell-spinner.tsx:64-75`: disabled animations show `still`,
        // default U+25AA, inside a single-cell box.
        assert_eq!(one_cell_render(false, 0, None), ONE_CELL_STILL);
        assert_eq!(one_cell_render(false, 0, Some("·")), "·");
        assert_eq!(one_cell_render(true, 240, None), "|");
        assert_eq!(ONE_CELL_MONO_INTERVAL_MS, 120);
    }

    // -- CP-13: interrupt arming, expiry, flash, detail suppression ---------

    #[test]
    fn cp13_interrupt_arms_expires_and_triggers_on_the_second_press() {
        let mut arm = InterruptArm::default();
        assert!(!arm.is_armed());
        assert_eq!(
            arm.press(0, true, false, true, false, true),
            InterruptOutcome::Armed(1)
        );
        assert_eq!(arm.count(), 1);
        assert_eq!(arm.deadline_ms(), Some(INTERRUPT_EXPIRY_MS));
        assert_eq!(arm.armed_at_ms(), Some(0));
        // Second press inside the window: the caller interrupts and the arm
        // resets to zero.
        assert_eq!(
            arm.press(4999, true, false, true, false, true),
            InterruptOutcome::Triggered
        );
        assert!(!arm.is_armed());
        assert_eq!(arm.deadline_ms(), None);
        // A third press starts a fresh arm rather than triggering.
        assert_eq!(
            arm.press(5001, true, false, true, false, true),
            InterruptOutcome::Armed(1)
        );
    }

    #[test]
    fn cp13_interrupt_expiry_boundary_is_4999_armed_5000_expired() {
        let mut arm = InterruptArm::default();
        assert_eq!(
            arm.press(0, true, false, true, false, true),
            InterruptOutcome::Armed(1)
        );
        arm.expire_at(4999);
        assert!(arm.is_armed());
        arm.expire_at(5000);
        assert!(!arm.is_armed());
        // Pressing after expiry re-arms instead of interrupting.
        assert_eq!(
            arm.press(5000, true, false, true, false, true),
            InterruptOutcome::Armed(1)
        );
    }

    #[test]
    fn cp13_interrupt_interceptions_follow_upstream_order() {
        // `index.tsx:500-531` order: enablement, autocomplete, focus, shell
        // mode, session id.
        let mut arm = InterruptArm::default();
        assert_eq!(
            arm.press(0, false, true, false, true, false),
            InterruptOutcome::Ignored(InterruptIgnore::NotRunning)
        );
        assert_eq!(
            arm.press(0, true, true, false, true, false),
            InterruptOutcome::Ignored(InterruptIgnore::AutocompleteOpen)
        );
        assert_eq!(
            arm.press(0, true, false, false, true, false),
            InterruptOutcome::Ignored(InterruptIgnore::InputUnfocused)
        );
        assert_eq!(
            arm.press(0, true, false, true, true, true),
            InterruptOutcome::ExitShellMode
        );
        assert_eq!(
            arm.press(0, true, false, true, false, false),
            InterruptOutcome::Ignored(InterruptIgnore::NoSession)
        );
        // Intercepted presses never arm.
        assert!(!arm.is_armed());
        assert_eq!(arm.press(0, true, false, true, false, true), InterruptOutcome::Armed(1));
    }

    #[test]
    fn cp13_interrupt_flash_tween_and_disabled_animations() {
        assert!((interrupt_flash_level(true, 0) - 0.75).abs() < f32::EPSILON);
        // The tween's default ease is smoothstep (`ui/animation.ts:32`); at the
        // midpoint, smoothstep(0.5) == 0.5, so it agrees with a linear ramp here.
        assert!((interrupt_flash_level(true, 110) - 0.375).abs() < 0.001);
        // Off-midpoint the two diverge: linear would give 0.5625, but
        // smoothstep(55/220) = smoothstep(0.25) = 0.15625 -> 0.75 * 0.84375.
        assert!(
            (interrupt_flash_level(true, 55) - 0.632_812_5).abs() < 1e-6,
            "smoothstep ease, got {}",
            interrupt_flash_level(true, 55)
        );
        assert!((interrupt_flash_level(true, 219) - 0.75 * (1.0 - smoothstep(219.0 / 220.0))).abs() < 1e-6);
        assert_eq!(interrupt_flash_level(true, 220), 0.0);
        assert_eq!(interrupt_flash_level(false, 0), 0.0);
        assert_eq!(INTERRUPT_FLASH_MS, 220);
        assert_eq!(INTERRUPT_FLASH_PEAK, 0.75);
    }

    #[test]
    fn cp13_armed_details_are_hidden_at_79_and_shown_at_80() {
        assert!(show_details(false, 20));
        assert!(show_details(false, 200));
        assert!(!show_details(true, 79));
        assert!(show_details(true, 80));
        assert_eq!(INTERRUPT_DETAILS_MIN_WIDTH, 80);

        // Armed, 79 columns: only the scanner + armed label remain; usage and
        // the command hint are suppressed.
        let usage = usage();
        let mut facts = RailFacts::new(79);
        facts.running = true;
        facts.usage = &usage;
        facts.command_shortcut = Some("ctrl+p");
        facts.agent_shortcut = Some("tab");
        facts.now_ms = 0;
        assert_eq!(
            facts.interrupt.press(0, true, false, true, false, true),
            InterruptOutcome::Armed(1)
        );
        let spans = rail_spans(&facts);
        assert_eq!(
            segments(&spans),
            vec![
                RailSegment::Spinner,
                RailSegment::Interrupt,
                RailSegment::Interrupt
            ]
        );
        assert_eq!(spans[2].text, "again to interrupt");
        assert_eq!(spans[2].role, RailRole::WarningFlash);
        assert!(!spans.iter().any(|span| span.segment == RailSegment::Usage));

        // At 80 columns the details return, still with the armed label; the
        // 0.22 s flash has finished by then.
        facts.terminal_width = 80;
        facts.now_ms = INTERRUPT_FLASH_MS;
        let spans = rail_spans(&facts);
        assert!(spans.iter().any(|span| span.segment == RailSegment::Usage));
        assert!(spans.iter().any(|span| span.segment == RailSegment::Commands));
        assert_eq!(span_of(&spans, RailSegment::Interrupt).role, RailRole::Warning);
    }

    // -- CP-14: directory/branch label, truncation, hover, actions ----------

    #[test]
    fn cp14_location_home_abbreviation_and_branch_suffix() {
        assert_eq!(abbreviate_home("/home/me/project", "/home/me"), "~/project");
        assert_eq!(abbreviate_home("/home/me", "/home/me"), "~");
        assert_eq!(abbreviate_home("/home/me/", "/home/me"), "/home/me/");
        assert_eq!(abbreviate_home("/other/project", "/home/me"), "/other/project");
        // Sibling prefixes are not abbreviated.
        assert_eq!(abbreviate_home("/home/me2/x", "/home/me"), "/home/me2/x");
        assert_eq!(abbreviate_home("/x", ""), "/x");
        assert_eq!(abbreviate_home("/x", "/"), "~/x");
        assert_eq!(location_label("/home/me/p", "/home/me", Some("main")), "~/p:main");
        assert_eq!(location_label("/home/me/p", "/home/me", None), "~/p");
        assert_eq!(location_label("/home/me/p", "/home/me", Some("")), "~/p");
    }

    #[test]
    fn cp14_long_paths_use_one_elision_segment_and_keep_the_basename() {
        let path = "/Users/me/project/src/components/very-long-file-name.ts";
        let truncated = truncate_path(path, 24);
        assert_eq!(truncated, "/…/very-long-file-na….ts");
        assert_eq!(cell_width(&truncated), 24);
        // A shorter budget keeps the extension.
        assert_eq!(
            truncate_path("/a/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.ts", 12),
            "/…/bbbbb….ts"
        );
        // Whole parent segments are kept while they fit.
        assert_eq!(truncate_path("/a/b/c/d/file.rs", 12), "/…/d/file.rs");
        // Root-only content falls back to a basename truncation.
        assert_eq!(truncate_path("/foo", 3), "/f…");
        assert_eq!(truncate_path("/verylongname", 1), "/");
        // Values that already fit pass through untouched.
        assert_eq!(truncate_path("~/p", 24), "~/p");
        assert_eq!(truncate_path("/very/long/path.rs", 0), "");
    }

    #[test]
    fn cp14_location_hover_role_and_actions_match_upstream() {
        let mut facts = RailFacts::new(80);
        facts.location = Some("/Users/me/project/src/components/x.ts");
        facts.location_width = 16;
        let spans = rail_spans(&facts);
        assert_eq!(spans[0].text, "/…/compone…/x.ts");
        assert_eq!(cell_width(&spans[0].text), 16);
        assert_eq!(spans[0].role, RailRole::TextMuted);
        facts.location_hovered = true;
        assert_eq!(rail_spans(&facts)[0].role, RailRole::TextBase);

        // `working-directory-actions.tsx:16-19`: a text selection suppresses
        // the click; otherwise the menu mirrors the location.
        assert_eq!(
            location_menu(Some("/w"), true, true),
            None,
            "selected text suppresses the action menu"
        );
        assert_eq!(location_menu(None, false, true), None);
        assert_eq!(
            location_menu(Some("/w"), false, true),
            Some(LocationMenu {
                directory: "/w",
                can_move: true
            })
        );
        assert!(location_menu(Some("/w"), false, false).is_some_and(|menu| !menu.can_move));
    }

    // -- CP-15: distinct editor-file slot -----------------------------------

    #[test]
    fn cp15_editor_slot_is_distinct_with_pending_accent() {
        let mut facts = RailFacts::new(80);
        facts.location = Some("~/project");
        facts.editor = Some(EditorContext {
            path: "/home/me/project/src/main.rs",
            ranges: &[],
            state: EditorState::Pending,
        });
        let spans = rail_spans(&facts);
        assert_eq!(segments(&spans), vec![RailSegment::Location, RailSegment::EditorFile]);
        assert_eq!(spans[1].slot(), RailSlot::File);
        assert_eq!(spans[1].text, "main.rs");
        assert_eq!(spans[1].role, RailRole::Accent);
        // The two slots stay separate even when concatenated.
        assert_eq!(slot_text(&spans, RailSlot::Status), "~/project");
        assert_eq!(slot_text(&spans, RailSlot::File), "main.rs");

        facts.editor = Some(EditorContext {
            path: "/home/me/project/src/main.rs",
            ranges: &[],
            state: EditorState::Attached,
        });
        let spans = rail_spans(&facts);
        assert_eq!(span_of(&spans, RailSegment::EditorFile).role, RailRole::TextMuted);

        // No editor selection: the file slot renders nothing.
        facts.editor = Some(EditorContext {
            path: "/home/me/project/src/main.rs",
            ranges: &[],
            state: EditorState::None,
        });
        assert_eq!(segments(&rail_spans(&facts)), vec![RailSegment::Location]);
        // No path at all also renders nothing.
        facts.editor = Some(EditorContext {
            path: "",
            ranges: &[],
            state: EditorState::Pending,
        });
        assert_eq!(segments(&rail_spans(&facts)), vec![RailSegment::Location]);
    }

    #[test]
    fn cp15_editor_label_composition_matches_upstream() {
        // `index.tsx:246-250`: basename, except `index.*` keeps its parent.
        assert_eq!(editor_file_name("/w/src/index.ts"), "src/index.ts");
        assert_eq!(editor_file_name("/w/src/index.tsx"), "src/index.tsx");
        assert_eq!(editor_file_name("/w/src/index.test.ts"), "index.test.ts");
        assert_eq!(editor_file_name("/w/src/main.rs"), "main.rs");
        assert_eq!(editor_file_name("index.ts"), "index.ts");
        // `index.tsx:240-245`: `#line` / `#start-end` plus ` +N`.
        let one_line = EditorRange {
            start_line: 12,
            start_character: 4,
            end_line: 12,
            end_character: 9,
        };
        assert_eq!(editor_selection_label(&[one_line]).as_deref(), Some("#12"));
        let span = EditorRange {
            start_line: 3,
            start_character: 0,
            end_line: 5,
            end_character: 0,
        };
        assert_eq!(
            editor_selection_label(&[span, one_line]).as_deref(),
            Some("#3-5 +1")
        );
        let collapsed = EditorRange {
            start_line: 7,
            start_character: 2,
            end_line: 7,
            end_character: 2,
        };
        assert_eq!(editor_selection_label(&[collapsed]), None);
        // A collapsed first range defers to the first selected one.
        assert_eq!(
            editor_selection_label(&[collapsed, one_line]).as_deref(),
            Some("#12 +1")
        );
        assert_eq!(
            editor_label_display("/w/src/index.tsx", &[span, one_line], 120).as_deref(),
            Some("src/index.tsx#3-5 +1")
        );
        // `index.tsx:251`: max(12, min(48, floor(width / 3))).
        assert_eq!(editor_label_width(20), 12);
        assert_eq!(editor_label_width(120), 40);
        assert_eq!(editor_label_width(400), 48);
        // The editor label is the basename (or `dir/index.*`), never the
        // parent directories, so a long directory does not force an ellipsis.
        assert_eq!(
            editor_label_display(
                "/w/a-very-long-directory-name/another-long-name/final-file.tsx",
                &[],
                60,
            )
            .as_deref(),
            Some("final-file.tsx")
        );
        // A long basename is middle-truncated at the width/3 budget.
        let long = editor_label_display("/w/src/a-very-long-component-file-name.tsx", &[], 60)
            .unwrap();
        assert!(long.contains('…'), "{long}");
        assert!(utf16_len(&long) <= editor_label_width(60), "{long}");
        assert_eq!(truncate_middle("abcdef", 5), "ab…ef");
        assert_eq!(truncate_middle("abc", 12), "abc");
    }

    // -- CP-16: live counts on the rail -------------------------------------

    #[test]
    fn cp16_live_counts_render_before_usage_after_the_editor_slot() {
        let usage = usage();
        let mut facts = RailFacts::new(80);
        facts.subagents = 2;
        facts.shells = 1;
        facts.usage = &usage;
        facts.command_shortcut = Some("ctrl+p");
        facts.agent_shortcut = Some("tab");
        facts.editor = Some(EditorContext {
            path: "/w/src/index.ts",
            ranges: &[],
            state: EditorState::Attached,
        });
        let spans = rail_spans(&facts);
        assert_eq!(
            segments(&spans),
            vec![
                RailSegment::EditorFile,
                RailSegment::Live,
                RailSegment::Live,
                RailSegment::Usage,
                RailSegment::Commands,
                RailSegment::Commands,
            ]
        );
        let texts = texts(&spans);
        assert_eq!(texts[0], "src/index.ts");
        // `footer.tsx:82-87`: the `session.child.first` key then the counts.
        assert_eq!(texts[1], "tab ");
        assert_eq!(texts[2], "2 subagents · 1 shell");
        // `footer.tsx:91-95`: ` · ` separator before the metrics.
        assert_eq!(texts[3], " · 1.2K (3%)");
        assert_eq!(texts[4], "ctrl+p ");
        assert_eq!(texts[5], "commands");
        // The live branch never carries the `agents` hint.
        assert!(!spans.iter().any(|span| span.segment == RailSegment::Agents));
    }

    // -- CP-19: footer priority and fitting ---------------------------------

    #[test]
    fn cp19_available_width_ramp_matches_the_reserve_formula() {
        assert_eq!(layout_width(20), 12);
        assert_eq!(layout_available_width(4), 0);
        assert_eq!(layout_available_width(20), 6);
        assert_eq!(layout_available_width(28), 10);
        assert_eq!(layout_available_width(36), 14);
        assert_eq!(layout_available_width(43), 18);
        assert_eq!(layout_available_width(44), 18);
        assert_eq!(layout_available_width(60), 26);
        assert_eq!(layout_available_width(61), 27);
        assert_eq!(layout_available_width(64), 28);
        assert_eq!(layout_available_width(80), 44);
        assert_eq!(layout_available_width(120), 84);
    }

    #[test]
    fn cp19_usage_outranks_shortcuts_at_the_fit_boundary() {
        let usage = usage(); // 9 cells
        // Both need 27 cells; available reaches 27 at 61 columns.
        for width in [28, 36, 43, 44, 60] {
            let layout = prompt_footer_layout(width, &usage, Some("ctrl+p"));
            assert!(layout.usage, "usage at {width}");
            assert!(!layout.shortcuts, "shortcuts at {width}: {layout:?}");
        }
        for width in [61, 62, 64, 80] {
            assert_eq!(
                prompt_footer_layout(width, &usage, Some("ctrl+p")),
                FooterLayout {
                    usage: true,
                    shortcuts: true
                },
                "width {width}"
            );
        }
        // Below the available width even usage is dropped.
        assert_eq!(
            prompt_footer_layout(20, &usage, Some("ctrl+p")),
            FooterLayout {
                usage: false,
                shortcuts: false
            }
        );
    }

    #[test]
    fn cp19_usage_alone_never_yields_to_a_shortcut_that_would_fit() {
        // 24-cell usage at 44 columns: available is 18, so usage cannot fit;
        // upstream still returns `{usage:false, shortcuts:false}` because the
        // shortcut-only branch requires *no* usage list at all.
        let long = vec!["123.4K (87%)".to_owned(), "$1,234.50".to_owned()];
        assert_eq!(
            prompt_footer_layout(44, &long, Some("ctrl+p")),
            FooterLayout {
                usage: false,
                shortcuts: false
            }
        );
        assert!(prompt_footer_layout(60, &long, Some("ctrl+p")).usage);
        assert!(!prompt_footer_layout(60, &long, Some("ctrl+p")).shortcuts);
    }

    #[test]
    fn cp19_shortcut_block_without_status_is_gated_on_width_44() {
        for width in [0, 20, 43] {
            assert!(
                !prompt_footer_layout(width, &[], Some("ctrl+p")).shortcuts,
                "width {width}"
            );
        }
        for width in [44, 45, 80, 120] {
            assert!(
                prompt_footer_layout(width, &[], Some("ctrl+p")).shortcuts,
                "width {width}"
            );
        }
        // Upstream returns `shortcuts: true` from 44 even with no configured
        // command key; the span itself still requires a key (tested below).
        assert!(prompt_footer_layout(80, &[], None).shortcuts);
        let mut facts = RailFacts::new(80);
        facts.location = Some("~/p");
        facts.agent_shortcut = Some("tab");
        let spans = rail_spans(&facts);
        assert_eq!(
            segments(&spans),
            vec![
                RailSegment::Location,
                RailSegment::Agents,
                RailSegment::Agents
            ]
        );
    }

    #[test]
    fn cp19_show_details_false_drops_metrics_and_hints_but_keeps_the_rail() {
        let usage = usage();
        let mut facts = RailFacts::new(79);
        facts.usage = &usage;
        facts.command_shortcut = Some("ctrl+p");
        facts.agent_shortcut = Some("tab");
        facts.location = Some("~/p");
        assert_eq!(
            facts.interrupt.press(0, true, false, true, false, true),
            InterruptOutcome::Armed(1)
        );
        // Not running: no scanner, but details are still suppressed.
        let spans = rail_spans(&facts);
        assert_eq!(segments(&spans), vec![RailSegment::Location]);
        facts.terminal_width = 80;
        let spans = rail_spans(&facts);
        assert_eq!(
            segments(&spans),
            vec![
                RailSegment::Location,
                RailSegment::Usage,
                RailSegment::Commands,
                RailSegment::Commands
            ]
        );
    }

    #[test]
    fn cp19_live_status_plus_metrics_compose_without_a_shortcut() {
        let usage = usage();
        let mut facts = RailFacts::new(80);
        facts.subagents = 1;
        facts.usage = &usage;
        let spans = rail_spans(&facts);
        let texts = texts(&spans);
        assert_eq!(texts, vec!["1 subagent", " · 1.2K (3%)"]);
        // No live counts and no usage: an empty status list renders no footer
        // body at all.
        facts.subagents = 0;
        facts.usage = &[];
        assert!(rail_spans(&facts).is_empty());
    }

    // -- CP-20: hover and disabled hint styling -----------------------------

    #[test]
    fn cp20_hover_and_disabled_hints_keep_their_shortcut_identity() {
        let mut facts = RailFacts::new(80);
        facts.subagents = 1;
        facts.agent_shortcut = Some("tab");
        facts.command_shortcut = Some("ctrl+p");
        facts.location = Some("~/p");
        let spans = rail_spans(&facts);
        // Muted counts, base shortcut key (`footer.tsx:82-88`).
        assert_eq!(span_of(&spans, RailSegment::Live).role, RailRole::TextBase);
        let counts = spans
            .iter()
            .filter(|span| span.segment == RailSegment::Live)
            .nth(1)
            .unwrap();
        assert_eq!(counts.role, RailRole::TextMuted);
        facts.live_hovered = true;
        let spans = rail_spans(&facts);
        let counts = spans
            .iter()
            .filter(|span| span.segment == RailSegment::Live)
            .nth(1)
            .unwrap();
        assert_eq!(counts.role, RailRole::TextBase);

        // A focused side panel disables both halves of the hint without
        // dropping the shortcut text (`footer.tsx:100-119`).
        facts.live_hovered = false;
        facts.interactive = false;
        let spans = rail_spans(&facts);
        let commands: Vec<&RailSpan> = spans
            .iter()
            .filter(|span| span.segment == RailSegment::Commands)
            .collect();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].text, "ctrl+p ");
        assert_eq!(commands[0].role, RailRole::TextDisabled);
        assert_eq!(commands[1].text, "commands");
        assert_eq!(commands[1].role, RailRole::TextDisabled);

        // Idle (no live branch): the agents hint carries the same roles.
        facts.subagents = 0;
        let spans = rail_spans(&facts);
        let agents: Vec<&RailSpan> = spans
            .iter()
            .filter(|span| span.segment == RailSegment::Agents)
            .collect();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].text, "tab ");
        assert_eq!(agents[0].role, RailRole::TextDisabled);
        assert_eq!(agents[1].text, "agents");
        assert_eq!(agents[1].role, RailRole::TextDisabled);
        facts.interactive = true;
        let spans = rail_spans(&facts);
        assert_eq!(span_of(&spans, RailSegment::Agents).role, RailRole::TextBase);
    }

    #[test]
    fn cp20_shell_mode_owns_the_footer_but_not_the_status_slot() {
        let usage = usage();
        let mut facts = RailFacts::new(80);
        facts.mode = PromptMode::Shell;
        facts.location = Some("~/project");
        facts.subagents = 3;
        facts.usage = &usage;
        facts.command_shortcut = Some("ctrl+p");
        let spans = rail_spans(&facts);
        assert_eq!(
            segments(&spans),
            vec![
                RailSegment::Location,
                RailSegment::ShellHint,
                RailSegment::ShellHint
            ]
        );
        assert_eq!(slot_text(&spans, RailSlot::Footer), "esc exit shell mode");
        assert!(!spans.iter().any(|span| span.segment == RailSegment::Live));
        facts.terminal_width = 43;
        let spans = rail_spans(&facts);
        assert_eq!(slot_text(&spans, RailSlot::Footer), "esc shell");
        assert_eq!(shell_mode_hint(43), "shell");
        assert_eq!(shell_mode_hint(44), "exit shell mode");
    }

    // -- CP-21: queue dock --------------------------------------------------

    #[test]
    fn cp21_queue_dock_geometry_and_label() {
        let facts = BottomFacts {
            enabled: true,
            ..BottomFacts::default()
        };
        let dock = queue_dock(&facts, &["first line\nsecond", "next"], false).unwrap();
        assert_eq!(dock.label, "2 queued · first line second");
        assert_eq!(dock.text_width(80), 76);
        assert_eq!(QUEUE_DOCK_HEIGHT, 3);
        assert_eq!(QUEUE_DOCK_MARGIN_BOTTOM, 1);
        assert_eq!(QUEUE_DOCK_PADDING_TOP, 1);
        assert_eq!(QUEUE_DOCK_PADDING_BOTTOM, 1);
        assert_eq!(QUEUE_DOCK_PADDING_LEFT, 2);
        assert_eq!(QUEUE_DOCK_PADDING_RIGHT, 1);
        // Pointer hover swaps the background; identity is unchanged.
        let hovered = queue_dock(&facts, &["a"], true).unwrap();
        assert!(hovered.hovered);
        assert_eq!(hovered.label, "1 queued · a");
        // An empty first prompt drops the suffix.
        assert_eq!(
            queue_dock(&facts, &["", "b"], false).unwrap().label,
            "2 queued"
        );
    }

    #[test]
    fn cp21_queue_dock_hidden_for_zero_prompts_open_composer_or_disabled() {
        let facts = BottomFacts {
            enabled: true,
            ..BottomFacts::default()
        };
        assert_eq!(queue_dock(&facts, &[], false), None);
        let open = BottomFacts {
            composer_open: true,
            ..facts
        };
        assert_eq!(queue_dock(&open, &["a"], false), None);
        let disabled = BottomFacts {
            enabled: false,
            ..facts
        };
        assert_eq!(queue_dock(&disabled, &["a"], false), None);
    }

    // -- CP-22: bottom precedence -------------------------------------------

    #[test]
    fn cp22_bottom_slot_precedence_covers_every_combination() {
        let base = BottomFacts {
            enabled: true,
            ..BottomFacts::default()
        };
        // bottom-priority-queue / plain prompt.
        assert_eq!(bottom_slot(&base), BottomSlot::Prompt);
        // The composer wins over everything once open.
        assert_eq!(
            bottom_slot(&BottomFacts {
                composer_open: true,
                permissions: 1,
                forms: 1,
                ..base
            }),
            BottomSlot::Composer
        );
        // Child sessions force the composer open while no form exists.
        assert_eq!(
            bottom_slot(&BottomFacts {
                child_session: true,
                permissions: 1,
                ..base
            }),
            BottomSlot::Composer
        );
        // bottom-priority-child_with_form: a form closes the composer first,
        // so the form owns the bottom.
        assert_eq!(
            bottom_slot(&BottomFacts {
                child_session: true,
                forms: 1,
                ..base
            }),
            BottomSlot::Form
        );
        // bottom-priority-permission / bottom-priority-dock_and_permission.
        assert_eq!(
            bottom_slot(&BottomFacts {
                permissions: 1,
                forms: 1,
                location_missing: true,
                ..base
            }),
            BottomSlot::Permission
        );
        // bottom-priority-form.
        assert_eq!(
            bottom_slot(&BottomFacts {
                forms: 1,
                location_missing: true,
                ..base
            }),
            BottomSlot::Form
        );
        // bottom-priority-missing_location.
        assert_eq!(
            bottom_slot(&BottomFacts {
                location_missing: true,
                ..base
            }),
            BottomSlot::LocationMissing
        );
        // A disabled route with no other surface renders nothing.
        assert_eq!(
            bottom_slot(&BottomFacts {
                enabled: false,
                ..base
            }),
            BottomSlot::Empty
        );
        // ... but a pending permission still renders.
        assert_eq!(
            bottom_slot(&BottomFacts {
                enabled: false,
                permissions: 1,
                ..base
            }),
            BottomSlot::Permission
        );
    }

    #[test]
    fn cp22_queue_dock_coexists_with_permission_and_child_close_navigates() {
        let facts = BottomFacts {
            enabled: true,
            permissions: 1,
            ..BottomFacts::default()
        };
        assert_eq!(bottom_slot(&facts), BottomSlot::Permission);
        assert!(queue_dock(&facts, &["queued"], false).is_some());
        // `index.tsx:1621-1626`: closing a child composer navigates to the
        // parent; otherwise it just closes.
        assert_eq!(
            composer_close(Some("ses_parent")),
            ComposerClose::NavigateToParent("ses_parent")
        );
        assert_eq!(composer_close(None), ComposerClose::Close);
        assert_eq!(composer_close(Some("")), ComposerClose::Close);
    }
}
