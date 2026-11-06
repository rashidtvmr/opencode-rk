#![forbid(unsafe_code)]
//! Message-row model for the session transcript: role gutter, row timing,
//! grouping of consecutive parts, part/group anchors with their scroll math,
//! tail-first mount budgets and history page boundaries.
//!
//! Source of truth: OpenCode TUI pin
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22`, inspected in
//! `packages/tui/src/`:
//!
//! - `routes/session/grouping/tree.ts:71-100` `groupEntries`: adjacent entries
//!   nest by path; a group header counts zero rows and `size` counts leaves.
//! - `routes/session/grouping/tree.ts:118-150` `mergeGroups` / `splitGroups`:
//!   seam-merge compatible groups, split at a depth-first leaf offset.
//! - `routes/session/grouping/session.ts:57-71` `projectEntries`: a group is
//!   completed when the next row is a group or does not close the previous,
//!   or, for reasoning, when every leaf reasoning part has a completion time.
//! - `routes/session/grouping/session.ts:90-104` `partPath`: questions stand
//!   alone; thoughts group as `reasoning`; read/glob/grep/webfetch/websearch
//!   group as `exploration`; Low wraps non-text parts in `activity`.
//! - `routes/session/grouping/session.ts:107-116` `messagePath`,
//!   `instructionPaths`: instruction loads group, other messages stand alone.
//! - `routes/session/grouping/session.ts:120-170` `append` /
//!   `completePrevious`: merge with the previous same-kind group, keep
//!   permission-pending leaves at the end, and tighten `completed` for
//!   reasoning.
//! - `routes/session/grouping/session.ts:172-207` `groupRefs`, `hasPart`,
//!   `partitionPending`: blocked exploration tools move after their group's
//!   other children; `pending` is filtered from the group's own leaf refs.
//! - `routes/session/rows.ts:343-411` `reduceSessionRows`: message ordering,
//!   pending-input/compaction placement, `.trim()` suppression of empty
//!   text/reasoning parts, text/reasoning ordinals, terminal footer rows and
//!   legacy turn flushing.
//! - `routes/session/rows.ts:325-341` `rowKey`: stable row identity for
//!   reconciliation (`groupID` for groups, message/part ids otherwise).
//! - `routes/session/rows.ts:413-492` `cacheReuseDrop`, `turnDuration`,
//!   `turnTokensPerSecond`, `legacyTurns`, `inputIndex`, `hasTokenUsage`.
//! - `routes/session/rows.ts:494-538` message boundary ids and
//!   `resolvePart` part identity.
//! - `routes/session/anchors.ts:6-85`: anchor keys, `entryRef`, `groupID`,
//!   `containsAnchor`, the mounted-anchor registry and `messagePositions`.
//! - `routes/session/index.tsx:553-620`: saved scroll anchor selection and
//!   restore math; `:524-531` away-from-bottom; `:847-875` message
//!   navigation; `:1472-1487` disclosure anchor hold.
//! - `routes/session/message-navigation.ts:8-48`: navigation slack and
//!   next/prev message boundary search.
//! - `routes/session/history.ts:1-61`: `createHistoryPrepend` coalescing,
//!   cancellation and post-layout scroll compensation.
//! - `routes/session/mount-budget.ts:10-35`: `rowWeight`, `rowsBefore`,
//!   `rowsAfter`.
//! - `routes/session/index.tsx:146-147` tail mount constants; `:1675-1681`
//!   one-row top margin per transcript row.
//! - `routes/session/group-view.tsx:224,279,383`: expanded activity gaps,
//!   selection guard on disclosure clicks, thought entry margin.
//! - `routes/session/index.tsx:2382-2427`: user card one-cell `┃` rail,
//!   two-cell left padding, one-row vertical padding, hover background.
//! - `routes/session/message-parts.tsx:77,184,222` and
//!   `routes/session/index.tsx:2038,2050,2142`: every non-user row uses a
//!   three-cell content gutter (`paddingLeft={3}` / `marginLeft={3}`).
//! - `routes/session/index.tsx:2055-2066`: assistant footer slots at widths
//!   27/28/35/36 (model at >= 28, duration below 28 or from 36).
//!
//! The model is pure: no clock, no IO, no rendering. Callers supply real
//! message facts and viewport geometry.

use std::collections::BTreeMap;

use crate::native_view_chat::{ChatMessage, ChatPart, ChatRole};

/// One-cell top margin before each top-level transcript row
/// (`routes/session/index.tsx:1680`).
pub const ROW_MARGIN_TOP: u16 = 1;
/// One-cell left rail glyph for user cards (`ui/border.ts` `SplitBorder`).
pub const USER_RAIL_CHAR: char = '┃';
/// Content column shared by user cards (1 rail + 2 padding) and every
/// non-user row (`paddingLeft={3}`).
pub const CONTENT_GUTTER: u16 = 3;
/// Newest rows mounted when a session opens (`index.tsx:146`).
pub const TRANSCRIPT_TAIL_ROWS: usize = 40;
/// Rows mounted per older/newer backfill step (`index.tsx:147`).
pub const TRANSCRIPT_BACKFILL_CHUNK: usize = 60;

// ---------------------------------------------------------------------------
// Verbosity, grouping mode and disclosure
// ---------------------------------------------------------------------------

/// Transcript detail level (`config/index.tsx:164-168`, cycle order
/// `low -> medium -> high -> low` at `routes/session/index.tsx:1206-1208`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Verbosity {
    Low,
    #[default]
    Medium,
    High,
}

impl Verbosity {
    /// Next level in the upstream cycle.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Low,
        }
    }
}

/// `config.session.grouping` (`auto` groups, `none` renders parts alone;
/// `config/index.tsx:161-164`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grouping {
    #[default]
    Auto,
    None,
}

/// `config.session.thinking` (`config/index.tsx:155-157`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThinkingMode {
    #[default]
    Hide,
    Show,
}

/// Group nesting kinds (`routes/session/grouping/session.ts:24`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GroupKind {
    Activity,
    Reasoning,
    Exploration,
    Instructions,
}

impl GroupKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Activity => "activity",
            Self::Reasoning => "reasoning",
            Self::Exploration => "exploration",
            Self::Instructions => "instructions",
        }
    }
}

/// Whether a group kind is currently rendered as a summary header
/// (`routes/session/index.tsx:253-254`): reasoning only while thinking is
/// hidden, exploration only while grouping is enabled, everything else always.
#[must_use]
pub fn grouped_kind(kind: GroupKind, thinking: ThinkingMode, grouping: Grouping) -> bool {
    match kind {
        GroupKind::Reasoning => thinking == ThinkingMode::Hide,
        GroupKind::Exploration => grouping == Grouping::Auto,
        GroupKind::Activity | GroupKind::Instructions => true,
    }
}

/// Per-session disclosure overrides. An absent override falls back to the
/// verbosity default: High opens exploration and instruction summaries,
/// everything else starts collapsed (`routes/session/index.tsx:248-252`).
///
/// Saved expansion is keyed by both session and group id, so changing one
/// session can never corrupt another session's expansion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisclosureState {
    saved: BTreeMap<(String, String), bool>,
}

impl DisclosureState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Upstream `groupExpanded(groupID, kind)`.
    #[must_use]
    pub fn group_expanded(
        &self,
        session_id: &str,
        group_id: &str,
        verbosity: Verbosity,
        kind: GroupKind,
    ) -> bool {
        self.saved
            .get(&(session_id.to_owned(), group_id.to_owned()))
            .copied()
            .unwrap_or(verbosity == Verbosity::High && opens_by_default(kind))
    }

    /// Persist an explicit expansion choice for one session and group.
    pub fn set_group_expanded(&mut self, session_id: &str, group_id: &str, expanded: bool) {
        self.saved
            .insert((session_id.to_owned(), group_id.to_owned()), expanded);
    }

    /// Number of saved overrides (diagnostics/tests only).
    #[must_use]
    pub fn saved_len(&self) -> usize {
        self.saved.len()
    }
}

fn opens_by_default(kind: GroupKind) -> bool {
    matches!(kind, GroupKind::Exploration | GroupKind::Instructions)
}

// ---------------------------------------------------------------------------
// Parts and part paths
// ---------------------------------------------------------------------------

/// Stable part identity: message id plus part id. Using the full pair keeps a
/// reused call/part id in another message from matching the wrong row.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartRef {
    pub message_id: String,
    pub part_id: String,
}

impl PartRef {
    #[must_use]
    pub fn new(message_id: impl Into<String>, part_id: impl Into<String>) -> Self {
        Self {
            message_id: message_id.into(),
            part_id: part_id.into(),
        }
    }
}

/// Appendable part shapes (`routes/session/grouping/session.ts:35-38`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppendPart {
    Text,
    Reasoning { completed_ms: Option<u64> },
    Tool { name: String },
}

const EXPLORATION_TOOLS: [&str; 5] = ["read", "glob", "grep", "webfetch", "websearch"];

/// Grouping path for one assistant part (`grouping/session.ts:90-104`).
/// Questions always stand alone; Low wraps every non-text part in `activity`.
#[must_use]
pub fn part_path(part: &AppendPart, verbosity: Verbosity) -> Vec<GroupKind> {
    if let AppendPart::Tool { name } = part {
        if name.eq_ignore_ascii_case("question") {
            return Vec::new();
        }
    }
    let mut path = Vec::new();
    if verbosity == Verbosity::Low && !matches!(part, AppendPart::Text) {
        path.push(GroupKind::Activity);
    }
    match part {
        AppendPart::Reasoning { .. } => path.push(GroupKind::Reasoning),
        AppendPart::Tool { name } if EXPLORATION_TOOLS.iter().any(|tool| name.eq_ignore_ascii_case(tool)) => {
            path.push(GroupKind::Exploration);
        }
        AppendPart::Tool { .. } | AppendPart::Text => {}
    }
    path
}

/// Grouping path for a whole message (`grouping/session.ts:107-116`):
/// instruction loads group, everything else stands alone.
#[must_use]
pub fn message_path(instruction_paths: &[String], verbosity: Verbosity) -> Vec<GroupKind> {
    if instruction_paths.is_empty() {
        return Vec::new();
    }
    if verbosity == Verbosity::Low {
        vec![GroupKind::Activity, GroupKind::Instructions]
    } else {
        vec![GroupKind::Instructions]
    }
}

// ---------------------------------------------------------------------------
// Group tree
// ---------------------------------------------------------------------------

/// One node of the grouping tree; `size` counts descendant leaves and a group
/// header counts zero rows (`grouping/tree.ts:1-10`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupNode<E, K> {
    Entry {
        entry: E,
    },
    Group {
        kind: K,
        children: Vec<GroupNode<E, K>>,
        size: usize,
    },
}

impl<E, K> GroupNode<E, K> {
    #[must_use]
    pub fn size(&self) -> usize {
        match self {
            Self::Entry { .. } => 1,
            Self::Group { size, .. } => *size,
        }
    }
}

/// Group adjacent entries by their configured nesting paths
/// (`grouping/tree.ts:71-100`).
pub fn group_entries<E: Clone, K: Clone + PartialEq>(
    entries: &[E],
    path: impl Fn(&E) -> Vec<K>,
) -> Vec<GroupNode<E, K>> {
    let mut nodes = Vec::new();
    for entry in entries {
        append_entry(&mut nodes, entry.clone(), &path(entry), 0);
    }
    nodes
}

fn append_entry<E: Clone, K: Clone + PartialEq>(
    nodes: &mut Vec<GroupNode<E, K>>,
    entry: E,
    path: &[K],
    depth: usize,
) {
    let Some(kind) = path.get(depth) else {
        nodes.push(GroupNode::Entry { entry });
        return;
    };
    if let Some(GroupNode::Group {
        kind: previous,
        children,
        size,
    }) = nodes.last_mut()
    {
        if previous == kind {
            *size += 1;
            append_entry(children, entry, path, depth + 1);
            return;
        }
    }
    let mut children = Vec::new();
    append_entry(&mut children, entry, path, depth + 1);
    nodes.push(GroupNode::Group {
        kind: kind.clone(),
        children,
        size: 1,
    });
}

/// Concatenate ordered chunks, merging only compatible groups at the seam
/// (`grouping/tree.ts:118-139`). Payloads are never deduplicated here.
#[must_use]
pub fn merge_groups<E: Clone, K: Clone + PartialEq>(
    left: &[GroupNode<E, K>],
    right: &[GroupNode<E, K>],
) -> Vec<GroupNode<E, K>> {
    if left.is_empty() {
        return right.to_vec();
    }
    if right.is_empty() {
        return left.to_vec();
    }
    let a = left.last().expect("left is non-empty");
    let b = right.first().expect("right is non-empty");
    match (a, b) {
        (
            GroupNode::Group {
                kind: left_kind,
                children: left_children,
                size: left_size,
            },
            GroupNode::Group {
                kind: right_kind,
                children: right_children,
                size: right_size,
            },
        ) if left_kind == right_kind => {
            let mut merged = left[..left.len() - 1].to_vec();
            merged.push(GroupNode::Group {
                kind: left_kind.clone(),
                size: left_size + right_size,
                children: merge_groups(left_children, right_children),
            });
            merged.extend_from_slice(&right[1..]);
            merged
        }
        _ => {
            let mut merged = left.to_vec();
            merged.extend_from_slice(right);
            merged
        }
    }
}

/// Split error (`grouping/tree.ts:141-150` throws `RangeError`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitError {
    InsideEntry,
    ExceedsEntryCount,
}

/// Split at a depth-first leaf offset; group headers count as zero rows and
/// only the ancestors crossing the cut are rebuilt
/// (`grouping/tree.ts:141-150`).
pub fn split_groups<E: Clone, K: Clone + PartialEq>(
    nodes: &[GroupNode<E, K>],
    count: usize,
) -> Result<(Vec<GroupNode<E, K>>, Vec<GroupNode<E, K>>), SplitError> {
    if count == 0 {
        return Ok((Vec::new(), nodes.to_vec()));
    }
    let mut offset = 0usize;
    for (index, node) in nodes.iter().enumerate() {
        let end = offset + node.size();
        if count == end {
            return Ok((nodes[..=index].to_vec(), nodes[index + 1..].to_vec()));
        }
        if count < end {
            let GroupNode::Group {
                kind,
                children,
                size,
            } = node
            else {
                return Err(SplitError::InsideEntry);
            };
            let left_size = count - offset;
            let (left, right) = split_groups(children, left_size)?;
            let mut left_nodes = nodes[..index].to_vec();
            left_nodes.push(GroupNode::Group {
                kind: kind.clone(),
                children: left,
                size: left_size,
            });
            let mut right_nodes = vec![GroupNode::Group {
                kind: kind.clone(),
                children: right,
                size: size - left_size,
            }];
            right_nodes.extend_from_slice(&nodes[index + 1..]);
            return Ok((left_nodes, right_nodes));
        }
        offset = end;
    }
    Err(SplitError::ExceedsEntryCount)
}

// ---------------------------------------------------------------------------
// Session entries, rows and projection
// ---------------------------------------------------------------------------

/// A concrete transcript entry (`grouping/session.ts:13-19`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionEntry {
    Message { message_id: String },
    CompactionQueued { inbox_id: String },
    Part { reference: PartRef },
    AssistantFooter { message_id: String },
    TurnUsage {
        message_ids: Vec<String>,
        previous_cache: Option<CacheUsage>,
    },
}

/// Cache read fact attached to a turn's token summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheUsage {
    pub read: u64,
    pub provider_id: String,
    pub model_id: String,
    pub variant: Option<String>,
}

/// One projected entry plus the append facts the grouping engine needs
/// (`grouping/session.ts:40-45`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionEntry {
    pub entry: SessionEntry,
    pub part: Option<AppendPart>,
    pub path: Vec<GroupKind>,
    /// Upstream `closesPrevious`: `false` only for still-pending messages.
    pub closes_previous: bool,
}

impl ProjectionEntry {
    #[must_use]
    pub fn leaf(entry: SessionEntry) -> Self {
        Self {
            entry,
            part: None,
            path: Vec::new(),
            closes_previous: true,
        }
    }

    #[must_use]
    fn part(entry: SessionEntry, part: AppendPart, path: Vec<GroupKind>) -> Self {
        Self {
            entry,
            part: Some(part),
            path,
            closes_previous: true,
        }
    }
}

pub type ProjectionNode = GroupNode<ProjectionEntry, GroupKind>;
pub type SessionNode = GroupNode<SessionEntry, GroupKind>;

/// A rendered transcript row: either a bare entry or a group
/// (`grouping/session.ts:24-32`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionRow {
    Entry(SessionEntry),
    Group(SessionGroup),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionGroup {
    pub kind: GroupKind,
    pub children: Vec<SessionNode>,
    pub size: usize,
    pub completed: bool,
    /// Permission-blocked leaves, retained in place (`exploration`/`activity`).
    pub pending: Vec<PartRef>,
}

/// Hydrate a fresh history batch in one pass (`grouping/session.ts:57-71`).
#[must_use]
pub fn project_entries(entries: &[ProjectionEntry]) -> Vec<SessionRow> {
    let nodes = group_entries(entries, |entry| entry.path.clone());
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| match node {
            GroupNode::Entry { entry } => SessionRow::Entry(entry.entry.clone()),
            GroupNode::Group { kind, children, size } => {
                let next_closes = match nodes.get(index + 1) {
                    Some(GroupNode::Group { .. }) => true,
                    Some(GroupNode::Entry { entry }) => entry.closes_previous,
                    None => false,
                };
                let reasoning_done = *kind == GroupKind::Reasoning
                    && children.iter().all(|child| match child {
                        GroupNode::Entry { entry } => matches!(
                            entry.part,
                            Some(AppendPart::Reasoning {
                                completed_ms: Some(_)
                            })
                        ),
                        GroupNode::Group { .. } => false,
                    });
                SessionRow::Group(SessionGroup {
                    kind: *kind,
                    children: children.iter().map(unwrap_node).collect(),
                    size: *size,
                    completed: next_closes || reasoning_done,
                    pending: Vec::new(),
                })
            }
        })
        .collect()
}

fn unwrap_node(node: &ProjectionNode) -> SessionNode {
    match node {
        GroupNode::Entry { entry } => GroupNode::Entry {
            entry: entry.entry.clone(),
        },
        GroupNode::Group { kind, children, size } => GroupNode::Group {
            kind: *kind,
            children: children.iter().map(unwrap_node).collect(),
            size: *size,
        },
    }
}

/// Insert one part at `index` and merge with the previous same-kind group
/// (`grouping/session.ts:120-154`). Pending exploration leaves stay at the end
/// of their group, so a new part is inserted before them.
pub fn append(
    rows: &mut Vec<SessionRow>,
    reference: PartRef,
    part: AppendPart,
    index: usize,
    verbosity: Verbosity,
) {
    let index = index.min(rows.len());
    let path = part_path(&part, verbosity);
    if path.is_empty() {
        complete_previous(rows, index);
        rows.insert(
            index,
            SessionRow::Entry(SessionEntry::Part { reference }),
        );
        return;
    }
    let projected = [ProjectionEntry::part(
        SessionEntry::Part { reference },
        part.clone(),
        path.clone(),
    )];
    let node = group_entries(&projected, |entry| entry.path.clone())
        .pop()
        .expect("one entry projects to one node");
    let kind = path[0];
    if index > 0 {
        if let SessionRow::Group(previous) = &mut rows[index - 1] {
            if previous.kind == kind {
                let pending = if previous.kind == GroupKind::Exploration {
                    previous.pending.len()
                } else {
                    0
                };
                let keep = previous.size.saturating_sub(pending);
                let (left, right) = split_groups(&previous.children, keep)
                    .expect("split inside the previous group is always valid");
                let incoming = match &node {
                    GroupNode::Group { children, .. } => children.iter().map(unwrap_node).collect(),
                    GroupNode::Entry { .. } => Vec::new(),
                };
                let merged = merge_groups(&merge_groups(&left, &incoming), &right);
                previous.size = merged.iter().map(|node| node.size()).sum();
                previous.children = merged;
                if matches!(part, AppendPart::Reasoning { completed_ms: None }) {
                    previous.completed = false;
                }
                return;
            }
        }
    }
    complete_previous(rows, index);
    let children = match node {
        GroupNode::Group {
            children, ..
        } => children.iter().map(unwrap_node).collect(),
        GroupNode::Entry { .. } => Vec::new(),
    };
    let completed = kind == GroupKind::Reasoning
        && matches!(part, AppendPart::Reasoning { completed_ms: Some(_) });
    rows.insert(
        index,
        SessionRow::Group(SessionGroup {
            kind,
            size: children.iter().map(|node| node.size()).sum(),
            children,
            completed,
            pending: Vec::new(),
        }),
    );
}

/// A row boundary closes the preceding group
/// (`grouping/session.ts:156-159`).
pub fn complete_previous(rows: &mut [SessionRow], index: usize) {
    if index == 0 {
        return;
    }
    if let Some(SessionRow::Group(previous)) = rows.get_mut(index - 1) {
        previous.completed = true;
    }
}

/// Leaf part refs of a group, optionally including permission-pending leaves
/// (`grouping/session.ts:161-170`).
#[must_use]
pub fn group_refs(row: &SessionGroup, include_pending: bool) -> Vec<PartRef> {
    let pending = if !include_pending
        && matches!(row.kind, GroupKind::Exploration | GroupKind::Activity)
    {
        row.pending.as_slice()
    } else {
        &[]
    };
    let mut refs = Vec::new();
    visit_refs(&row.children, pending, &mut refs);
    refs
}

fn visit_refs(nodes: &[SessionNode], pending: &[PartRef], refs: &mut Vec<PartRef>) {
    for node in nodes {
        match node {
            SessionNode::Group { children, .. } => visit_refs(children, pending, refs),
            SessionNode::Entry {
                entry: SessionEntry::Part { reference },
            } => {
                if !pending.contains(reference) {
                    refs.push(reference.clone());
                }
            }
            SessionNode::Entry { .. } => {}
        }
    }
}

/// Whether a part ref is already represented anywhere in the rows
/// (`grouping/session.ts:182-189`).
#[must_use]
pub fn has_part(rows: &[SessionRow], reference: &PartRef) -> bool {
    rows.iter().any(|row| match row {
        SessionRow::Entry(entry) => entry_ref(entry).as_ref() == Some(reference),
        SessionRow::Group(group) => group_refs(group, true).contains(reference),
    })
}

/// Partition permission-blocked leaves for exploration/activity groups
/// (`grouping/session.ts:172-180`).
///
/// The pending set is matched by full `(message, part)` identity, not by part
/// id alone: a reused call id in another message must not mark or reorder the
/// wrong row (MSG-003).
pub fn partition_pending(rows: &mut [SessionRow], pending: &[PartRef]) {
    for row in rows.iter_mut() {
        let SessionRow::Group(group) = row else {
            continue;
        };
        if !matches!(group.kind, GroupKind::Exploration | GroupKind::Activity) {
            continue;
        }
        if group.kind == GroupKind::Exploration {
            let (blocked, unblocked): (Vec<SessionNode>, Vec<SessionNode>) =
                group.children.iter().cloned().partition(|node| match node {
                    SessionNode::Entry {
                        entry: SessionEntry::Part { reference },
                    } => pending.contains(reference),
                    _ => false,
                });
            group.children = unblocked.into_iter().chain(blocked).collect();
        }
        group.pending = group_refs(group, true)
            .into_iter()
            .filter(|reference| pending.contains(reference))
            .collect();
    }
}

/// Stable reconciliation key (`rows.ts:325-341`).
#[must_use]
pub fn row_key(row: &SessionRow) -> Option<String> {
    Some(match row {
        SessionRow::Group(group) => group_id(group, 0)?,
        SessionRow::Entry(SessionEntry::Part { reference }) => format!(
            "[\"part\",{},{}]",
            json_string(&reference.message_id),
            json_string(&reference.part_id)
        ),
        SessionRow::Entry(SessionEntry::Message { message_id }) => {
            format!("[\"message\",{}]", json_string(message_id))
        }
        SessionRow::Entry(SessionEntry::AssistantFooter { message_id }) => {
            format!("[\"assistant-footer\",{}]", json_string(message_id))
        }
        SessionRow::Entry(SessionEntry::CompactionQueued { inbox_id }) => {
            format!("[\"compaction-queued\",{}]", json_string(inbox_id))
        }
        SessionRow::Entry(SessionEntry::TurnUsage { message_ids, .. }) => {
            format!(
                "[\"turn-usage\",{}]",
                json_string(message_ids.first().map(String::as_str).unwrap_or(""))
            )
        }
    })
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// Message facts and reduction
// ---------------------------------------------------------------------------

/// Role of one message as far as row layout is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowRole {
    User,
    Assistant,
    Synthetic,
    Compaction,
    Idle,
    Shell,
    System,
}

/// Compaction lifecycle relevant to row ordering (`rows.ts:350,384`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactionStatus {
    Running,
    Completed,
}

/// One assistant part as projected by the native history/live stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowPart {
    Text { text: String },
    Reasoning { text: String, completed_ms: Option<u64> },
    Tool { id: String, name: String },
}

impl RowPart {
    fn append_part(&self) -> AppendPart {
        match self {
            Self::Text { .. } => AppendPart::Text,
            Self::Reasoning { completed_ms, .. } => AppendPart::Reasoning {
                completed_ms: *completed_ms,
            },
            Self::Tool { name, .. } => AppendPart::Tool { name: name.clone() },
        }
    }
}

/// Real timestamps for one message; absent values stay absent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowTiming {
    pub created_ms: u64,
    pub completed_ms: Option<u64>,
    pub streamed_ms: Option<u64>,
}

/// Provider token usage of one assistant step (`rows.ts:484-492`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl TokenUsage {
    #[must_use]
    pub fn total(self) -> u64 {
        self.input + self.output + self.reasoning + self.cache_read + self.cache_write
    }
}

/// The message facts the row reducer needs. This is a projection of the
/// backend `SessionMessageInfo` union, not a new wire format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowMessage {
    pub id: String,
    pub role: RowRole,
    pub text: String,
    pub description: Option<String>,
    pub instruction_paths: Vec<String>,
    pub parts: Vec<RowPart>,
    pub finish: Option<String>,
    pub error: bool,
    pub retry: bool,
    pub tokens: Option<TokenUsage>,
    pub timing: RowTiming,
    pub agent: Option<String>,
    pub model: Option<String>,
    /// Explicit `inputs` set membership (still-pending user input).
    pub pending_input: bool,
    pub compaction_status: Option<CompactionStatus>,
}

impl RowMessage {
    #[must_use]
    pub fn user(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::base(id, RowRole::User, text)
    }

    #[must_use]
    pub fn assistant(id: impl Into<String>, parts: Vec<RowPart>) -> Self {
        let mut message = Self::base(id, RowRole::Assistant, String::new());
        message.parts = parts;
        message
    }

    #[must_use]
    pub fn notice(id: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut message = Self::base(id, RowRole::Synthetic, text.clone());
        message.description = Some(text);
        message
    }

    #[must_use]
    pub fn compaction(
        id: impl Into<String>,
        status: CompactionStatus,
        summary: impl Into<String>,
    ) -> Self {
        let mut message = Self::base(id, RowRole::Compaction, summary);
        message.compaction_status = Some(status);
        message
    }

    #[must_use]
    pub fn idle(id: impl Into<String>) -> Self {
        Self::base(id, RowRole::Idle, String::new())
    }

    fn base(id: impl Into<String>, role: RowRole, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role,
            text: text.into(),
            description: None,
            instruction_paths: Vec::new(),
            parts: Vec::new(),
            finish: None,
            error: false,
            retry: false,
            tokens: None,
            timing: RowTiming::default(),
            agent: None,
            model: None,
            pending_input: false,
            compaction_status: None,
        }
    }
}

/// Token summary accumulated across one turn (`rows.ts:352-372`).
#[derive(Clone, Debug, Default)]
struct TurnAccumulator {
    steps: Vec<(String, CacheUsage)>,
    previous_cache: Option<CacheUsage>,
}

impl TurnAccumulator {
    fn flush(&mut self, rows: &mut Vec<ProjectionEntry>) {
        if self.steps.is_empty() {
            return;
        }
        let last = self
            .steps
            .last()
            .expect("non-empty steps")
            .1
            .clone();
        rows.push(ProjectionEntry::leaf(SessionEntry::TurnUsage {
            message_ids: self.steps.iter().map(|(id, _)| id.clone()).collect(),
            previous_cache: self.previous_cache.clone(),
        }));
        self.previous_cache = Some(last);
        self.steps.clear();
    }
}

/// Reduce backend message facts into rows (`rows.ts:343-411`).
///
/// Ordering: visible messages, then running compactions, then pending inputs;
/// running compactions and inputs are excluded from the first group. Idle
/// markers flush turn usage; synthetic messages without a description are
/// skipped. Assistant text/reasoning parts that only contain whitespace are
/// dropped after consuming their ordinal.
#[must_use]
pub fn reduce_session_rows(
    messages: &[RowMessage],
    verbosity: Verbosity,
    turn_tokens: bool,
) -> Vec<SessionRow> {
    let pending_compactions: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, message)| {
            message.role == RowRole::Compaction
                && message.compaction_status == Some(CompactionStatus::Running)
        })
        .map(|(index, _)| index)
        .collect();
    let mut pending = std::collections::BTreeSet::new();
    for index in &pending_compactions {
        pending.insert(messages[*index].id.clone());
    }
    for message in messages {
        if message.pending_input {
            pending.insert(message.id.clone());
        }
    }

    let legacy = !messages.iter().any(|message| message.role == RowRole::Idle);
    let mut usage = turn_tokens.then(TurnAccumulator::default);
    let mut entries: Vec<ProjectionEntry> = Vec::with_capacity(messages.len());

    let mut ordered: Vec<&RowMessage> = Vec::with_capacity(messages.len());
    ordered.extend(messages.iter().filter(|message| !pending.contains(&message.id)));
    ordered.extend(pending_compactions.iter().map(|index| &messages[*index]));
    ordered.extend(messages.iter().filter(|message| message.pending_input));

    for message in ordered {
        if message.role != RowRole::Assistant {
            if message.role == RowRole::Idle {
                if let Some(usage) = usage.as_mut() {
                    usage.flush(&mut entries);
                }
                continue;
            }
            if message.role == RowRole::Synthetic
                && message
                    .description
                    .as_deref()
                    .map_or(true, |description| description.trim().is_empty())
            {
                continue;
            }
            if message.role == RowRole::Compaction
                && message.compaction_status == Some(CompactionStatus::Completed)
            {
                if let Some(usage) = usage.as_mut() {
                    usage.previous_cache = None;
                }
            }
            entries.push(ProjectionEntry {
                entry: SessionEntry::Message {
                    message_id: message.id.clone(),
                },
                part: None,
                path: message_path(&message.instruction_paths, verbosity),
                closes_previous: !pending.contains(&message.id),
            });
            continue;
        }

        if let Some(usage) = usage.as_mut() {
            if let Some(tokens) = message.tokens {
                if tokens.total() > 0 {
                    usage.steps.push((
                        message.id.clone(),
                        cache_usage(tokens, message.model.as_deref()),
                    ));
                }
            }
        }

        let mut text_ordinal = 0usize;
        let mut reasoning_ordinal = 0usize;
        for part in &message.parts {
            let part_id = match part {
                RowPart::Tool { id, .. } => id.clone(),
                RowPart::Text { .. } => {
                    let id = format!("text:{text_ordinal}");
                    text_ordinal += 1;
                    id
                }
                RowPart::Reasoning { .. } => {
                    let id = format!("reasoning:{reasoning_ordinal}");
                    reasoning_ordinal += 1;
                    id
                }
            };
            let blank = match part {
                RowPart::Text { text } | RowPart::Reasoning { text, .. } => text.trim().is_empty(),
                RowPart::Tool { .. } => false,
            };
            if blank {
                continue;
            }
            let append = part.append_part();
            entries.push(ProjectionEntry::part(
                SessionEntry::Part {
                    reference: PartRef::new(message.id.clone(), part_id),
                },
                append.clone(),
                part_path(&append, verbosity),
            ));
        }

        let terminal = message
            .finish
            .as_deref()
            .is_some_and(|finish| !matches!(finish, "tool-calls" | "unknown"))
            || message.error;
        if terminal || message.retry {
            entries.push(ProjectionEntry::leaf(SessionEntry::AssistantFooter {
                message_id: message.id.clone(),
            }));
        }
        if terminal && legacy {
            if let Some(usage) = usage.as_mut() {
                usage.flush(&mut entries);
            }
        }
    }

    project_entries(&entries)
}

fn cache_usage(tokens: TokenUsage, model: Option<&str>) -> CacheUsage {
    let (provider_id, model_id) = model
        .and_then(|model| model.split_once('/'))
        .map_or((String::new(), String::new()), |(provider, model)| {
            (provider.to_owned(), model.to_owned())
        });
    CacheUsage {
        read: tokens.cache_read,
        provider_id,
        model_id,
        variant: None,
    }
}

/// Project typed chat messages into the same row model. Used by the bounded
/// history log and the daemon projection so grouping has one implementation.
#[must_use]
pub fn rows_from_chat_entries(
    entries: &[(String, ChatMessage)],
    verbosity: Verbosity,
) -> Vec<SessionRow> {
    let messages: Vec<RowMessage> = entries
        .iter()
        .map(|(id, message)| row_message_from_chat(id, message))
        .collect();
    reduce_session_rows(&messages, verbosity, false)
}

/// Map one typed chat message onto row facts. Only real part kinds become
/// assistant parts; persisted reasoning summaries stay reasoning parts.
#[must_use]
pub fn row_message_from_chat(id: &str, message: &ChatMessage) -> RowMessage {
    let text = message
        .parts
        .iter()
        .filter_map(|part| match part {
            ChatPart::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut row = match message.role {
        ChatRole::User => RowMessage::user(id, text),
        ChatRole::Assistant => RowMessage::assistant(id, Vec::new()),
        ChatRole::System | ChatRole::Tool | ChatRole::Compaction => {
            let mut row = RowMessage::notice(id, text.clone());
            row.role = if message.role == ChatRole::Tool {
                RowRole::Shell
            } else {
                RowRole::System
            };
            row.description = Some(text);
            row
        }
    };
    row.agent = message.agent.clone();
    row.model = message.model.clone();
    if message.role == ChatRole::Assistant {
        row.parts = message
            .parts
            .iter()
            .enumerate()
            .filter_map(|(index, part)| match part {
                ChatPart::Text(text) if !text.trim().is_empty() => {
                    Some(RowPart::Text { text: text.clone() })
                }
                ChatPart::Thinking(text) if !text.trim().is_empty() => Some(RowPart::Reasoning {
                    text: text.clone(),
                    completed_ms: None,
                }),
                ChatPart::ToolState { name, id, .. } | ChatPart::ToolCall { name, id } => {
                    Some(RowPart::Tool {
                        id: id.clone(),
                        name: name.clone(),
                    })
                }
                ChatPart::ToolResult { name, .. } => Some(RowPart::Tool {
                    id: format!("result:{index}"),
                    name: name.clone(),
                }),
                _ => None,
            })
            .collect();
    }
    row
}

// ---------------------------------------------------------------------------
// Turn duration and throughput
// ---------------------------------------------------------------------------

/// Sessions without idle markers end a turn at its terminal step; marker-era
/// sessions keep the turn open across steers (`rows.ts:427-429,465-467`).
#[must_use]
pub fn legacy_turns(messages: &[RowMessage]) -> bool {
    !messages.iter().any(|message| message.role == RowRole::Idle)
}

/// Walk back to the turn's input boundary (`rows.ts:469-482`).
///
/// Legacy sessions return the nearest input; marker-era sessions return the
/// first input after the previous idle marker (steers and model switches stay
/// inside the turn).
#[must_use]
pub fn input_index(messages: &[RowMessage], end: usize, legacy: bool) -> Option<usize> {
    let mut input = None;
    let mut index = end.min(messages.len());
    while index > 0 {
        index -= 1;
        let message = &messages[index];
        if message.role == RowRole::Idle {
            return input;
        }
        if !matches!(message.role, RowRole::User | RowRole::Synthetic) {
            continue;
        }
        if legacy {
            return Some(index);
        }
        input = Some(index);
    }
    input
}

/// One turn's wall duration (`rows.ts:430-440`). Zero when the assistant step
/// has no completion time; an absent input falls back to the assistant's own
/// creation time, so the result stays zero rather than negative.
#[must_use]
pub fn turn_duration(
    message: &RowMessage,
    messages: &[RowMessage],
    position: Option<usize>,
    legacy: bool,
) -> u64 {
    let Some(completed) = message.timing.completed_ms else {
        return 0;
    };
    let index = position
        .or_else(|| messages.iter().position(|item| item.id == message.id))
        .unwrap_or(messages.len());
    let input = input_index(messages, index, legacy)
        .and_then(|index| messages.get(index))
        .map_or(message.timing.created_ms, |input| input.timing.created_ms);
    completed.saturating_sub(input)
}

/// Provider-active throughput for the turn (`rows.ts:442-463`). Returns `None`
/// when any step lacks a real streamed timestamp, when no step has output, or
/// when the aggregate duration is zero. Tool time is never included: only
/// `time.streamed - time.created` of assistant steps is summed.
#[must_use]
pub fn turn_tokens_per_second(
    message: &RowMessage,
    messages: &[RowMessage],
    position: Option<usize>,
    legacy: bool,
) -> Option<f64> {
    let index = position.or_else(|| messages.iter().position(|item| item.id == message.id))?;
    let end = index + 1;
    // Upstream `inputIndex` returns -1 when no input precedes the step, and
    // the slice still starts at 0 (`rows.ts:448-456`).
    let start = input_index(messages, end, legacy).map_or(0, |index| index + 1);
    let steps: Vec<&RowMessage> = messages[start..end]
        .iter()
        .filter(|item| item.role == RowRole::Assistant)
        .collect();
    let mut duration = 0u64;
    for step in &steps {
        let streamed = step.timing.streamed_ms?;
        duration = duration.saturating_add(streamed.saturating_sub(step.timing.created_ms));
    }
    if steps.is_empty() {
        return None;
    }
    let output: u64 = steps
        .iter()
        .map(|step| {
            step.tokens
                .map_or(0, |tokens| tokens.output + tokens.reasoning)
        })
        .sum();
    if output == 0 || duration == 0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    Some(output as f64 / (duration as f64 / 1_000.0))
}

/// Cache reuse drop between consecutive steps (`rows.ts:413-425`). Model
/// switches suppress the drop; OpenAI's 1–2 bucket jitter is tolerated.
#[must_use]
pub fn cache_reuse_drop(previous: Option<&CacheUsage>, current: &CacheUsage) -> Option<u64> {
    let previous = previous?;
    if previous.provider_id != current.provider_id
        || previous.model_id != current.model_id
        || previous.variant != current.variant
    {
        return None;
    }
    let drop = previous.read.saturating_sub(current.read);
    if current.provider_id == "openai" && (1_024..=2_048).contains(&drop) {
        return None;
    }
    (drop > 0).then_some(drop)
}

// ---------------------------------------------------------------------------
// Anchors and scroll math
// ---------------------------------------------------------------------------

/// A mounted anchor target (`routes/session/anchors.ts:6`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnchorTarget {
    Part(PartRef),
    Group { group_id: String },
}

/// Registry key (`anchors.ts:9-13`).
#[must_use]
pub fn anchor_key(target: &AnchorTarget) -> String {
    match target {
        AnchorTarget::Part(reference) => format!(
            "[\"part\",{},{}]",
            json_string(&reference.message_id),
            json_string(&reference.part_id)
        ),
        AnchorTarget::Group { group_id } => {
            format!("[\"group\",{}]", json_string(group_id))
        }
    }
}

/// Canonical scroll reference for an entry (`anchors.ts:15-22`). Whole
/// non-assistant messages use the synthetic part id `message`; derived
/// footer/usage rows have no reference of their own.
#[must_use]
pub fn entry_ref(entry: &SessionEntry) -> Option<PartRef> {
    match entry {
        SessionEntry::Part { reference } => Some(reference.clone()),
        SessionEntry::Message { message_id } => Some(PartRef::new(message_id, "message")),
        SessionEntry::AssistantFooter { .. }
        | SessionEntry::CompactionQueued { .. }
        | SessionEntry::TurnUsage { .. } => None,
    }
}

/// Stable group identity: first descendant part ref plus kind and level
/// (`anchors.ts:24-33`).
#[must_use]
pub fn group_id(group: &SessionGroup, level: usize) -> Option<String> {
    let reference = first_ref(&group.children)?;
    Some(format!(
        "[{},{},{},{}]",
        json_string(&reference.message_id),
        json_string(&reference.part_id),
        json_string(group.kind.as_str()),
        level
    ))
}

fn first_ref(nodes: &[SessionNode]) -> Option<PartRef> {
    for node in nodes {
        let reference = match node {
            SessionNode::Entry { entry } => entry_ref(entry),
            SessionNode::Group { children, .. } => first_ref(children),
        };
        if reference.is_some() {
            return reference;
        }
    }
    None
}

/// Whether a row contains an anchor target (`anchors.ts:35-48`).
#[must_use]
pub fn contains_anchor(row: &SessionRow, target: &AnchorTarget) -> bool {
    match row {
        SessionRow::Entry(entry) => {
            matches!(target, AnchorTarget::Part(reference) if entry_ref(entry).as_ref() == Some(reference))
        }
        SessionRow::Group(group) => contains_group_anchor(group, target, 0),
    }
}

fn contains_group_anchor(group: &SessionGroup, target: &AnchorTarget, level: usize) -> bool {
    if let AnchorTarget::Group { group_id: wanted } = target {
        if group_id(group, level).as_deref() == Some(wanted.as_str()) {
            return true;
        }
    }
    group.children.iter().any(|child| match child {
        SessionNode::Entry { entry } => {
            matches!(target, AnchorTarget::Part(reference) if entry_ref(entry).as_ref() == Some(reference))
        }
        SessionNode::Group { children, kind, .. } => {
            let child_group = SessionGroup {
                kind: *kind,
                children: children.clone(),
                size: children.iter().map(|node| node.size()).sum(),
                completed: false,
                pending: Vec::new(),
            };
            contains_group_anchor(&child_group, target, level + 1)
        }
    })
}

/// One mounted anchor with its layout geometry (`anchors.ts:5-7`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorRecord {
    pub target: AnchorTarget,
    pub y: i64,
    pub height: u16,
    pub destroyed: bool,
}

/// Registry of mounted anchors; only mounted, non-empty headers register
/// (`anchors.ts:51-85`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimelineAnchors {
    records: BTreeMap<String, AnchorRecord>,
}

impl TimelineAnchors {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) one anchor; returns its registry key.
    pub fn register(&mut self, record: AnchorRecord) -> String {
        let key = anchor_key(&record.target);
        self.records.insert(key.clone(), record);
        key
    }

    /// Remove an anchor, as the component cleanup does (`anchors.ts:59-63`).
    pub fn unregister(&mut self, target: &AnchorTarget) {
        self.records.remove(&anchor_key(target));
    }

    /// Live anchor for a target, or `None` when destroyed/zero-height.
    #[must_use]
    pub fn get(&self, target: &AnchorTarget) -> Option<&AnchorRecord> {
        self.records
            .get(&anchor_key(target))
            .filter(|record| !record.destroyed && record.height > 0)
    }

    /// Live anchors in visual order (`anchors.ts:52-58`).
    #[must_use]
    pub fn list(&self) -> Vec<AnchorRecord> {
        let mut list: Vec<AnchorRecord> = self
            .records
            .values()
            .filter(|record| !record.destroyed && record.height > 0)
            .cloned()
            .collect();
        list.sort_by_key(|record| record.y);
        list
    }

    /// First live part anchor for a message (`anchors.ts:64-66`).
    #[must_use]
    pub fn for_message(&self, message_id: &str) -> Option<AnchorRecord> {
        self.list().into_iter().find(|record| match &record.target {
            AnchorTarget::Part(reference) => reference.message_id == message_id,
            AnchorTarget::Group { .. } => false,
        })
    }

    /// First live anchor position per message (`anchors.ts:67-77`).
    #[must_use]
    pub fn message_positions(&self) -> Vec<(String, i64)> {
        let mut seen = std::collections::BTreeSet::new();
        let mut positions = Vec::new();
        for record in self.list() {
            if let AnchorTarget::Part(reference) = &record.target {
                if seen.insert(reference.message_id.clone()) {
                    positions.push((reference.message_id.clone(), record.y));
                }
            }
        }
        positions
    }
}

/// Saved viewport anchor (`index.tsx:553-571`): target plus the row's screen
/// Y at save time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedScrollAnchor {
    pub target: AnchorTarget,
    pub screen_y: i64,
}

/// Choose the anchor whose top is at/above the viewport inset. Groups and
/// assistant messages use a one-row inset; everything else uses zero. Falls
/// back to the first mounted anchor (`index.tsx:558-570`).
#[must_use]
pub fn save_scroll_anchor(
    anchors: &TimelineAnchors,
    viewport_y: i64,
    is_assistant_message: impl Fn(&str) -> bool,
) -> Option<SavedScrollAnchor> {
    let mounted = anchors.list();
    let first = mounted.first()?;
    let mut chosen: Option<SavedScrollAnchor> = None;
    for record in &mounted {
        let screen_y = record.y - viewport_y;
        let inset = match &record.target {
            AnchorTarget::Group { .. } => 1,
            AnchorTarget::Part(reference) if is_assistant_message(&reference.message_id) => 1,
            AnchorTarget::Part(_) => 0,
        };
        if screen_y <= inset
            && chosen
                .as_ref()
                .map_or(true, |anchor| screen_y > anchor.screen_y)
        {
            chosen = Some(SavedScrollAnchor {
                target: record.target.clone(),
                screen_y,
            });
        }
    }
    chosen.or_else(|| {
        Some(SavedScrollAnchor {
            target: first.target.clone(),
            screen_y: first.y - viewport_y,
        })
    })
}

/// Restore scroll target once the anchor's new layout is known
/// (`index.tsx:593-615`): keep the anchor at its saved screen row.
#[must_use]
pub fn restore_scroll_target(
    scroll_top: i64,
    anchor_y: i64,
    viewport_y: i64,
    saved_screen_y: i64,
) -> i64 {
    let content_y = scroll_top + anchor_y - viewport_y;
    content_y - saved_screen_y
}

/// Maximum scroll offset for the current content.
#[must_use]
pub fn max_scroll(scroll_height: i64, viewport_height: i64) -> i64 {
    (scroll_height - viewport_height).max(0)
}

/// Extra bottom slack needed when a restored target is past the content's
/// maximum (`message-navigation.ts:8-16`).
#[must_use]
pub fn message_navigation_slack(
    top: i64,
    viewport_height: i64,
    scroll_height: i64,
    current_slack: i64,
) -> i64 {
    let content_height = scroll_height - current_slack;
    (top + viewport_height - content_height).max(0)
}

/// A reader is away from the bottom when rows are hidden below, or the scroll
/// box is not at its sticky tail (`index.tsx:518-531`).
#[must_use]
pub fn is_away_from_bottom(
    scroll_top: i64,
    scroll_height: i64,
    viewport_height: i64,
    visible_end: usize,
    rows_len: usize,
) -> bool {
    if visible_end < rows_len {
        return true;
    }
    scroll_top < max_scroll(scroll_height, viewport_height)
}

/// One mounted message child for navigation (`message-navigation.ts:3-6`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavChild {
    pub id: String,
    pub y: i64,
}

/// One navigation target (`message-navigation.ts:33-41`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavTarget {
    pub id: String,
    pub y: i64,
    pub top: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavDirection {
    Next,
    Prev,
}

/// Find the next/previous readable message boundary
/// (`message-navigation.ts:18-48`). User messages need non-blank text;
/// assistant messages need a non-blank text part and use a one-row top inset.
#[must_use]
pub fn find_message_boundary(
    direction: NavDirection,
    children: &[NavChild],
    messages: &[RowMessage],
    scroll_top: i64,
    viewport_y: i64,
    current_id: Option<&str>,
    user_only: bool,
) -> Option<NavTarget> {
    let mut visible: Vec<NavTarget> = children
        .iter()
        .filter_map(|child| {
            let message = messages.iter().find(|message| message.id == child.id)?;
            if message.role == RowRole::User && !message.text.trim().is_empty() {
                let y = scroll_top + child.y - viewport_y;
                return Some(NavTarget {
                    id: child.id.clone(),
                    y,
                    top: y,
                });
            }
            if user_only || message.role != RowRole::Assistant {
                return None;
            }
            let has_text = message
                .parts
                .iter()
                .any(|part| matches!(part, RowPart::Text { text } if !text.trim().is_empty()));
            if !has_text {
                return None;
            }
            let y = scroll_top + child.y - viewport_y;
            Some(NavTarget {
                id: child.id.clone(),
                y,
                top: (y - 1).max(0),
            })
        })
        .collect();
    visible.sort_by_key(|target| target.y);
    if let Some(current_id) = current_id {
        if let Some(index) = visible.iter().position(|target| target.id == current_id) {
            let offset = if direction == NavDirection::Next { 1 } else { -1 };
            return index
                .checked_add_signed(offset)
                .and_then(|index| visible.get(index))
                .cloned();
        }
    }
    match direction {
        NavDirection::Next => visible
            .into_iter()
            .find(|target| target.y > scroll_top),
        NavDirection::Prev => visible
            .into_iter()
            .rev()
            .find(|target| target.y < scroll_top),
    }
}

// ---------------------------------------------------------------------------
// Mount budget and history paging boundaries
// ---------------------------------------------------------------------------

/// Rendered-entry weight of one row (`mount-budget.ts:10-22`): a collapsed or
/// ungrouped group costs its leaf count; an expanded group costs the visible
/// children.
#[must_use]
pub fn row_weight(
    row: &SessionRow,
    expanded: impl Fn(&str, GroupKind) -> bool,
    grouped: impl Fn(GroupKind) -> bool,
) -> usize {
    match row {
        SessionRow::Entry(_) => 1,
        SessionRow::Group(group) => tree_weight(
            group.kind,
            &group.children,
            group.size,
            0,
            &expanded,
            &grouped,
        ),
    }
}

fn tree_weight(
    kind: GroupKind,
    children: &[SessionNode],
    size: usize,
    level: usize,
    expanded: &impl Fn(&str, GroupKind) -> bool,
    grouped: &impl Fn(GroupKind) -> bool,
) -> usize {
    if !grouped(kind) {
        return size;
    }
    let group = SessionGroup {
        kind,
        children: children.to_vec(),
        size,
        completed: false,
        pending: Vec::new(),
    };
    let Some(id) = group_id(&group, level) else {
        return 1;
    };
    if !expanded(&id, kind) {
        return 1;
    }
    // An expanded group always paints its own disclosure/header row before
    // its children.  Keep that row in the mount budget (the upstream
    // `rowWeight` implementation starts the reduction at `1`); otherwise a
    // backfill can stop one row too early and the visible anchor moves when a
    // group is expanded or when the terminal is resized.
    1 + children
        .iter()
        .map(|child| match child {
            SessionNode::Entry { .. } => 1,
            SessionNode::Group {
                kind,
                children,
                size,
            } => tree_weight(*kind, children, *size, level + 1, expanded, grouped),
        })
        .sum::<usize>()
}

/// First row index such that rows `[index, end)` spend at least `budget`
/// (`mount-budget.ts:24-30`).
#[must_use]
pub fn rows_before(weights: &[usize], end: usize, budget: usize) -> usize {
    let mut index = end.min(weights.len());
    let mut spent = 0usize;
    while index > 0 && spent < budget {
        index -= 1;
        spent += weights[index];
    }
    index
}

/// End row index such that rows `[start, index)` spend at least `budget`
/// (`mount-budget.ts:32-35`).
#[must_use]
pub fn rows_after(weights: &[usize], start: usize, budget: usize) -> usize {
    let mut index = start.min(weights.len());
    let mut spent = 0usize;
    while index < weights.len() && spent < budget {
        spent += weights[index];
        index += 1;
    }
    index
}

/// A page boundary can cut a group in half; while the oldest row is a group
/// keep loading older pages (`rows.ts:27-45`).
#[must_use]
pub fn boundary_fill_required(rows: &[SessionRow], more: bool) -> bool {
    more && matches!(rows.first(), Some(SessionRow::Group(_)))
}

/// A page that adds nothing must end the boundary loop (`rows.ts:41-43`).
#[must_use]
pub fn boundary_fill_progressed(before: usize, after: usize) -> bool {
    after > before
}

/// Pending prepend operation (`history.ts:12`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrependPending {
    pub scroll_by: i64,
    pub continuation: bool,
    pub after: bool,
}

/// Outcome of requesting a prepend (`history.ts:14-26`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrependDecision {
    Start,
    Coalesced,
    Ignored,
}

/// Completed prepend: caller scrolls by `scroll_by`, then runs the
/// continuation and deferred action (`history.ts:29-48`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrependCompletion {
    pub scroll_by: i64,
    pub continuation: bool,
    pub after: bool,
}

/// `createHistoryPrepend` state (`history.ts:1-61`): one in-flight older-page
/// load; repeated requests coalesce into it; cancellation keeps the scroll
/// compensation but drops callbacks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryPrepend {
    pending: Option<PrependPending>,
}

impl HistoryPrepend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn pending(&self) -> Option<PrependPending> {
        self.pending
    }

    /// Request a prepend. A request while one is in flight replaces its
    /// scroll delta when it carries a continuation or a fresh delta; otherwise
    /// it is ignored. `more == false` refuses to start.
    pub fn request(&mut self, scroll_by: i64, continuation: bool, more: bool) -> PrependDecision {
        if let Some(pending) = self.pending.as_mut() {
            if continuation || (pending.scroll_by == 0 && scroll_by != 0) {
                pending.scroll_by = scroll_by;
                pending.continuation = continuation;
                return PrependDecision::Coalesced;
            }
            return PrependDecision::Ignored;
        }
        if !more {
            return PrependDecision::Ignored;
        }
        self.pending = Some(PrependPending {
            scroll_by,
            continuation,
            after: false,
        });
        PrependDecision::Start
    }

    /// Drop queued callbacks but keep pending scroll compensation.
    pub fn cancel(&mut self) {
        if let Some(pending) = self.pending.as_mut() {
            pending.continuation = false;
            pending.after = false;
        }
    }

    /// A jump supersedes deferred scrolling but waits for anchor compensation.
    /// Returns `true` when there was nothing pending and the callback may run
    /// immediately (`history.ts:53-59`).
    pub fn defer_after(&mut self) -> bool {
        match self.pending.as_mut() {
            Some(pending) => {
                pending.scroll_by = 0;
                pending.after = true;
                false
            }
            None => true,
        }
    }

    /// Finish the in-flight load after layout. Returns `None` when the session
    /// is no longer active (the load is dropped, matching upstream).
    pub fn complete(
        &mut self,
        height_before: i64,
        height_after: i64,
        active: bool,
    ) -> Option<PrependCompletion> {
        let pending = self.pending.take()?;
        if !active {
            return None;
        }
        Some(PrependCompletion {
            scroll_by: height_after - height_before + pending.scroll_by,
            continuation: pending.continuation,
            after: pending.after,
        })
    }

    /// Finish a failed load; returns whether the deferred callback should run.
    pub fn fail(&mut self, active: bool) -> bool {
        let Some(pending) = self.pending.take() else {
            return false;
        };
        active && pending.after
    }
}

// ---------------------------------------------------------------------------
// Row chrome: role gutter, timestamps, selection guard
// ---------------------------------------------------------------------------

/// Chrome around one transcript row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleGutter {
    /// Left rail glyph, if the role draws one.
    pub border: Option<char>,
    /// User rail color source; the renderer resolves this against the theme.
    pub rail_color: Option<UserRailColor>,
    /// Left padding inside the card (user) or gutter (others).
    pub padding_left: u16,
    /// Vertical padding inside the card.
    pub padding_vertical: u16,
    /// Column where content starts.
    pub content_column: u16,
    /// Whether hover deepens the card background.
    pub hover_background: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserRailColor {
    Agent,
    BorderBase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserCardSurface {
    RaisedBase,
    DecreasedRaisedBase,
}

#[must_use]
pub fn user_card_surface(hovered: bool) -> UserCardSurface {
    if hovered {
        UserCardSurface::DecreasedRaisedBase
    } else {
        UserCardSurface::RaisedBase
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserChipLayout {
    pub top_padding: u16,
    pub gap: u16,
    pub wraps: bool,
}

#[must_use]
pub const fn user_chip_layout() -> UserChipLayout {
    UserChipLayout {
        top_padding: 1,
        gap: 1,
        wraps: true,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserChipKind {
    Skill,
    File,
    Directory,
}

/// MSG-005: one user-card chip (accent label + muted name). Upstream paints
/// `" skill " | " dir " | " file "` in the accent bucket and `" {name} "` in
/// the muted/raised surface (`packages/tui/src/routes/session/index.tsx:2432-2472`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserChip {
    pub kind: UserChipKind,
    pub name: String,
}

impl UserChip {
    /// Build a skill chip (`index.tsx:2438`).
    #[must_use]
    pub fn skill(name: impl Into<String>) -> Self {
        Self { kind: UserChipKind::Skill, name: name.into() }
    }

    /// Build a file/directory chip; the mime alone selects the label
    /// (`index.tsx:2457`).
    #[must_use]
    pub fn file(mime: &str, name: impl Into<String>) -> Self {
        Self { kind: user_file_chip_kind(mime), name: name.into() }
    }
}

#[must_use]
pub fn user_file_chip_kind(mime: &str) -> UserChipKind {
    if mime == "application/x-directory" {
        UserChipKind::Directory
    } else {
        UserChipKind::File
    }
}

/// Resolve the user card's left-rail colour (`index.tsx:2384`):
/// `delivery() ? theme.border.base : color()`. The choice is MUTUALLY
/// EXCLUSIVE - the pending rail is the flat border colour and never a blend
/// with the agent colour. `agent_color` is the upstream `local.agent.color`
/// result (`context/local.tsx:128-134`); when the caller has no agent colour it
/// falls back to `border_base`, matching the prompt rail's own fallback.
#[must_use]
pub fn user_rail_color(
    rail: UserRailColor,
    agent_color: Option<[u8; 4]>,
    border_base: [u8; 4],
) -> [u8; 4] {
    match rail {
        UserRailColor::BorderBase => border_base,
        UserRailColor::Agent => agent_color.unwrap_or(border_base),
    }
}

/// The full delivery-dependent rail colour for a user message
/// (`index.tsx:2370,2384`): the pending state selects `border_base`, the
/// settled state selects the agent colour. No blend, no tint.
#[must_use]
pub fn user_rail_color_for_delivery(
    delivery_pending: bool,
    agent_color: Option<[u8; 4]>,
    border_base: [u8; 4],
) -> [u8; 4] {
    user_rail_color(
        if delivery_pending {
            UserRailColor::BorderBase
        } else {
            UserRailColor::Agent
        },
        agent_color,
        border_base,
    )
}

/// Accent-chip label for one user-card chip (`index.tsx:2438` `" skill "`,
/// `:2457` `file.mime === "application/x-directory" ? "dir" : "file"`).
#[must_use]
pub fn user_chip_label(kind: UserChipKind) -> &'static str {
    match kind {
        UserChipKind::Skill => "skill",
        UserChipKind::File => "file",
        UserChipKind::Directory => "dir",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserImageStripLayout {
    /// Each preview cell's height (`SessionImages` `height()`).
    pub height: u16,
    /// Each preview cell's width (`height() * 2`).
    pub width: u16,
    /// Preview cells actually painted (`images().slice(0, 3)`).
    pub visible: usize,
    /// Painted `+N more` overflow chip and its count (`images().length - visible`).
    pub overflow: Option<usize>,
    pub padding_top: u16,
    pub padding_bottom: u16,
    pub padding_left: u16,
    pub padding_right: u16,
    pub gap: u16,
    /// Fixed width of the `+N more` chip box (`SessionImages` overflow box).
    pub overflow_width: u16,
}

/// `SessionImages` (`routes/session/index.tsx:2660-2700`): `height() =
/// max(4, min(8, floor(terminal.height / 4)))`, preview cells `height() * 2`
/// wide, at most three shown plus a fixed-width `+N more` chip when there are
/// more. `padding_left` defaults to 3 and is passed `2` from the user card
/// (`:2383`).
#[must_use]
pub fn user_image_strip_layout(
    terminal_height: u16,
    image_count: usize,
    padding_left: u16,
) -> Option<UserImageStripLayout> {
    if image_count == 0 {
        return None;
    }
    let height = (terminal_height / 4).clamp(4, 8);
    let visible = image_count.min(3);
    Some(UserImageStripLayout {
        height,
        width: height * 2,
        visible,
        overflow: (image_count > visible).then_some(image_count - visible),
        padding_top: 1,
        padding_bottom: 1,
        padding_left,
        padding_right: 2,
        gap: 1,
        overflow_width: 8,
    })
}

/// User cards draw the one-cell heavy `┃` rail with two cells of padding and
/// one row above/below; every other row starts at content column three with no
/// rail (`index.tsx:2382-2427`, `message-parts.tsx:77,184,222`).
#[must_use]
pub fn role_gutter(role: ChatRole) -> RoleGutter {
    role_gutter_for_delivery(role, false)
}

/// Pending delivery overrides the user rail with `theme.border.base`.
#[must_use]
pub fn role_gutter_for_delivery(role: ChatRole, delivery_pending: bool) -> RoleGutter {
    match role {
        ChatRole::User => RoleGutter {
            border: Some(USER_RAIL_CHAR),
            rail_color: Some(if delivery_pending {
                UserRailColor::BorderBase
            } else {
                UserRailColor::Agent
            }),
            padding_left: 2,
            padding_vertical: 1,
            content_column: CONTENT_GUTTER,
            hover_background: true,
        },
        ChatRole::Assistant | ChatRole::Tool | ChatRole::System | ChatRole::Compaction => RoleGutter {
            border: None,
            rail_color: None,
            padding_left: CONTENT_GUTTER,
            padding_vertical: 0,
            content_column: CONTENT_GUTTER,
            hover_background: false,
        },
    }
}

/// Top margin before a row at `level` (`index.tsx:1680`,
/// `group-view.tsx:224,383`). Top-level rows always take exactly one row;
/// nested entries take none, except a nested thought entry which takes one.
/// Nested containers therefore never add a second gap on top of the row's own.
#[must_use]
pub fn row_margin_top(level: usize, thought_entry: bool) -> u16 {
    if level == 0 || thought_entry {
        ROW_MARGIN_TOP
    } else {
        0
    }
}

/// Actions that must not fire on mouse-up while text is selected
/// (`group-view.tsx:279`, `index.tsx:2397`, tool details and child-session
/// links share the same guard).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowAction {
    MessageActions,
    GroupToggle,
    ToolDetails,
    OpenChildSession,
}

/// Whether a click action may run (`false` while a drag selection exists).
#[must_use]
pub fn click_allowed(has_text_selection: bool) -> bool {
    !has_text_selection
}

/// Whether a click on `action` should be acted on.
#[must_use]
pub fn click_acts(has_text_selection: bool, _action: RowAction) -> bool {
    click_allowed(has_text_selection)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn references(row: &SessionRow) -> Vec<PartRef> {
        match row {
            SessionRow::Entry(SessionEntry::Part { reference }) => vec![reference.clone()],
            SessionRow::Group(group) => group_refs(group, true),
            _ => Vec::new(),
        }
    }

    fn read(id: &str) -> RowPart {
        RowPart::Tool {
            id: id.to_owned(),
            name: "read".to_owned(),
        }
    }

    fn thought(text: &str, completed: bool) -> RowPart {
        RowPart::Reasoning {
            text: text.to_owned(),
            completed_ms: completed.then_some(10),
        }
    }

    // MSG-001 ---------------------------------------------------------------

    #[test]
    fn msg001_default_medium_groups_thoughts_and_exploration() {
        let message = RowMessage::assistant(
            "m1",
            vec![
                thought("plan", true),
                thought("plan more", true),
                read("call_1"),
                RowPart::Text {
                    text: "answer".into(),
                },
            ],
        );
        let rows = reduce_session_rows(&[RowMessage::user("u1", "go"), message], Verbosity::Medium, false);
        assert_eq!(rows.len(), 4);
        assert!(matches!(&rows[0], SessionRow::Entry(SessionEntry::Message { .. })));
        match &rows[1] {
            SessionRow::Group(group) => {
                assert_eq!(group.kind, GroupKind::Reasoning);
                assert_eq!(group.size, 2);
                assert!(group.completed, "next exploration group closes reasoning");
            }
            other => panic!("expected reasoning group: {other:?}"),
        }
        match &rows[2] {
            SessionRow::Group(group) => {
                assert_eq!(group.kind, GroupKind::Exploration);
                assert_eq!(group.size, 1);
            }
            other => panic!("expected exploration group: {other:?}"),
        }
        assert!(matches!(&rows[3], SessionRow::Entry(SessionEntry::Part { .. })));
    }

    #[test]
    fn msg001_questions_stand_alone() {
        let message = RowMessage::assistant(
            "m1",
            vec![
                read("call_1"),
                RowPart::Tool {
                    id: "call_2".into(),
                    name: "Question".into(),
                },
                read("call_3"),
            ],
        );
        let rows = reduce_session_rows(&[RowMessage::user("u1", "go"), message], Verbosity::Medium, false);
        let row_refs: Vec<Vec<PartRef>> = rows.iter().map(references).collect();
        assert!(row_refs[0].is_empty(), "user message row");
        assert_eq!(row_refs[1], vec![PartRef::new("m1", "call_1")]);
        assert_eq!(row_refs[2], vec![PartRef::new("m1", "call_2")]);
        assert_eq!(row_refs[3], vec![PartRef::new("m1", "call_3")]);
        assert!(matches!(rows[2], SessionRow::Entry(SessionEntry::Part { .. })));
    }

    #[test]
    fn msg001_low_wraps_non_text_parts_in_one_activity_summary() {
        let message = RowMessage::assistant(
            "m1",
            vec![
                thought("plan", true),
                read("call_1"),
                RowPart::Text {
                    text: "answer".into(),
                },
            ],
        );
        let rows = reduce_session_rows(&[message], Verbosity::Low, false);
        assert_eq!(rows.len(), 2);
        match (&rows[0], &rows[1]) {
            (SessionRow::Group(activity), SessionRow::Entry(SessionEntry::Part { .. })) => {
                assert_eq!(activity.kind, GroupKind::Activity);
                assert_eq!(activity.children.len(), 2, "reasoning and exploration nested under activity");
                let first = match &activity.children[0] {
                    SessionNode::Group { kind, .. } => *kind,
                    other => panic!("expected nested reasoning group: {other:?}"),
                };
                assert_eq!(first, GroupKind::Reasoning);
            }
            other => panic!("unexpected rows: {other:?}"),
        }
    }

    #[test]
    fn msg001_row_spacing_is_one_row_without_nested_double_gaps() {
        assert_eq!(ROW_MARGIN_TOP, 1);
        assert_eq!(row_margin_top(0, false), 1, "top-level entry");
        assert_eq!(row_margin_top(0, true), 1, "top-level thought");
        assert_eq!(row_margin_top(1, true), 1, "nested thought keeps one row");
        assert_eq!(row_margin_top(1, false), 0, "nested entry adds no gap");
        assert_eq!(row_margin_top(3, false), 0, "deep nesting adds no gap");
    }

    // MSG-002 ---------------------------------------------------------------

    #[test]
    fn msg002_default_disclosures_follow_verbosity_and_kind() {
        let state = DisclosureState::new();
        for kind in [GroupKind::Exploration, GroupKind::Instructions] {
            assert!(!state.group_expanded("s", "g", Verbosity::Low, kind));
            assert!(!state.group_expanded("s", "g", Verbosity::Medium, kind));
            assert!(state.group_expanded("s", "g", Verbosity::High, kind));
        }
        assert!(!state.group_expanded("s", "g", Verbosity::High, GroupKind::Reasoning));
        assert!(!state.group_expanded("s", "g", Verbosity::High, GroupKind::Activity));
    }

    #[test]
    fn msg002_thinking_and_grouping_modes_decide_rendering() {
        assert!(grouped_kind(GroupKind::Reasoning, ThinkingMode::Hide, Grouping::Auto));
        assert!(!grouped_kind(GroupKind::Reasoning, ThinkingMode::Show, Grouping::Auto));
        assert!(grouped_kind(GroupKind::Exploration, ThinkingMode::Hide, Grouping::Auto));
        assert!(!grouped_kind(GroupKind::Exploration, ThinkingMode::Hide, Grouping::None));
        assert!(grouped_kind(GroupKind::Activity, ThinkingMode::Hide, Grouping::None));
        assert!(grouped_kind(GroupKind::Instructions, ThinkingMode::Hide, Grouping::None));
    }

    #[test]
    fn msg002_saved_expansion_is_isolated_per_session() {
        let mut state = DisclosureState::new();
        state.set_group_expanded("session-a", "g1", true);
        assert!(state.group_expanded("session-a", "g1", Verbosity::Low, GroupKind::Exploration));
        assert!(
            !state.group_expanded("session-b", "g1", Verbosity::Low, GroupKind::Exploration),
            "another session keeps its own default"
        );
        assert!(!state.group_expanded("session-a", "g2", Verbosity::Low, GroupKind::Exploration));
        state.set_group_expanded("session-a", "g1", false);
        assert!(!state.group_expanded("session-a", "g1", Verbosity::High, GroupKind::Exploration));
        assert_eq!(state.saved_len(), 1);
    }

    // MSG-003 ---------------------------------------------------------------

    #[test]
    fn msg003_pending_tool_stays_visible_outside_collapsed_group() {
        let message = RowMessage::assistant(
            "m1",
            vec![read("call_1"), read("call_2"), read("call_3")],
        );
        let mut rows = reduce_session_rows(&[message], Verbosity::Medium, false);
        let blocked = PartRef::new("m1", "call_2");
        partition_pending(&mut rows, std::slice::from_ref(&blocked));
        let SessionRow::Group(group) = &rows[0] else {
            panic!("expected exploration group: {rows:?}");
        };
        assert_eq!(group.pending, vec![blocked.clone()]);
        assert_eq!(
            group_refs(group, false),
            vec![PartRef::new("m1", "call_1"), PartRef::new("m1", "call_3")],
            "blocked leaf is excluded from the collapsed summary"
        );
        assert!(group_refs(group, true).contains(&blocked));
        let order: Vec<PartRef> = group
            .children
            .iter()
            .filter_map(|node| match node {
                SessionNode::Entry {
                    entry: SessionEntry::Part { reference },
                } => Some(reference.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(order.last(), Some(&blocked), "blocked tools move to the group end");
    }

    #[test]
    fn msg003_reused_part_id_in_another_message_does_not_hide_the_wrong_row() {
        // Text rows separate the two groups so adjacent exploration groups do
        // not merge across messages.
        let first = RowMessage::assistant(
            "m1",
            vec![read("call_1"), RowPart::Text { text: "one".into() }],
        );
        let second = RowMessage::assistant(
            "m2",
            vec![read("call_1"), RowPart::Text { text: "two".into() }],
        );
        let mut rows = reduce_session_rows(&[first, second], Verbosity::Medium, false);
        let blocked = PartRef::new("m2", "call_1");
        partition_pending(&mut rows, std::slice::from_ref(&blocked));
        let (a, b) = match (&rows[0], &rows[2]) {
            (SessionRow::Group(a), SessionRow::Group(b)) => (a, b),
            other => panic!("expected two groups: {other:?}"),
        };
        assert!(a.pending.is_empty(), "m1's call_1 is not blocked");
        assert_eq!(b.pending, vec![blocked]);
        assert!(has_part(&rows, &PartRef::new("m1", "call_1")));
        assert!(has_part(&rows, &PartRef::new("m2", "call_1")));
    }

    // MSG-004 ---------------------------------------------------------------

    fn anchor_part(message: &str, part: &str) -> AnchorTarget {
        AnchorTarget::Part(PartRef::new(message, part))
    }

    #[test]
    fn msg004_saved_anchor_restores_the_same_screen_row() {
        let mut anchors = TimelineAnchors::new();
        anchors.register(AnchorRecord {
            target: anchor_part("m1", "text:0"),
            y: 4,
            height: 3,
            destroyed: false,
        });
        anchors.register(AnchorRecord {
            target: anchor_part("m2", "text:0"),
            y: 20,
            height: 2,
            destroyed: false,
        });
        let saved = save_scroll_anchor(&anchors, 10, |_| false).expect("anchor saved");
        assert_eq!(saved.target, anchor_part("m1", "text:0"));
        assert_eq!(saved.screen_y, -6);
        // A resize/backfill pushes the anchor down; scroll_top 100 keeps it.
        let target = restore_scroll_target(100, 30, 10, saved.screen_y);
        assert_eq!(target, 126);
        assert_eq!(max_scroll(200, 10), 190);
    }

    #[test]
    fn msg004_assistant_and_group_targets_use_one_row_inset() {
        let mut anchors = TimelineAnchors::new();
        anchors.register(AnchorRecord {
            target: anchor_part("m1", "text:0"),
            y: 10,
            height: 2,
            destroyed: false,
        });
        anchors.register(AnchorRecord {
            target: AnchorTarget::Group {
                group_id: "[\"m1\",\"call_1\",\"exploration\",0]".into(),
            },
            y: 11,
            height: 2,
            destroyed: false,
        });
        let saved = save_scroll_anchor(&anchors, 10, |message| message == "m1").expect("anchor");
        assert_eq!(saved.screen_y, 1, "group header within the one-row inset");
        assert!(matches!(saved.target, AnchorTarget::Group { .. }));
        // A group target always uses the one-row inset; drop it to observe the
        // zero inset that a non-assistant part row uses.
        anchors.unregister(&AnchorTarget::Group {
            group_id: "[\"m1\",\"call_1\",\"exploration\",0]".into(),
        });
        let saved = save_scroll_anchor(&anchors, 10, |_| false).expect("anchor");
        assert_eq!(saved.screen_y, 0, "non-assistant row uses the zero inset");
        assert_eq!(saved.target, anchor_part("m1", "text:0"));
    }

    #[test]
    fn msg004_group_anchor_survives_expansion_and_resize() {
        let message = RowMessage::assistant("m1", vec![read("call_1"), read("call_2")]);
        let rows = reduce_session_rows(&[message], Verbosity::Medium, false);
        let group = match &rows[0] {
            SessionRow::Group(group) => group,
            other => panic!("expected group: {other:?}"),
        };
        let id = group_id(group, 0).expect("group id");
        let target = AnchorTarget::Group { group_id: id.clone() };
        assert!(contains_anchor(&rows[0], &target));
        // Expansion changes geometry, not identity: a saved anchor still
        // restores to the same screen row after the header moved.
        let saved = SavedScrollAnchor {
            target: target.clone(),
            screen_y: 1,
        };
        let moved = restore_scroll_target(40, 50, 10, saved.screen_y);
        assert_eq!(moved, 79);
        assert!(contains_anchor(&rows[0], &saved.target));
    }

    #[test]
    fn msg004_away_from_bottom_does_not_reset_when_reading_history() {
        assert!(is_away_from_bottom(10, 200, 10, 40, 40));
        assert!(!is_away_from_bottom(190, 200, 10, 40, 40));
        assert!(is_away_from_bottom(190, 200, 10, 39, 40), "hidden newer rows");
    }

    // MSG-005 ---------------------------------------------------------------

    #[test]
    fn msg005_user_card_uses_one_cell_rail_and_shared_content_column() {
        let user = role_gutter(ChatRole::User);
        assert_eq!(user.border, Some('┃'));
        assert_eq!(user.padding_left, 2);
        assert_eq!(user.padding_vertical, 1);
        assert_eq!(user.content_column, CONTENT_GUTTER);
        assert!(user.hover_background);
        for role in [ChatRole::Assistant, ChatRole::System, ChatRole::Tool] {
            let gutter = role_gutter(role);
            assert_eq!(gutter.border, None, "{role:?} draws no rail");
            assert_eq!(gutter.padding_left, 3);
            assert_eq!(gutter.content_column, 3);
            assert_eq!(gutter.padding_vertical, 0);
            assert!(!gutter.hover_background);
        }
    }

    #[test]
    fn msg005_user_card_delivery_chips_hover_and_image_strip_match_upstream() {
        assert_eq!(
            role_gutter_for_delivery(ChatRole::User, false).rail_color,
            Some(UserRailColor::Agent)
        );
        assert_eq!(
            role_gutter_for_delivery(ChatRole::User, true).rail_color,
            Some(UserRailColor::BorderBase)
        );
        assert_eq!(role_gutter_for_delivery(ChatRole::Assistant, true).rail_color, None);
        assert_eq!(user_card_surface(false), UserCardSurface::RaisedBase);
        assert_eq!(user_card_surface(true), UserCardSurface::DecreasedRaisedBase);
        assert_eq!(
            user_chip_layout(),
            UserChipLayout { top_padding: 1, gap: 1, wraps: true }
        );
        assert_eq!(user_file_chip_kind("application/x-directory"), UserChipKind::Directory);
        assert_eq!(user_file_chip_kind("image/png"), UserChipKind::File);
        let strip = user_image_strip_layout(24, 5, 2).expect("images make a strip");
        assert_eq!((strip.height, strip.width, strip.visible), (6, 12, 3));
        assert_eq!(strip.overflow, Some(2));
        assert_eq!(
            (
                strip.padding_top,
                strip.padding_bottom,
                strip.padding_left,
                strip.padding_right,
                strip.gap,
                strip.overflow_width,
            ),
            (1, 1, 2, 2, 1, 8)
        );
        // Default paddingLeft is 3 for a non-card caller (`:2665`).
        assert_eq!(user_image_strip_layout(24, 3, 3).unwrap().padding_left, 3);
        // Fewer than four images: no overflow chip.
        assert_eq!(user_image_strip_layout(24, 3, 2).unwrap().overflow, None);
        assert_eq!(user_image_strip_layout(24, 0, 2), None);
        // Height is bounded to [4, 8].
        assert_eq!(user_image_strip_layout(4, 1, 2).unwrap().height, 4);
        assert_eq!(user_image_strip_layout(400, 1, 2).unwrap().height, 8);
    }

    #[test]
    fn msg005_user_rail_color_is_delivery_exclusive_no_blend() {
        const GREEN: [u8; 4] = [0, 128, 0, 255];
        const BORDER: [u8; 4] = [72, 72, 72, 255];
        // Pending delivery selects the flat border base, never a blend.
        assert_eq!(
            user_rail_color_for_delivery(true, Some(GREEN), BORDER),
            BORDER
        );
        // Settled delivery selects the agent colour.
        assert_eq!(
            user_rail_color_for_delivery(false, Some(GREEN), BORDER),
            GREEN
        );
        // No agent colour falls back to border base (prompt-rail fallback).
        assert_eq!(user_rail_color_for_delivery(false, None, BORDER), BORDER);
        // The enum resolver is the same mutually exclusive choice.
        assert_eq!(user_rail_color(UserRailColor::BorderBase, Some(GREEN), BORDER), BORDER);
        assert_eq!(user_rail_color(UserRailColor::Agent, Some(GREEN), BORDER), GREEN);
        // Chip labels match upstream `" skill " | " dir " | " file "`.
        assert_eq!(user_chip_label(UserChipKind::Skill), "skill");
        assert_eq!(user_chip_label(UserChipKind::Directory), "dir");
        assert_eq!(user_chip_label(UserChipKind::File), "file");
    }

    // MSG-006 ---------------------------------------------------------------

    #[test]
    fn msg006_text_selection_blocks_row_actions() {
        for action in [
            RowAction::MessageActions,
            RowAction::GroupToggle,
            RowAction::ToolDetails,
            RowAction::OpenChildSession,
        ] {
            assert!(!click_acts(true, action), "selection suppresses {action:?}");
            assert!(click_acts(false, action), "plain click runs {action:?}");
        }
    }

    // MSG-010 ---------------------------------------------------------------

    fn timed_step(
        id: &str,
        created: u64,
        completed: u64,
        streamed: Option<u64>,
        output: u64,
    ) -> RowMessage {
        let mut message = RowMessage::assistant(
            id,
            vec![RowPart::Text {
                text: "answer".into(),
            }],
        );
        message.timing = RowTiming {
            created_ms: created,
            completed_ms: Some(completed),
            streamed_ms: streamed,
        };
        message.tokens = Some(TokenUsage {
            output,
            ..TokenUsage::default()
        });
        message
    }

    #[test]
    fn msg010_legacy_turn_duration_uses_nearest_input() {
        let mut input = RowMessage::user("u1", "prompt");
        input.timing.created_ms = 1_000;
        let step = timed_step("a1", 1_100, 2_100, Some(2_100), 10);
        let messages = vec![input, step.clone()];
        assert!(legacy_turns(&messages));
        assert_eq!(turn_duration(&step, &messages, Some(1), true), 1_100);
        assert_eq!(turn_tokens_per_second(&step, &messages, Some(1), true), Some(10.0));
    }

    #[test]
    fn msg010_marker_era_steers_and_model_switches_stay_in_turn() {
        let mut input = RowMessage::user("u1", "prompt");
        input.timing.created_ms = 1_000;
        let mut switched = RowMessage::notice("n1", "model switched");
        switched.timing.created_ms = 1_200;
        let mut steer = RowMessage::user("u2", "steer");
        steer.timing.created_ms = 1_300;
        let step = timed_step("a1", 1_400, 3_000, Some(3_000), 20);
        let mut idle = RowMessage::idle("i1");
        idle.timing.created_ms = 3_100;
        let messages = vec![input, switched, steer, step.clone(), idle];
        assert!(!legacy_turns(&messages));
        assert_eq!(turn_duration(&step, &messages, Some(3), false), 2_000);
        assert_eq!(
            turn_tokens_per_second(&step, &messages, Some(3), false),
            Some(12.5),
            "1600ms provider-active time for 20 tokens"
        );
    }

    #[test]
    fn msg010_missing_stream_timestamp_disables_throughput_only() {
        let mut input = RowMessage::user("u1", "prompt");
        input.timing.created_ms = 0;
        let step = timed_step("a1", 100, 1_100, None, 30);
        let messages = vec![input, step.clone()];
        assert_eq!(turn_duration(&step, &messages, Some(1), true), 1_100);
        assert_eq!(turn_tokens_per_second(&step, &messages, Some(1), true), None);
        let zero_output = timed_step("a2", 100, 1_100, Some(1_000), 0);
        let messages = vec![RowMessage::user("u0", "p"), zero_output.clone()];
        assert_eq!(turn_tokens_per_second(&zero_output, &messages, Some(1), true), None);
    }

    #[test]
    fn msg010_absent_input_anchor_falls_back_to_assistant_creation() {
        let step = timed_step("a1", 5_000, 6_000, Some(5_500), 10);
        let messages = vec![step.clone()];
        assert_eq!(input_index(&messages, 1, true), None);
        // No input precedes the step: upstream falls back to the step's own
        // creation time (`rows.ts:438-439`).
        assert_eq!(turn_duration(&step, &messages, Some(0), true), 1_000);
        assert_eq!(turn_duration(&step, &messages, Some(0), false), 1_000);
        // Throughput still aggregates the step from index 0 (`rows.ts:448-456`).
        assert_eq!(turn_tokens_per_second(&step, &messages, Some(0), true), Some(20.0));
        let mut unfinished = step.clone();
        unfinished.timing.completed_ms = None;
        assert_eq!(turn_duration(&unfinished, &messages, Some(0), true), 0);
    }

    #[test]
    fn msg010_throughput_sums_provider_active_time_not_tool_time() {
        let mut input = RowMessage::user("u1", "prompt");
        input.timing.created_ms = 0;
        let first = timed_step("a1", 100, 600, Some(600), 10);
        let mut second = timed_step("a2", 10_000, 30_000, Some(11_000), 30);
        second.parts.push(RowPart::Tool {
            id: "call_1".into(),
            name: "bash".into(),
        });
        let messages = vec![input, first, second.clone()];
        // 40 tokens over (500 + 1000)ms = 26.67 tok/s; the 19s tool wait is
        // excluded because only provider streamed durations are summed.
        let tps = turn_tokens_per_second(&second, &messages, Some(2), true).expect("throughput");
        assert!((tps - 26.666_666_666).abs() < 1e-6, "{tps}");
    }

    #[test]
    fn turn_usage_flushes_at_idle_and_keeps_previous_cache() {
        let mut first = timed_step("a1", 100, 900, Some(900), 10);
        first.tokens = Some(TokenUsage {
            input: 5,
            output: 10,
            cache_read: 3,
            ..TokenUsage::default()
        });
        first.model = Some("openai/gpt-5".into());
        let idle = RowMessage::idle("i1");
        let rows = reduce_session_rows(&[first, idle], Verbosity::Medium, true);
        let usage = rows
            .iter()
            .find_map(|row| match row {
                SessionRow::Entry(SessionEntry::TurnUsage {
                    message_ids,
                    previous_cache,
                }) => Some((message_ids.clone(), previous_cache.clone())),
                _ => None,
            })
            .expect("turn usage row");
        assert_eq!(usage.0, vec!["a1".to_owned()]);
        assert!(usage.1.is_none());
        assert_eq!(
            cache_reuse_drop(
                None,
                &CacheUsage {
                    read: 1,
                    provider_id: "openai".into(),
                    model_id: "gpt-5".into(),
                    variant: None,
                }
            ),
            None
        );
        let previous = CacheUsage {
            read: 4_096,
            provider_id: "openai".into(),
            model_id: "gpt-5".into(),
            variant: None,
        };
        let jitter = CacheUsage {
            read: 2_560,
            provider_id: "openai".into(),
            model_id: "gpt-5".into(),
            variant: None,
        };
        assert_eq!(cache_reuse_drop(Some(&previous), &jitter), None, "OpenAI bucket jitter");
        let switched = CacheUsage {
            read: 0,
            provider_id: "anthropic".into(),
            model_id: "claude".into(),
            variant: None,
        };
        assert_eq!(cache_reuse_drop(Some(&previous), &switched), None);
        assert_eq!(cache_reuse_drop(Some(&previous), &CacheUsage { read: 0, ..previous.clone() }), Some(4_096));
    }

    // Grouping/tree/anchors/paging primitives -------------------------------

    #[test]
    fn group_split_and_merge_are_seam_inverse() {
        let message = RowMessage::assistant(
            "m1",
            vec![read("c1"), read("c2"), read("c3")],
        );
        let rows = reduce_session_rows(&[message], Verbosity::Medium, false);
        let SessionRow::Group(group) = &rows[0] else {
            panic!("expected group");
        };
        let (left, right) = split_groups(&group.children, 2).expect("split");
        assert_eq!(left.iter().map(|node| node.size()).sum::<usize>(), 2);
        assert_eq!(right.iter().map(|node| node.size()).sum::<usize>(), 1);
        assert_eq!(merge_groups(&left, &right), group.children);
        assert_eq!(
            split_groups::<SessionEntry, GroupKind>(&group.children, 9),
            Err(SplitError::ExceedsEntryCount)
        );
        // Splitting inside a group node rebuilds both ancestors, so merging
        // the halves restores the single group.
        let wrapped = vec![SessionNode::Group {
            kind: group.kind,
            children: group.children.clone(),
            size: group.size,
        }];
        let (left, right) = split_groups(&wrapped, 2).expect("split inside group");
        assert_eq!(left.len(), 1);
        assert_eq!(right.len(), 1);
        assert_eq!(merge_groups(&left, &right), wrapped);
    }

    #[test]
    fn append_merges_same_kind_group_and_closes_previous() {
        let mut rows = Vec::new();
        append(
            &mut rows,
            PartRef::new("m1", "reasoning:0"),
            AppendPart::Reasoning { completed_ms: None },
            0,
            Verbosity::Medium,
        );
        append(
            &mut rows,
            PartRef::new("m1", "reasoning:1"),
            AppendPart::Reasoning { completed_ms: Some(5) },
            1,
            Verbosity::Medium,
        );
        assert_eq!(rows.len(), 1);
        let SessionRow::Group(group) = &rows[0] else {
            panic!("expected reasoning group");
        };
        assert_eq!(group.size, 2);
        assert!(!group.completed, "an open reasoning group is not completed yet");
        let key = row_key(&rows[0]);
        let expected = group_id(group, 0);
        append(
            &mut rows,
            PartRef::new("m1", "text:0"),
            AppendPart::Text,
            1,
            Verbosity::Medium,
        );
        assert_eq!(rows.len(), 2);
        assert!(
            matches!(&rows[0], SessionRow::Group(group) if group.completed),
            "the following text row completes the reasoning group"
        );
        assert!(!has_part(&rows, &PartRef::new("m1", "missing")));
        assert_eq!(key, expected);
    }

    #[test]
    fn anchor_registry_skips_destroyed_and_reports_positions() {
        let mut anchors = TimelineAnchors::new();
        let key = anchors.register(AnchorRecord {
            target: anchor_part("m1", "text:0"),
            y: 4,
            height: 2,
            destroyed: false,
        });
        assert_eq!(key, "[\"part\",\"m1\",\"text:0\"]");
        anchors.register(AnchorRecord {
            target: anchor_part("m1", "text:1"),
            y: 6,
            height: 2,
            destroyed: false,
        });
        anchors.register(AnchorRecord {
            target: anchor_part("m2", "text:0"),
            y: 9,
            height: 0,
            destroyed: false,
        });
        anchors.register(AnchorRecord {
            target: anchor_part("m3", "text:0"),
            y: 12,
            height: 2,
            destroyed: true,
        });
        let positions = anchors.message_positions();
        assert_eq!(positions, vec![("m1".to_owned(), 4)]);
        assert_eq!(anchors.for_message("m2"), None, "zero-height never registers");
        assert_eq!(anchors.list().len(), 2);
        anchors.unregister(&anchor_part("m1", "text:0"));
        assert!(anchors.get(&anchor_part("m1", "text:0")).is_none());
    }

    #[test]
    fn history_boundary_fill_stops_on_group_or_no_progress() {
        let message = RowMessage::assistant("m1", vec![read("c1")]);
        let group_row = reduce_session_rows(&[message], Verbosity::Medium, false);
        assert!(boundary_fill_required(&group_row, true));
        assert!(!boundary_fill_required(&group_row, false), "no older pages");
        assert!(!boundary_fill_progressed(10, 10), "empty page ends the loop");
        assert!(boundary_fill_progressed(10, 11));
        let user_row = reduce_session_rows(&[RowMessage::user("u1", "hi")], Verbosity::Medium, false);
        assert!(!boundary_fill_required(&user_row, true));
    }

    #[test]
    fn history_prepend_coalesces_cancels_and_compensates() {
        let mut prepend = HistoryPrepend::new();
        assert_eq!(prepend.request(0, false, false), PrependDecision::Ignored);
        assert_eq!(prepend.request(0, false, true), PrependDecision::Start);
        assert_eq!(prepend.request(-4, false, true), PrependDecision::Coalesced);
        assert_eq!(prepend.request(0, true, true), PrependDecision::Coalesced);
        let completion = prepend.complete(100, 130, true).expect("completion");
        assert_eq!(completion.scroll_by, 30);
        assert!(completion.continuation);
        assert!(!completion.after);
        assert_eq!(prepend.pending(), None);

        assert_eq!(prepend.request(5, true, true), PrependDecision::Start);
        prepend.cancel();
        let completion = prepend.complete(100, 101, true).expect("completion");
        assert_eq!(completion.scroll_by, 6);
        assert!(!completion.continuation, "cancel drops callbacks");
        assert!(!completion.after);

        assert_eq!(prepend.request(0, false, true), PrependDecision::Start);
        assert!(!prepend.defer_after());
        let completion = prepend.complete(100, 110, true).expect("completion");
        assert_eq!(completion.scroll_by, 10);
        assert!(completion.after);
        assert!(prepend.defer_after(), "nothing pending runs immediately");

        assert_eq!(prepend.request(1, true, true), PrependDecision::Start);
        assert!(!prepend.fail(false));
        assert_eq!(prepend.pending(), None);
    }

    #[test]
    fn mount_budget_counts_collapsed_groups_as_one() {
        let message = RowMessage::assistant("m1", vec![read("c1"), read("c2"), read("c3")]);
        let rows = reduce_session_rows(&[RowMessage::user("u1", "go"), message], Verbosity::Medium, false);
        let collapsed = row_weight(&rows[1], |_, _| false, |_| true);
        assert_eq!(collapsed, 1);
        let expanded = row_weight(&rows[1], |_, _| true, |_| true);
        assert_eq!(expanded, 4, "expanded group includes its disclosure header");
        let ungrouped = row_weight(&rows[1], |_, _| true, |_| false);
        assert_eq!(ungrouped, 3);
        let weights = vec![1, 1, 3, 1];
        assert_eq!(rows_before(&weights, 4, 3), 2);
        assert_eq!(rows_after(&weights, 1, 3), 3);
        assert_eq!(rows_before(&weights, 0, 10), 0);
        assert_eq!(rows_after(&weights, 4, 10), 4);
    }

    #[test]
    fn mount_budget_recurses_nested_groups_exactly_like_upstream() {
        // Upstream `rowWeight` recursion (`mount-budget.ts:15-21`): an expanded
        // group costs its own header (the reduce is seeded with 1) plus each
        // child — a collapsed nested group costs 1, an expanded nested group
        // its own header plus its leaves, and an ungrouped nested group its raw
        // `size` regardless of expansion. The live transcript flattens nested
        // groups into one level, so only this model-level test pins the
        // recursive semantics a flat-sum regression would silently break.
        let entries = |ids: &[&str]| -> Vec<SessionNode> {
            ids.iter()
                .map(|id| GroupNode::Entry {
                    entry: SessionEntry::Message {
                        message_id: (*id).to_owned(),
                    },
                })
                .collect()
        };
        let nested = SessionRow::Group(SessionGroup {
            kind: GroupKind::Activity,
            size: 5,
            completed: false,
            pending: Vec::new(),
            children: vec![
                GroupNode::Group {
                    kind: GroupKind::Reasoning,
                    size: 3,
                    children: entries(&["r1", "r2", "r3"]),
                },
                GroupNode::Group {
                    kind: GroupKind::Exploration,
                    size: 2,
                    children: entries(&["e1", "e2"]),
                },
            ],
        });
        // Everything expanded except the nested reasoning group (collapsed):
        // 1 header + 1 collapsed + (1 header + 2 leaves) = 5.
        assert_eq!(
            row_weight(&nested, |_, kind| kind != GroupKind::Reasoning, |_| true),
            5,
            "expanded activity = 1 header + collapsed reasoning (1) + expanded exploration (1 + 2)"
        );
        // Every level expanded: 1 + (1 + 3) + (1 + 2) = 8.
        assert_eq!(row_weight(&nested, |_, _| true, |_| true), 8);
        // Grouping disabled for reasoning only: its raw size counts even
        // though every level stays expanded: 1 + 3 + (1 + 2) = 7 (with
        // grouping on, case two above, the same tree costs 8).
        assert_eq!(
            row_weight(&nested, |_, kind| kind != GroupKind::Reasoning, |kind| kind != GroupKind::Reasoning),
            7,
            "ungrouped nested reasoning costs its size, not its expansion state"
        );
    }

    #[test]
    fn message_navigation_uses_user_text_and_assistant_text_parts() {
        let messages = vec![
            RowMessage::user("u1", "first"),
            RowMessage::assistant("a1", vec![read("c1")]),
            RowMessage::assistant(
                "a2",
                vec![RowPart::Text {
                    text: "readable".into(),
                }],
            ),
            RowMessage::notice("n1", "notice"),
            RowMessage::user("u2", "second"),
        ];
        let children: Vec<NavChild> = ["u1", "a1", "a2", "n1", "u2"]
            .iter()
            .enumerate()
            .map(|(index, id)| NavChild {
                id: (*id).to_owned(),
                y: index as i64 * 5,
            })
            .collect();
        let next = find_message_boundary(
            NavDirection::Next,
            &children,
            &messages,
            0,
            0,
            None,
            false,
        )
        .expect("next");
        assert_eq!(next.id, "a2", "the row at scroll_top is not after it");
        let next = find_message_boundary(
            NavDirection::Next,
            &children,
            &messages,
            -1,
            0,
            None,
            false,
        )
        .expect("next from above");
        assert_eq!(
            next.id, "a2",
            "a row whose transformed y equals scroll_top is not strictly after it (message-navigation.ts:46)"
        );
        let prev = find_message_boundary(
            NavDirection::Prev,
            &children,
            &messages,
            0,
            12,
            None,
            false,
        )
        .expect("prev");
        assert_eq!(prev.id, "a2", "tool-only assistant rows are skipped");
        assert_eq!(prev.y, -2, "viewport-relative y (message-navigation.ts:39)");
        assert_eq!(prev.top, 0, "assistant top inset clamps at zero (message-navigation.ts:40)");
        let user_only = find_message_boundary(
            NavDirection::Next,
            &children,
            &messages,
            0,
            0,
            Some("u1"),
            true,
        )
        .expect("next user");
        assert_eq!(user_only.id, "u2");
    }

    #[test]
    fn anchor_hold_keeps_disclosure_header_on_its_viewport_row() {
        // `holdAnchor` scrolls to `layoutTop(node) - held.top`; the same
        // arithmetic as the restore path with zero viewport offset.
        assert_eq!(restore_scroll_target(0, 42, 0, 7), 35);
        assert_eq!(message_navigation_slack(50, 10, 55, 0), 5);
        assert_eq!(message_navigation_slack(40, 10, 55, 0), 0);
    }

    #[test]
    fn chat_entries_project_into_the_same_grouped_rows() {
        let mut assistant = ChatMessage::text(
            ChatRole::Assistant,
            "visible",
        );
        assistant.parts.insert(
            0,
            ChatPart::Thinking("thinking about it".to_owned()),
        );
        assistant.parts.push(ChatPart::ToolState {
            name: "read".into(),
            id: "call_1".into(),
            status: "completed".into(),
            args: String::new(),
            output: String::new(),
            is_error: false,
            metadata: String::new(),
        });
        let entries = vec![
            ("u1".to_owned(), ChatMessage::text(ChatRole::User, "prompt")),
            ("a1".to_owned(), assistant),
            ("s1".to_owned(), ChatMessage::text(ChatRole::System, "notice")),
        ];
        let rows = rows_from_chat_entries(&entries, Verbosity::Medium);
        assert_eq!(rows.len(), 5);
        assert!(matches!(rows[0], SessionRow::Entry(SessionEntry::Message { .. })));
        assert!(matches!(&rows[1], SessionRow::Group(group) if group.kind == GroupKind::Reasoning));
        assert!(matches!(&rows[2], SessionRow::Entry(SessionEntry::Part { .. })));
        assert!(matches!(&rows[3], SessionRow::Group(group) if group.kind == GroupKind::Exploration));
        assert!(matches!(rows[4], SessionRow::Entry(SessionEntry::Message { .. })));
    }

    #[test]
    fn empty_text_and_reasoning_parts_consume_ordinals_but_do_not_render() {
        let message = RowMessage::assistant(
            "m1",
            vec![
                RowPart::Text {
                    text: "   ".into(),
                },
                RowPart::Text {
                    text: "kept".into(),
                },
                thought("  ", true),
                thought("thought", true),
            ],
        );
        let rows = reduce_session_rows(&[message], Verbosity::Medium, false);
        let refs: Vec<PartRef> = rows.iter().flat_map(references).collect();
        assert_eq!(
            refs,
            vec![
                PartRef::new("m1", "text:1"),
                PartRef::new("m1", "reasoning:1"),
            ]
        );
    }
}
