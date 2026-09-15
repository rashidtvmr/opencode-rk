//! Plugin panel list (UI-012, REQ-005 plugins+UI): pure toggle logic.
//!
//! No rendering, no IO. Caller supplies rows; toggle flips existing or
//! appends enabled on new name. Bounded by MAX_PANEL_PLUGINS.

use thiserror::Error;

/// Max plugin rows retained in the panel.
pub const MAX_PANEL_PLUGINS: usize = 128;

/// One plugin row in the panel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginRow {
    pub name: String,
    pub enabled: bool,
}

/// Failure modes for plugin panel toggle.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PluginPanelError {
    #[error("plugin name is empty")]
    EmptyName,
    #[error("too many plugins: max {max}, actual {actual}")]
    TooManyPlugins { max: usize, actual: usize },
}

/// Toggle plugin by name. Existing name flips `enabled` and returns new
/// state. New name pushes `enabled=true`. Empty name rejected. New name
/// at/over bound rejected.
pub fn toggle_plugin(rows: &mut Vec<PluginRow>, name: &str) -> Result<bool, PluginPanelError> {
    if name.is_empty() {
        return Err(PluginPanelError::EmptyName);
    }
    if let Some(row) = rows.iter_mut().find(|r| r.name == name) {
        row.enabled = !row.enabled;
        return Ok(row.enabled);
    }
    if rows.len() >= MAX_PANEL_PLUGINS {
        return Err(PluginPanelError::TooManyPlugins {
            max: MAX_PANEL_PLUGINS,
            actual: rows.len(),
        });
    }
    rows.push(PluginRow {
        name: name.to_owned(),
        enabled: true,
    });
    Ok(true)
}
