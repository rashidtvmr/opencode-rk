#![forbid(unsafe_code)]
//! Session composer auxiliary dock: tab container, chrome and terminals tab.
//!
//! Ports the pinned upstream composer shell
//! (`packages/tui/src/routes/session/composer/index.tsx:21-157`) and its
//! terminals tab (`.../composer/terminals-tab.tsx:9-114`). The queue dock's
//! *visibility gate* and row text are included because they share this slot
//! (`routes/session/index.tsx:1591-1593`, `2485-2519`); the queue dock's
//! geometry belongs to package P09 (`native_footer.rs`) and is not duplicated
//! here.
//!
//! Upstream facts encoded below, with the exact source line:
//! - tabs are stored in registration order and the first registration becomes
//!   active when none is (`index.tsx:29,49`);
//! - opening the composer picks `defaultTab` when registered, else the first
//!   tab, and re-runs whenever the tab list changes while open
//!   (`index.tsx:34-40`);
//! - `left`/`right` switch with `(idx + dir + len) % len` and are inert with a
//!   single tab (`index.tsx:65-70`), `escape` and `ctrl+c` both close
//!   (`index.tsx:79-80`);
//! - one tab renders a single bold `text.base` label, several render a gap-2
//!   row where the active tab is bold `text.base` and the rest are
//!   `text.muted`, and a muted `esc` sits right-aligned and closes on click
//!   (`index.tsx:98-126`);
//! - the hint row renders each hint as bold `text.base` label + space +
//!   `text.muted` shortcut with gap 2, and appends `tabs ←/→` when more than
//!   one tab is registered (`index.tsx:132-151`);
//! - the terminals tab is mounted only when the route passes
//!   `terminals` (`index.tsx:129-131`);
//! - terminals rows use `foregroundProcess ?? title ?? "+ New terminal"`,
//!   active (focused) rows beat the currently visible terminal's row, and the
//!   list always ends with the New-terminal row
//!   (`terminals-tab.tsx:71,74,80-106`);
//! - terminal up/down wrap over `entries.length + 1` rows
//!   (`terminals-tab.tsx:51,57`);
//! - selecting closes the composer first, then selects an existing terminal or
//!   creates one, and any failure shows an error toast
//!   (`terminals-tab.tsx:14,30-40`).

use crate::native_view_layout::fit;

/// Upstream `scrollbox maxHeight={5}` shared by every dock list
/// (`shell-tab.tsx:115`, `subagents-tab.tsx:200`, `terminals-tab.tsx:70`).
pub const MAX_DOCK_ROWS: usize = 5;
/// Left SplitBorder glyph (`ui/border.ts:15-20`, `index.tsx:88-96`).
pub const DOCK_BORDER_LEFT: char = '┃';
pub const DOCK_PADDING_TOP: u16 = 1;
pub const DOCK_PADDING_BOTTOM: u16 = 1;
pub const DOCK_PADDING_LEFT: u16 = 1;
pub const DOCK_PADDING_RIGHT: u16 = 2;
/// `box gap={1}` between the title row, list rows and hint row
/// (`index.tsx:97`).
pub const DOCK_GAP: u16 = 1;
/// `box flexDirection="row" gap={2}` for the tab row (`index.tsx:107`) and the
/// hint row (`index.tsx:132`).
pub const DOCK_TAB_GAP: u16 = 2;
pub const DOCK_HINT_GAP: u16 = 2;
/// Title and hint rows carry their own `paddingLeft={1}` (`index.tsx:98,132`),
/// and list rows `paddingLeft={1}` (`terminals-tab.tsx:78`).
pub const DOCK_ROW_PADDING_LEFT: u16 = 1;
/// Display column where dock row text starts: 1 border + 1 outer padding +
/// 1 row padding.
pub const DOCK_TEXT_X: u16 = 1 + DOCK_PADDING_LEFT + DOCK_ROW_PADDING_LEFT;
/// Exact right-aligned affordance (`index.tsx:123-125`).
pub const DOCK_ESC_LABEL: &str = "esc";
/// Appended hint when more than one tab is registered (`index.tsx:143-150`).
pub const DOCK_TABS_HINT_LABEL: &str = "tabs";
pub const DOCK_TABS_HINT_SHORTCUT: &str = "←/→";
/// The New-terminal row label (`terminals-tab.tsx:105`).
pub const NEW_TERMINAL_LABEL: &str = "+ New terminal";
/// Exact terminal failure toast text (`terminals-tab.tsx:14`).
pub const TERMINAL_FAILURE_TOAST: &str = "Unable to load terminal";
/// First composer tab id/label (`subagents-tab.tsx:120-121`).
pub const SUBAGENTS_TAB_ID: &str = "subagents";
pub const SUBAGENTS_TAB_LABEL: &str = "Subagents";
/// Shell tab id/label (`shell-tab.tsx:51-52`).
pub const SHELL_TAB_ID: &str = "shell";
pub const SHELL_TAB_LABEL: &str = "Shell";
/// Terminals tab id/label (`terminals-tab.tsx:20`).
pub const TERMINALS_TAB_ID: &str = "terminals";
pub const TERMINALS_TAB_LABEL: &str = "Terminals";

/// Registration order is the upstream JSX mount order: SubagentsTab,
/// ShellTab, then the guarded TerminalsTab (`index.tsx:127-131`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DockTabId {
    Subagents,
    Shell,
    Terminals,
}

impl DockTabId {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Subagents => SUBAGENTS_TAB_ID,
            Self::Shell => SHELL_TAB_ID,
            Self::Terminals => TERMINALS_TAB_ID,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Subagents => SUBAGENTS_TAB_LABEL,
            Self::Shell => SHELL_TAB_LABEL,
            Self::Terminals => TERMINALS_TAB_LABEL,
        }
    }

    #[must_use]
    pub const fn mount_order(self) -> u8 {
        match self {
            Self::Subagents => 0,
            Self::Shell => 1,
            Self::Terminals => 2,
        }
    }
}

/// One `ComposerHint` (`composer/context.ts:3-6`) plus its resolution state.
/// `shortcut == None` is an unresolved/disabled keybind: upstream renders
/// `shortcuts.get(id) ?? ""`, i.e. the label without a shortcut
/// (`shell-tab.tsx:56-58`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DockHint {
    pub label: String,
    pub shortcut: Option<String>,
}

impl DockHint {
    #[must_use]
    pub fn new(label: impl Into<String>, shortcut: Option<String>) -> Self {
        Self {
            label: label.into(),
            shortcut,
        }
    }
}

/// Paint class of one dock span. Theme token names mirror upstream
/// (`@opentui` theme lookups) without owning palette values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockStyle {
    /// `text.base` + BOLD for a single tab or the active tab
    /// (`index.tsx:102-104,113-114`).
    TabActive,
    /// `text.muted` for the non-active tabs (`index.tsx:113`).
    TabInactive,
    /// `text.muted` right-aligned close affordance (`index.tsx:123`).
    Esc,
    /// `text.base` bold hint label (`index.tsx:136-138`).
    HintLabel,
    /// `text.muted` hint shortcut (`index.tsx:139`).
    HintShortcut,
    /// `text.action.primary.focused` on `background.action.primary.focused`
    /// for the focused terminal row (`terminals-tab.tsx:80-99`).
    TerminalActive,
    /// selected/current terminal row (`terminals-tab.tsx:83-99`).
    TerminalCurrent,
    /// ordinary terminal row (`terminals-tab.tsx:85-99`).
    TerminalBase,
    /// focused shell row (`shell-tab.tsx:125-137`).
    ShellActive,
    /// ordinary shell row (`shell-tab.tsx:125-137`).
    ShellBase,
    /// focused subagent row: `background.action.primary.focused`
    /// (`subagents-tab.tsx:217-239`).
    SubagentActive,
    /// row of the session currently on the route
    /// (`background.action.primary.selected`, `subagents-tab.tsx:217-239`).
    SubagentCurrent,
    /// ordinary subagent row (`background.action.primary.base`).
    SubagentBase,
    /// `Running` cell of the focused subagent row (`text.action.primary.focused`,
    /// `subagents-tab.tsx:247`).
    SubagentStatusActive,
    /// `Running` cell of an unfocused subagent row (`text.muted`,
    /// `subagents-tab.tsx:247`).
    SubagentStatusMuted,
    /// `text.base` count of queued prompts (`index.tsx:2511`).
    QueueCount,
    /// `text.muted` first queued prompt preview (`index.tsx:2511-2513`).
    QueuePreview,
}

/// One styled dock span positioned in display cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DockSpan {
    pub text: String,
    pub style: DockStyle,
    pub bold: bool,
    pub x: u16,
}

/// One dock row: concatenated text plus positioned spans.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DockLine {
    pub text: String,
    pub spans: Vec<DockSpan>,
}

impl DockLine {
    fn push(&mut self, text: impl Into<String>, style: DockStyle, bold: bool) {
        let text = text.into();
        let x = self.text.chars().count() as u16;
        self.text.push_str(&text);
        self.spans.push(DockSpan {
            text,
            style,
            bold,
            x,
        });
    }

    fn push_at(&mut self, text: impl Into<String>, style: DockStyle, bold: bool, x: u16) {
        self.spans.push(DockSpan {
            text: text.into(),
            style,
            bold,
            x,
        });
    }

    /// Place a span at an absolute cell offset *and* append it to
    /// [`Self::text`]. The offset is measured in the composed row (border and
    /// container padding included), while `text` holds only the row content, so
    /// layout gaps between spans are emitted by explicit [`Self::advance`]
    /// calls. Off-flow spans (the right-aligned `esc`) use [`Self::push_at`]
    /// and stay out of the row text.
    fn push_here(&mut self, text: impl Into<String>, style: DockStyle, bold: bool, x: u16) {
        let text = text.into();
        self.text.push_str(&text);
        self.spans.push(DockSpan {
            text,
            style,
            bold,
            x,
        });
    }

    /// Row layout gap (`box gap={2}`): blank cells with no paint span of their
    /// own. They still occupy the rendered row.
    fn advance(&mut self, cells: u16) {
        self.text
            .extend(std::iter::repeat_n(' ', usize::from(cells)));
    }

    #[must_use]
    pub fn height(&self) -> u16 {
        u16::from(!self.spans.is_empty() || !self.text.is_empty())
    }
}

/// Dock chrome geometry, mirrored value-for-value from `index.tsx:87-96`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DockGeometry {
    pub border_left: bool,
    pub padding_top: u16,
    pub padding_bottom: u16,
    pub padding_left: u16,
    pub padding_right: u16,
    pub gap: u16,
    pub title_padding_left: u16,
    pub row_padding_left: u16,
    pub row_padding_right: u16,
    pub tab_gap: u16,
    pub hint_gap: u16,
    pub max_rows: usize,
}

impl Default for DockGeometry {
    fn default() -> Self {
        Self {
            border_left: true,
            padding_top: DOCK_PADDING_TOP,
            padding_bottom: DOCK_PADDING_BOTTOM,
            padding_left: DOCK_PADDING_LEFT,
            padding_right: DOCK_PADDING_RIGHT,
            gap: DOCK_GAP,
            title_padding_left: DOCK_ROW_PADDING_LEFT,
            row_padding_left: DOCK_ROW_PADDING_LEFT,
            row_padding_right: 1,
            tab_gap: DOCK_TAB_GAP,
            hint_gap: DOCK_HINT_GAP,
            max_rows: MAX_DOCK_ROWS,
        }
    }
}

/// Literal layer binds of the composer itself (`index.tsx:77-80`). These four
/// are layer `bind`s, not `composer.*` keybind ids, so they have no config
/// entry to disable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockKey {
    Left,
    Right,
    Escape,
    CtrlC,
}

/// Result of pressing a composer-layer key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockKeyOutcome {
    /// Nothing happened (composer closed, or a single tab cannot wrap).
    Ignored,
    /// Active tab changed.
    Switched(DockTabId),
    /// `onClose` ran (`index.tsx:42-44,79-80`).
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DockTab {
    id: DockTabId,
    hints: Vec<DockHint>,
}

/// The composer container: tab registry, active tab, visibility gate, chrome
/// lines and the queue-dock gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposerDock {
    tabs: Vec<DockTab>,
    active: Option<DockTabId>,
    /// Route store flag (`composer.open`): set by the keymap/dialog that opens
    /// the composer, and the flag the queue dock tests (`index.tsx:1591`).
    open: bool,
    /// Child sessions open the dock by default while no form is shown
    /// (`index.tsx:1597`).
    child_route: bool,
    forms: usize,
    terminals_enabled: bool,
    default_tab: Option<DockTabId>,
    close_requests: u32,
    queued: Vec<String>,
    disabled: bool,
}

impl Default for ComposerDock {
    fn default() -> Self {
        Self::new()
    }
}

impl ComposerDock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            open: false,
            child_route: false,
            forms: 0,
            terminals_enabled: false,
            default_tab: None,
            close_requests: 0,
            queued: Vec::new(),
            disabled: false,
        }
    }

    /// `register` from `composer/context.ts:14-17`: the first registration
    /// becomes active when nothing is active, and tabs keep mount order.
    pub fn register(&mut self, id: DockTabId) {
        if self.tabs.iter().any(|tab| tab.id == id) {
            return;
        }
        if id == DockTabId::Terminals && !self.terminals_enabled {
            return;
        }
        self.tabs.push(DockTab {
            id,
            hints: Vec::new(),
        });
        if self.active.is_none() {
            self.active = Some(id);
        }
        self.sync_active();
    }

    /// Unregister (upstream's `setStore("tabs", tab.id, undefined!)` cleanup).
    pub fn unregister(&mut self, id: DockTabId) {
        self.tabs.retain(|tab| tab.id != id);
        if self.active == Some(id) {
            self.active = None;
        }
        self.sync_active();
    }

    /// Gate for the guarded `<Show when={props.terminals}>` mount
    /// (`index.tsx:129-131`): enabling registers in canonical order,
    /// disabling unmounts the tab (`terminals-tab.tsx:20-21` cleanup).
    pub fn set_terminals_enabled(&mut self, enabled: bool) {
        self.terminals_enabled = enabled;
        if enabled {
            self.register(DockTabId::Terminals);
        } else {
            self.unregister(DockTabId::Terminals);
        }
    }

    #[must_use]
    pub fn terminals_enabled(&self) -> bool {
        self.terminals_enabled
    }

    #[must_use]
    pub fn registered_tabs(&self) -> Vec<DockTabId> {
        self.tabs.iter().map(|tab| tab.id).collect()
    }

    #[must_use]
    pub fn active(&self) -> Option<DockTabId> {
        self.active
            .filter(|id| self.tabs.iter().any(|tab| tab.id == *id))
    }

    /// `active(id)` from `composer/context.ts:18-20`: a tab is active only
    /// while the composer is open.
    #[must_use]
    pub fn is_active(&self, id: DockTabId) -> bool {
        self.effective_open() && self.active() == Some(id)
    }

    /// The `open` prop passed to the Composer (`index.tsx:1597`):
    /// `composer.open || (parentID && forms().length === 0)`.
    #[must_use]
    pub fn effective_open(&self) -> bool {
        self.open || (self.child_route && self.forms == 0)
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.effective_open()
    }

    /// Route store flag only (`composer.open`).
    #[must_use]
    pub fn store_open(&self) -> bool {
        self.open
    }

    /// Session route facts that drive the mount gate: whether the session has
    /// a parent and how many forms are open (`index.tsx:1597-1598,1611`).
    pub fn set_route(&mut self, child_route: bool, forms: usize) {
        self.child_route = child_route;
        self.forms = forms;
        self.sync_active();
    }

    #[must_use]
    pub fn close_requests(&self) -> u32 {
        self.close_requests
    }

    /// Open the composer with the route-provided default tab
    /// (`index.tsx:34-40,1597-1598`).
    pub fn open(&mut self, default_tab: Option<DockTabId>) {
        self.open = true;
        self.default_tab = default_tab;
        self.sync_active();
    }

    /// The `createEffect` at `index.tsx:34-40`: with an open composer and a
    /// non-empty tab list, `defaultTab` wins when registered, else the first
    /// tab. It re-runs on every tab-list change, so it is also called from
    /// [`Self::register`]/[`Self::unregister`].
    fn sync_active(&mut self) {
        if !self.effective_open() || self.tabs.is_empty() {
            return;
        }
        let preferred = self
            .default_tab
            .filter(|id| self.tabs.iter().any(|tab| tab.id == *id));
        self.active = Some(preferred.unwrap_or(self.tabs[0].id));
    }

    /// `onClose` request (`index.tsx:42-44`). Navigating a child session to its
    /// parent is the caller's route decision; the dock only records the close
    /// and stops claiming the slot.
    pub fn close(&mut self) {
        self.close_requests = self.close_requests.saturating_add(1);
        self.open = false;
    }

    /// `index.tsx:235-236`: a muted prompt force-closes an open composer.
    pub fn apply_prompt_muted(&mut self, prompt_muted: bool) {
        if prompt_muted && self.open {
            self.close();
        }
    }

    /// Mount gate at `index.tsx:1597`: `composer.open || (parentID &&
    /// forms().length === 0)`. The same condition hides the
    /// permission/form/prompt branch at `index.tsx:1611`.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.effective_open()
    }

    /// `switchTab` at `index.tsx:65-70`: inert for a single tab, otherwise
    /// wraps with `(idx + dir + len) % len` and `idx == -1` when the active id
    /// is no longer registered (JS `findIndex`).
    pub fn switch_tab(&mut self, dir: i32) -> DockKeyOutcome {
        let len = self.tabs.len();
        if !self.effective_open() || len <= 1 {
            return DockKeyOutcome::Ignored;
        }
        let len_i = len as i32;
        let idx = self
            .active()
            .and_then(|id| self.tabs.iter().position(|tab| tab.id == id))
            .map_or(-1, |index| index as i32);
        let next = (idx + dir + len_i) % len_i;
        let next = self.tabs[next.rem_euclid(len_i) as usize].id;
        self.active = Some(next);
        DockKeyOutcome::Switched(next)
    }

    /// Composer layer commands (`index.tsx:72-82`).
    pub fn key(&mut self, key: DockKey) -> DockKeyOutcome {
        if !self.effective_open() {
            return DockKeyOutcome::Ignored;
        }
        match key {
            DockKey::Left => self.switch_tab(-1),
            DockKey::Right => self.switch_tab(1),
            DockKey::Escape | DockKey::CtrlC => {
                self.close();
                DockKeyOutcome::Closed
            }
        }
    }

    /// Late-bound hints (`hints: () => ...`, `context.ts:7-8`).
    pub fn set_hints(&mut self, id: DockTabId, hints: Vec<DockHint>) {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
            tab.hints = hints;
        }
    }

    /// `footerHints` at `index.tsx:31`: the active tab's hints, followed by
    /// the `tabs ←/→` hint when more than one tab is registered.
    #[must_use]
    pub fn footer_hints(&self) -> Vec<DockHint> {
        let mut hints = self
            .active()
            .and_then(|id| self.tabs.iter().find(|tab| tab.id == id))
            .map(|tab| tab.hints.clone())
            .unwrap_or_default();
        if self.tabs.len() > 1 {
            hints.push(DockHint::new(
                DOCK_TABS_HINT_LABEL,
                Some(DOCK_TABS_HINT_SHORTCUT.into()),
            ));
        }
        hints
    }

    /// Tab row (`index.tsx:98-126`): single label, or gap-2 tab labels plus a
    /// right-aligned `esc`. Returns `None` while closed (the whole dock box has
    /// `visible={props.open}`).
    #[must_use]
    pub fn title_line(&self, width: u16) -> Option<DockLine> {
        if !self.effective_open() {
            return None;
        }
        let mut line = DockLine::default();
        let mut x = DOCK_TEXT_X;
        match self.tabs.as_slice() {
            [] => {}
            [tab] => {
                line.push_here(tab.id.label(), DockStyle::TabActive, true, x);
            }
            tabs => {
                for (index, tab) in tabs.iter().enumerate() {
                    let active = self.active() == Some(tab.id);
                    let label = tab.id.label();
                    line.push_here(
                        label,
                        if active {
                            DockStyle::TabActive
                        } else {
                            DockStyle::TabInactive
                        },
                        active,
                        x,
                    );
                    let label_len = label.chars().count() as u16;
                    x = x.saturating_add(label_len);
                    if index + 1 < tabs.len() {
                        line.advance(DOCK_TAB_GAP);
                        x = x.saturating_add(DOCK_TAB_GAP);
                    }
                }
            }
        }
        // Right-aligned `esc` inside the outer right padding (`index.tsx:123`).
        let esc_x = width
            .saturating_sub(DOCK_PADDING_RIGHT)
            .saturating_sub(DOCK_ESC_LABEL.chars().count() as u16);
        line.push_at(DOCK_ESC_LABEL, DockStyle::Esc, false, esc_x);
        // The row text is the left-aligned content; the esc affordance lives at
        // `esc_x` (see `esc_hit`).
        Some(line)
    }

    /// Click target of the right-aligned `esc` affordance; the whole label is
    /// clickable and closes (`index.tsx:123`).
    #[must_use]
    pub fn esc_hit(&self, width: u16) -> Option<(u16, u16)> {
        if !self.effective_open() {
            return None;
        }
        let start = width
            .saturating_sub(DOCK_PADDING_RIGHT)
            .saturating_sub(DOCK_ESC_LABEL.chars().count() as u16);
        Some((start, start + DOCK_ESC_LABEL.chars().count() as u16 - 1))
    }

    /// Hint row (`index.tsx:132-151`): each hint is `bold label + " " +
    /// muted shortcut`, entries separated by gap 2.
    #[must_use]
    pub fn hint_line(&self) -> Option<DockLine> {
        if !self.effective_open() {
            return None;
        }
        let mut line = DockLine::default();
        let hints = self.footer_hints();
        let mut x = DOCK_TEXT_X;
        for (index, hint) in hints.iter().enumerate() {
            let label_len = hint.label.chars().count() as u16;
            line.push_here(&hint.label, DockStyle::HintLabel, true, x);
            line.push_here(" ", DockStyle::HintLabel, false, x + label_len);
            let shortcut = hint.shortcut.clone().unwrap_or_default();
            let shortcut_len = shortcut.chars().count() as u16;
            line.push_here(shortcut, DockStyle::HintShortcut, false, x + label_len + 1);
            x = x.saturating_add(label_len + 1 + shortcut_len);
            if index + 1 < hints.len() {
                line.advance(DOCK_HINT_GAP);
                x = x.saturating_add(DOCK_HINT_GAP);
            }
        }
        Some(line)
    }

    /// Queue dock gate at `index.tsx:1591`: shown only when the composer store
    /// flag is closed, the prompt is enabled, and prompts are queued.
    #[must_use]
    pub fn queue_visible(&self) -> bool {
        !self.open && !self.disabled && !self.queued.is_empty()
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    pub fn set_queued(&mut self, prompts: Vec<String>) {
        self.queued = prompts;
    }

    /// Queue dock row text (`index.tsx:2509-2514`): `{n} queued` plus
    /// ` · {first prompt}` with newlines flattened, or just the count while the
    /// first prompt is unavailable.
    #[must_use]
    pub fn queue_line(&self) -> Option<DockLine> {
        if !self.queue_visible() {
            return None;
        }
        let mut line = DockLine::default();
        line.push(
            format!("{} queued", self.queued.len()),
            DockStyle::QueueCount,
            true,
        );
        if let Some(first) = self.queued.first() {
            let flat = first.replace('\n', " ");
            if !flat.is_empty() {
                line.push(format!(" · {flat}"), DockStyle::QueuePreview, false);
            }
        }
        Some(line)
    }

    #[must_use]
    pub fn geometry(&self) -> DockGeometry {
        DockGeometry::default()
    }
}

/// One terminal record rendered by the terminals tab
/// (`terminals-tab.tsx:71-105`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalEntry {
    pub id: String,
    pub foreground_process: Option<String>,
    pub title: Option<String>,
}

impl TerminalEntry {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        foreground_process: Option<String>,
        title: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            foreground_process,
            title,
        }
    }

    /// `terminal?.foregroundProcess ?? terminal?.title ?? "+ New terminal"`
    /// with JS `??` semantics (`terminals-tab.tsx:105`): an empty string is a
    /// real value, `None` falls through.
    #[must_use]
    pub fn label(&self) -> &str {
        self.foreground_process
            .as_deref()
            .or(self.title.as_deref())
            .unwrap_or(NEW_TERMINAL_LABEL)
    }
}

/// One rendered terminal row, including the trailing New-terminal row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalRow {
    pub label: String,
    pub active: bool,
    pub current: bool,
    pub style: DockStyle,
}

/// Requests emitted by the terminals tab; the caller performs IO.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalEffect {
    /// Nothing selected: `if (index === undefined) return`
    /// (`terminals-tab.tsx:31-32`).
    Noop,
    /// `terminals.selectTerminal(sessionID, id)` (`terminals-tab.tsx:36`).
    Select(String),
    /// `terminals.newTerminal(sessionID)` for the New-terminal row
    /// (`terminals-tab.tsx:39`).
    Create,
}

/// One `select()` result: the composer closes first, then the effect runs
/// (`terminals-tab.tsx:30-40`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSelection {
    pub close_composer: bool,
    pub effect: TerminalEffect,
}

/// List state of the terminals tab.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalsTab {
    entries: Vec<TerminalEntry>,
    visible_id: Option<String>,
    selected: Option<usize>,
    toast: Option<String>,
}

impl TerminalsTab {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the session's terminals. Selection is *not* re-clamped:
    /// upstream only recomputes it when the tab becomes active, and `select()`
    /// treats an out-of-range index as the New-terminal row
    /// (`terminals-tab.tsx:24-28,30-40`).
    pub fn set_entries(&mut self, entries: Vec<TerminalEntry>) {
        self.entries = entries;
    }

    #[must_use]
    pub fn entries(&self) -> &[TerminalEntry] {
        &self.entries
    }

    /// Effect at `terminals-tab.tsx:24-28`: on activation the selection is the
    /// row of the currently visible terminal, or none when it is absent.
    pub fn activate(&mut self, visible_terminal_id: Option<&str>) {
        self.visible_id = visible_terminal_id.map(str::to_owned);
        let index = visible_terminal_id
            .and_then(|id| self.entries.iter().position(|terminal| terminal.id == id));
        self.selected = index;
    }

    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Mouse move selects the hovered row (`terminals-tab.tsx:87`).
    pub fn hover(&mut self, index: Option<usize>) {
        self.selected = index;
    }

    /// `composer.terminal.up`: `((index ?? 0) + len) % (len + 1)`
    /// (`terminals-tab.tsx:51`). `len == 0` still selects the New row.
    pub fn up(&mut self) {
        let len = self.entries.len();
        let index = self.selected.map_or(0, |index| index);
        self.selected = Some((index + len) % (len + 1));
    }

    /// `composer.terminal.down`: `((index ?? -1) + 1) % (len + 1)`
    /// (`terminals-tab.tsx:57`).
    pub fn down(&mut self) {
        let len = self.entries.len();
        let index = self.selected.map_or(-1, |index| index as i64);
        self.selected = Some(((index + 1) % (len as i64 + 1)) as usize);
    }

    /// Rows in paint order: every terminal then the New-terminal row
    /// (`terminals-tab.tsx:71`). Active (focused) wins over current
    /// (`terminals-tab.tsx:80-86`).
    #[must_use]
    pub fn rows(&self) -> Vec<TerminalRow> {
        let mut rows: Vec<TerminalRow> = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, terminal)| {
                let active = self.selected == Some(index);
                let current = self.visible_id.as_deref() == Some(terminal.id.as_str());
                TerminalRow {
                    label: terminal.label().to_owned(),
                    active,
                    current,
                    style: if active {
                        DockStyle::TerminalActive
                    } else if current {
                        DockStyle::TerminalCurrent
                    } else {
                        DockStyle::TerminalBase
                    },
                }
            })
            .collect();
        let index = self.entries.len();
        let active = self.selected == Some(index);
        rows.push(TerminalRow {
            label: NEW_TERMINAL_LABEL.to_owned(),
            active,
            current: false,
            style: if active {
                DockStyle::TerminalActive
            } else {
                DockStyle::TerminalBase
            },
        });
        rows
    }

    /// Painted label clipped to the row's available cells
    /// (`paddingLeft={1}`, `paddingRight={1}`, `truncate`
    /// `terminals-tab.tsx:78-79,103`).
    #[must_use]
    pub fn row_text(row: &TerminalRow, width: u16) -> String {
        let available = width.saturating_sub(DOCK_TEXT_X).saturating_sub(1);
        fit(&row.label, available)
    }

    /// `composer.terminal.select` (`terminals-tab.tsx:30-40`): close the
    /// composer, then select the terminal under the cursor or create a new one.
    /// An index past the last terminal also lands on New-terminal, because
    /// `entries()[index]` is `undefined` there.
    pub fn select(&mut self) -> TerminalSelection {
        let Some(index) = self.selected else {
            return TerminalSelection {
                close_composer: false,
                effect: TerminalEffect::Noop,
            };
        };
        let effect = self
            .entries
            .get(index)
            .map(|terminal| TerminalEffect::Select(terminal.id.clone()))
            .unwrap_or(TerminalEffect::Create);
        TerminalSelection {
            close_composer: true,
            effect,
        }
    }

    /// `.catch(failure)` for select/create (`terminals-tab.tsx:14,36,39`):
    /// one error toast with the exact upstream message.
    pub fn fail(&mut self) {
        self.toast = Some(TERMINAL_FAILURE_TOAST.to_owned());
    }

    #[must_use]
    pub fn toast(&self) -> Option<&str> {
        self.toast.as_deref()
    }

    pub fn take_toast(&mut self) -> Option<String> {
        self.toast.take()
    }
}

#[cfg(test)]
mod native_dock_tests {
    use super::*;

    fn dock_with_two_tabs() -> ComposerDock {
        let mut dock = ComposerDock::new();
        dock.register(DockTabId::Subagents);
        dock.register(DockTabId::Shell);
        dock
    }

    // CP-23: one/two/three registered tabs in mount order.
    #[test]
    fn cp23_one_two_three_registered_tabs_and_order() {
        // The JSX child order is SubagentsTab, ShellTab, the guarded
        // TerminalsTab, and `Object.values(store.tabs)` keeps that insertion
        // order (`index.tsx:31,127-131`).
        assert!(DockTabId::Subagents.mount_order() < DockTabId::Shell.mount_order());
        assert!(DockTabId::Shell.mount_order() < DockTabId::Terminals.mount_order());
        let mut dock = ComposerDock::new();
        dock.register(DockTabId::Subagents);
        assert_eq!(dock.registered_tabs(), vec![DockTabId::Subagents]);
        dock.register(DockTabId::Shell);
        assert_eq!(
            dock.registered_tabs(),
            vec![DockTabId::Subagents, DockTabId::Shell]
        );
        // Terminals only mount when the route passes `terminals`.
        dock.register(DockTabId::Terminals);
        assert_eq!(
            dock.registered_tabs(),
            vec![DockTabId::Subagents, DockTabId::Shell]
        );
        dock.set_terminals_enabled(true);
        assert_eq!(
            dock.registered_tabs(),
            vec![DockTabId::Subagents, DockTabId::Shell, DockTabId::Terminals]
        );
        // First registration becomes active (`index.tsx:47-49`).
        assert_eq!(dock.active(), Some(DockTabId::Subagents));
        // Later registrations leave the active tab alone (`if (!store.active)`).
        let mut shellless = ComposerDock::new();
        shellless.register(DockTabId::Shell);
        shellless.register(DockTabId::Subagents);
        assert_eq!(shellless.active(), Some(DockTabId::Shell));
    }

    #[test]
    fn cp23_open_default_tab_and_remount_effect() {
        let mut dock = dock_with_two_tabs();
        dock.set_terminals_enabled(true);
        dock.open(None);
        assert_eq!(dock.active(), Some(DockTabId::Subagents));
        dock.close();
        dock.open(Some(DockTabId::Shell));
        assert_eq!(dock.active(), Some(DockTabId::Shell));
        // A default tab that is not registered falls back to the first tab.
        dock.close();
        dock.open(Some(DockTabId::Terminals));
        assert_eq!(dock.active(), Some(DockTabId::Terminals));
        dock.set_terminals_enabled(false);
        assert_eq!(
            dock.active(),
            Some(DockTabId::Subagents),
            "the open effect re-runs on tab-list changes (index.tsx:34-40)"
        );
    }

    // CP-23: left/right wrap with (idx + dir + len) % len.
    #[test]
    fn cp23_tab_wrap_left_right() {
        let mut dock = dock_with_two_tabs();
        dock.open(None);
        assert_eq!(
            dock.key(DockKey::Left),
            DockKeyOutcome::Switched(DockTabId::Shell)
        );
        assert_eq!(dock.active(), Some(DockTabId::Shell));
        assert_eq!(
            dock.key(DockKey::Right),
            DockKeyOutcome::Switched(DockTabId::Subagents)
        );
        assert_eq!(dock.active(), Some(DockTabId::Subagents));
        assert_eq!(
            dock.key(DockKey::Left),
            DockKeyOutcome::Switched(DockTabId::Shell),
            "a single right step from the first tab wraps to the last"
        );
        // A single tab never wraps.
        let mut single = ComposerDock::new();
        single.register(DockTabId::Shell);
        single.open(None);
        assert_eq!(single.key(DockKey::Right), DockKeyOutcome::Ignored);
        assert_eq!(single.active(), Some(DockTabId::Shell));
    }

    // CP-23: Escape and Ctrl+C both close; a closed composer ignores keys.
    #[test]
    fn cp23_escape_and_ctrl_c_close() {
        let mut dock = dock_with_two_tabs();
        dock.open(None);
        assert_eq!(dock.key(DockKey::Escape), DockKeyOutcome::Closed);
        assert!(!dock.is_open());
        assert_eq!(dock.close_requests(), 1);
        dock.open(None);
        assert_eq!(dock.key(DockKey::CtrlC), DockKeyOutcome::Closed);
        assert_eq!(dock.close_requests(), 2);
        assert_eq!(dock.key(DockKey::Right), DockKeyOutcome::Ignored);
        assert_eq!(dock.title_line(80), None, "closed dock paints no title");
        assert_eq!(dock.hint_line(), None);
    }

    // CP-23: title row and hint row layout, styles and positions.
    #[test]
    fn cp23_title_and_hint_layout() {
        let mut dock = dock_with_two_tabs();
        dock.open(None);
        let title = dock.title_line(40).expect("open dock paints a title");
        assert_eq!(title.text, "Subagents  Shell");
        assert_eq!(title.spans.len(), 3);
        assert_eq!(
            title.spans[0],
            DockSpan {
                text: "Subagents".into(),
                style: DockStyle::TabActive,
                bold: true,
                x: DOCK_TEXT_X,
            }
        );
        assert_eq!(title.spans[1].style, DockStyle::TabInactive);
        assert!(!title.spans[1].bold);
        // Gap 2 between tabs: 3 + 9 + 2.
        assert_eq!(title.spans[1].x, DOCK_TEXT_X + 9 + DOCK_TAB_GAP);
        // Right-aligned esc: width - right padding - 3.
        assert_eq!(title.spans[2].style, DockStyle::Esc);
        assert_eq!(title.spans[2].x, 40 - DOCK_PADDING_RIGHT - 3);
        assert_eq!(dock.esc_hit(40), Some((35, 37)));

        // The active tab follows the switch.
        dock.key(DockKey::Right);
        let title = dock.title_line(40).unwrap();
        assert_eq!(title.spans[0].style, DockStyle::TabInactive);
        assert_eq!(title.spans[1].style, DockStyle::TabActive);
        assert!(title.spans[1].bold);

        // Hint layout: [label bold][space][shortcut muted], gap 2, then the
        // `tabs ←/→` hint appended because two tabs are registered.
        dock.set_hints(
            DockTabId::Shell,
            vec![
                DockHint::new("output", Some("return".into())),
                DockHint::new("kill", None),
            ],
        );
        let hints = dock.hint_line().unwrap();
        assert_eq!(hints.text, "output return  kill   tabs ←/→");
        let spans: Vec<(String, DockStyle, bool, u16)> = hints
            .spans
            .iter()
            .map(|span| (span.text.clone(), span.style, span.bold, span.x))
            .collect();
        assert_eq!(
            spans,
            vec![
                ("output".into(), DockStyle::HintLabel, true, DOCK_TEXT_X),
                (" ".into(), DockStyle::HintLabel, false, DOCK_TEXT_X + 6),
                (
                    "return".into(),
                    DockStyle::HintShortcut,
                    false,
                    DOCK_TEXT_X + 7
                ),
                ("kill".into(), DockStyle::HintLabel, true, DOCK_TEXT_X + 15),
                (" ".into(), DockStyle::HintLabel, false, DOCK_TEXT_X + 19),
                // Disabled keybind: label rendered, empty shortcut span.
                ("".into(), DockStyle::HintShortcut, false, DOCK_TEXT_X + 20),
                ("tabs".into(), DockStyle::HintLabel, true, DOCK_TEXT_X + 22),
                (" ".into(), DockStyle::HintLabel, false, DOCK_TEXT_X + 26),
                (
                    "←/→".into(),
                    DockStyle::HintShortcut,
                    false,
                    DOCK_TEXT_X + 27
                ),
            ]
        );

        // One tab: a single bold label and no `tabs` hint.
        let mut single = ComposerDock::new();
        single.register(DockTabId::Subagents);
        single.open(None);
        let title = single.title_line(40).unwrap();
        assert_eq!(title.text, "Subagents");
        assert_eq!(title.spans[0].style, DockStyle::TabActive);
        assert!(single.footer_hints().is_empty());
        assert_eq!(single.hint_line().unwrap().text, "");
    }

    // CP-23 also pins the chrome geometry (`index.tsx:87-96`).
    #[test]
    fn cp23_geometry_constants() {
        let geometry = ComposerDock::new().geometry();
        assert!(geometry.border_left);
        assert_eq!(geometry.padding_top, 1);
        assert_eq!(geometry.padding_bottom, 1);
        assert_eq!(geometry.padding_left, 1);
        assert_eq!(geometry.padding_right, 2);
        assert_eq!(geometry.gap, 1);
        assert_eq!(geometry.title_padding_left, 1);
        assert_eq!(geometry.tab_gap, 2);
        assert_eq!(geometry.hint_gap, 2);
        assert_eq!(geometry.max_rows, 5);
        assert_eq!(DOCK_BORDER_LEFT, '┃');
        assert_eq!(DOCK_TEXT_X, 3);
    }

    // Mount / precedence gates: the composer claims the slot, the queue dock
    // only exists while it is closed.
    #[test]
    fn dock_visibility_and_queue_gate() {
        let mut dock = ComposerDock::new();
        dock.register(DockTabId::Subagents);
        assert!(!dock.visible());
        dock.set_route(true, 1);
        assert!(!dock.visible(), "an open form outranks the child dock");
        dock.set_route(true, 0);
        assert!(dock.visible(), "child sessions open the dock by default");
        assert!(dock.is_active(DockTabId::Subagents));
        dock.set_route(false, 0);
        assert!(!dock.visible());
        dock.open(None);
        assert!(dock.visible());
        assert!(dock.title_line(80).is_some());
        // A muted prompt force-closes the open composer.
        dock.apply_prompt_muted(true);
        assert!(!dock.is_open());
        assert_eq!(dock.close_requests(), 1);

        dock.set_queued(vec!["first\nsecond".into(), "other".into()]);
        assert!(dock.queue_visible());
        let line = dock.queue_line().unwrap();
        assert_eq!(line.text, "2 queued · first second");
        assert_eq!(line.spans[0].style, DockStyle::QueueCount);
        assert!(line.spans[0].bold);
        assert_eq!(line.spans[1].style, DockStyle::QueuePreview);
        assert!(!line.spans[1].bold);
        dock.set_disabled(true);
        assert!(!dock.queue_visible());
        assert_eq!(dock.queue_line(), None);
        dock.set_disabled(false);
        dock.open(None);
        assert!(!dock.queue_visible(), "open composer hides the queue dock");
        assert_eq!(dock.queue_line(), None);
        dock.close();
        assert!(dock.queue_visible());
        dock.set_queued(Vec::new());
        assert!(!dock.queue_visible());
        // The queue gate reads the `composer.open` store flag only, not the
        // effective child-open prop (`index.tsx:1591`), so a child session
        // whose composer opened implicitly still shows the queue dock.
        dock.set_route(true, 0);
        dock.set_queued(vec!["queued".into()]);
        assert!(dock.visible());
        assert!(dock.queue_visible());
        assert_eq!(
            dock.queue_line().map(|line| line.text),
            Some("1 queued · queued".into())
        );
    }

    // CP-27: foreground process, then title, then the New-terminal row.
    #[test]
    fn cp27_terminal_rows_use_process_then_title_then_new_row() {
        let mut tab = TerminalsTab::new();
        tab.set_entries(vec![
            TerminalEntry::new("t1", Some("vim".into()), Some("editor".into())),
            TerminalEntry::new("t2", None, Some("server".into())),
            TerminalEntry::new("t3", None, None),
        ]);
        tab.activate(None);
        let rows = tab.rows();
        assert_eq!(rows.len(), 4);
        let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["vim", "server", NEW_TERMINAL_LABEL, NEW_TERMINAL_LABEL]
        );
        assert_eq!(rows[3].style, DockStyle::TerminalBase);
        assert!(!rows[3].current);
        // Empty strings are real values under `??`.
        let empty = TerminalEntry::new("t4", Some(String::new()), None);
        assert_eq!(empty.label(), "");
    }

    // CP-27: visible (current) versus focused (active) row styling.
    #[test]
    fn cp27_visible_versus_focused_row() {
        let mut tab = TerminalsTab::new();
        tab.set_entries(vec![
            TerminalEntry::new("t1", Some("vim".into()), None),
            TerminalEntry::new("t2", Some("build".into()), None),
        ]);
        tab.activate(Some("t1"));
        assert_eq!(tab.selected(), Some(0));
        let rows = tab.rows();
        assert!(rows[0].active && rows[0].current);
        assert_eq!(rows[0].style, DockStyle::TerminalActive);
        assert!(!rows[1].active && !rows[1].current);
        assert_eq!(rows[1].style, DockStyle::TerminalBase);
        // Focus moves off the visible row: current keeps the selected style.
        tab.hover(Some(1));
        let rows = tab.rows();
        assert_eq!(rows[0].style, DockStyle::TerminalCurrent);
        assert!(!rows[0].active);
        assert!(rows[0].current);
        assert_eq!(rows[1].style, DockStyle::TerminalActive);
        assert!(rows[1].active);
        // A visible terminal that is not in the list selects nothing.
        tab.activate(Some("missing"));
        assert_eq!(tab.selected(), None);
        assert_eq!(tab.rows()[0].style, DockStyle::TerminalBase);
    }

    // CP-27: up/down wrap across entries plus the New-terminal row.
    #[test]
    fn cp27_wrap_includes_new_terminal_row() {
        let mut tab = TerminalsTab::new();
        tab.set_entries(vec![
            TerminalEntry::new("t1", Some("a".into()), None),
            TerminalEntry::new("t2", Some("b".into()), None),
        ]);
        tab.activate(None);
        assert_eq!(tab.selected(), None);
        tab.down();
        assert_eq!(tab.selected(), Some(0));
        tab.down();
        assert_eq!(tab.selected(), Some(1));
        tab.down();
        assert_eq!(tab.selected(), Some(2), "New-terminal row");
        assert!(tab.rows()[2].active);
        tab.down();
        assert_eq!(tab.selected(), Some(0));
        tab.up();
        assert_eq!(tab.selected(), Some(2));
        tab.up();
        assert_eq!(tab.selected(), Some(1));
        // No terminals: up/down stay on the only (New) row.
        let mut empty = TerminalsTab::new();
        empty.activate(None);
        empty.up();
        assert_eq!(empty.selected(), Some(0));
        empty.down();
        assert_eq!(empty.selected(), Some(0));
        assert_eq!(empty.rows().len(), 1);
    }

    // CP-27: select versus create, close-first ordering, failure toast.
    #[test]
    fn cp27_select_and_create_close_and_failure_toast() {
        let mut tab = TerminalsTab::new();
        tab.set_entries(vec![TerminalEntry::new("t1", Some("vim".into()), None)]);
        tab.activate(None);
        assert_eq!(
            tab.select(),
            TerminalSelection {
                close_composer: false,
                effect: TerminalEffect::Noop,
            }
        );
        tab.activate(Some("t1"));
        assert_eq!(
            tab.select(),
            TerminalSelection {
                close_composer: true,
                effect: TerminalEffect::Select("t1".into()),
            }
        );
        tab.hover(Some(1));
        assert_eq!(
            tab.select(),
            TerminalSelection {
                close_composer: true,
                effect: TerminalEffect::Create,
            }
        );
        // A stale index past the list also creates (JS `entries()[index]`).
        tab.hover(Some(7));
        assert_eq!(tab.select().effect, TerminalEffect::Create);
        assert_eq!(tab.toast(), None);
        tab.fail();
        assert_eq!(tab.toast(), Some(TERMINAL_FAILURE_TOAST));
        assert_eq!(tab.take_toast().as_deref(), Some(TERMINAL_FAILURE_TOAST));
        assert_eq!(tab.toast(), None);
    }

    // CP-27: a narrow row clips by display cells instead of overflowing.
    #[test]
    fn cp27_row_text_is_clipped_to_the_row_budget() {
        let row = TerminalRow {
            label: "a-very-long-foreground-process-name".into(),
            active: false,
            current: false,
            style: DockStyle::TerminalBase,
        };
        let text = TerminalsTab::row_text(&row, 12);
        assert_eq!(text.chars().count(), 12 - usize::from(DOCK_TEXT_X) - 1);
        assert!(text.starts_with("a-very"));
    }
}
