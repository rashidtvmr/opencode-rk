//! Bounded MCP selection and bulk-action planning.
//!
//! This module owns no MCP registry and performs no lifecycle work. The caller
//! supplies an in-memory entry snapshot and remains responsible for executing
//! the returned plan. Keeping planning separate makes confirmation and unknown
//! entry handling deterministic without starting a process or contacting a
//! server.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum number of MCP ids held by one selection.
pub const MAX_BULK_SELECTION: usize = 64;
/// Maximum number of characters in an MCP id.
pub const MAX_ID_LEN: usize = 128;
/// Safe code used when a selected id is absent from the caller's snapshot.
pub const UNKNOWN_CODE: &str = "unknown";

/// Caller-owned MCP registry projection used by the planner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpEntry {
    pub id: String,
    pub enabled: bool,
}

impl McpEntry {
    /// Build an entry projection. Validation is performed when it is used by a
    /// selection or plan, so this constructor cannot create side effects.
    #[must_use]
    pub fn new(id: impl Into<String>, enabled: bool) -> Self {
        Self {
            id: id.into(),
            enabled,
        }
    }
}

/// Bounded, idempotent MCP selection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// Selected MCP ids. Mutate this field through the methods below to keep
    /// the selection bounded and duplicate-free.
    pub ids: Vec<String>,
}

/// Bulk-action failures. Errors do not mutate the selection or caller state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum BulkError {
    #[error("MCP selection is empty")]
    EmptySelection,
    #[error("MCP selection exceeds the maximum of {MAX_BULK_SELECTION}")]
    TooManySelected,
    #[error("explicit confirmation is required for this MCP action")]
    ConsentRequired,
    #[error("MCP id is empty")]
    EmptyId,
    #[error("MCP id exceeds the maximum length")]
    IdTooLong,
}

/// Supported bulk operations. Execution remains with the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BulkAction {
    Enable,
    Disable,
    Remove,
    Refresh,
    Install,
}

impl BulkAction {
    /// Whether this action can install or remove a server and therefore needs
    /// an explicit caller confirmation.
    #[must_use]
    pub const fn requires_confirmation(self) -> bool {
        matches!(self, Self::Remove | Self::Install)
    }
}

/// Per-entry result of a planned action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Applied,
    Skipped,
    Failed { code: String },
}

/// Compatibility aliases for callers that name the result kind explicitly.
pub type ItemResult = Outcome;
pub type ActionOutcome = Outcome;

/// Per-entry bulk result. It contains only the selected id and a safe outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemOutcome {
    pub id: String,
    pub outcome: Outcome,
}

/// Deterministic bounded bulk-action report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkReport {
    pub per_item: Vec<ItemOutcome>,
    pub succeeded: u32,
    pub failed: u32,
}

impl Selection {
    /// Create an empty selection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of selected ids.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether no ids are selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Borrow selected ids in selection order.
    #[must_use]
    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    /// Return whether `id` is selected.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.ids.iter().any(|selected| selected == id)
    }

    /// Select one id. Duplicate selection is idempotent.
    pub fn select(&mut self, id: impl AsRef<str>) -> Result<(), BulkError> {
        let id = id.as_ref();
        validate_id(id)?;
        if self.contains(id) {
            return Ok(());
        }
        if self.ids.len() >= MAX_BULK_SELECTION {
            return Err(BulkError::TooManySelected);
        }
        self.ids.push(id.to_string());
        Ok(())
    }

    /// Deselect one id. Missing ids are harmless and return `false`.
    pub fn deselect(&mut self, id: &str) -> bool {
        match self.ids.iter().position(|selected| selected == id) {
            Some(index) => {
                self.ids.remove(index);
                true
            }
            None => false,
        }
    }

    /// Select all ids from an iterator atomically.
    ///
    /// Duplicate ids do not consume capacity. Any invalid id or overflow
    /// leaves the original selection unchanged.
    pub fn select_all<I, S>(&mut self, ids: I) -> Result<(), BulkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut candidate = self.clone();
        for id in ids {
            candidate.select(id)?;
        }
        self.ids = candidate.ids;
        Ok(())
    }

    /// Clear every selected id.
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// Replace the selection with the complement over `known_ids`.
    ///
    /// Unknown ids currently selected are not carried into the complement.
    /// Duplicate known ids are ignored. Failure leaves the selection intact.
    pub fn invert<I, S>(&mut self, known_ids: I) -> Result<(), BulkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut inverted = Vec::new();
        for id in known_ids {
            let id = id.as_ref();
            validate_id(id)?;
            if self.contains(id) || inverted.iter().any(|selected: &String| selected == id) {
                continue;
            }
            if inverted.len() >= MAX_BULK_SELECTION {
                return Err(BulkError::TooManySelected);
            }
            inverted.push(id.to_string());
        }
        self.ids = inverted;
        Ok(())
    }

    /// Plan one action against a caller-owned entry snapshot.
    pub fn apply(
        &self,
        entries: &[McpEntry],
        action: BulkAction,
        confirm: bool,
    ) -> Result<BulkReport, BulkError> {
        apply(entries, self, action, confirm)
    }
}

/// Apply a bounded action plan to a caller-owned MCP snapshot.
///
/// This function does not mutate `entries`, spawn work, perform I/O, or
/// contact MCP servers. Report order is lexicographic by selected id, making
/// repeated calls byte-identical for equivalent inputs.
pub fn apply(
    entries: &[McpEntry],
    selection: &Selection,
    action: BulkAction,
    confirm: bool,
) -> Result<BulkReport, BulkError> {
    validate_selection(selection)?;
    if action.requires_confirmation() && !confirm {
        return Err(BulkError::ConsentRequired);
    }

    let mut ids = selection.ids.clone();
    ids.sort_unstable();

    let mut per_item = Vec::with_capacity(ids.len());
    let mut succeeded = 0_u32;
    let mut failed = 0_u32;

    for id in ids {
        let outcome = match entries.iter().find(|entry| entry.id == id) {
            None => {
                failed += 1;
                Outcome::Failed {
                    code: UNKNOWN_CODE.to_string(),
                }
            }
            Some(entry) => {
                let outcome = match action {
                    BulkAction::Enable if entry.enabled => Outcome::Skipped,
                    BulkAction::Disable if !entry.enabled => Outcome::Skipped,
                    BulkAction::Enable
                    | BulkAction::Disable
                    | BulkAction::Remove
                    | BulkAction::Refresh
                    | BulkAction::Install => Outcome::Applied,
                };
                if matches!(outcome, Outcome::Applied) {
                    succeeded += 1;
                }
                outcome
            }
        };
        per_item.push(ItemOutcome { id, outcome });
    }

    Ok(BulkReport {
        per_item,
        succeeded,
        failed,
    })
}

/// Alias emphasizing that this is a plan over a snapshot, not execution.
pub fn plan(
    entries: &[McpEntry],
    selection: &Selection,
    action: BulkAction,
    confirm: bool,
) -> Result<BulkReport, BulkError> {
    apply(entries, selection, action, confirm)
}

fn validate_id(id: &str) -> Result<(), BulkError> {
    if id.is_empty() {
        return Err(BulkError::EmptyId);
    }
    if id.chars().count() > MAX_ID_LEN {
        return Err(BulkError::IdTooLong);
    }
    Ok(())
}

fn validate_selection(selection: &Selection) -> Result<(), BulkError> {
    if selection.ids.is_empty() {
        return Err(BulkError::EmptySelection);
    }
    if selection.ids.len() > MAX_BULK_SELECTION {
        return Err(BulkError::TooManySelected);
    }
    for (index, id) in selection.ids.iter().enumerate() {
        validate_id(id)?;
        if selection.ids[..index].iter().any(|previous| previous == id) {
            return Err(BulkError::TooManySelected);
        }
    }
    Ok(())
}
