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

#[derive(Debug, Default)]
pub(crate) struct ContextView {
    items: Vec<ContextItem>,
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
        let checkpoint = parse_checkpoint(object.get("checkpoint"))?;
        let page_budget = parse_pair(
            object.get("page_budget"),
            "stored_message_bytes",
            "preview_bytes",
        )?;
        let input_limits = parse_pair(object.get("input_limits"), "items", "bytes")?;
        self.items = parsed;
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
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let mut rows = Vec::new();
        rows.push(format!(
            "Context · {} items · projection {}",
            self.items.len(),
            self.projection.status
        ));
        if let Some(item) = self.selected() {
            rows.push(format!(
                "{} {} · {} · {} · {}",
                if item.selectable { "▸" } else { "•" },
                item.kind,
                item.role,
                if item.represented_by_checkpoint {
                    "compacted"
                } else if item.included {
                    "included"
                } else {
                    "excluded"
                },
                item.source
            ));
            rows.push(format!(
                "{} bytes / {} tokens · {}",
                shown(item.bytes),
                shown(item.estimated_tokens),
                reason_label(item)
            ));
        } else {
            rows.push("Context unloaded; authenticated session GET required".into());
        }
        if height <= 2 && self.selected().is_some() {
            rows.remove(0);
        }
        if let Some(e) = &self.error {
            rows.push(format!("Refresh failed; showing last loaded context: {e}"));
        }
        let footer = "j/k select · Enter include/exclude · f/b detail · p older · r latest";
        let reserve_footer = height >= 4;
        let detail_height = height.saturating_sub(rows.len() + usize::from(reserve_footer));
        let mut detail: Vec<String> = self
            .metadata_lines()
            .into_iter()
            .flat_map(|line| wrap(&line, width.max(1)))
            .collect();
        if let Some(item) = self.selected() {
            detail.extend(item_detail(item, width.max(1)));
        }
        let start = self.detail_offset.min(detail.len());
        rows.extend(detail.into_iter().skip(start).take(detail_height));
        if reserve_footer {
            rows.push(footer.into());
        }
        rows.into_iter()
            .take(height)
            .map(|s| crate::native_view_layout::fit(&s, width.min(u16::MAX as usize) as u16))
            .collect()
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
