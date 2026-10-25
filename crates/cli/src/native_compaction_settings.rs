#![forbid(unsafe_code)]
//! ACK-only session policy display and a separate, explicitly applied draft.

use opencode_rk_sessions::SessionCompactionSettings as Policy;
use serde_json::Value;

const FIELDS: [&str; 4] = [
    "enabled",
    "threshold_tokens",
    "keep_recent_turns",
    "strategy",
];

#[derive(Clone, Debug)]
struct Metadata {
    overrides: Vec<String>,
    native: String,
    checkpoint_model: Option<String>,
    run_active: bool,
    checkpoint: String,
}

#[derive(Clone, Debug, Default)]
pub struct CompactionSettingsView {
    configured: Option<Policy>,
    effective: Option<Policy>,
    draft: Option<Policy>,
    metadata: Option<Metadata>,
    selected: usize,
    editing: Option<String>,
    manual_offset: Option<usize>,
    error: Option<String>,
}

impl CompactionSettingsView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate(&mut self) {
        *self = Self::new();
    }

    pub fn set_error(&mut self, error: &str) {
        self.error = Some(
            error
                .chars()
                .filter(|c| !c.is_control())
                .take(512)
                .collect(),
        );
    }

    /// Parse into temporary state first: a malformed ACK never replaces a
    /// previous acknowledged policy, effective policy, or unsaved draft.
    pub fn load_response(&mut self, body: &str) -> Result<(), String> {
        if body.len() > 32 * 1024 {
            return Err("compaction settings response too large".into());
        }
        let value: Value =
            serde_json::from_str(body).map_err(|_| "invalid compaction settings JSON")?;
        if value["scope"].as_str() != Some("session") {
            return Err("compaction settings scope must be session".into());
        }
        let configured = parse_policy(&value["configured"])?;
        let effective = parse_policy(&value["effective"])?;
        let entries = value["overridden_by_environment"]
            .as_array()
            .filter(|entries| entries.len() <= 4)
            .ok_or("invalid compaction environment overrides")?;
        let mut overrides = Vec::new();
        for entry in entries {
            let entry = entry
                .as_str()
                .filter(|key| FIELDS.contains(key))
                .ok_or("unknown compaction environment override")?;
            if overrides.iter().any(|old| old == entry) {
                return Err("duplicate compaction override".into());
            }
            overrides.push(entry.to_owned());
        }
        let native = &value["capabilities"]["native"];
        let availability = native["availability"]
            .as_str()
            .filter(|value| matches!(*value, "unverified" | "verified-for-checkpoint-model"))
            .ok_or("invalid native compaction capability provenance")?;
        if native["retention"].as_str() != Some("provider-owned")
            || value["capabilities"]["fallback"]["availability"].as_str()
                != Some("responses-summary")
            || value["capabilities"]["fallback"]["retention"].as_str() != Some("keep_recent_turns")
        {
            return Err("invalid compaction retention metadata".into());
        }
        let checkpoint_model = match native.get("model") {
            None | Some(Value::Null) if availability == "unverified" => None,
            Some(Value::String(model))
                if availability == "verified-for-checkpoint-model"
                    && !model.is_empty()
                    && model.len() <= 512
                    && !model.chars().any(char::is_control) =>
            {
                Some(model.clone())
            }
            _ => return Err("invalid native checkpoint model identity".into()),
        };
        let run_active = value["run_active"]
            .as_bool()
            .ok_or("invalid compaction run state")?;
        let checkpoint = match &value["checkpoint"] {
            Value::Null => "none".to_owned(),
            Value::Object(checkpoint) => {
                let strategy = checkpoint
                    .get("strategy")
                    .and_then(Value::as_str)
                    .filter(|strategy| matches!(*strategy, "native" | "fallback" | "unrecognized"))
                    .ok_or("invalid compaction checkpoint metadata")?;
                let boundary = checkpoint
                    .get("boundary_message_id")
                    .and_then(Value::as_str)
                    .filter(|id| id.len() <= 128 && !id.chars().any(char::is_control))
                    .ok_or("invalid compaction checkpoint boundary")?;
                format!("{strategy}; boundary {boundary}")
            }
            _ => return Err("invalid compaction checkpoint metadata".into()),
        };
        self.draft = Some(configured.clone());
        self.configured = Some(configured);
        self.effective = Some(effective);
        self.metadata = Some(Metadata {
            overrides,
            native: availability.into(),
            checkpoint_model,
            run_active,
            checkpoint,
        });
        self.editing = None;
        self.manual_offset = None;
        self.error = None;
        Ok(())
    }

    /// Only Apply yields a request body. Until an authenticated ACK is loaded,
    /// configured and effective values remain unchanged.
    pub fn key(&mut self, key: u8) -> Option<String> {
        self.configured.as_ref()?;
        if let Some(mut input) = self.editing.take() {
            match key {
                b'0'..=b'9' if input.len() < 8 => {
                    input.push(char::from(key));
                    self.editing = Some(input);
                }
                8 | 127 => {
                    input.pop();
                    self.editing = Some(input);
                }
                b'\r' | b'\n' => {
                    let maximum = if self.selected == 2 { 10_000_000 } else { 1000 };
                    if let Some(number) = input
                        .parse::<u64>()
                        .ok()
                        .filter(|number| (1..=maximum).contains(number))
                    {
                        if let Some(draft) = self.draft.as_mut() {
                            if self.selected == 2 {
                                draft.threshold_tokens = number;
                            } else {
                                draft.keep_recent_turns = number as usize;
                            }
                        }
                        self.error = None;
                    } else {
                        self.set_error(&format!(
                            "Enter a number within 1..={maximum}; previous draft retained"
                        ));
                    }
                }
                27 => {}
                _ => self.editing = Some(input),
            }
            return None;
        }
        match key {
            b'j' => {
                self.selected = (self.selected + 1).min(5);
                self.manual_offset = None;
            }
            b'k' => {
                self.selected = self.selected.saturating_sub(1);
                self.manual_offset = None;
            }
            b'f' => {
                self.manual_offset = Some(self.manual_offset.unwrap_or(0).saturating_add(3).min(24))
            }
            b'b' => self.manual_offset = Some(self.manual_offset.unwrap_or(0).saturating_sub(3)),
            b'\r' | b'\n' | b' ' => match self.selected {
                0 => {
                    let draft = self.draft.as_mut()?;
                    draft.enabled = !draft.enabled;
                }
                1 => {
                    use opencode_rk_sessions::CompactionStrategy::*;
                    let draft = self.draft.as_mut()?;
                    draft.strategy = match draft.strategy {
                        Auto => Native,
                        Native => Fallback,
                        Fallback => Auto,
                    };
                }
                2 | 3 => {
                    self.editing = Some(String::new());
                    self.error = None;
                }
                4 => {
                    return self
                        .draft
                        .as_ref()
                        .and_then(|draft| serde_json::to_string(draft).ok());
                }
                5 => {
                    self.draft = self.configured.clone();
                    self.editing = None;
                    self.error = None;
                }
                _ => {}
            },
            b'+' | b'-' => {
                let draft = self.draft.as_mut()?;
                if self.selected == 2 {
                    draft.threshold_tokens = if key == b'+' {
                        draft.threshold_tokens.saturating_add(1000).min(10_000_000)
                    } else {
                        draft.threshold_tokens.saturating_sub(1000).max(1)
                    };
                } else if self.selected == 3 {
                    draft.keep_recent_turns = if key == b'+' {
                        (draft.keep_recent_turns + 1).min(1000)
                    } else {
                        draft.keep_recent_turns.saturating_sub(1).max(1)
                    };
                }
            }
            _ => {}
        }
        None
    }

    pub fn render(&self, width: usize, height: usize) -> Vec<String> {
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let mut rows = vec!["Compaction settings (session)".to_owned()];
        if let Some(error) = &self.error {
            rows.push(format!("Error: {error}"));
        }
        let (Some(configured), Some(effective), Some(draft), Some(metadata)) = (
            &self.configured,
            &self.effective,
            &self.draft,
            &self.metadata,
        ) else {
            rows.push("Settings unloaded; authenticated session GET required".into());
            return fit_rows(rows, 0, width, height);
        };
        rows.push(format!(
            "Configured (last ACK): Enabled: {}; Strategy: {}",
            configured.enabled,
            strategy(configured)
        ));
        rows.push(format!(
            "  Threshold: {}; Keep recent turns: {}",
            configured.threshold_tokens, configured.keep_recent_turns
        ));
        rows.push(format!(
            "Effective: {}; Strategy: {}",
            effective.enabled,
            strategy(effective)
        ));
        rows.push(format!(
            "  Threshold: {}; Keep recent turns: {}",
            effective.threshold_tokens, effective.keep_recent_turns
        ));
        rows.push(format!(
            "Environment overrides: {}",
            if metadata.overrides.is_empty() {
                "none".into()
            } else {
                metadata.overrides.join(", ")
            }
        ));
        let control_start = rows.len();
        let labels = [
            format!("Enabled: {} (automatic compaction)", draft.enabled),
            format!("Strategy: {}", strategy(draft)),
            format!("Threshold: {} estimated tokens", draft.threshold_tokens),
            format!("Keep recent turns: {} (fallback)", draft.keep_recent_turns),
            "Apply".into(),
            "Discard draft".into(),
        ];
        for (index, label) in labels.into_iter().enumerate() {
            let editing = if self.selected == index {
                self.editing.as_deref()
            } else {
                None
            };
            rows.push(format!(
                "{} {label}{}",
                if self.selected == index { ">" } else { " " },
                editing.map_or_else(String::new, |input| format!(" [enter number: {input}_]"))
            ));
        }
        rows.push(
            if draft == configured {
                "No unsaved changes"
            } else {
                "Unsaved draft; Apply requires backend acknowledgement"
            }
            .into(),
        );
        rows.push(format!(
            "Native: {}{}",
            metadata.native,
            metadata
                .checkpoint_model
                .as_ref()
                .map_or_else(String::new, |model| format!(" ({model})"))
        ));
        rows.push("Native retention: provider-owned; fallback: keep_recent_turns".into());
        rows.push("Auto: explicit native policy, otherwise Responses summary fallback".into());
        rows.push("Threshold is estimated: UTF-8 bytes / 4, not measured tokenizer usage".into());
        rows.push("Input limits: 100 items, 512 KiB; current instructions rebuilt".into());
        rows.push(format!(
            "Checkpoint: {}; run active: {}",
            metadata.checkpoint, metadata.run_active
        ));
        rows.push("j/k or arrows: select; Enter: edit/apply; +/-: step; f/b: scroll".into());
        let offset = self.manual_offset.unwrap_or_else(|| {
            (control_start + self.selected).saturating_sub(height.saturating_sub(1))
        });
        fit_rows(rows, offset, width, height)
    }
}

fn parse_policy(value: &Value) -> Result<Policy, String> {
    let policy: Policy =
        serde_json::from_value(value.clone()).map_err(|_| "invalid compaction policy")?;
    policy
        .validate()
        .map_err(|_| "compaction policy outside supported bounds")?;
    Ok(policy)
}

fn strategy(policy: &Policy) -> &'static str {
    use opencode_rk_sessions::CompactionStrategy::*;
    match policy.strategy {
        Auto => "auto",
        Native => "native",
        Fallback => "fallback",
    }
}

fn fit_rows(rows: Vec<String>, offset: usize, width: usize, height: usize) -> Vec<String> {
    let offset = offset.min(rows.len().saturating_sub(1));
    rows.into_iter()
        .skip(offset)
        .take(height)
        .map(|row| crate::native_view_layout::fit(&row, width.min(u16::MAX as usize) as u16))
        .collect()
}
