//! Native first-run configuration-import dialog: shown-once offer state,
//! redacted preview, explicit confirmation, and rendered-row hit testing.
//!
//! The caller supplies an already-normalized patch (`Value`) from the
//! foundation `config_import` adapter, an explicit own-config destination,
//! source provenance, and the offer-state path. There is deliberately **no**
//! default or hardcoded destination here: a destination is only usable when
//! the native runtime actually reads that file. Until such a reader exists,
//! wiring must stay in preview/consent mode rather than write an unread file.
//!
//! Wiring contract (owned by the integration writer, not this module):
//! - startup (after onboarding, before the first frame): `startup(...)`;
//! - render only when `is_open()`, using `render(width, height)`;
//! - pointer/keyboard activation: `action_at(...)` or `activate_selected(...)`;
//! - Settings "Import...": `from_settings(...)`;
//! - broker adapter implements `ConfigWritePolicy`; `action(Apply, ..)` is the
//!   user's explicit confirmation and still requires that policy grant;
//! - `Esc`/dismiss maps to `NotNow` on the offer and `Cancel` on the preview.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::native_config_import::{
    apply_confirmed, mark_offer_decision, preview, read_existing_config, reopen_offer,
    show_offer, ConfigWritePolicy, ImportError, OfferDecision, Preview, SourceMeta,
};

/// Maximum redacted-preview/conflict rows kept in the scroll-free detail head.
const MAX_DETAIL_LINES: usize = 32;
/// Maximum source labels shown before an elision line.
const MAX_SOURCE_LINES: usize = 8;
/// Maximum conflict pointers shown before an elision line.
const MAX_CONFLICT_LINES: usize = 8;
/// Maximum characters of an error message rendered.
const MAX_ERROR_CHARS: usize = 240;

/// One clickable/keyboard-activatable import affordance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportAction {
    Import,
    NotNow,
    Apply,
    Cancel,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Hidden,
    Offer,
    Preview,
    Applied,
    Error,
}

/// UI state for the configuration-import offer and preview. Holds the real
/// merged preview privately; only [`Self::redacted_preview`] is display-safe.
pub struct ImportDialog {
    phase: Phase,
    state_path: PathBuf,
    dest: PathBuf,
    sources: Vec<SourceMeta>,
    patch: Value,
    preview: Option<Preview>,
    conflicts: Vec<String>,
    error: Option<String>,
    written: Option<u64>,
    selected: usize,
    /// Bounded non-secret provenance/unsupported diagnostics supplied by the
    /// discovery layer for the visible preview.
    details: Vec<String>,
}

impl ImportDialog {
    fn new(
        state_path: &Path,
        dest: &Path,
        sources: Vec<SourceMeta>,
        patch: Value,
    ) -> Self {
        Self {
            phase: Phase::Hidden,
            state_path: state_path.to_path_buf(),
            dest: dest.to_path_buf(),
            sources,
            patch,
            preview: None,
            conflicts: Vec::new(),
            error: None,
            written: None,
            selected: 0,
            details: Vec::new(),
        }
    }

    /// Attach display-safe discovery diagnostics after construction. This is
    /// intentionally additive: it never changes the patch or write policy.
    pub fn set_details(&mut self, details: Vec<String>) {
        self.details = details.into_iter().take(MAX_DETAIL_LINES).collect();
    }

    /// Startup entry point. Shows the offer only when the persisted marker
    /// does not already record a decision; never writes the marker here.
    pub fn startup(
        state_path: &Path,
        dest: &Path,
        sources: Vec<SourceMeta>,
        patch: Value,
    ) -> Result<Self, ImportError> {
        let mut dialog = Self::new(state_path, dest, sources, patch);
        if show_offer(state_path)? {
            dialog.phase = Phase::Offer;
        }
        Ok(dialog)
    }

    /// Settings "Import..." entry point. Clears the shown-once marker and
    /// immediately offers again; the next decision persists a fresh marker.
    pub fn from_settings(
        state_path: &Path,
        dest: &Path,
        sources: Vec<SourceMeta>,
        patch: Value,
    ) -> Result<Self, ImportError> {
        reopen_offer(state_path)?;
        let mut dialog = Self::new(state_path, dest, sources, patch);
        dialog.phase = Phase::Offer;
        Ok(dialog)
    }

    /// Settings-triggered reopen with already discovered normalized data.
    pub fn from_settings_parts(
        state_path: &Path,
        dest: &Path,
        sources: Vec<SourceMeta>,
        patch: Value,
    ) -> Result<Self, ImportError> {
        Self::from_settings(state_path, dest, sources, patch)
    }

    /// Whether this dialog currently owns its overlay/input.
    pub fn is_open(&self) -> bool {
        self.phase != Phase::Hidden
    }

    /// Display-safe merged preview. Never returns the raw merged document.
    pub fn redacted_preview(&self) -> Option<&Value> {
        self.preview.as_ref().map(|preview| &preview.redacted)
    }

    /// Conflict pointers from the latest preview or applied result.
    pub fn conflicts(&self) -> &[String] {
        &self.conflicts
    }

    /// Display-safe failure text, if the dialog is in its error phase.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Selection index for keyboard dispatch (0-based within current choices).
    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn move_selection(&mut self, forward: bool) {
        let count = self.choice_count();
        if count == 0 {
            self.selected = 0;
            return;
        }
        self.selected = if forward {
            (self.selected + 1) % count
        } else {
            (self.selected + count - 1) % count
        };
    }

    /// Activate the currently selected choice, if any.
    pub fn activate_selected(&mut self, policy: &dyn ConfigWritePolicy) {
        if let Some(action) = self.selected_action() {
            self.action(action, policy);
        }
    }

    pub fn selected_action(&self) -> Option<ImportAction> {
        match self.phase {
            Phase::Offer => Some([ImportAction::Import, ImportAction::NotNow][self.selected]),
            Phase::Preview => Some([ImportAction::Apply, ImportAction::Cancel][self.selected]),
            Phase::Applied | Phase::Error => Some(ImportAction::Close),
            Phase::Hidden => None,
        }
    }

    /// Action used by Esc for the current phase; unlike selected_action this
    /// is independent of the highlighted row.
    pub fn dismiss_action(&self) -> Option<ImportAction> {
        match self.phase {
            Phase::Offer => Some(ImportAction::NotNow),
            Phase::Preview => Some(ImportAction::Cancel),
            Phase::Applied | Phase::Error => Some(ImportAction::Close),
            Phase::Hidden => None,
        }
    }

    /// Apply one affordance. Actions that are invalid for the current phase
    /// are ignored; recoverable failures become the visible error phase.
    pub fn action(&mut self, action: ImportAction, policy: &dyn ConfigWritePolicy) {
        match (self.phase, action) {
            (Phase::Offer, ImportAction::Import) => self.begin_preview(),
            (Phase::Offer, ImportAction::NotNow) => self.decline(),
            (Phase::Preview, ImportAction::Apply) => self.apply(policy),
            (Phase::Preview, ImportAction::Cancel) => self.close(),
            (Phase::Applied | Phase::Error, ImportAction::Close) => self.close(),
            _ => {}
        }
    }

    /// Rendered block, padded to `height`. The layout worker performs the
    /// final terminal-cell width pass; this clips by scalar for bounded output.
    pub fn render(&self, width: usize, height: usize) -> Vec<String> {
        self.layout(width, height).0
    }

    /// Hit test a rendered content row/column. `row` is relative to this
    /// dialog's rendered block (the caller maps pointer geometry). A choice
    /// row activates only when its full (sanitized) label fits the given
    /// width: a cropped label is visible but deliberately inert.
    pub fn action_at(
        &self,
        width: usize,
        height: usize,
        row: usize,
        column: usize,
    ) -> Option<ImportAction> {
        let (lines, hits) = self.layout(width, height);
        let line = lines.get(row)?;
        if column >= line.chars().count() {
            return None;
        }
        hits.get(row).copied().flatten()
    }

    fn begin_preview(&mut self) {
        if let Err(error) = mark_offer_decision(&self.state_path, OfferDecision::Accepted) {
            self.fail(error);
            return;
        }
        let result = read_existing_config(&self.dest)
            .and_then(|existing| preview(existing.as_ref(), &self.patch));
        match result {
            Ok(preview) => {
                self.conflicts = preview.conflicts.clone();
                self.preview = Some(preview);
                self.phase = Phase::Preview;
                self.selected = 0;
            }
            Err(error) => self.fail(error),
        }
    }

    fn decline(&mut self) {
        match mark_offer_decision(&self.state_path, OfferDecision::Declined) {
            Ok(()) => self.close(),
            Err(error) => self.fail(error),
        }
    }

    fn apply(&mut self, policy: &dyn ConfigWritePolicy) {
        match apply_confirmed(&self.dest, &self.patch, true, &self.sources, policy) {
            Ok(report) => {
                self.conflicts = report.conflicts;
                self.written = Some(report.bytes_written);
                self.phase = Phase::Applied;
                self.selected = 0;
            }
            Err(error) => self.fail(error),
        }
    }

    fn fail(&mut self, error: ImportError) {
        self.error = Some(clip(&error.to_string(), MAX_ERROR_CHARS));
        self.preview = None;
        self.phase = Phase::Error;
        self.selected = 0;
    }

    fn close(&mut self) {
        self.phase = Phase::Hidden;
        self.preview = None;
        self.error = None;
        self.written = None;
        self.selected = 0;
    }

    fn choice_count(&self) -> usize {
        match self.phase {
            Phase::Offer | Phase::Preview => 2,
            Phase::Applied | Phase::Error => 1,
            Phase::Hidden => 0,
        }
    }

    fn layout(
        &self,
        width: usize,
        height: usize,
    ) -> (Vec<String>, Vec<Option<ImportAction>>) {
        if height == 0 || self.phase == Phase::Hidden {
            return (Vec::new(), Vec::new());
        }
        let (head, choices, hint): (Vec<String>, &[(ImportAction, &str)], &str) =
            match self.phase {
                Phase::Hidden => (Vec::new(), &[], ""),
                Phase::Offer => {
                    let mut head = vec![
                        "Import settings from OpenCode?".to_owned(),
                        format!("Destination: {}", self.dest.display()),
                        format!("Sources ({}):", self.sources.len()),
                    ];
                    for source in self.sources.iter().take(MAX_SOURCE_LINES) {
                        head.push(format!("  - {}", source.label));
                    }
                    if self.sources.len() > MAX_SOURCE_LINES {
                        head.push(format!(
                            "  … {} more",
                            self.sources.len() - MAX_SOURCE_LINES
                        ));
                    }
                    head.push(String::new());
                    head.push("Existing values win on conflicts. Nothing is executed.".to_owned());
                    head.extend(self.details.iter().map(|line| format!("  {line}")));
                    (
                        head,
                        &[
                            (ImportAction::Import, "Import (preview first)"),
                            (ImportAction::NotNow, "Not now"),
                        ][..],
                        "Enter/click selects · Esc dismisses",
                    )
                }
                Phase::Preview => {
                    let mut head = vec![
                        "Import preview — nothing written yet".to_owned(),
                        format!("Destination: {}", self.dest.display()),
                        format!("Conflicts ({}):", self.conflicts.len()),
                    ];
                    for conflict in self.conflicts.iter().take(MAX_CONFLICT_LINES) {
                        head.push(format!("  ! {conflict}"));
                    }
                    if self.conflicts.len() > MAX_CONFLICT_LINES {
                        head.push(format!(
                            "  ! … {} more",
                            self.conflicts.len() - MAX_CONFLICT_LINES
                        ));
                    }
                    if let Some(preview) = &self.preview {
                        head.push("Redacted result:".to_owned());
                        if let Ok(json) = serde_json::to_string_pretty(&preview.redacted) {
                            for line in json.lines().take(MAX_DETAIL_LINES) {
                                head.push(format!("  {line}"));
                            }
                        }
                    }
                    head.extend(self.details.iter().map(|line| format!("  {line}")));
                    (
                        head,
                        &[
                            (ImportAction::Apply, "Apply import"),
                            (ImportAction::Cancel, "Cancel"),
                        ][..],
                        "Enter/click applies only after this explicit confirmation",
                    )
                }
                Phase::Applied => {
                    let mut head = vec!["Import applied".to_owned()];
                    if let Some(written) = self.written {
                        head.push(format!("Wrote {written} bytes to {}", self.dest.display()));
                    }
                    head.push(format!("Conflicts kept: {}", self.conflicts.len()));
                    for conflict in self.conflicts.iter().take(MAX_CONFLICT_LINES) {
                        head.push(format!("  ! {conflict}"));
                    }
                    (
                        head,
                        &[(ImportAction::Close, "Close")][..],
                        "The source files were not modified",
                    )
                }
                Phase::Error => (
                    vec![
                        "Import failed".to_owned(),
                        self.error.clone().unwrap_or_else(|| "unknown error".to_owned()),
                    ],
                    &[(ImportAction::Close, "Close")][..],
                    "Nothing was written by a failed preview or apply",
                ),
            };

        let reserve = choices.len() + usize::from(!hint.is_empty() && height > choices.len());
        let head_budget = height.saturating_sub(reserve);
        let mut lines = Vec::with_capacity(height);
        let mut hits = Vec::with_capacity(height);
        for line in head.into_iter().take(head_budget) {
            lines.push(clip(&line, width));
            hits.push(None);
        }
        for (index, (action, label)) in choices.iter().enumerate() {
            if lines.len() >= height {
                break;
            }
            let marker = if index == self.selected { ">" } else { " " };
            let raw = format!("{marker} {}. {label}", index + 1);
            let fits = raw.chars().count() <= width;
            lines.push(clip(&raw, width));
            hits.push(if fits { Some(*action) } else { None });
        }
        if !hint.is_empty() && lines.len() < height {
            lines.push(clip(hint, width));
            hits.push(None);
        }
        while lines.len() < height {
            lines.push(String::new());
            hits.push(None);
        }
        (lines, hits)
    }
}

fn clip(text: &str, width: usize) -> String {
    text.chars()
        .map(|character| if character.is_control() { ' ' } else { character })
        .take(width)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_dialog_has_no_hits() {
        let dialog = ImportDialog::new(
            Path::new("/tmp/unused-state.json"),
            Path::new("/tmp/unused-config.json"),
            Vec::new(),
            serde_json::json!({}),
        );
        assert!(!dialog.is_open());
        assert!(dialog.render(80, 20).is_empty());
        assert_eq!(dialog.action_at(80, 20, 0, 0), None);
    }

    #[test]
    fn choices_are_the_only_hit_targets() {
        let mut dialog = ImportDialog::new(
            Path::new("/tmp/unused-state.json"),
            Path::new("/tmp/unused-config.json"),
            Vec::new(),
            serde_json::json!({}),
        );
        dialog.phase = Phase::Offer;
        let lines = dialog.render(80, 20);
        let hits: Vec<_> = (0..lines.len())
            .filter_map(|row| dialog.action_at(80, 20, row, 0))
            .collect();
        assert_eq!(hits, vec![ImportAction::Import, ImportAction::NotNow]);
        assert_eq!(dialog.action_at(80, 20, 0, 0), None);
        assert_eq!(
            dialog.action_at(80, 20, 0, 10_000),
            None,
            "past the clipped text is inert"
        );
    }

    #[test]
    fn dismiss_action_is_phase_aware_and_not_selection_dependent() {
        let state = Path::new("/tmp/import-dismiss-state.json");
        let dest = Path::new("/tmp/import-dismiss-config.json");
        let mut dialog = ImportDialog::new(state, dest, Vec::new(), serde_json::json!({}));
        assert_eq!(dialog.dismiss_action(), None);
        dialog.phase = Phase::Offer;
        dialog.selected = 1;
        assert_eq!(dialog.dismiss_action(), Some(ImportAction::NotNow));
        dialog.phase = Phase::Preview;
        dialog.selected = 0;
        assert_eq!(dialog.dismiss_action(), Some(ImportAction::Cancel));
        dialog.phase = Phase::Applied;
        assert_eq!(dialog.dismiss_action(), Some(ImportAction::Close));
        dialog.phase = Phase::Error;
        assert_eq!(dialog.dismiss_action(), Some(ImportAction::Close));
    }

    #[test]
    fn only_apply_action_can_receive_explicit_write_policy() {
        struct Deny;
        impl ConfigWritePolicy for Deny {
            fn authorize_config_import(&self, _: &Path, _: &[SourceMeta]) -> Result<(), ImportError> {
                Err(ImportError::PolicyDenied("denied".into()))
            }
        }
        let mut dialog = ImportDialog::new(Path::new("/tmp/s"), Path::new("/tmp/d"), Vec::new(), serde_json::json!({}));
        dialog.phase = Phase::Offer;
        dialog.action(ImportAction::NotNow, &Deny);
        assert!(!dialog.is_open());
    }
}
