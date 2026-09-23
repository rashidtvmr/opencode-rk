#![forbid(unsafe_code)]
//! Dialog model (mirrors `packages/tui/src/ui/dialog*.tsx`).
//!
//! Variants: alert (`DialogAlertProps{title,message}` dialog-alert.tsx:6),
//! confirm (`DialogConfirmProps{title,message}` dialog-confirm.tsx:9, result
//! `boolean|undefined`), select (`DialogSelectProps{title,options}`
//! dialog-select.tsx:23), prompt (`DialogPromptProps{title,value?}`
//! dialog-prompt.tsx:9), help (no props, fixed "Help" dialog-help.tsx:6),
//! export (filename/flags dialog-export-options.tsx:8). All share the
//! title/message/actions pattern seen in component/dialog-*.tsx.

/// Max dialog message chars.
pub const MAX_MESSAGE: usize = 4096;
/// Max dialog title chars.
pub const MAX_DIALOG_TITLE: usize = 256;
/// Max select options (bounded 64).
pub const MAX_OPTIONS: usize = 64;
/// Max prompt initial value chars.
pub const MAX_INITIAL: usize = 4096;

/// Dialog variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogKind {
    Alert,
    Confirm,
    Select { options: Vec<String> },
    Prompt { initial: String },
    Help,
    Export,
}

/// Validated dialog (`title`/`message` + kind).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialog {
    pub title: String,
    pub message: String,
    pub kind: DialogKind,
}

impl Dialog {
    pub fn new(title: &str, message: &str, kind: DialogKind) -> Result<Self, &'static str> {
        if title.is_empty() || title.chars().count() > MAX_DIALOG_TITLE {
            return Err("bad title");
        }
        if message.chars().count() > MAX_MESSAGE {
            return Err("message too long");
        }
        match &kind {
            DialogKind::Select { options } => {
                if options.is_empty() || options.len() > MAX_OPTIONS {
                    return Err("bad options");
                }
                if options.iter().any(|o| o.is_empty()) {
                    return Err("empty option");
                }
            }
            DialogKind::Prompt { initial } => {
                if initial.chars().count() > MAX_INITIAL {
                    return Err("initial too long");
                }
            }
            _ => {}
        }
        Ok(Self { title: title.to_string(), message: message.to_string(), kind })
    }

    /// Validate an existing instance (post-construction check).
    #[must_use]
    pub fn validate(&self) -> bool {
        Self::new(&self.title.clone(), &self.message.clone(), self.kind.clone()).is_ok()
    }
}

/// Dialog outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogResult {
    Confirmed(bool),
    Selected(Option<usize>),
    Input(String),
    Dismissed,
}

/// Key binding ids (mirror `useBindings` `key` strings).
pub const KEY_RETURN: &str = "return";
pub const KEY_ESCAPE: &str = "escape";
pub const KEY_LEFT: &str = "left";
pub const KEY_RIGHT: &str = "right";

/// Alert extras (dialog-alert.tsx:6-57).
///
/// Divergence: TS `onConfirm?: () => void` closure modeled as
/// `confirm_action` action-id string; host maps id to callback.
/// `esc` label (:36) is label-only text with mouse dismiss
/// (`onMouseUp={() => dialog.clear()}`), no keyboard binding;
/// only `return` (:19) confirms. Button label `ok` (:52).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertDialog {
    pub confirm_action: Option<String>,
    pub ok_label: String,
}

impl AlertDialog {
    pub const OK_LABEL: &str = "ok";
    /// Keyboard binding that confirms (:19).
    pub const KEY_CONFIRM: &str = KEY_RETURN;

    #[must_use]
    pub fn new(confirm_action: Option<&str>) -> Self {
        Self {
            confirm_action: confirm_action.map(str::to_string),
            ok_label: Self::OK_LABEL.to_string(),
        }
    }
}

/// Focused confirm button (dialog-confirm.tsx:23, default `"confirm"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfirmButton {
    #[default]
    Confirm,
    Cancel,
}

/// Confirm extras (dialog-confirm.tsx:9-108).
///
/// Divergence: TS `onConfirm`/`onCancel` closures (:12-13) modeled as
/// action-id strings. `label` (:14) overrides the cancel button text only
/// (`props.label ?? key`, :83); `result` mirrors `DialogConfirmResult`
/// (`boolean|undefined`, :17): `None` = dismissed via esc/mouse with no
/// callback, `Some(true/false)` = confirm/cancel. Buttons render
/// `["cancel","confirm"]` (:70), `active` toggled by `left`/`right`
/// (:39-53), `return` (:29) fires the active action.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfirmDialog {
    pub confirm_action: Option<String>,
    pub cancel_action: Option<String>,
    pub label: Option<String>,
    pub result: Option<bool>,
    pub active: ConfirmButton,
}

impl ConfirmDialog {
    pub const BUTTONS: [&'static str; 2] = ["cancel", "confirm"];
    pub const KEY_CONFIRM: &str = KEY_RETURN;
    pub const KEY_PREV: &str = KEY_LEFT;
    pub const KEY_NEXT: &str = KEY_RIGHT;

    #[must_use]
    pub fn new(
        confirm_action: Option<&str>,
        cancel_action: Option<&str>,
        label: Option<&str>,
    ) -> Self {
        Self {
            confirm_action: confirm_action.map(str::to_string),
            cancel_action: cancel_action.map(str::to_string),
            label: label.map(str::to_string),
            result: None,
            active: ConfirmButton::Confirm,
        }
    }

    /// Toggle active button (`left`/`right` handlers, :39-53).
    pub fn toggle(&mut self) {
        self.active = match self.active {
            ConfirmButton::Confirm => ConfirmButton::Cancel,
            ConfirmButton::Cancel => ConfirmButton::Confirm,
        };
    }

    /// Cancel-button text (`Locale.titlecase(props.label ?? "cancel")`, :83).
    #[must_use]
    pub fn cancel_text(&self) -> String {
        self.label.clone().unwrap_or_else(|| "cancel".to_string())
    }
}

/// Help dialog (dialog-help.tsx:6-40, no props).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HelpDialog;

impl HelpDialog {
    pub const TITLE: &str = "Help";
    pub const OK_LABEL: &str = "ok";
    /// Close bindings: `return` (:13) and `escape` (:14).
    pub const KEY_CLOSE_1: &str = KEY_RETURN;
    pub const KEY_CLOSE_2: &str = KEY_ESCAPE;

    /// Fixed message with dynamic `commandShortcut()` (:9, :29-31).
    #[must_use]
    pub fn text(command_shortcut: &str) -> String {
        format!(
            "Press {command_shortcut} to see all available actions and commands in any context."
        )
    }
}

/// Free-function alias for `HelpDialog::text` (dynamic shortcut message).
#[must_use]
pub fn help_text(command_shortcut: &str) -> String {
    HelpDialog::text(command_shortcut)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alert() -> Dialog {
        Dialog::new("T", "M", DialogKind::Alert).unwrap()
    }

    #[test]
    fn select_empty_errs() {
        assert!(Dialog::new("T", "M", DialogKind::Select { options: vec![] }).is_err());
        assert!(Dialog::new("T", "M", DialogKind::Select { options: vec!["".into()] }).is_err());
        assert!(Dialog::new("T", "M", DialogKind::Select { options: (0..MAX_OPTIONS + 1).map(|i| format!("o{i}")).collect() }).is_err());
        assert!(Dialog::new("T", "M", DialogKind::Select { options: vec!["a".into(), "b".into()] }).is_ok());
    }

    #[test]
    fn prompt_carries_initial() {
        let d = Dialog::new("T", "M", DialogKind::Prompt { initial: "hello".into() }).unwrap();
        match d.kind {
            DialogKind::Prompt { initial } => assert_eq!(initial, "hello"),
            _ => panic!("wrong kind"),
        }
    }

    #[test]
    fn confirm_result() {
        assert_eq!(DialogResult::Confirmed(true), DialogResult::Confirmed(true));
        assert_ne!(DialogResult::Confirmed(true), DialogResult::Confirmed(false));
        assert_ne!(DialogResult::Confirmed(true), DialogResult::Dismissed);
        assert_eq!(DialogResult::Selected(Some(1)), DialogResult::Selected(Some(1)));
        assert_eq!(DialogResult::Input("x".into()), DialogResult::Input("x".into()));
        assert!(alert().validate());
    }

    #[test]
    fn oversize_errs() {
        assert!(Dialog::new(&"t".repeat(MAX_DIALOG_TITLE + 1), "M", DialogKind::Alert).is_err());
        assert!(Dialog::new("", "M", DialogKind::Alert).is_err());
        assert!(Dialog::new("T", &"m".repeat(MAX_MESSAGE + 1), DialogKind::Alert).is_err());
        assert!(Dialog::new("T", "M", DialogKind::Prompt { initial: "x".repeat(MAX_INITIAL + 1) }).is_err());
    }

    #[test]
    fn alert_ok_label_and_confirm_key() {
        let a = AlertDialog::new(Some("alert.confirm"));
        assert_eq!(a.ok_label, "ok");
        assert_eq!(a.confirm_action.as_deref(), Some("alert.confirm"));
        assert_eq!(AlertDialog::KEY_CONFIRM, KEY_RETURN);
        assert_ne!(KEY_ESCAPE, AlertDialog::KEY_CONFIRM);
        // esc is label-only dismiss: mouse clear, no keyboard binding.
    }

    #[test]
    fn alert_no_action_ok() {
        let a = AlertDialog::new(None);
        assert!(a.confirm_action.is_none());
        assert_eq!(a.ok_label, AlertDialog::OK_LABEL);
    }

    #[test]
    fn confirm_actions_and_default_active() {
        let c = ConfirmDialog::new(Some("dlg.confirm"), Some("dlg.cancel"), None);
        assert_eq!(c.active, ConfirmButton::Confirm);
        assert_eq!(ConfirmDialog::BUTTONS, ["cancel", "confirm"]);
        assert_eq!(ConfirmDialog::KEY_CONFIRM, KEY_RETURN);
        assert_eq!(ConfirmDialog::KEY_PREV, KEY_LEFT);
        assert_eq!(ConfirmDialog::KEY_NEXT, KEY_RIGHT);
        assert_eq!(c.result, None);
    }

    #[test]
    fn confirm_toggle_and_cancel_text() {
        let mut c = ConfirmDialog::new(None, None, None);
        c.toggle();
        assert_eq!(c.active, ConfirmButton::Cancel);
        c.toggle();
        assert_eq!(c.active, ConfirmButton::Confirm);
        assert_eq!(c.cancel_text(), "cancel");
        let custom = ConfirmDialog::new(None, None, Some("Skip it"));
        assert_eq!(custom.cancel_text(), "Skip it");
        // label overrides cancel button only; confirm stays "confirm".
    }

    #[test]
    fn confirm_result_shape() {
        // DialogConfirmResult boolean|undefined: None = dismissed.
        let mut c = ConfirmDialog::new(None, None, None);
        c.result = Some(true);
        assert_eq!(c.result, Some(true));
        c.result = Some(false);
        assert_eq!(c.result, Some(false));
        c.result = None;
        assert_eq!(c.result, None);
        assert_eq!(ConfirmDialog::default().active, ConfirmButton::Confirm);
    }

    #[test]
    fn help_fixed_title_ok_and_close_keys() {
        assert_eq!(HelpDialog::TITLE, "Help");
        assert_eq!(HelpDialog::OK_LABEL, "ok");
        assert_eq!(HelpDialog::KEY_CLOSE_1, KEY_RETURN);
        assert_eq!(HelpDialog::KEY_CLOSE_2, KEY_ESCAPE);
        assert_eq!(
            HelpDialog::text("ctrl+k"),
            "Press ctrl+k to see all available actions and commands in any context."
        );
        assert_eq!(help_text("ctrl+k"), HelpDialog::text("ctrl+k"));
    }
}
