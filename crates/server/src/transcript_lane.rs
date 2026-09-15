//! WEB-007: transcript geometry + per-message action-bar model.
//!
//! Server-side pure model of the transcript row contract (full-width rows,
//! role alignment, below-message action bars, Fork branch popover, capability
//! gating, chronological reading order, bounded previews). The browser DOM in
//! `web/` is owned elsewhere; this module exposes the geometry/capability
//! decisions the DOM must render, with no spawn, no network, no I/O.

/// Largest retained preview in bytes (plus a small `"..."` suffix marker).
pub const MAX_PREVIEW_BYTES: usize = 4 * 1024;
/// Largest action count exposed per message row.
pub const MAX_ACTIONS: usize = 8;

/// Transcript message role (persisted + streamed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Geometry decision for one transcript row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowGeometry {
    /// Row spans the full transcript width.
    pub full_width: bool,
    /// True: content aligned right (user). False: aligned left.
    pub align_right: bool,
    /// Actions render below the message, never floating over it.
    pub actions_below: bool,
}

/// Geometry for a persisted or streamed row. Chronological `index` is carried
/// for reading-order checks; it never changes the geometry decision.
#[must_use]
pub fn geometry_for(role: MessageRole, index: usize) -> RowGeometry {
    let _ = index;
    RowGeometry {
        full_width: true,
        align_right: matches!(role, MessageRole::User),
        actions_below: true,
    }
}

/// Stable per-row action identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionId {
    Fork,
    Retry,
    Edit,
    Copy,
    Regenerate,
}

/// One below-message action with capability gating + accessible metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowAction {
    pub id: ActionId,
    pub visible: bool,
    pub enabled: bool,
    /// Accessible name announced to assistive tech; never empty.
    pub accessible_name: &'static str,
    /// Set when disabled/omitted so the UI can explain why.
    pub explanation: Option<&'static str>,
    /// Always false: unavailable actions never simulate success.
    pub simulated_success: bool,
}

/// Role-appropriate below-message action bar. `retry_available` is the real
/// backend capability flag: when false, Retry is exposed disabled with a
/// reason instead of being faked.
#[must_use]
pub fn row_actions(role: MessageRole, retry_available: bool) -> Vec<RowAction> {
    let mut actions = Vec::new();
    match role {
        MessageRole::User => {
            actions.push(RowAction {
                id: ActionId::Fork,
                visible: true,
                enabled: true,
                accessible_name: "Fork message",
                explanation: None,
                simulated_success: false,
            });
            actions.push(RowAction {
                id: ActionId::Edit,
                visible: true,
                enabled: true,
                accessible_name: "Edit message",
                explanation: None,
                simulated_success: false,
            });
            actions.push(RowAction {
                id: ActionId::Copy,
                visible: true,
                enabled: true,
                accessible_name: "Copy message",
                explanation: None,
                simulated_success: false,
            });
            if retry_available {
                actions.push(RowAction {
                    id: ActionId::Retry,
                    visible: true,
                    enabled: true,
                    accessible_name: "Retry request",
                    explanation: None,
                    simulated_success: false,
                });
            } else {
                actions.push(RowAction {
                    id: ActionId::Retry,
                    visible: true,
                    enabled: false,
                    accessible_name: "Retry request (unavailable)",
                    explanation: Some("retry backend unavailable"),
                    simulated_success: false,
                });
            }
        }
        MessageRole::Assistant => {
            actions.push(RowAction {
                id: ActionId::Fork,
                visible: true,
                enabled: true,
                accessible_name: "Fork message",
                explanation: None,
                simulated_success: false,
            });
            actions.push(RowAction {
                id: ActionId::Copy,
                visible: true,
                enabled: true,
                accessible_name: "Copy message",
                explanation: None,
                simulated_success: false,
            });
            actions.push(RowAction {
                id: ActionId::Regenerate,
                visible: true,
                enabled: retry_available,
                accessible_name: if retry_available {
                    "Regenerate answer"
                } else {
                    "Regenerate answer (unavailable)"
                },
                explanation: if retry_available {
                    None
                } else {
                    Some("regenerate backend unavailable")
                },
                simulated_success: false,
            });
            // Canonical Retry entry kept for capability honesty on assistant rows.
            actions.push(RowAction {
                id: ActionId::Retry,
                visible: true,
                enabled: retry_available,
                accessible_name: if retry_available {
                    "Retry request"
                } else {
                    "Retry request (unavailable)"
                },
                explanation: if retry_available {
                    None
                } else {
                    Some("retry backend unavailable")
                },
                simulated_success: false,
            });
        }
        MessageRole::System | MessageRole::Tool => {}
    }
    debug_assert!(actions.len() <= MAX_ACTIONS);
    actions.truncate(MAX_ACTIONS);
    actions
}

/// Popover items owned by one action trigger. Fork always offers branch.
#[must_use]
pub fn popover_items(action: ActionId) -> &'static [&'static str] {
    match action {
        ActionId::Fork => &["Branch in new chat"],
        ActionId::Retry => &["Retry now"],
        ActionId::Edit => &["Edit and resend"],
        ActionId::Copy => &["Copy text"],
        ActionId::Regenerate => &["Regenerate answer"],
    }
}

/// Whether a persisted/streamed role is a valid visible branch boundary.
/// Only user/assistant rows expose Fork; system/tool rows do not.
#[must_use]
pub fn is_branch_boundary(role: MessageRole) -> bool {
    matches!(role, MessageRole::User | MessageRole::Assistant)
}

/// Normalize a persisted legacy role string to the render model.
/// Unknown roles return `None` so callers omit (never fake) the row.
#[must_use]
pub fn normalize_persisted_role(raw: &str) -> Option<MessageRole> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "user" => Some(MessageRole::User),
        "assistant" => Some(MessageRole::Assistant),
        "system" => Some(MessageRole::System),
        "tool" => Some(MessageRole::Tool),
        _ => None,
    }
}

/// DOM reading order for a chronological transcript slice: identity order.
/// Asserts the store never reorders rows for layout (assistant tests pin it).
#[must_use]
pub fn reading_order(roles: &[MessageRole]) -> Vec<usize> {
    let _ = roles;
    (0..roles.len()).collect()
}

/// Keyboard focus target for the Fork popover lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    ForkTrigger,
    PopoverFirstItem,
}

/// Minimal Fork popover state: open flag + focus placement.
/// Escape closes and returns focus to the Fork trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopoverState {
    pub open: bool,
    pub focus: FocusTarget,
}

impl PopoverState {
    #[must_use]
    pub fn closed() -> Self {
        Self {
            open: false,
            focus: FocusTarget::ForkTrigger,
        }
    }

    /// Keyboard/mouse activation of the Fork trigger.
    pub fn open_fork(&mut self) {
        self.open = true;
        self.focus = FocusTarget::PopoverFirstItem;
    }

    /// Escape: close and return focus to Fork.
    pub fn escape(&mut self) {
        self.open = false;
        self.focus = FocusTarget::ForkTrigger;
    }
}

/// Bounded preview of a large message for layout-budget checks.
/// Returns at most `MAX_PREVIEW_BYTES` bytes plus a `"..."` marker when cut.
/// Cuts on a `char` boundary so the preview is always valid UTF-8.
#[must_use]
pub fn trunc_preview(text: &str) -> String {
    if text.len() <= MAX_PREVIEW_BYTES {
        return text.to_string();
    }
    let mut end = MAX_PREVIEW_BYTES;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = String::with_capacity(end + 3);
    out.push_str(&text[..end]);
    out.push_str("...");
    out
}

/// Motion budget in ms: zero when the user prefers reduced motion.
#[must_use]
pub fn animation_ms(reduced_motion: bool) -> u64 {
    if reduced_motion {
        0
    } else {
        150
    }
}
