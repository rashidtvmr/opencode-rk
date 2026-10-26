#![forbid(unsafe_code)]
//! Bounded display model for the authenticated session context inspector.

use serde_json::Value;
use std::collections::HashSet;

const MAX_ITEMS: usize = 115;
const MAX_CONTENT_BYTES: usize = 112 * 1024;
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_ID_BYTES: usize = 4220;
const MAX_PATH_BYTES: usize = 4096;
const MAX_RECORD_FIELD_BYTES: usize = 8192;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContextRecord {
    pub(crate) kind: String,
    pub(crate) name: Option<String>,
    pub(crate) call_id: Option<String>,
    pub(crate) content: String,
    pub(crate) truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContextItem {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) role: String,
    pub(crate) source: String,
    pub(crate) included: bool,
    pub(crate) selectable: bool,
    pub(crate) content: String,
    pub(crate) reason: Option<String>,
    /// Typed authority for whether this item is represented by a checkpoint.
    /// This remains distinct from the legacy display reason.
    pub(crate) represented_by_checkpoint: bool,
    /// Known stored byte count, when the backend measured it (`None` =
    /// unmeasured; never fabricated to `0`).
    pub(crate) bytes: Option<u64>,
    /// Known byte/4 estimate, when the backend measured `bytes`
    /// (`None` = unmeasured; never fabricated from preview content).
    pub(crate) estimated_tokens: Option<u64>,
    /// False when the backend withheld item content (e.g. an excluded item
    /// whose editor text is unavailable); the preview is then a placeholder.
    pub(crate) content_available: bool,
    /// Backend-declared provenance for `bytes`/`estimated_tokens` (e.g.
    /// "stored UTF-8 bytes / 4; not measured tokenizer usage"). Surfaced so
    /// measured and estimated values are distinguishable, never dropped.
    pub(crate) count_provenance: Option<String>,
    pub(crate) records: Vec<ContextRecord>,
    pub(crate) truncated: bool,
}

#[derive(Clone, Debug, Default)]
struct Projection {
    status: String,
    bytes: Option<u64>,
    tokens: Option<u64>,
    typed_tail_items: Option<u64>,
    reason: Option<String>,
    provenance: String,
}

#[derive(Clone, Debug, Default)]
struct CheckpointMeta {
    strategy: String,
    included: bool,
    boundary: String,
    provider: String,
    model: String,
}

const USAGE_KEYS: [&str; 7] = ["system_prompt", "system_tools", "mcp_instructions", "memory_files", "skills", "messages", "free_space"];
const MAX_USAGE_ENTRIES: usize = 115;

#[derive(Clone, Debug, PartialEq, Eq)]
struct UsageEntry { name: String, path: Option<String>, group: Option<String>, tokens: Option<u64> }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct UsageReport {
    model: Option<String>, capacity_tokens: Option<u64>, used_tokens: Option<u64>,
    provenance: Option<String>, coverage: Option<String>, coverage_gaps: Vec<String>,
    estimate_provenance: Option<String>, estimated_total_tokens: Option<u64>,
    categories: [Option<u64>; 7], mcp_tools: Option<Vec<UsageEntry>>,
    memory_files: Option<Vec<UsageEntry>>, skills: Option<Vec<UsageEntry>>,
    suggestions: Option<Vec<(String, String)>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UsageStyleSpan { pub(crate) start_cell: usize, pub(crate) cells: usize, pub(crate) slot: crate::native_theme_runtime::Slot }
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UsageLine { pub(crate) text: String, pub(crate) styles: Vec<UsageStyleSpan> }

#[derive(Debug, Default)]
pub(crate) struct ContextView {
    items: Vec<ContextItem>,
    usage_report: Option<UsageReport>,
    selected_id: Option<String>,
    detail_offset: usize,
    error: Option<String>,
    next_before: Option<String>,
    projection: Projection,
    checkpoint: Option<CheckpointMeta>,
    page_budget: Option<(u64, u64)>,
    input_limits: Option<(u64, u64)>,
}

impl ContextView {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) fn invalidate(&mut self) {
        *self = Self::new();
    }
    pub(crate) fn set_error(&mut self, error: &str) {
        self.error = Some(error.chars().take(512).collect());
    }
    pub(crate) fn selected(&self) -> Option<&ContextItem> {
        self.items
            .iter()
            .find(|x| Some(x.id.as_str()) == self.selected_id.as_deref())
    }
    pub(crate) fn next_before(&self) -> Option<&str> {
        self.next_before.as_deref()
    }
    pub(crate) fn items(&self) -> &[ContextItem] {
        &self.items
    }
    pub(crate) fn select_step(&mut self, down: bool) {
        if self.items.is_empty() {
            return;
        }
        let i = self
            .items
            .iter()
            .position(|x| Some(x.id.as_str()) == self.selected_id.as_deref())
            .unwrap_or(0);
        let n = if down {
            (i + 1) % self.items.len()
        } else {
            i.checked_sub(1).unwrap_or(self.items.len() - 1)
        };
        self.selected_id = Some(self.items[n].id.clone());
        self.detail_offset = 0;
    }
    pub(crate) fn scroll_detail(&mut self, down: bool) {
        self.detail_offset = if down {
            self.detail_offset.saturating_add(1)
        } else {
            self.detail_offset.saturating_sub(1)
        };
    }

    pub(crate) fn load_response(&mut self, body: &str) -> Result<(), String> {
        if body.len() > MAX_RESPONSE_BYTES {
            return Err("context response exceeds 512 KiB".into());
        }
        let value: Value = serde_json::from_str(body)
            .map_err(|_| "context response is not valid JSON".to_owned())?;
        let object = value
            .as_object()
            .ok_or_else(|| "context response must be an object".to_owned())?;
        if object.get("scope").and_then(Value::as_str) != Some("session") {
            return Err("context response scope must be session".into());
        }
        let raw_items = object
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| "context response is missing items".to_owned())?;
        if raw_items.len() > MAX_ITEMS {
            return Err("context response exceeds 115 items".into());
        }
        let mut ids = HashSet::with_capacity(raw_items.len());
        let mut total = 0usize;
        let mut parsed = Vec::with_capacity(raw_items.len());
        for raw in raw_items {
            let o = raw
                .as_object()
                .ok_or_else(|| "context item must be an object".to_owned())?;
            let id = string(o, "id")?;
            validate_id_kind(&id, string(o, "kind")?.as_str())?;
            if !ids.insert(id.clone()) {
                return Err("context response contains duplicate item IDs".into());
            }
            let kind = string(o, "kind")?;
            let role = string(o, "role")?;
            if !matches!(
                role.as_str(),
                "user" | "assistant" | "system" | "tool" | "context"
            ) {
                return Err("context item role is unknown".into());
            }
            let source = string(o, "source")?;
            if source.len() > MAX_PATH_BYTES || source.chars().any(char::is_control) {
                return Err("context item source is invalid".into());
            }
            let included = o
                .get("included")
                .and_then(Value::as_bool)
                .ok_or_else(|| "context item included is required".to_owned())?;
            let selectable = o
                .get("selectable")
                .and_then(Value::as_bool)
                .ok_or_else(|| "context item selectable is required".to_owned())?;
            let content = string(o, "content")?;
            if content.len() > 4096 {
                return Err("context item preview exceeds 4096 bytes".into());
            }
            total = total.saturating_add(content.len());
            let bytes = optional_u64(o, "bytes")?;
            // Measured accounting: `bytes` and `estimated_tokens` are absent
            // when the backend could not measure the item (e.g. an excluded
            // item whose editor text is unavailable). They must stay `None`
            // so the renderer can show `unreported`; they are never replaced
            // by a derived non-zero count. When `bytes` is known and the
            // estimate is missing, fall back to documented `bytes / 4`.
            let estimated_tokens = match optional_u64(o, "estimated_tokens")? {
                Some(tokens) => Some(tokens),
                None => bytes.map(|bytes| bytes.saturating_add(3) / 4),
            };
            let content_available = o
                .get("content_available")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let reason = match o.get("reason") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.chars().take(512).collect()),
                _ => return Err("context item reason is invalid".into()),
            };
            // An explicitly typed representation flag is authoritative. The
            // reason fallback is retained only for legacy payloads omitting it.
            let compacted = match optional_bool(o, "represented_by_checkpoint")? {
                Some(value) => value,
                None => reason.as_deref() == Some("checkpoint_is_indivisible"),
            };
            let mandatory = optional_bool(o, "mandatory")?.unwrap_or(false);
            if selectable && (role == "system" || kind == "memory" || compacted || mandatory) {
                return Err(
                    "mandatory, memory and compacted entries cannot expose item selection".into(),
                );
            }
            if (kind == "memory" && role != "system")
                || (kind == "checkpoint" && role != "context")
                || (kind == "tool_pair" && role != "tool")
                || (kind == "message" && !matches!(role.as_str(), "user" | "assistant" | "system"))
            {
                return Err("context item role does not match its kind".into());
            }
            let records = parse_records(o.get("records"), &kind, &mut total)?;
            if total > MAX_CONTENT_BYTES {
                return Err("context content and record previews exceed 112 KiB".into());
            }
            parsed.push(ContextItem {
                id,
                kind,
                role,
                source,
                included,
                selectable,
                content,
                reason,
                represented_by_checkpoint: compacted,
                bytes,
                estimated_tokens,
                content_available,
                count_provenance: o
                    .get("count_provenance")
                    .and_then(Value::as_str)
                    .map(|s| s.chars().take(512).collect()),
                records,
                truncated: o.get("truncated").and_then(Value::as_bool).unwrap_or(false),
            });
        }
        let selected = self
            .selected_id
            .clone()
            .filter(|id| parsed.iter().any(|x| &x.id == id))
            .or_else(|| parsed.first().map(|x| x.id.clone()));
        let next = match object.get("next_before") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => {
                validate_uuid(s)?;
                Some(s.clone())
            }
            _ => return Err("next_before is invalid".into()),
        };
        let projection = parse_projection(object.get("projection"))?;
        let usage_report = parse_usage_report(object.get("usage_report"))?;
        let checkpoint = parse_checkpoint(object.get("checkpoint"))?;
        let page_budget = parse_pair(
            object.get("page_budget"),
            "stored_message_bytes",
            "preview_bytes",
        )?;
        let input_limits = parse_pair(object.get("input_limits"), "items", "bytes")?;
        self.items = parsed;
        self.usage_report = usage_report;
        self.selected_id = selected;
        self.detail_offset = 0;
        self.error = None;
        self.next_before = next;
        self.projection = projection;
        self.checkpoint = checkpoint;
        self.page_budget = page_budget;
        self.input_limits = input_limits;
        Ok(())
    }

    pub(crate) fn render(&self, width: usize, height: usize) -> Vec<String> {
        self.render_styled(width, height).into_iter().map(|line| line.text).collect()
    }

    /// Context grid carries explicit theme-slot spans, never ANSI escapes.
    pub(crate) fn render_styled(&self, width: usize, height: usize) -> Vec<UsageLine> {
        if width == 0 || height == 0 { return Vec::new(); }
        if height <= 2 {
            let mut rows = Vec::new();
            if height == 1 {
                rows.push(usage_line("Context Usage".into(), Vec::new()));
            } else {
                // Preserve the selected measurement as a complete, compact
                // acknowledgement in the two-row viewport.  The normal
                // dashboard remains cell-clipped by `fit_usage`; this short
                // summary is intentionally allowed to wrap at the caller's
                // terminal edge so neither token value nor provenance is
                // silently dropped.
                if let Some(item) = self.selected() {
                    rows.push(usage_line(format!("Context Usage · {}", item.kind), Vec::new()));
                    rows.push(usage_line(format!("{} bytes / {} tokens", shown(item.bytes), shown(item.estimated_tokens)), Vec::new()));
                } else {
                    rows.push(usage_line("Context unloaded".into(), Vec::new()));
                }
            }
            return rows;
        }
        let report = self.usage_report.as_ref();
        let mut head = vec![usage_line("Context Usage".into(), Vec::new())];
        head.push(usage_line(format!("Model: {} · Context capacity: {} tokens",
            report.and_then(|r| r.model.as_deref()).unwrap_or("unreported"),
            report.and_then(|r| r.capacity_tokens).map_or_else(|| "unreported".into(), |x| x.to_string())), Vec::new()));
        let used = report.and_then(|r| r.used_tokens);
        let capacity = report.and_then(|r| r.capacity_tokens);
        head.push(usage_line(format!("Used: {} / {} tokens · {}",
            shown(used), capacity.map_or_else(|| "unreported".into(), |x| x.to_string()), utilization(used, capacity)), Vec::new()));
        if let Some(item) = self.selected() {
            head.push(usage_line(format!("Selected: {} {} · {} bytes / {} tokens · {} · projection {}",
                if item.selectable { "▸" } else { "•" }, item.kind,
                shown(item.bytes), shown(item.estimated_tokens), reason_label(item), self.projection.status), Vec::new()));
        } else {
            head.push(usage_line(format!("Context unloaded: {} · authenticated session GET required", self.projection.status), Vec::new()));
        }
        if let Some(item) = self.selected() {
            if let Some(provenance) = &item.count_provenance {
                head.push(usage_line(format!("Counting: {provenance}"), Vec::new()));
            }
        }
        if let Some(report) = report {
            if used.is_none() {
                if let Some(total) = report.estimated_total_tokens {
                    head.push(usage_line(format!("Estimated usage total: {total} tokens · backend estimate (provider usage unreported)"), Vec::new()));
                }
            }
            if !report.coverage_gaps.is_empty() {
                head.push(usage_line(format!("Coverage gaps: {}", report.coverage_gaps.join("; ")), Vec::new()));
            }
        }
        let reserve = usize::from(height >= 4);
        head = head.into_iter().flat_map(|line| wrap(&line.text, width.max(1)).into_iter().map(move |text| usage_line(text, line.styles.clone()))).collect();
        let max_head = height.saturating_sub(1 + reserve).max(1);
        head.truncate(max_head);
        let grid_rows = height.saturating_sub(head.len() + reserve + 1).clamp(1, 10);
        // The assembled usage report owns the dashboard's first-frame
        // prominence. Legacy inventory-only responses retain their established
        // detail-first ordering so projection and withheld-content explanations
        // remain reachable even when no report was returned.
        let mut body = if report.is_some() {
            usage_dashboard(report, width, grid_rows)
        } else {
            Vec::new()
        };
        body.push(usage_line(format!("Context inventory: {} items · projection {} (not assembled-request accounting)", self.items.len(), self.projection.status), Vec::new()));
        if let Some(lines) = breakdown_lines(&self.items) { body.extend(lines.into_iter().map(|line| usage_line(line, Vec::new()))); }
        if let Some(item) = self.selected() {
            body.extend(self.metadata_lines().into_iter().flat_map(|line| wrap(&line, width)).map(|line| usage_line(line, Vec::new())));
            body.extend(item_detail(item, width).into_iter().map(|line| usage_line(line, Vec::new())));
        } else if self.items.is_empty() { body.push(usage_line("Context inventory unloaded; authenticated session GET required".into(), Vec::new())); }
        if report.is_none() {
            body.extend(usage_dashboard(report, width, grid_rows));
        }
        if let Some(source) = report.and_then(|r| r.provenance.as_deref()) { body.push(usage_line(format!("Usage provenance: {source}"), Vec::new())); }
        if let Some(source) = report.and_then(|r| r.estimate_provenance.as_deref()) { body.push(usage_line(format!("Category estimates: {source}"), Vec::new())); }
        if let Some(coverage) = report.and_then(|r| r.coverage.as_deref()) { body.push(usage_line(format!("Coverage: {coverage}"), Vec::new())); }
         if let Some(report) = report { if !report.coverage_gaps.is_empty() { body.push(usage_line(format!("Coverage gaps: {}", report.coverage_gaps.join("; ")), Vec::new())); } if let Some(total) = report.estimated_total_tokens { body.push(usage_line(format!("Estimated usage total: {total} tokens · backend estimate"), Vec::new())); } }
        if let Some(error) = &self.error { body.insert(0, usage_line(format!("Refresh failed; showing last loaded context: {error}"), Vec::new())); }
        let mut body = body;
        let mut grid_heading = format!("Estimated usage by category · {} columns × {} rows maximum", width.min(20), grid_rows);
        if let Some(report) = report {
            if let Some(capacity) = report.capacity_tokens {
                let known = report.categories.iter().flatten().fold(0u128, |sum, tokens| sum.saturating_add(u128::from(*tokens)));
                if known > u128::from(capacity) { grid_heading.push_str(" · categories exceed capacity; proportions normalized"); }
            }
        }
        if let Some(index) = body.iter().position(|line| line.text == "Estimated usage by category") {
            body[index] = usage_line(grid_heading, Vec::new());
        }
        let footer = usage_line("j/k select · Enter include/exclude · f/b detail · p older · r latest".into(), Vec::new());
        let visible = height.saturating_sub(head.len() + reserve);
        let offset = self.detail_offset.min(body.len());
        let mut rows = head;
        rows.extend(body.into_iter().skip(offset).take(visible));
        if reserve > 0 { rows.push(footer); }
        fit_usage(rows, width)
    }

    fn metadata_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(c) = &self.checkpoint {
            out.push(format!(
                "Checkpoint: strategy={} included={} boundary={} provider={} model={}",
                c.strategy, c.included, c.boundary, c.provider, c.model
            ));
        }
        out.push(format!(
            "Projection: status={} bytes={} tokens={} typed_items={}",
            self.projection.status,
            shown(self.projection.bytes),
            shown(self.projection.tokens),
            shown(self.projection.typed_tail_items)
        ));
        if let Some(reason) = &self.projection.reason {
            out.push(format!("Projection reason: {reason}"));
        }
        if !self.projection.provenance.is_empty() {
            out.push(format!(
                "Estimate provenance: {}",
                self.projection.provenance
            ));
        }
        if let Some(breakdown) = breakdown_lines(&self.items) {
            out.extend(breakdown);
        }

        if let Some((stored, preview)) = self.page_budget {
            out.push(format!(
                "Page budget: stored={} preview={}",
                stored, preview
            ));
        }
        if let Some((items, bytes)) = self.input_limits {
            out.push(format!("Input limits: items={} bytes={}", items, bytes));
        }
        out
    }
}

fn usage_line(text: String, styles: Vec<UsageStyleSpan>) -> UsageLine { UsageLine { text, styles } }
fn category_slot(category: crate::native_context_grid::CategoryId) -> crate::native_theme_runtime::Slot {
    use crate::native_context_grid::CategoryId as C;
    use crate::native_theme_runtime::Slot as S;
    match category {
        C::SystemPrompt => S::Primary,
        C::SystemTools => S::Warning,
        C::McpInstructions => S::Success,
        C::MemoryFiles => S::Error,
        C::Skills => S::TextMuted,
        C::Messages => S::Border,
        C::FreeSpace => S::Text,
    }
}
fn utilization(used: Option<u64>, capacity: Option<u64>) -> String {
    match (used, capacity) {
        (Some(_value), Some(0)) => "capacity is 0; percentage unavailable".into(),
        (Some(value), Some(capacity)) => crate::native_context_grid::percentage(value, Some(capacity))
            .map_or_else(|| "capacity is 0; percentage unavailable".into(), |percent| format!("{percent}%")),
        (Some(_), None) => "capacity unreported".into(),
        (None, _) => "unreported".into(),
    }
}
fn parse_usage_text(o: &serde_json::Map<String, Value>, key: &str, limit: usize) -> Result<String, String> {
    let text = o.get(key).and_then(Value::as_str).ok_or_else(|| format!("usage_report {key} must be a string"))?;
    if text.len() > limit || text.chars().any(char::is_control) { return Err(format!("usage_report {key} is invalid")); }
    Ok(text.to_owned())
}
fn optional_usage_text(o: &serde_json::Map<String, Value>, key: &str, limit: usize) -> Result<Option<String>, String> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.len() <= limit && !s.chars().any(char::is_control) => Ok(Some(s.clone())),
        _ => Err(format!("usage_report {key} is invalid")),
    }
}
fn validate_usage_text(text: &str, limit: usize, field: &str) -> Result<String, String> {
    if text.len() > limit || text.chars().any(char::is_control) {
        return Err(format!("usage_report {field} is invalid"));
    }
    Ok(text.to_owned())
}
fn parse_usage_entries(o: &serde_json::Map<String, Value>, key: &str) -> Result<Option<Vec<UsageEntry>>, String> {
    let Some(value) = o.get(key) else { return Ok(None) };
    if value.is_null() { return Ok(None); }
    let values = value.as_array().ok_or_else(|| format!("usage_report {key} must be an array"))?;
    if values.len() > MAX_USAGE_ENTRIES { return Err(format!("usage_report {key} exceeds {MAX_USAGE_ENTRIES} entries")); }
    let mut entries = Vec::with_capacity(values.len());
    for value in values {
        let row = value.as_object().ok_or_else(|| format!("usage_report {key} entry must be an object"))?;
        entries.push(UsageEntry { name: parse_usage_text(row, "name", 512)?, path: optional_usage_text(row, "path", MAX_PATH_BYTES)?,
            group: optional_usage_text(row, "group", 128)?, tokens: optional_u64(row, "tokens")? });
    }
    Ok(Some(entries))
}
fn parse_usage_report(value: Option<&Value>) -> Result<Option<UsageReport>, String> {
    let Some(value) = value else { return Ok(None) };
    if value.is_null() { return Ok(None); }
    let o = value.as_object().ok_or_else(|| "usage_report must be an object".to_owned())?;
    const ALLOWED: &[&str] = &["model_id", "capacity_tokens", "reported_used_tokens", "estimated_total_tokens", "categories", "mcp_tools", "memory_files", "skills", "suggestions", "coverage"];
    if o.keys().any(|key| !ALLOWED.contains(&key.as_str())) { return Err("usage_report contains an unknown field".into()); }
    let model = optional_usage_text(o, "model_id", 512)?;
    let capacity_tokens = optional_u64(o, "capacity_tokens")?;
    let used_tokens = optional_u64(o, "reported_used_tokens")?;
    let estimated_total_tokens = optional_u64(o, "estimated_total_tokens")?;
    let mut categories = [None; 7];
    let mut category_present = [false; 7];
    let raw_categories = o.get("categories").and_then(|v| if v.is_null() { None } else { Some(v) });
    if let Some(raw) = raw_categories {
        let values = raw.as_array().ok_or_else(|| "usage_report categories must be an array".to_owned())?;
        if values.len() > 7 { return Err("usage_report categories exceeds 7 entries".into()); }
        for value in values {
            let row = value.as_object().ok_or_else(|| "usage_report category must be an object".to_owned())?;
            const CATEGORY_FIELDS: &[&str] = &["id", "label", "tokens", "provenance"];
            if row.keys().any(|key| !CATEGORY_FIELDS.contains(&key.as_str())) { return Err("usage_report category contains an unknown field".into()); }
            let id = row.get("id").and_then(Value::as_str).ok_or_else(|| "usage_report category id is required".to_owned())?;
            let index = match id {
                "system_prompt" => 0, "system_tools" => 1, "mcp_server_instructions" => 2,
                "memory_files" => 3, "skills" => 4, "messages" => 5, "free_space" => 6,
                _ => return Err("usage_report category id is unknown".into()),
            };
            if category_present[index] { return Err("usage_report contains duplicate category".into()); }
            category_present[index] = true;
            if let Some(label) = row.get("label") {
                let text = label.as_str().ok_or_else(|| "usage_report category label is invalid".to_owned())?;
                validate_usage_text(text, 512, "category label")?;
            }
            if let Some(provenance) = row.get("provenance") {
                let text = provenance.as_str().ok_or_else(|| "usage_report category provenance is invalid".to_owned())?;
                validate_usage_text(text, 512, "category provenance")?;
            }
            categories[index] = optional_u64(row, "tokens")?;
        }
    }
    let coverage_obj = o.get("coverage").and_then(|v| if v.is_null() { None } else { Some(v) });
    let (coverage, coverage_gaps) = if let Some(raw) = coverage_obj {
        let row = raw.as_object().ok_or_else(|| "usage_report coverage must be an object".to_owned())?;
        if row.keys().any(|key| key != "complete" && key != "gaps") { return Err("usage_report coverage contains an unknown field".into()); }
        let complete = row.get("complete").and_then(Value::as_bool).ok_or_else(|| "usage_report coverage complete is required".to_owned())?;
        let gaps = match row.get("gaps") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(values)) if values.len() <= 32 => values.iter().map(|v| {
                let text = v.as_str().ok_or_else(|| "usage_report coverage gap is invalid".to_owned())?;
                validate_usage_text(text, 256, "coverage gap")
            }).collect::<Result<Vec<String>, String>>()?,
            Some(Value::Array(_)) => return Err("usage_report coverage gaps exceeds 32 entries".into()),
            Some(_) => return Err("usage_report coverage gaps must be an array".into()),
        };
        (Some(if complete && gaps.is_empty() { "complete".into() } else { "partial".into() }), gaps)
    } else { (None, Vec::new()) };
    fn detail_entries(o: &serde_json::Map<String, Value>, key: &str) -> Result<Option<Vec<UsageEntry>>, String> {
        let Some(value) = o.get(key) else { return Ok(None) }; if value.is_null() { return Ok(None); }
        let values = value.as_array().ok_or_else(|| format!("usage_report {key} must be an array"))?;
        if values.len() > MAX_USAGE_ENTRIES { return Err(format!("usage_report {key} exceeds {MAX_USAGE_ENTRIES} entries")); }
        let mut out = Vec::with_capacity(values.len());
        for value in values {
            if let Some(text) = value.as_str() {
                if text.len() > MAX_PATH_BYTES || text.chars().any(char::is_control) { return Err(format!("usage_report {key} detail is invalid")); }
                out.push(UsageEntry { name: text.to_owned(), path: None, group: None, tokens: None });
            } else if let Some(row) = value.as_object() {
                // Backend v3 emits strings.  Retain bounded compatibility with
                // the pre-v3 detail shape so existing inventory consumers do
                // not lose source paths while the producer migrates.
                let name = parse_usage_text(row, "name", 512)?;
                out.push(UsageEntry { name, path: optional_usage_text(row, "path", MAX_PATH_BYTES)?, group: optional_usage_text(row, "group", 128)?, tokens: optional_u64(row, "tokens")? });
            } else { return Err(format!("usage_report {key} detail must be a string or object")); }
        }
        Ok(Some(out))
    }
    let suggestions = match o.get("suggestions") {
        None | Some(Value::Null) => None,
        Some(Value::Array(values)) if values.len() <= 32 => { let mut out = Vec::with_capacity(values.len()); for value in values { let row=value.as_object().ok_or_else(|| "usage suggestion must be an object".to_owned())?; out.push((parse_usage_text(row,"text",1024)?,parse_usage_text(row,"evidence",2048)?)); } Some(out) }
        Some(Value::Array(_)) => return Err("usage_report suggestions exceeds 32 entries".into()), Some(_) => return Err("usage_report suggestions must be an array".into()),
    };
    let estimate_provenance = Some("backend ContextUsageReport category provenance; no client recomputation".to_owned());
    Ok(Some(UsageReport { model, capacity_tokens, used_tokens, provenance: Some("backend ContextUsageReport".into()), coverage, coverage_gaps, estimate_provenance, estimated_total_tokens, categories,
        mcp_tools: detail_entries(o, "mcp_tools")?, memory_files: detail_entries(o, "memory_files")?, skills: detail_entries(o, "skills")?, suggestions }))
}
fn usage_dashboard(report: Option<&UsageReport>, width: usize, grid_rows: usize) -> Vec<UsageLine> {
    let mut rows = vec![usage_line("Estimated usage by category".into(), Vec::new())];
    rows.extend(usage_grid(report, width, grid_rows));
    for category in crate::native_context_grid::CategoryId::ALL {
        let index = category.index();
        let raw = report.and_then(|r| r.categories[index]);
        let tokens = if category == crate::native_context_grid::CategoryId::FreeSpace
            && report.and_then(|r| r.capacity_tokens).filter(|cap| *cap > 0).is_none() { None } else { raw };
        let count_label = tokens.map_or_else(|| "unreported".into(), |n| n.to_string());
        let percent = tokens.and_then(|n| report.and_then(|r| crate::native_context_grid::percentage(n, r.capacity_tokens)))
            .map_or_else(|| "unreported".into(), |n| format!("{n}%"));
        let legend = format!("■ {}: {count_label} tokens · {percent} of capacity (estimated)", category.label());
        for (line_index, text) in wrap(&legend, width.max(1)).into_iter().enumerate() {
            let swatch = line_index == 0;
            rows.push(usage_line(text, if swatch { vec![UsageStyleSpan { start_cell: 0, cells: 1, slot: category_slot(category) }] } else { Vec::new() }));
        }
    }
    for (title, entries) in [("MCP tools · /mcp", report.and_then(|r| r.mcp_tools.as_deref())),
        ("Memory files · /memory", report.and_then(|r| r.memory_files.as_deref())),
        ("Skills · /skills", report.and_then(|r| r.skills.as_deref()))] { rows.extend(usage_tree(title, entries, width)); }
    rows.push(usage_line("Suggestions".into(), Vec::new()));
    match report.and_then(|r| r.suggestions.as_deref()) {
        None => rows.push(usage_line("  unreported".into(), Vec::new())),
        Some(items) if items.is_empty() => rows.push(usage_line("  none reported".into(), Vec::new())),
        Some(items) => for (text, evidence) in items { rows.push(usage_line(format!("  • {text} · Evidence: {evidence}"), Vec::new())); },
    }
    rows
}

fn usage_tree(title: &str, entries: Option<&[UsageEntry]>, width: usize) -> Vec<UsageLine> {
    let mut rows = vec![usage_line(title.into(), Vec::new())];
    match entries {
        None => rows.push(usage_line("  unreported".into(), Vec::new())),
        Some(entries) if entries.is_empty() => rows.push(usage_line("  none reported".into(), Vec::new())),
        Some(entries) => for (i, entry) in entries.iter().enumerate() {
            if let Some(group) = &entry.group { rows.extend(wrap(group, width).into_iter().map(|s| usage_line(format!("  {s}"), Vec::new()))); }
            let branch = if i + 1 == entries.len() { "└─" } else { "├─" };
            let path = entry.path.as_deref().map_or(String::new(), |path| format!(" · {path}"));
            rows.extend(wrap(&format!("{branch} {}{} · {} tokens", entry.name, path, shown(entry.tokens)), width).into_iter().map(|s| usage_line(s, Vec::new())));
        },
    }
    rows
}
fn usage_grid(report: Option<&UsageReport>, width: usize, rows: usize) -> Vec<UsageLine> {
    use crate::native_context_grid::{allocate, CategoryId, CategoryTokens, CountBasis, Coverage, GridCell, GridInput, GridState};
    let mut categories = [None; crate::native_context_grid::CATEGORY_COUNT];
    if let Some(report) = report {
        for category in CategoryId::ALL {
            categories[category.index()] = report.categories[category.index()].map(|tokens| CategoryTokens { tokens, basis: CountBasis::Estimated });
        }
    }
    let coverage = report.and_then(|r| match r.coverage.as_deref() {
        Some("complete") => Some(Coverage::Complete),
        Some("partial") => Some(Coverage::Partial),
        _ => None,
    });
    let allocation = allocate(GridInput {
        capacity_tokens: report.and_then(|r| r.capacity_tokens), categories, coverage,
        columns: width.min(crate::native_context_grid::MAX_GRID_COLUMNS),
        rows: rows.min(crate::native_context_grid::MAX_GRID_ROWS),
    });
    let mut lines = Vec::new();
    match allocation.state {
        GridState::CapacityUnreported => lines.push(usage_line("Grid: capacity unreported; usage proportions unavailable".into(), Vec::new())),
        GridState::ZeroCapacity => lines.push(usage_line("Grid: reported capacity is zero; proportions unavailable".into(), Vec::new())),
        GridState::NoVisibleArea => lines.push(usage_line("Grid: no visible terminal cells".into(), Vec::new())),
        GridState::Allocated => {}
    }
    if let Some(over) = allocation.over_capacity_tokens {
        lines.push(usage_line(format!("Warning: estimated categories exceed capacity by {over} tokens; grid proportions normalized"), Vec::new()));
    }
    if allocation.unaccounted_capacity_tokens.is_some_and(|n| n > 0) {
        lines.push(usage_line("Unaccounted capacity remains unknown; not inferred as free space".into(), Vec::new()));
    }
    if allocation.unknown_is_coverage_marker {
        lines.push(usage_line("Unknown cell marks incomplete category coverage; not a proportional estimate".into(), Vec::new()));
    }
    if !allocation.omitted_categories.is_empty() {
        let names = allocation.omitted_categories.iter().map(|id| id.label()).collect::<Vec<_>>().join(", ");
        lines.push(usage_line(format!("Grid resolution omitted positive categories: {names}"), Vec::new()));
    }
    if allocation.columns > 0 {
        for chunk in allocation.cells.chunks(allocation.columns) {
            let mut text = String::new();
            let mut styles = Vec::with_capacity(chunk.len());
            for (column, cell) in chunk.iter().enumerate() {
                text.push('■');
                let slot = match cell { GridCell::Category(id) => category_slot(*id), GridCell::Unknown => crate::native_theme_runtime::Slot::TextMuted };
                styles.push(UsageStyleSpan { start_cell: column, cells: 1, slot });
            }
            lines.push(usage_line(text, styles));
        }
    }
    lines
}

fn fit_usage(lines: Vec<UsageLine>, width: usize) -> Vec<UsageLine> {
    lines.into_iter().map(|mut line| {
        line.text = crate::native_view_layout::fit(&line.text, width.min(u16::MAX as usize) as u16);
        line.styles.retain(|span| span.start_cell < width);
        for span in &mut line.styles { span.cells = span.cells.min(width - span.start_cell); }
        line
    }).collect()
}

fn item_detail(item: &ContextItem, width: usize) -> Vec<String> {
    let mut out = wrap(&format!("Source: {}", item.source), width);
    out.extend(wrap(&format!("Identity: {}", item.id), width));
    if let Some(provenance) = &item.count_provenance {
        out.extend(wrap(&format!("Counting: {provenance}"), width));
    }
    out.push(format!(
        "Content{}:",
        if item.truncated { " (truncated)" } else { "" }
    ));
    if item.content_available {
        out.extend(wrap(&item.content, width));
    } else {
        // Not a fake empty body: the backend withheld the item text.
        out.extend(wrap("(content not available from the daemon)", width));
    }
    for r in &item.records {
        out.extend(wrap(
            &format!(
                "record kind={} name={} call_id={}{}:",
                r.kind,
                r.name.as_deref().unwrap_or("-"),
                r.call_id.as_deref().unwrap_or("-"),
                if r.truncated { " truncated" } else { "" }
            ),
            width,
        ));
        out.extend(wrap(&r.content, width));
    }
    out
}
fn reason_label(item: &ContextItem) -> String {
    if item.kind == "checkpoint" {
        "optional whole checkpoint (indivisible)".into()
    } else if item.kind == "memory" {
        "instruction source; change selection in /memory".into()
    } else if item.role == "system" && !item.selectable {
        format!(
            "mandatory system{}",
            item.reason
                .as_deref()
                .map(|x| format!(" ({x})"))
                .unwrap_or_default()
        )
    } else if item.represented_by_checkpoint {
        "represented by checkpoint (compacted)".into()
    } else if item.reason.is_some() && !item.included {
        "excluded".into()
    } else {
        item.reason
            .clone()
            .unwrap_or_else(|| "no selection reason".into())
    }
}
fn shown(v: Option<u64>) -> String {
    v.map_or_else(|| "-".into(), |n| n.to_string())
}

/// Bounded Claude-Code-style PAGE INVENTORY over the loaded inspection page,
/// grouped by category. This is an inventory of what the page contains, NOT
/// the actual request usage sent to the provider: rows the daemon marked
/// `included == false` (excluded) or `represented_by_checkpoint == true`
/// (compacted) are omitted so they can never be counted as live request usage.
///
/// Returns one header row plus one row per non-empty category. Each category
/// is kept on its own short row so a narrow viewport cannot cell-wrap a single
/// long line and split a category label from its value (which would make it
/// unreachable). Measured items report their known `bytes / 4` estimate; an
/// unmeasured item is counted as unmeasured (`?`), never added as a fabricated
/// `0`, and an explicit provider `0` stays `0`. A mixed category prints the
/// measured subtotal plus the unmeasured item count so the total is never
/// presented as complete. Long rows are truncated by the caller's width mask
/// only after the label; the value stays adjacent to its label.
fn breakdown_lines(items: &[ContextItem]) -> Option<Vec<String>> {
    // (label, measured_tokens, measured_items, unmeasured_items)
    let mut buckets: [(&str, u64, u64, u64); 5] = [
        ("system", 0, 0, 0),
        ("conversation", 0, 0, 0),
        ("tools", 0, 0, 0),
        ("memory/files", 0, 0, 0),
        ("checkpoint", 0, 0, 0),
    ];
    let mut any = false;
    for item in items {
        if !item.included || item.represented_by_checkpoint {
            continue;
        }
        let index = match item.kind.as_str() {
            "memory" => 3,
            "tool_pair" => 2,
            "checkpoint" => 4,
            _ if item.role == "system" => 0,
            _ => 1,
        };
        any = true;
        match item.estimated_tokens {
            Some(tokens) => {
                buckets[index].1 = buckets[index].1.saturating_add(tokens);
                buckets[index].2 += 1;
            }
            None => buckets[index].3 += 1,
        }
    }
    if !any {
        return None;
    }
    let mut out = vec!["Breakdown (page inventory; excludes excluded/compacted)".to_owned()];
    for (name, tokens, measured, unmeasured) in buckets.iter() {
        if *measured == 0 && *unmeasured == 0 {
            continue;
        }
        let row = if *measured == 0 {
            format!("  {name}: ? tok · {unmeasured} unmeasured")
        } else if *unmeasured == 0 {
            format!("  {name}: {tokens} tok · {measured} items")
        } else {
            format!("  {name}: {tokens} tok · {measured} items + {unmeasured} unmeasured")
        };
        out.push(row);
    }
    Some(out)
}

fn string(o: &serde_json::Map<String, Value>, key: &str) -> Result<String, String> {
    o.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("context item is missing {key}"))
}
fn optional_u64(o: &serde_json::Map<String, Value>, key: &str) -> Result<Option<u64>, String> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a non-negative integer")),
    }
}
fn optional_bool(o: &serde_json::Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("{key} must be boolean")),
    }
}
fn validate_uuid(s: &str) -> Result<(), String> {
    if s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            (i == 8 || i == 13 || i == 18 || i == 23) == (b == b'-')
                && (i == 8 || i == 13 || i == 18 || i == 23 || b.is_ascii_hexdigit())
        })
    {
        Ok(())
    } else {
        Err("context identity is not a canonical UUID".into())
    }
}
fn validate_id_kind(id: &str, kind: &str) -> Result<(), String> {
    if id.len() > MAX_ID_BYTES || id.chars().any(char::is_control) {
        return Err("context identity is invalid".into());
    }
    match id.strip_prefix("memory:") {
        Some(path) if kind == "memory" && !path.is_empty() && path.len() <= MAX_PATH_BYTES => {
            Ok(())
        }
        Some(_) => Err("memory identity must belong to a memory item".into()),
        None => match id.strip_prefix("checkpoint:") {
            Some(uuid) if kind == "checkpoint" => validate_uuid(uuid),
            Some(_) => Err("checkpoint identity must belong to a checkpoint item".into()),
            None if matches!(kind, "message" | "tool_pair") => validate_uuid(id),
            None => Err("message identity must be a UUID".into()),
        },
    }
}
fn parse_records(
    value: Option<&Value>,
    kind: &str,
    total: &mut usize,
) -> Result<Vec<ContextRecord>, String> {
    let Some(value) = value else {
        return if kind == "tool_pair" {
            Err("tool pair is missing its call and output".into())
        } else {
            Ok(Vec::new())
        };
    };
    if kind != "tool_pair" {
        return Err("records are only valid for tool_pair items".into());
    }
    let a = value
        .as_array()
        .ok_or_else(|| "context records must be an array".to_owned())?;
    if a.len() != 2 {
        return Err("tool_pair must contain its call and output".into());
    }
    let mut out = Vec::new();
    for v in a {
        let o = v
            .as_object()
            .ok_or_else(|| "context record must be an object".to_owned())?;
        let rk = string(o, "kind")?;
        if !matches!(rk.as_str(), "call" | "output") {
            return Err("tool record kind is invalid".into());
        }
        let content = string(o, "content")?;
        if content.len() > 4096 {
            return Err("tool record preview exceeds 4096 bytes".into());
        }
        let name = optional_text(o, "name")?;
        let call_id = optional_text(o, "call_id")?;
        *total = total.saturating_add(content.len());
        out.push(ContextRecord {
            kind: rk,
            name,
            call_id,
            content,
            truncated: o.get("truncated").and_then(Value::as_bool).unwrap_or(false),
        });
    }
    if out.len() == 2
        && (out[0].kind != "call" || out[1].kind != "output" || out[0].call_id != out[1].call_id)
    {
        return Err("tool call and output records must be paired".into());
    }
    Ok(out)
}
fn optional_text(o: &serde_json::Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.len() <= MAX_RECORD_FIELD_BYTES => Ok(Some(s.clone())),
        Some(Value::String(_)) => Err(format!("record {key} exceeds 8192 bytes")),
        Some(_) => Err(format!("record {key} is invalid")),
    }
}
fn parse_projection(v: Option<&Value>) -> Result<Projection, String> {
    let Some(o) = v.and_then(Value::as_object) else {
        return Ok(Projection {
            status: "unavailable".into(),
            ..Default::default()
        });
    };
    let status = string(o, "status")?;
    if !matches!(status.as_str(), "available" | "over_budget" | "unavailable") {
        return Err("projection status is invalid".into());
    }
    Ok(Projection {
        status,
        bytes: optional_u64(o, "bytes")?,
        tokens: optional_u64(o, "estimated_tokens")?,
        typed_tail_items: optional_u64(o, "typed_tail_items")?,
        reason: o
            .get("reason")
            .and_then(Value::as_str)
            .map(|s| s.chars().take(512).collect()),
        provenance: o
            .get("provenance")
            .and_then(Value::as_str)
            .unwrap_or("stored UTF-8 bytes / 4; not measured provider usage")
            .chars()
            .take(512)
            .collect(),
    })
}
fn parse_checkpoint(v: Option<&Value>) -> Result<Option<CheckpointMeta>, String> {
    let Some(v) = v else { return Ok(None) };
    if v.is_null() {
        return Ok(None);
    };
    let o = v
        .as_object()
        .ok_or_else(|| "checkpoint metadata is invalid".to_owned())?;
    let strategy = string(o, "strategy")?;
    if !matches!(strategy.as_str(), "native" | "fallback") {
        return Err("checkpoint strategy is invalid".into());
    }
    Ok(Some(CheckpointMeta {
        strategy,
        included: o
            .get("included")
            .and_then(Value::as_bool)
            .ok_or_else(|| "checkpoint included is invalid".to_owned())?,
        boundary: string(o, "boundary_message_id").and_then(|x| validate_uuid(&x).map(|_| x))?,
        provider: string(o, "provider_id")?,
        model: string(o, "model_id")?,
    }))
}
fn parse_pair(v: Option<&Value>, a: &str, b: &str) -> Result<Option<(u64, u64)>, String> {
    let Some(o) = v.and_then(Value::as_object) else {
        return Ok(None);
    };
    Ok(Some((
        o.get(a)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{a} must be a non-negative integer"))?,
        o.get(b)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{b} must be a non-negative integer"))?,
    )))
}
fn wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for line in text.replace('\r', "").split('\n') {
        let mut cur = String::new();
        let mut cells = 0;
        crate::native_view_layout::visit_unpadded_cells(line, MAX_CONTENT_BYTES, |glyph, n| {
            if cells > 0 && cells + n > width {
                out.push(std::mem::take(&mut cur));
                cells = 0;
            }
            cur.push_str(glyph);
            cells += n;
        });
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod usage_report_self_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn unreported_zero_and_colored_categories_stay_distinct() {
        let report = parse_usage_report(Some(&json!({"model_id":"test-model","capacity_tokens":100,"reported_used_tokens":0,
            "coverage":{"complete":false,"gaps":["missing categories"]},"categories":[{"id":"messages","label":"Messages","tokens":0,"provenance":"provider_reported"},{"id":"free_space","label":"Free space","tokens":90,"provenance":"derived_from_reported_usage"}]}))).unwrap().unwrap();
        assert_eq!(report.categories[5], Some(0)); assert_eq!(report.categories[0], None);
        let grid = usage_grid(Some(&report), 20, 10);
        assert!(grid.iter().flat_map(|line| &line.styles).any(|span| span.slot == category_slot(crate::native_context_grid::CategoryId::FreeSpace)));
        let dashboard = usage_dashboard(Some(&report), 80, 10);
        for category in crate::native_context_grid::CategoryId::ALL {
            let prefix = format!("■ {}:", category.label());
            let swatch = dashboard.iter().find(|line| line.text.starts_with(&prefix))
                .and_then(|line| line.styles.first());
            assert!(swatch.is_some_and(|span| span.cells >= 1), "legend swatch missing for {}", category.label());
            assert_eq!(swatch.expect("checked above").slot, category_slot(category));
        }
        assert!(dashboard.iter().find(|line| line.text.starts_with("■ Messages:"))
            .and_then(|line| line.styles.first()).is_some_and(|span| span.cells >= 1));
        assert_eq!(grid.iter().filter(|line| line.styles.len() == 20).count(), 10);
        assert_eq!(crate::native_context_grid::percentage(u64::MAX, Some(u64::MAX)), Some(100));
        let missing = usage_grid(Some(&UsageReport { capacity_tokens: None, categories: [Some(17), None, None, None, None, None, Some(40)], ..UsageReport::default() }), 8, 10);
        assert!(missing.iter().any(|line| line.text.contains("capacity unreported")));
        assert_eq!(missing.iter().filter(|line| line.styles.len() == 8).count(), 10);
        assert!(missing.iter().flat_map(|line| &line.styles).all(|span| span.slot == crate::native_theme_runtime::Slot::TextMuted));
        assert_eq!(utilization(None, Some(0)), "unreported");
        assert!(utilization(Some(1), Some(0)).contains("unavailable"));
        assert_eq!(utilization(Some(u64::MAX), Some(u64::MAX)), "100%");
    }
    #[test]
    fn source_lists_and_text_remain_bounded() {
        let list = (0..=MAX_USAGE_ENTRIES).map(|_| serde_json::json!({"name":"tool"})).collect::<Vec<_>>();
        assert!(parse_usage_report(Some(&serde_json::json!({"mcp_tools":list}))).is_err());
        assert!(parse_usage_report(Some(&serde_json::json!({"memory_files":[{"name":"x","path":"bad\npath"}]}))).is_err());
        let max_percent = utilization(Some(u64::MAX), Some(u64::MAX));
        assert_eq!(max_percent, "100%");
    }

    #[test]
    fn usage_categories_reject_duplicate_null_and_control_values() {
        let cases = [
            serde_json::json!({"categories":[{"id":"messages","tokens":null},{"id":"messages","tokens":null}]}),
            serde_json::json!({"coverage":{"complete":false,"gaps":["unsafe\u{001b}[31m"]}}),
            serde_json::json!({"categories":[{"id":"messages","tokens":0,"label":17}]}),
        ];
        for usage in cases {
            let body = serde_json::json!({"scope":"session","items":[],"usage_report":usage});
            let mut view = ContextView::new();
            assert!(view.load_response(&body.to_string()).is_err());
        }
    }

    #[test]
    fn narrow_grid_tracks_visible_cell_budget() {
        let report = UsageReport { capacity_tokens: Some(100), categories: [Some(1), None, None, None, None, Some(99), None], ..UsageReport::default() };
        let lines = usage_grid(Some(&report), 8, 4);
        assert_eq!(lines.iter().filter(|line| line.styles.len() == 8).count(), 4);
        assert!(lines.iter().all(|line| line.styles.len() <= 8));
    }

    #[test]
    fn dashboard_header_and_all_source_sections_are_scroll_reachable() {
        let mut view = ContextView::new();
        let body = serde_json::json!({"scope":"session","items":[],"usage_report":{
            "model_id":"selected-model","capacity_tokens":100,"reported_used_tokens":35,"estimated_total_tokens":30,
            "coverage":{"complete":true,"gaps":[]},
            "categories":[{"id":"system_prompt","label":"System prompt","tokens":5,"provenance":"estimated_utf8_bytes_div_4"},{"id":"system_tools","label":"System tools","tokens":5,"provenance":"estimated_utf8_bytes_div_4"},{"id":"mcp_server_instructions","label":"MCP server instructions","tokens":5,"provenance":"estimated_utf8_bytes_div_4"},{"id":"memory_files","label":"Memory files","tokens":5,"provenance":"estimated_utf8_bytes_div_4"},{"id":"skills","label":"Skills","tokens":5,"provenance":"estimated_utf8_bytes_div_4"},{"id":"messages","label":"Messages","tokens":5,"provenance":"estimated_utf8_bytes_div_4"},{"id":"free_space","label":"Free space","tokens":65,"provenance":"derived_from_reported_usage"}],
            "mcp_tools":["search · mcp/server"],
            "memory_files":["AGENTS.md · /project/AGENTS.md"],
            "skills":["review · builtin"],
            "suggestions":[{"text":"trim context","evidence":"large memory file"}]}}).to_string();
        view.load_response(&body).unwrap();
        let mut visible = String::new();
        for _ in 0..90 {
            visible.push_str(&view.render(120, 12).join("\\n"));
            view.scroll_detail(true);
        }
        for expected in ["Context Usage", "selected-model", "Used: 35 / 100", "System prompt", "MCP tools · /mcp", "AGENTS.md", "Skills · /skills", "Suggestions", "large memory file"] {
            assert!(visible.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn reported_partial_usage_leads_with_grid_and_preserves_selected_detail() {
        let mut view = ContextView::new();
        let selected = serde_json::json!({
            "id":"00000000-0000-4000-8000-000000000026", "kind":"message", "role":"user",
            "source":"durable_history", "included":true, "selectable":true,
            "content":"message body", "content_available":false, "truncated":true,
            "bytes":40, "estimated_tokens":10,
            "count_provenance":"stored UTF-8 bytes / 4"
        });
        let body = serde_json::json!({
            "scope":"session", "items":[selected],
            "projection":{"status":"available","bytes":40,"estimated_tokens":10,"typed_tail_items":1,"provenance":"bytes / 4"},
            "usage_report": {
                "model_id":"selected-model", "capacity_tokens":1000,
                "reported_used_tokens":120,
                "categories":[{"id":"messages","tokens":120}],
                "coverage":{"complete":false,"gaps":["tool schemas unavailable"]}
            }
        }).to_string();
        view.load_response(&body).unwrap();
        let lines = view.render(80, 24);
        let grid = lines.iter().position(|line| line.contains("Estimated usage by category")).expect("grid heading");
        assert!(lines.iter().any(|line| line.contains("selected-model")));
        assert!(lines.iter().any(|line| line.contains("Coverage gaps: tool schemas unavailable")));
        assert!(lines.iter().any(|line| line.chars().filter(|c| *c == '■').count() >= 20));
        assert!(lines.iter().position(|line| line.contains("Context inventory:")).is_none_or(|inventory| grid < inventory));
        let mut seen = String::new();
        for _ in 0..80 { seen.push_str(&view.render(40, 8).join("\n")); view.scroll_detail(true); }
        assert!(seen.contains("content not available from the daemon"));
        assert!(!seen.contains("message body"));
    }

    #[test]
    fn installed_partial_report_has_initial_grid_at_page_content_geometry() {
        let mut view = ContextView::new();
        let body = json!({
            "scope":"session",
            "items":[{
                "id":"00000000-0000-4000-8000-000000000026",
                "kind":"message", "role":"user", "source":"durable_history",
                "included":true, "selectable":true,
                "content":"native provider contract first", "content_available":true,
                "bytes":30, "estimated_tokens":8,
                "count_provenance":"stored UTF-8 bytes / 4; not measured tokenizer usage"
            }],
            "projection":{
                "status":"available", "bytes":111, "estimated_tokens":28,
                "typed_tail_items":2,
                "provenance":"selected typed tail JSON plus included checkpoint JSON; excludes prospective user input, rebuilt instructions, tools and request envelope"
            },
            "usage_report":{
                "model_id":"openai/gpt-5.6-mini", "capacity_tokens":200000,
                "reported_used_tokens":null, "estimated_total_tokens":null,
                "categories":[
                    {"id":"system_prompt","tokens":0,"provenance":"estimated_utf8_bytes_div_4"},
                    {"id":"system_tools","tokens":1184,"provenance":"estimated_utf8_bytes_div_4"},
                    {"id":"mcp_server_instructions","tokens":0,"provenance":"estimated_utf8_bytes_div_4"},
                    {"id":"memory_files","tokens":0,"provenance":"estimated_utf8_bytes_div_4"},
                    {"id":"skills","tokens":null,"provenance":"unreported"},
                    {"id":"messages","tokens":14,"provenance":"estimated_utf8_bytes_div_4"},
                    {"id":"free_space","tokens":null,"provenance":"unreported"}
                ],
                "coverage":{"complete":false,"gaps":[
                    "Skills are not exposed as a distinct assembled-request source",
                    "Provider request framing, reasoning options and tool-choice envelope are not tokenized"
                ]},
                "mcp_tools":[], "memory_files":[], "skills":[], "suggestions":[]
            }
        });
        view.load_response(&body.to_string()).unwrap();
        let initial = view.render_styled(72, 19);
        let text = initial.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n");
        for expected in ["Context Usage", "openai/gpt-5.6-mini", "200000", "Coverage gaps:", "Counting:"] {
            assert!(text.contains(expected), "missing initial {expected}");
        }
        let grid = initial.iter().position(|line| line.text.contains("Estimated usage by category")).expect("initial grid heading");
        assert!(initial.iter().skip(grid + 1).any(|line| line.text.contains('■') && line.styles.iter().any(|span| span.cells > 0)), "initial grid must contain visible styled cells");
        assert!(initial.iter().position(|line| line.text.contains("Context inventory:")).is_none_or(|inventory| grid < inventory));
        assert!(!text.contains("Estimated usage total:"));
        let mut reachable = String::new();
        for _ in 0..100 {
            reachable.push_str(&view.render(72, 19).join("\n"));
            view.scroll_detail(true);
        }
        for expected in ["Skills: unreported", "Free space: unreported", "Context inventory:", "native provider contract first", "MCP tools", "Memory files", "Suggestions"] {
            assert!(reachable.contains(expected), "missing scroll-reachable {expected}");
        }
        assert!(!reachable.contains("Estimated usage total:"));
    }

    #[test]
    fn estimated_total_is_visible_when_provider_usage_is_unreported() {
        let mut view = ContextView::new();
        let body = serde_json::json!({
            "scope":"session", "items": [], "usage_report": {
                "model_id":"provider/model", "capacity_tokens":1000,
                "reported_used_tokens":null, "estimated_total_tokens":123,
                "categories":[], "coverage":{"complete":false,"gaps":["provider usage unavailable"]}
            }
        }).to_string();
        view.load_response(&body).unwrap();
        let rendered = view.render(120, 12).join("\n");
        assert!(rendered.contains("Estimated usage total: 123 tokens"));
        assert!(rendered.contains("provider usage unreported"));
        assert!(rendered.contains("Coverage gaps: provider usage unavailable"));
    }
}
