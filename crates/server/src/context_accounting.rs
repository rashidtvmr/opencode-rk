//! LANE-CONTEXT-ACCOUNT: bounded context accounting service (roadmap 3.3).
//!
//! Given the typed pieces of what a provider round carries (system prompt
//! bytes, rules segment, history message bytes, tool schemas, current turn),
//! produces a per-segment token estimate (documented chars/4 estimator, no
//! tokenizer dep), total vs a model context limit, compaction headroom flag
//! with threshold, and a bounded snapshot struct (the /CONTEXT render source).
//!
//! Deterministic, cap on history pieces counted (documented), saturating math
//! (no overflow). Standalone std-only module.

#![forbid(unsafe_code)]

/// Maximum history message pieces counted. When more exist, later pieces
/// are dropped and [`ContextAccountReport::truncated`] is set.
pub const MAX_HISTORY_PIECES: usize = 256;

/// Token estimator: one token ≈ 4 bytes of UTF-8 text.
///
/// This is a well-known heuristic for English text; exact counts depend on
/// the tokenizer but this keeps the module self-contained and deterministic.
#[must_use]
pub fn estimate_tokens(byte_len: usize) -> u64 {
    (byte_len / 4) as u64
}

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// One history message piece for token estimation.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct HistoryPiece {
    pub bytes: usize,
}

/// Minimal turn-state input for [`build_account_report`].
///
/// Every field is optional (absent → zero). The caller supplies raw byte
/// lengths; tokens are estimated internally.
#[derive(Clone, Debug, Default)]
pub struct AccountInput {
    pub system_prompt_bytes: usize,
    pub rules_bytes: usize,
    pub history_messages: Vec<HistoryPiece>,
    pub tool_schemas_bytes: Vec<usize>,
    pub current_turn_bytes: usize,
    pub model_limit: u64,
    pub compaction_threshold: f64,
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// The kind of context segment.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SegmentKind {
    System,
    Rules,
    History,
    ToolSchemas,
    CurrentTurn,
}

/// A single token-counted segment of the context window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    pub kind: SegmentKind,
    pub tokens: u64,
    pub bytes: usize,
}

/// Failures for [`build_account_report`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountError {
    /// Model limit is zero (cannot compute headroom).
    ZeroModelLimit,
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroModelLimit => write!(f, "model_limit must be greater than zero"),
        }
    }
}

impl std::error::Error for AccountError {}

/// Bounded snapshot of context-window token usage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextAccountReport {
    pub total_tokens: u64,
    pub model_limit: u64,
    pub segments: Vec<Segment>,
    /// True when the source had more history pieces than [`MAX_HISTORY_PIECES`].
    pub truncated: bool,
    /// Tokens remaining below the model limit after all included segments.
    pub headroom: u64,
    /// True when token usage exceeds [`AccountInput::compaction_threshold`]
    /// of the model limit, signalling the UI should offer compaction.
    pub needs_compaction: bool,
}

/// Serializable snapshot for /CONTEXT render.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContextAccountSnapshot {
    pub total_tokens: u64,
    pub model_limit: u64,
    pub segments: Vec<SnapshotSegment>,
    pub truncated: bool,
    pub headroom: u64,
    pub needs_compaction: bool,
}

/// Snapshot segment (string kind for JSON).
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotSegment {
    pub kind: String,
    pub tokens: u64,
    pub bytes: usize,
}

impl ContextAccountReport {
    /// Produce a serializable snapshot.
    pub fn snapshot(&self) -> String {
        let snap = ContextAccountSnapshot {
            total_tokens: self.total_tokens,
            model_limit: self.model_limit,
            segments: self
                .segments
                .iter()
                .map(|s| SnapshotSegment {
                    kind: format!("{:?}", s.kind),
                    tokens: s.tokens,
                    bytes: s.bytes,
                })
                .collect(),
            truncated: self.truncated,
            headroom: self.headroom,
            needs_compaction: self.needs_compaction,
        };
        serde_json::to_string(&snap).expect("snapshot serialization")
    }
}

impl ContextAccountSnapshot {
    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Result<Self, AccountError> {
        serde_json::from_str(json).map_err(|_| AccountError::ZeroModelLimit)
    }
}

/// Build a bounded [`ContextAccountReport`] from an [`AccountInput`].
///
/// Segments are emitted in stable order: System, Rules, History (one segment
/// per piece, capped at [`MAX_HISTORY_PIECES`]), ToolSchemas (one per entry),
/// CurrentTurn. When more than [`MAX_HISTORY_PIECES`] exist, later pieces are
/// dropped and `truncated` is set.
///
/// Token estimation uses the documented `bytes / 4` heuristic.
///
/// # Errors
///
/// Returns [`AccountError::ZeroModelLimit`] if `model_limit` is zero.
pub fn build_account_report(input: &AccountInput) -> Result<ContextAccountReport, AccountError> {
    if input.model_limit == 0 {
        return Err(AccountError::ZeroModelLimit);
    }

    let model_limit = input.model_limit;
    let mut segments: Vec<Segment> = Vec::new();

    // 1. System
    if input.system_prompt_bytes > 0 {
        segments.push(Segment {
            kind: SegmentKind::System,
            tokens: estimate_tokens(input.system_prompt_bytes),
            bytes: input.system_prompt_bytes,
        });
    }

    // 2. Rules
    if input.rules_bytes > 0 {
        segments.push(Segment {
            kind: SegmentKind::Rules,
            tokens: estimate_tokens(input.rules_bytes),
            bytes: input.rules_bytes,
        });
    }

    // 3. History (capped at MAX_HISTORY_PIECES)
    let history_count = input.history_messages.len().min(MAX_HISTORY_PIECES);
    let truncated = input.history_messages.len() > MAX_HISTORY_PIECES;
    for piece in &input.history_messages[..history_count] {
        segments.push(Segment {
            kind: SegmentKind::History,
            tokens: estimate_tokens(piece.bytes),
            bytes: piece.bytes,
        });
    }

    // 4. Tool schemas (one per entry, zero-byte entries skipped)
    for &bytes in &input.tool_schemas_bytes {
        if bytes > 0 {
            segments.push(Segment {
                kind: SegmentKind::ToolSchemas,
                tokens: estimate_tokens(bytes),
                bytes,
            });
        }
    }

    // 5. CurrentTurn
    if input.current_turn_bytes > 0 {
        segments.push(Segment {
            kind: SegmentKind::CurrentTurn,
            tokens: estimate_tokens(input.current_turn_bytes),
            bytes: input.current_turn_bytes,
        });
    }

    let total_tokens: u64 = segments
        .iter()
        .fold(0u64, |acc, s| acc.saturating_add(s.tokens));
    let headroom = model_limit.saturating_sub(total_tokens);

    // Compaction threshold: needs_compaction when total/limit > threshold
    let usage_ratio = if model_limit > 0 {
        total_tokens as f64 / model_limit as f64
    } else {
        1.0
    };
    let needs_compaction = usage_ratio > input.compaction_threshold;

    Ok(ContextAccountReport {
        total_tokens,
        model_limit,
        segments,
        truncated,
        headroom,
        needs_compaction,
    })
}


// Extension contract: call only with component byte lengths from the actual
// assembled provider request. Do not construct these values from context_http's
// paginated history or memory inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageCategoryId {
    SystemPrompt,
    SystemTools,
    McpServerInstructions,
    MemoryFiles,
    Skills,
    Messages,
    FreeSpace,
}

impl UsageCategoryId {
    fn label(self) -> &'static str {
        match self {
            Self::SystemPrompt => "System prompt",
            Self::SystemTools => "System tools",
            Self::McpServerInstructions => "MCP server instructions",
            Self::MemoryFiles => "Memory files",
            Self::Skills => "Skills",
            Self::Messages => "Messages",
            Self::FreeSpace => "Free space",
        }
    }
}

pub const MAX_USAGE_COMPONENTS: usize = 512;
pub const MAX_USAGE_DETAILS: usize = 32;
pub const MAX_USAGE_DETAIL_BYTES: usize = 256;
pub const MAX_USAGE_COMPONENT_ID_BYTES: usize = 256;

/// One non-overlapping component in the assembled request. `id` is the request
/// slot identity, not a content hash: repeated content in distinct request
/// slots is counted twice because the provider receives it twice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageComponent {
    pub id: String,
    pub category: UsageCategoryId,
    pub bytes: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct UsageSuggestion {
    pub text: String,
    /// Evidence identifier/path observed in this request; required to display.
    pub evidence: String,
}

/// Values captured at the request assembly boundary. A category omitted from
/// `captured_categories` is unreported, not zero. `Some(0)` bytes is an
/// explicitly observed empty category.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssembledUsageInput {
    pub model_id: Option<String>,
    pub capacity_tokens: Option<u64>,
    /// Provider's inclusive input token count; cache details are subsets and
    /// reasoning tokens are output details, so neither is added here.
    pub reported_used_tokens: Option<u64>,
    pub components: Vec<UsageComponent>,
    pub captured_categories: Vec<UsageCategoryId>,
    pub coverage_gaps: Vec<String>,
    pub mcp_tools: Vec<String>,
    pub memory_files: Vec<String>,
    pub skills: Vec<String>,
    pub suggestions: Vec<UsageSuggestion>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct UsageCategory {
    pub id: UsageCategoryId,
    pub label: &'static str,
    pub tokens: Option<u64>,
    /// `estimated_utf8_bytes_div_4`, `provider_reported`,
    /// `derived_from_reported_usage`, `derived_from_estimate`, or `unreported`.
    pub provenance: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct UsageReportCoverage {
    pub complete: bool,
    pub gaps: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ContextUsageReport {
    pub model_id: Option<String>,
    pub capacity_tokens: Option<u64>,
    pub reported_used_tokens: Option<u64>,
    pub estimated_total_tokens: Option<u64>,
    pub categories: Vec<UsageCategory>,
    pub mcp_tools: Vec<String>,
    pub memory_files: Vec<String>,
    pub skills: Vec<String>,
    pub suggestions: Vec<UsageSuggestion>,
    pub coverage: UsageReportCoverage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UsageReportError {
    TooManyComponents,
    TooManyDetails,
    OversizedDetail,
    DuplicateComponentId,
    DuplicateCapturedCategory,
    UnknownComponentCategory,
}

impl std::fmt::Display for UsageReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid assembled usage snapshot: {self:?}")
    }
}

impl std::error::Error for UsageReportError {}

fn validate_details(values: &[String]) -> Result<(), UsageReportError> {
    if values.len() > MAX_USAGE_DETAILS {
        return Err(UsageReportError::TooManyDetails);
    }
    if values.iter().any(|value| value.len() > MAX_USAGE_DETAIL_BYTES) {
        return Err(UsageReportError::OversizedDetail);
    }
    Ok(())
}

/// Convert a real assembly snapshot into a JSON-ready usage report. Estimate
/// each category from aggregate UTF-8 bytes / 4. Partial categories remain
/// visible; the total is absent unless all six request categories are captured.
pub fn build_usage_report(
    input: &AssembledUsageInput,
) -> Result<ContextUsageReport, UsageReportError> {
    if input.components.len() > MAX_USAGE_COMPONENTS {
        return Err(UsageReportError::TooManyComponents);
    }
    validate_details(&input.mcp_tools)?;
    validate_details(&input.memory_files)?;
    validate_details(&input.skills)?;
    validate_details(&input.coverage_gaps)?;
    if input.model_id.as_ref().is_some_and(|value| value.len() > MAX_USAGE_DETAIL_BYTES) {
        return Err(UsageReportError::OversizedDetail);
    }
    if input.suggestions.len() > MAX_USAGE_DETAILS {
        return Err(UsageReportError::TooManyDetails);
    }
    for suggestion in &input.suggestions {
        validate_details(std::slice::from_ref(&suggestion.text))?;
        validate_details(std::slice::from_ref(&suggestion.evidence))?;
    }

    let request_categories = [
        UsageCategoryId::SystemPrompt,
        UsageCategoryId::SystemTools,
        UsageCategoryId::McpServerInstructions,
        UsageCategoryId::MemoryFiles,
        UsageCategoryId::Skills,
        UsageCategoryId::Messages,
    ];
    let mut seen_categories = std::collections::BTreeSet::new();
    for category in &input.captured_categories {
        if !request_categories.contains(category) {
            return Err(UsageReportError::UnknownComponentCategory);
        }
        if !seen_categories.insert(*category) {
            return Err(UsageReportError::DuplicateCapturedCategory);
        }
    }
    let mut component_ids = std::collections::BTreeSet::new();
    let mut bytes = std::collections::BTreeMap::<UsageCategoryId, usize>::new();
    let mut unknown = std::collections::BTreeSet::<UsageCategoryId>::new();
    for component in &input.components {
        if component.id.is_empty() || component.id.len() > MAX_USAGE_COMPONENT_ID_BYTES {
            return Err(UsageReportError::OversizedDetail);
        }
        if !request_categories.contains(&component.category) {
            return Err(UsageReportError::UnknownComponentCategory);
        }
        if !component_ids.insert(component.id.as_str()) {
            return Err(UsageReportError::DuplicateComponentId);
        }
        match component.bytes {
            Some(count) => {
                let total = bytes.entry(component.category).or_default();
                *total = total.saturating_add(count);
            }
            None => {
                unknown.insert(component.category);
            }
        }
    }

    let mut categories = Vec::with_capacity(7);
    let mut total = Some(0_u64);
    for id in request_categories {
        let tokens = if !seen_categories.contains(&id) || unknown.contains(&id) {
            None
        } else {
            Some(estimate_tokens(bytes.get(&id).copied().unwrap_or(0)))
        };
        if let Some(value) = tokens {
            if let Some(sum) = &mut total {
                *sum = sum.saturating_add(value);
            }
        } else {
            total = None;
        }
        categories.push(UsageCategory {
            id,
            label: id.label(),
            tokens,
            provenance: if tokens.is_some() {
                "estimated_utf8_bytes_div_4"
            } else {
                "unreported"
            },
        });
    }

    let complete = total.is_some() && input.coverage_gaps.is_empty();
    let estimated_total_tokens = complete.then_some(total).flatten();
    let free_basis = input
        .reported_used_tokens
        .map(|value| (value, "derived_from_reported_usage"))
        .or_else(|| estimated_total_tokens.map(|value| (value, "derived_from_estimate")));
    let free_tokens = input
        .capacity_tokens
        .zip(free_basis)
        .map(|(capacity, (used, _))| capacity.saturating_sub(used));
    let free_provenance = if let Some((_, provenance)) = free_basis {
        provenance
    } else {
        "unreported"
    };
    categories.push(UsageCategory {
        id: UsageCategoryId::FreeSpace,
        label: UsageCategoryId::FreeSpace.label(),
        tokens: free_tokens,
        provenance: free_provenance,
    });

    Ok(ContextUsageReport {
        model_id: input.model_id.clone(),
        capacity_tokens: input.capacity_tokens,
        reported_used_tokens: input.reported_used_tokens,
        estimated_total_tokens,
        categories,
        mcp_tools: input.mcp_tools.clone(),
        memory_files: input.memory_files.clone(),
        skills: input.skills.clone(),
        // No savings estimate exists in this contract. Display only evidence-backed text.
        suggestions: input
            .suggestions
            .iter()
            .filter(|suggestion| !suggestion.evidence.trim().is_empty())
            .cloned()
            .collect(),
        coverage: UsageReportCoverage {
            complete,
            gaps: input.coverage_gaps.clone(),
        },
    })
}

#[cfg(test)]
mod usage_report_tests {
    use super::*;

    fn complete_input() -> AssembledUsageInput {
        let categories = [
            UsageCategoryId::SystemPrompt,
            UsageCategoryId::SystemTools,
            UsageCategoryId::McpServerInstructions,
            UsageCategoryId::MemoryFiles,
            UsageCategoryId::Skills,
            UsageCategoryId::Messages,
        ];
        AssembledUsageInput {
            model_id: Some("provider/model".into()),
            capacity_tokens: Some(1000),
            reported_used_tokens: Some(120),
            captured_categories: categories.to_vec(),
            components: vec![UsageComponent {
                id: "messages.0".into(),
                category: UsageCategoryId::Messages,
                bytes: Some(400),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn explicit_zero_differs_from_missing_and_partial_total_is_absent() {
        let mut input = complete_input();
        input.components.push(UsageComponent {
            id: "system.0".into(),
            category: UsageCategoryId::SystemPrompt,
            bytes: Some(0),
        });
        input.captured_categories.retain(|id| *id != UsageCategoryId::Skills);
        let report = build_usage_report(&input).unwrap();
        assert_eq!(report.categories[0].tokens, Some(0));
        assert_eq!(report.categories[4].tokens, None);
        assert_eq!(report.estimated_total_tokens, None);
        assert_eq!(report.reported_used_tokens, Some(120));
        assert_eq!(report.categories[6].tokens, Some(880));
        assert!(!report.coverage.complete);
    }

    #[test]
    fn inclusive_provider_usage_does_not_add_cache_or_reasoning_again() {
        let mut input = complete_input();
        // Cached input and reasoning are reported subsets/details, not additive usage.
        let report = build_usage_report(&input).unwrap();
        assert_eq!(report.reported_used_tokens, Some(120));
        assert_eq!(report.estimated_total_tokens, Some(100));
        assert_eq!(report.categories[6].tokens, Some(880));
        input.reported_used_tokens = None;
        let estimate_only = build_usage_report(&input).unwrap();
        assert_eq!(estimate_only.categories[6].tokens, Some(900));
        assert_eq!(estimate_only.categories[6].provenance, "derived_from_estimate");
    }

    #[test]
    fn missing_inserted_memory_is_unknown_but_absent_memory_is_explicit_zero() {
        use opencode_rk_providers::responses::{ResponsesItem, ResponsesRole};

        let input = [ResponsesItem::Text {
            role: ResponsesRole::User,
            content: "request".into(),
        }];
        let injected_but_unrecognized = capture_assembled_request(
            "provider/model", Some(4096), &input, &[], true, &[], &[], false, false,
        );
        let injected_report = build_usage_report(&injected_but_unrecognized).unwrap();
        let memory_component = injected_but_unrecognized.components.iter()
            .find(|component| component.id == "input.memory").unwrap();
        assert_eq!(memory_component.bytes, None);
        assert_eq!(injected_report.categories[3].tokens, None);
        assert!(!injected_report.coverage.complete);
        assert!(injected_report.coverage.gaps.iter()
            .any(|gap| gap == "Inserted memory prompt could not be attributed to its request component"));

        let absent = capture_assembled_request(
            "provider/model", Some(4096), &input, &[], false, &[], &[], false, false,
        );
        let absent_report = build_usage_report(&absent).unwrap();
        let memory_component = absent.components.iter()
            .find(|component| component.id == "input.memory").unwrap();
        assert_eq!(memory_component.bytes, Some(0));
        assert_eq!(absent_report.categories[3].tokens, Some(0));
        assert_eq!(absent_report.categories[3].provenance, "estimated_utf8_bytes_div_4");
    }

    #[test]
    fn duplicate_request_component_identity_is_rejected() {
        let mut input = complete_input();
        input.components.push(input.components[0].clone());
        assert_eq!(
            build_usage_report(&input),
            Err(UsageReportError::DuplicateComponentId)
        );
    }
}


/// Capture byte lengths from the final typed input and tool schemas immediately
/// before provider dispatch. Stores sizes and bounded names only, never prompt
/// text, credentials, tool arguments or responses.
pub fn capture_assembled_request(
    model_id: &str,
    capacity_tokens: Option<u64>,
    input: &[opencode_rk_providers::responses::ResponsesItem],
    tools: &[opencode_rk_providers::responses::ResponsesTool],
    memory_prompt_inserted: bool,
    mcp_tools: &[String],
    memory_files: &[String],
    memory_paths_truncated: bool,
    compaction_window: bool,
) -> AssembledUsageInput {
    use opencode_rk_providers::responses::{ResponsesItem, ResponsesRole};

    let mut captured_categories = vec![
        UsageCategoryId::Messages,
        UsageCategoryId::SystemTools,
        UsageCategoryId::McpServerInstructions,
        UsageCategoryId::SystemPrompt,
    ];
    let mut components = Vec::with_capacity(5);
    let mut input_bytes = [Some(0usize); 3]; // messages, system prompt, memory prompt
    let mut memory_seen = false;
    let mut compaction_item_seen = false;
    for (index, item) in input.iter().enumerate() {
        let slot = match item {
            ResponsesItem::Text { role: ResponsesRole::System, .. }
                if memory_prompt_inserted && index == 0 => {
                    memory_seen = true;
                    2
                }
            ResponsesItem::Text { role: ResponsesRole::System, .. } => 1,
            _ => 0,
        };
        let bytes = if matches!(item, ResponsesItem::Compaction { .. }) {
            compaction_item_seen = true;
            None
        } else {
            serde_json::to_vec(item).ok().map(|value| value.len())
        };
        input_bytes[slot] = match (input_bytes[slot], bytes) {
            (Some(total), Some(bytes)) => Some(total.saturating_add(bytes)),
            _ => None,
        };
    }
    if memory_prompt_inserted && !memory_seen {
        input_bytes[2] = None;
    }
    components.extend([
        UsageComponent { id: "input.messages".into(), category: UsageCategoryId::Messages, bytes: input_bytes[0] },
        UsageComponent { id: "input.system".into(), category: UsageCategoryId::SystemPrompt, bytes: input_bytes[1] },
        UsageComponent { id: "input.memory".into(), category: UsageCategoryId::MemoryFiles, bytes: input_bytes[2] },
    ]);
    if !memory_prompt_inserted || memory_seen {
        captured_categories.push(UsageCategoryId::MemoryFiles);
    }

    // Count each serialized tool definition once. MCP descriptions and schemas
    // are provider-visible server instructions; builtin schemas are system tools.
    let mut builtin_tool_bytes = Some(0usize);
    let mut mcp_tool_bytes = Some(0usize);
    for tool in tools.iter().take(MAX_USAGE_COMPONENTS) {
        let size = serde_json::to_vec(tool).ok().map(|value| value.len());
        let is_mcp = mcp_tools.iter().any(|name| name == &tool.name);
        let target = if is_mcp { &mut mcp_tool_bytes } else { &mut builtin_tool_bytes };
        *target = match (*target, size) {
            (Some(total), Some(size)) => Some(total.saturating_add(size)),
            _ => None,
        };
    }
    if tools.len() > MAX_USAGE_COMPONENTS {
        builtin_tool_bytes = None;
        mcp_tool_bytes = None;
    }
    components.push(UsageComponent { id: "tools.builtin".into(),
        category: UsageCategoryId::SystemTools, bytes: builtin_tool_bytes });
    components.push(UsageComponent { id: "tools.mcp".into(),
        category: UsageCategoryId::McpServerInstructions, bytes: mcp_tool_bytes });

    let mut gaps = vec![
        "Skills are not exposed as a distinct assembled-request source".to_owned(),
        "Provider request framing, reasoning options and tool-choice envelope are not tokenized".to_owned(),
    ];
    if memory_prompt_inserted && !memory_seen {
        gaps.push("Inserted memory prompt could not be attributed to its request component".into());
    }
    if !memory_prompt_inserted && !memory_files.is_empty() {
        gaps.push("Memory source paths were returned without a corresponding inserted prompt".into());
    }
    if memory_paths_truncated || memory_files.iter().any(|path| path.len() > MAX_USAGE_DETAIL_BYTES) {
        gaps.push("Memory source path details are truncated to configured count/byte limits".into());
    }
    if compaction_window {
        gaps.push("Opaque provider compaction window is sent with the tail but its token bytes are unavailable".into());
    }
    if compaction_item_seen {
        gaps.push("Opaque ResponsesItem::Compaction payload is present; its token bytes are unavailable".into());
    }
    if mcp_tools.len() > MAX_USAGE_DETAILS || mcp_tools.iter().any(|name| name.len() > MAX_USAGE_DETAIL_BYTES) {
        gaps.push("MCP tool name details are truncated to configured count/byte limits".into());
    }
    if model_id.len() > MAX_USAGE_DETAIL_BYTES {
        gaps.push("Model identity is truncated to the detail byte limit".into());
    }
    if tools.len() > MAX_USAGE_COMPONENTS {
        gaps.push("Tool schema count exceeds accounting component bound".into());
    }
    AssembledUsageInput {
        model_id: Some(bounded_detail(model_id, MAX_USAGE_DETAIL_BYTES)),
        capacity_tokens,
        reported_used_tokens: None,
        components,
        captured_categories,
        coverage_gaps: gaps,
        mcp_tools: mcp_tools.iter().take(MAX_USAGE_DETAILS).map(|name| bounded_detail(name, MAX_USAGE_DETAIL_BYTES)).collect(),
        memory_files: memory_files.iter().take(MAX_USAGE_DETAILS).map(|path| bounded_detail(path, MAX_USAGE_DETAIL_BYTES)).collect(),
        skills: Vec::new(),
        suggestions: Vec::new(),
    }
}

const MAX_LATEST_USAGE_SESSIONS: usize = 256;
static NEXT_USAGE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LATEST_USAGE: std::sync::OnceLock<std::sync::Mutex<
    std::collections::BTreeMap<opencode_rk_contracts::SessionId, (u64, ContextUsageReport)>,
>> = std::sync::OnceLock::new();

fn bounded_detail(value: &str, limit: usize) -> String {
    let end = value.char_indices()
        .take_while(|(index, character)| index.saturating_add(character.len_utf8()) <= limit)
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0);
    value[..end].to_owned()
}

/// Process-local, bounded latest assembled request snapshot. Lock poisoning or
/// report validation failure degrades to unreported data, never turn failure.
pub fn publish_usage_report(
    session_id: opencode_rk_contracts::SessionId,
    input: &AssembledUsageInput,
    reported_used_tokens: Option<u64>,
) {
    let mut input = input.clone();
    input.reported_used_tokens = reported_used_tokens;
    let Ok(report) = build_usage_report(&input) else { return };
    let snapshots = LATEST_USAGE.get_or_init(Default::default);
    let Ok(mut snapshots) = snapshots.lock() else { return };
    if !snapshots.contains_key(&session_id) && snapshots.len() >= MAX_LATEST_USAGE_SESSIONS {
        if let Some(oldest) = snapshots.iter().min_by_key(|(_, (sequence, _))| sequence)
            .map(|(id, _)| *id) {
            snapshots.remove(&oldest);
        }
    }
    let sequence = NEXT_USAGE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    snapshots.insert(session_id, (sequence, report));
}

pub fn latest_usage_report(
    session_id: opencode_rk_contracts::SessionId,
) -> Option<ContextUsageReport> {
    LATEST_USAGE.get()?.lock().ok()?.get(&session_id).map(|(_, report)| report.clone())
}
