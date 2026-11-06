#![forbid(unsafe_code)]
//! Mermaid code-fence lifecycle for the native Markdown flow (package P13b).
//!
//! Scope and depth limit (explicit)
//! --------------------------------
//! This module models the **fence lifecycle only**: fence/language recognition,
//! info-string parsing, a bounded input, the deterministic fallback to the
//! plain code block, last-good/partial recovery, a render-output cache keyed by
//! `(source, width, theme)` and the row/cell footprint of the resulting block.
//!
//! It deliberately does **not** model a full Mermaid grammar. Flowchart
//! detection/layout/paint is implemented by [`CellDiagramPainter`] below; the
//! remaining kinds fail closed. Without any painter every fence falls back to
//! the plain code block (fail-closed), which is also what upstream does
//! whenever the diagram cannot be produced.
//!
//! Evidence pins (OpenCode pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`)
//! --------------------------------------------------------------------
//! * `packages/tui/src/plugin/builtins.ts:15,29` – the Merman plugin is a
//!   builtin.
//! * `packages/merman/src/plugin.ts:5-14` – registers the code-block renderer
//!   for the single language key `"mermaid"`.
//! * `packages/merman/src/detect.ts:9-16` – detection order: flowchart, gantt,
//!   gitGraph, sequence, state, timeline; unknown content yields `undefined`.
//! * Diagram header matchers:
//!   `flowchart/parser.ts:17,277-279`, `gantt/parser.ts:5,11-13`,
//!   `gitgraph/parser.ts:5,8-10`, `sequence/parser.ts:120-122`,
//!   `state/parser.ts:30-32,77-79`, `timeline/parser.ts:5,10-12`; blank lines
//!   and `%%` comments are skipped by `core/mermaid.ts:19-25`.
//! * `packages/merman/src/core/canvas.ts:43-56,79-82` – non-negative safe
//!   integer dimensions and the 250 000-cell canvas limit
//!   (`DiagramCanvasSizeError`).
//! * `packages/merman/src/diagnostics.ts:9-18` – `MermaidSyntaxError` carries a
//!   **1-based** `lineNumber` for the offending line.
//! * `packages/merman/src/markdown.ts:66-75` – the diagram renderable is
//!   `width: "100%"`, `height: prepared.height`, `wrapMode: "none"`,
//!   `selectable: false`, `marginTop: 1`.
//! * `packages/merman/src/markdown.ts:76-108` – left-button drag scrolls
//!   horizontally (`scrollX -= dx`), clears the selection on mouse-down and
//!   consumes only horizontal wheel scroll; vertical wheel passes through to
//!   the transcript.
//! * `packages/merman/src/markdown.ts:268-320` – the renderer: `lastGood` keyed
//!   by the stable default-render block id (`272-277`), configured
//!   `layoutMaxWidth` default 120 clamped to `ctx.width` (`279-281`), success
//!   path (`283-287`), syntax-error recovery (`289-315`) and size-error
//!   fallback (`316-318`).
//! * `packages/merman/src/markdown.ts:322-336` – `claimLastGood` copies the
//!   prepared diagram, stores it per block id and drops it when the owning
//!   renderable is destroyed (identity-checked microtask).
//! * `packages/merman/src/markdown.ts:298` – a syntax error on line <= 2, or
//!   with any non-blank line after it, falls back (renderer returns
//!   `undefined`, i.e. the normal code block); otherwise it retries the prefix
//!   `lines.slice(0, lineNumber - 1)` (`303`) and requires a non-zero height
//!   (`307`).
//!
//! The code-block fallback paints one row per source line; the Markdown flow
//! splits fence content the same way
//! (`crates/cli/src/native_markdown_flow.rs`, `fence_parts`).
//!
//! Integration notes for `native_markdown_flow`
//! --------------------------------------------
//! `MarkdownBlock` exposes `id` (stable block identity), `body`
//! (`BlockBody::Code { lines, lang }`, where `lines.join("\n")` is upstream
//! `token.text`) and `source_raw` (upstream `token.raw`). Call
//! [`MermaidFence::from_code_block`] with the block id, the fence language and
//! the joined code body, then [`MermaidRenderer::render`] with the current
//! terminal width. A [`MermaidOutcome::Fallback`] means: keep painting the
//! normal code block with the preserved source. A [`MermaidOutcome::Rendered`]
//! carries the footprint to reserve (see [`DiagramFrame::footprint_rows`])
//! and a [`DiagramViewport`] for drag/wheel scrolling.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;

/// Upstream registers exactly this code-fence language
/// (`packages/merman/src/plugin.ts:8-13`).
pub const MERMAID_LANGUAGE: &str = "mermaid";
/// Default `layoutMaxWidth` (`packages/merman/src/markdown.ts:279-280`).
pub const DEFAULT_LAYOUT_MAX_WIDTH: usize = 120;
/// `MAX_DIAGRAM_CELLS` (`packages/merman/src/core/canvas.ts:43`).
pub const MAX_DIAGRAM_CELLS: usize = 250_000;
/// Diagram block `marginTop` (`packages/merman/src/markdown.ts:74`).
pub const DIAGRAM_MARGIN_TOP: usize = 1;
/// Bounded Mermaid source accepted by this module. Matches the Markdown flow
/// bound (`native_markdown_flow::MAX_FLOW_BYTES`); a larger fence fails closed.
pub const MAX_MERMAID_SOURCE_BYTES: usize = 64 * 1024;
/// Maximum cached render outputs (OC2 bound; upstream caches per block id).
pub const MAX_CACHE_ENTRIES: usize = 32;
/// Maximum retained last-good frames (OC2 bound; upstream uses one map entry
/// per live Markdown block).
pub const MAX_LAST_GOOD_BLOCKS: usize = 32;

/// Mermaid diagram kinds in upstream detection order (`detect.ts:9-16`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MermaidDiagramKind {
    Flowchart,
    Gantt,
    GitGraph,
    Sequence,
    State,
    Timeline,
}

impl MermaidDiagramKind {
    /// Upstream kind tag (`diagnostics.ts:1`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flowchart => "flowchart",
            Self::Gantt => "gantt",
            Self::GitGraph => "gitGraph",
            Self::Sequence => "sequence",
            Self::State => "state",
            Self::Timeline => "timeline",
        }
    }
}

impl fmt::Display for MermaidDiagramKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Fence recognition and info-string parsing ─────────────────────────

/// Parsed opening fence line. `language` is the first info-string token,
/// lowercased, with `{`, `}` and `.` trimmed, matching the Markdown flow's
/// `fence_parts` (`Markdown.ts:1171-1186`: the painted content is
/// `token.text`, the info string is not painted).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FenceInfo {
    pub marker: char,
    pub marker_len: usize,
    pub info: String,
    pub language: Option<String>,
}

/// Parse the opening fence line of `raw`. Returns `None` when the first line
/// is not a CommonMark fence opener (`<= 3` spaces indent, at least three
/// backticks or tildes).
#[must_use]
pub fn parse_fence_info(raw: &str) -> Option<FenceInfo> {
    let first = raw.split('\n').next().unwrap_or("").trim_end_matches('\r');
    let indent = first.len() - first.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let trimmed = first.trim_start_matches(' ');
    let marker = *trimmed.as_bytes().first()? as char;
    if marker != '`' && marker != '~' {
        return None;
    }
    let marker_len = trimmed
        .as_bytes()
        .iter()
        .take_while(|byte| **byte == marker as u8)
        .count();
    if marker_len < 3 {
        return None;
    }
    let info = trimmed[marker_len..].trim().to_owned();
    let language = canonical_language(&info);
    Some(FenceInfo {
        marker,
        marker_len,
        info,
        language,
    })
}

fn canonical_language(info: &str) -> Option<String> {
    let token = info.split_whitespace().next()?;
    let cleaned = token.trim_matches(['{', '}', '.']).to_ascii_lowercase();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// True when `language` is the Mermaid fence language (case-insensitive).
#[must_use]
pub fn accepts_language(language: &str) -> bool {
    canonical_language(language).is_some_and(|language| language == MERMAID_LANGUAGE)
}

// ── Detection ─────────────────────────────────────────────────────────

/// First non-blank, non-`%%` line, trimmed (`core/mermaid.ts:19-25`).
#[must_use]
pub fn first_meaningful_line(source: &str) -> Option<&str> {
    source
        .split('\n')
        .map(|line| line.trim_end_matches('\r').trim())
        .find(|line| !line.is_empty() && !line.starts_with("%%"))
}

fn word_and_tail(line: &str) -> (&str, Option<&str>) {
    match line.find(char::is_whitespace) {
        Some(index) => (&line[..index], Some(line[index..].trim_start())),
        None => (line, None),
    }
}

fn is_flowchart_header(line: &str) -> bool {
    // /^(flowchart|graph)(?:\s+(TB|TD|BT|LR|RL))?$/i (`flowchart/parser.ts:17`)
    let (head, tail) = word_and_tail(line);
    if !head.eq_ignore_ascii_case("flowchart") && !head.eq_ignore_ascii_case("graph") {
        return false;
    }
    match tail {
        None => true,
        Some(direction) => matches!(
            direction.to_ascii_uppercase().as_str(),
            "TB" | "TD" | "BT" | "LR" | "RL"
        ),
    }
}

fn is_gantt_header(line: &str) -> bool {
    // /^gantt$/i (`gantt/parser.ts:5`)
    line.eq_ignore_ascii_case("gantt")
}

fn is_gitgraph_header(line: &str) -> bool {
    // /^gitGraph(?:\s+(LR|TB|BT))?\s*:?$/i (`gitgraph/parser.ts:5`)
    let line = line.trim();
    let line = line.trim_end_matches(':').trim_end();
    let (head, tail) = word_and_tail(line);
    if !head.eq_ignore_ascii_case("gitgraph") {
        return false;
    }
    match tail {
        None => true,
        Some(direction) => matches!(direction.trim().to_ascii_uppercase().as_str(), "LR" | "TB" | "BT"),
    }
}

fn is_sequence_header(line: &str) -> bool {
    // `sequence/parser.ts:120-122` compares the lowercased line.
    line.trim().eq_ignore_ascii_case("sequencediagram")
}

fn is_state_header(line: &str) -> bool {
    // `state/parser.ts:30-32`
    let line = line.trim();
    line.eq_ignore_ascii_case("statediagram-v2") || line.eq_ignore_ascii_case("statediagram")
}

fn is_timeline_header(line: &str) -> bool {
    // /^timeline(?:\s+(TD|LR))?$/i (`timeline/parser.ts:5`)
    let (head, tail) = word_and_tail(line.trim());
    if !head.eq_ignore_ascii_case("timeline") {
        return false;
    }
    match tail {
        None => true,
        Some(direction) => matches!(direction.to_ascii_uppercase().as_str(), "TD" | "LR"),
    }
}

/// Mirrors `detectMermaidDiagram` including its precedence
/// (`packages/merman/src/detect.ts:9-16`). Only headers are inspected; the
/// body grammar is the painter's concern.
#[must_use]
pub fn detect_kind(source: &str) -> Option<MermaidDiagramKind> {
    let line = first_meaningful_line(source)?;
    if is_flowchart_header(line) {
        Some(MermaidDiagramKind::Flowchart)
    } else if is_gantt_header(line) {
        Some(MermaidDiagramKind::Gantt)
    } else if is_gitgraph_header(line) {
        Some(MermaidDiagramKind::GitGraph)
    } else if is_sequence_header(line) {
        Some(MermaidDiagramKind::Sequence)
    } else if is_state_header(line) {
        Some(MermaidDiagramKind::State)
    } else if is_timeline_header(line) {
        Some(MermaidDiagramKind::Timeline)
    } else {
        None
    }
}

// ── Fence value object ────────────────────────────────────────────────

/// One Mermaid fence handed to the renderer. Owns a bounded copy of the
/// source so that fallback and last-good frames can preserve it verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MermaidFence {
    /// Stable Markdown block identity (`MarkdownBlock.id`; upstream
    /// `context.defaultRender()?.id`, `markdown.ts:276-277`).
    pub block_id: usize,
    /// Canonical lowercased language token (`"mermaid"`).
    pub language: String,
    /// `token.text`: fence content without the fence lines and info string.
    pub source: String,
    /// Original source length in bytes (may exceed `source.len()`).
    pub total_bytes: usize,
    /// True when `source` was clipped at `MAX_MERMAID_SOURCE_BYTES`.
    pub truncated: bool,
}

impl MermaidFence {
    /// Build a fence from the flow's code-block fields. Returns `None` when
    /// the language is not `mermaid`, so the caller can keep the plain code
    /// block without consulting a renderer.
    #[must_use]
    pub fn from_code_block(info_language: Option<&str>, source: &str, block_id: usize) -> Option<Self> {
        let language = canonical_language(info_language?)?;
        if language != MERMAID_LANGUAGE {
            return None;
        }
        let total_bytes = source.len();
        let (source, truncated) = clip_source(source);
        Some(Self {
            block_id,
            language,
            total_bytes,
            source,
            truncated,
        })
    }

    /// True when the fence is larger than the accepted bound.
    #[must_use]
    pub fn exceeds_bound(&self) -> bool {
        self.truncated
    }
}

fn clip_source(source: &str) -> (String, bool) {
    if source.len() <= MAX_MERMAID_SOURCE_BYTES {
        return (source.to_owned(), false);
    }
    let mut end = MAX_MERMAID_SOURCE_BYTES;
    while end > 0 && !source.is_char_boundary(end) {
        end -= 1;
    }
    (source[..end].to_owned(), true)
}

// ── Theme key ─────────────────────────────────────────────────────────

/// Color mode (`packages/merman/src/palette.ts:69-80`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ColorMode {
    Dark,
    Light,
}

/// Cache identity of the resolved diagram palette. The real palette is derived
/// from theme RGBA tokens (`palette.ts:5-45`, `69-80`); this module keeps only
/// the stable identity used as the cache key and deliberately does not resolve
/// colors.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MermaidTheme {
    pub name: String,
    pub mode: ColorMode,
}

impl MermaidTheme {
    #[must_use]
    pub fn new(name: impl Into<String>, mode: ColorMode) -> Self {
        Self {
            name: name.into(),
            mode,
        }
    }

    #[must_use]
    pub fn dark(name: impl Into<String>) -> Self {
        Self::new(name, ColorMode::Dark)
    }

    #[must_use]
    pub fn light(name: impl Into<String>) -> Self {
        Self::new(name, ColorMode::Light)
    }

    /// Stable cache identity.
    #[must_use]
    pub fn id(&self) -> String {
        let mode = match self.mode {
            ColorMode::Dark => "dark",
            ColorMode::Light => "light",
        };
        format!("{}/{mode}", self.name)
    }
}

impl Default for MermaidTheme {
    fn default() -> Self {
        Self::dark("opencode")
    }
}

// ── Painter boundary ──────────────────────────────────────────────────

/// Request handed to the painter. `width` is the effective layout width
/// (`min(layoutMaxWidth, terminal width)`, `markdown.ts:279-281`).
#[derive(Clone, Copy, Debug)]
pub struct DiagramRequest<'a> {
    pub kind: MermaidDiagramKind,
    pub source: &'a str,
    pub width: usize,
    pub theme: &'a MermaidTheme,
}

/// Geometry returned by a painter: the canvas size only. Glyph cells are the
/// painter's business (and are not modelled here).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagramFrameSize {
    pub width: usize,
    pub height: usize,
}

impl DiagramFrameSize {
    #[must_use]
    pub const fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }
}

/// Painter failure, mirroring the two upstream exception classes plus a
/// fail-closed "no renderer configured" state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MermaidFailure {
    /// `MermaidSyntaxError` (`diagnostics.ts:9-18`); `line` is 1-based.
    UnsupportedSyntax { line: usize },
    /// `DiagramCanvasSizeError` (`core/canvas.ts:45-57`).
    CanvasTooLarge { width: usize, height: usize },
    /// No painter is installed; the fence must keep its code-block form.
    Unavailable,
}

/// Diagram renderer boundary. Implementations own Mermaid parsing/drawing;
/// this module owns everything else.
pub trait DiagramPainter {
    fn paint(&self, request: &DiagramRequest<'_>) -> Result<DiagramFrameSize, MermaidFailure>;
}

/// Fail-closed painter: every fence falls back to the plain code block.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDiagramPainter;

impl DiagramPainter for NoDiagramPainter {
    fn paint(&self, _request: &DiagramRequest<'_>) -> Result<DiagramFrameSize, MermaidFailure> {
        Err(MermaidFailure::Unavailable)
    }
}

// ── OC2 live painter: flowchart cells ─────────────────────────────────

/// OC2's live [`DiagramPainter`] for the native transcript. The renderer owns
/// the fence lifecycle (detection, cache, recovery); this painter owns the
/// glyphs: it lays a detected flowchart out on a bounded ASCII rail grid.
/// Any other detected kind fails closed with
/// [`MermaidFailure::UnsupportedSyntax`], so its fence keeps the plain code
/// block (upstream `markdown.ts:289-318`).
#[derive(Clone, Copy, Debug, Default)]
pub struct CellDiagramPainter;

impl DiagramPainter for CellDiagramPainter {
    fn paint(&self, request: &DiagramRequest<'_>) -> Result<DiagramFrameSize, MermaidFailure> {
        let rows = layout_diagram(request)?;
        Ok(DiagramFrameSize {
            width: rows.iter().map(|row| row.chars().count()).max().unwrap_or(0),
            height: rows.len(),
        })
    }
}

impl CellDiagramPainter {
    /// Glyph rows of the same deterministic layout [`DiagramPainter::paint`]
    /// sized. The live view calls this after a `Rendered` frame and paints
    /// one row per line, replacing the fence code block entirely.
    #[must_use]
    pub fn cells(&self, request: &DiagramRequest<'_>) -> Option<Vec<String>> {
        layout_diagram(request).ok()
    }
}

#[derive(Clone, Debug)]
struct FlowNode {
    id: String,
    label: String,
    level: usize,
}

#[derive(Clone, Copy, Debug)]
struct FlowEdge {
    from: usize,
    to: usize,
}

fn flow_failure(line: usize) -> MermaidFailure {
    MermaidFailure::UnsupportedSyntax { line }
}

/// Header line: `flowchart` or `graph`, optionally with one of the five
/// direction tokens (`flowchart/parser.ts:17,277-279`).
fn parse_flow_header(line: &str) -> Option<()> {
    let mut tokens = line.split_whitespace();
    match tokens.next()? {
        "flowchart" | "graph" => {}
        _ => return None,
    }
    match tokens.next() {
        None => Some(()),
        Some("TD" | "TB" | "BT" | "LR" | "RL") if tokens.next().is_none() => Some(()),
        _ => None,
    }
}

/// Node statement: `ID`, `ID[label]`, `ID{label}` or `ID(label)`
/// (`flowchart/parser.ts` node shapes). Unclosed wrappers and trailing junk
/// fail closed with the 1-based line number (`diagnostics.ts:9-18`).
fn parse_flow_node(text: &str, line: usize) -> Result<(String, String), MermaidFailure> {
    let text = text.trim();
    let id_len: usize = text
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .map(char::len_utf8)
        .sum();
    if id_len == 0 {
        return Err(flow_failure(line));
    }
    let id = text[..id_len].to_owned();
    let rest = &text[id_len..];
    let label = match rest.chars().next() {
        None => id.clone(),
        Some(open @ ('[' | '{' | '(')) => {
            let close = match open {
                '[' => ']',
                '{' => '}',
                _ => ')',
            };
            let body = rest
                .strip_prefix(open)
                .and_then(|inner| inner.strip_suffix(close))
                .ok_or_else(|| flow_failure(line))?;
            let body = body.trim();
            if body.is_empty() {
                id.clone()
            } else {
                body.to_owned()
            }
        }
        Some(_) => return Err(flow_failure(line)),
    };
    Ok((id, label))
}

/// Exclusive end of an arrow starting at `start`, or `None`. Accepts `-->`,
/// `==>`, `-.->`, `->`, longer dash/equal/dot runs ending in `>` and open
/// links of three or more dashes (`---`).
fn arrow_end_at(bytes: &[u8], start: usize) -> Option<usize> {
    let mut end = start;
    let mut dashes = 0usize;
    let mut dots = 0usize;
    while let Some(&byte) = bytes.get(end) {
        match byte {
            b'-' => dashes += 1,
            b'.' => dots += 1,
            b'=' => {}
            _ => break,
        }
        end += 1;
    }
    if dashes + dots == 0 {
        return None;
    }
    if bytes.get(end) == Some(&b'>') {
        return Some(end + 1);
    }
    if dots == 0 && dashes >= 3 {
        return Some(end);
    }
    None
}

/// One statement line: either a chain `A[Label] --> B{Label}` or a bare
/// `ID[Label]` definition. Everything else fails closed.
fn parse_flow_statement(
    line: &str,
    number: usize,
    nodes: &mut Vec<FlowNode>,
    edges: &mut Vec<FlowEdge>,
) -> Result<(), MermaidFailure> {
    let bytes = line.as_bytes();
    let mut segments: Vec<(String, String)> = Vec::new();
    let mut segment_start = 0usize;
    let mut scan = 0usize;
    while scan < bytes.len() {
        if let Some(end) = arrow_end_at(bytes, scan) {
            let text = &line[segment_start..scan];
            let text = text.trim_matches(['-', '.', '=', ' ']);
            segments.push(parse_flow_node(text, number)?);
            scan = end;
            segment_start = end;
        } else {
            scan += 1;
        }
    }
    let text = &line[segment_start..];
    let text = text.trim_matches(['-', '.', '=', ' ']);
    segments.push(parse_flow_node(text, number)?);

    if let [single] = segments.as_slice() {
        flow_node_index(nodes, single.0.clone(), single.1.clone());
    } else {
        for window in segments.windows(2) {
            let from = flow_node_index(nodes, window[0].0.clone(), window[0].1.clone());
            let to = flow_node_index(nodes, window[1].0.clone(), window[1].1.clone());
            if from != to {
                edges.push(FlowEdge { from, to });
            }
        }
    }
    Ok(())
}

/// First-seen node index for `id`; a later wrapped definition upgrades a bare
/// id's label.
fn flow_node_index(nodes: &mut Vec<FlowNode>, id: String, label: String) -> usize {
    if let Some(existing) = nodes.iter().position(|node| node.id == id) {
        if nodes[existing].label == nodes[existing].id && label != id {
            nodes[existing].label = label;
        }
        existing
    } else {
        nodes.push(FlowNode { id: id.clone(), label, level: 0 });
        nodes.len() - 1
    }
}

/// Longest-path level assignment; cycles are clamped so the fixpoint
/// terminates within a bounded pass count.
fn assign_flow_levels(nodes: &mut [FlowNode], edges: &[FlowEdge]) {
    let cap = nodes.len().min(512);
    let passes = nodes.len().min(256).max(1);
    for _ in 0..passes {
        let mut changed = false;
        for edge in edges {
            if edge.from == edge.to {
                continue;
            }
            let next = (nodes[edge.from].level + 1).min(cap);
            if next > nodes[edge.to].level {
                nodes[edge.to].level = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

/// ponytail: single longest-path rail layout with straight connector rails,
/// not a dagre clone; multi-level edges drop their intermediate rails and
/// label cell widths are char-counted. Add real routing only when upstream
/// geometry parity demands it.
fn compose_flow(nodes: &[FlowNode], edges: &[FlowEdge], width: usize) -> Vec<String> {
    let width = width.max(1);
    let gap = 2usize;
    let max_level = nodes.iter().map(|node| node.level).max().unwrap_or(0);

    // Phase 1: box runs and box centers per level. Cells are bounded by the
    // layout width, so a box that no longer fits is dropped.
    let mut level_boxes: Vec<Vec<(usize, usize, String)>> = Vec::with_capacity(max_level + 1);
    let mut level_centers: Vec<Vec<Option<usize>>> = Vec::with_capacity(max_level + 1);
    for level in 0..=max_level {
        let members: Vec<usize> = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.level == level)
            .map(|(index, _)| index)
            .collect();
        let mut centers = vec![None; nodes.len()];
        let mut boxes: Vec<(usize, usize, String)> = Vec::new();
        if !members.is_empty() {
            let count = members.len();
            let available = width.saturating_sub(gap * (count - 1));
            let label_limit = (available / count).saturating_sub(4).max(1);
            let mut x = 0usize;
            for index in members.iter().copied() {
                let label = clip_label(&nodes[index].label, label_limit);
                let box_width = label.chars().count() + 4;
                if x + box_width > width {
                    break;
                }
                centers[index] = Some(x + box_width / 2);
                boxes.push((x, box_width, label));
                x += box_width + gap;
            }
        }
        level_boxes.push(boxes);
        level_centers.push(centers);
    }

    // Phase 2: emit one box line pair per level with connector rails between.
    let mut rows: Vec<String> = Vec::with_capacity(max_level * 3 + 2);
    for level in 0..=max_level {
        let boxes = &level_boxes[level];
        let mut top = vec![' '; width];
        let mut body = vec![' '; width];
        for (start, box_width, label) in boxes.iter() {
            let end = (*start + *box_width).min(width);
            if end - start < 2 {
                continue;
            }
            top[*start] = '+';
            top[end - 1] = '+';
            for fill in *start + 1..end - 1 {
                top[fill] = '-';
            }
            body[*start] = '|';
            body[end - 1] = '|';
            let inner = end - *start - 2;
            let text: Vec<char> = clip_label(label, inner).chars().collect();
            let left = inner.saturating_sub(text.len()) / 2;
            for (offset, glyph) in text.into_iter().enumerate() {
                body[*start + 1 + left + offset] = glyph;
            }
        }
        rows.push(buffer_to_row(top));
        rows.push(buffer_to_row(body));
        if level == max_level {
            break;
        }
        let centers = &level_centers[level];
        let next_centers = &level_centers[level + 1];
        let mut rail = vec![' '; width];
        for edge in edges {
            if nodes[edge.from].level == level {
                if let Some(center) = centers[edge.from] {
                    rail[center.min(width - 1)] = '|';
                }
            }
            if nodes[edge.from].level == level && nodes[edge.to].level == level + 1 {
                if let Some(center) = next_centers[edge.to] {
                    rail[center.min(width - 1)] = 'v';
                }
            }
        }
        rows.push(buffer_to_row(rail));
    }
    rows
}

fn clip_label(label: &str, limit: usize) -> String {
    label.chars().take(limit).collect()
}

fn buffer_to_row(buffer: Vec<char>) -> String {
    let row: String = buffer.into_iter().collect();
    row.trim_end().to_owned()
}

/// Lay `request.source` out as diagram rows. `None`-producing failures carry
/// the 1-based offending line; a header-only fence yields no rows so the
/// renderer falls back to the code block (`markdown.ts:307`).
fn layout_diagram(request: &DiagramRequest<'_>) -> Result<Vec<String>, MermaidFailure> {
    // Mermaid registers all six kinds at the pinned TUI.  Flowcharts have a
    // structured cell layout below; the other registered kinds still need a
    // diagram surface rather than leaking back to the fenced source block.
    // Their full parsers belong to the upstream merman package, so OC2 uses a
    // bounded, syntax-preserving text diagram until those grammars are ported.
    if request.kind != MermaidDiagramKind::Flowchart {
        return layout_text_diagram(request);
    }
    let mut nodes: Vec<FlowNode> = Vec::new();
    let mut edges: Vec<FlowEdge> = Vec::new();
    let mut header_seen = false;
    for (index, raw_line) in request.source.split('\n').enumerate() {
        let number = index + 1;
        let trimmed = raw_line.trim_end_matches('\r').trim();
        // Blank lines and comments are skipped (`core/mermaid.ts:19-25`).
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }
        if !header_seen {
            parse_flow_header(trimmed).ok_or_else(|| flow_failure(number))?;
            header_seen = true;
            continue;
        }
        parse_flow_statement(trimmed, number, &mut nodes, &mut edges)?;
    }
    if nodes.is_empty() {
        return Ok(Vec::new());
    }
    assign_flow_levels(&mut nodes, &edges);
    Ok(compose_flow(&nodes, &edges, request.width))
}

/// Render a supported non-flow Mermaid kind as a bounded diagram card.  This
/// deliberately does not pretend to implement that kind's grammar: malformed
/// input is still represented as a diagram (matching the dedicated upstream
/// renderer's replacement semantics), while the source remains readable and
/// the canvas stays within the same width bound as flowcharts.
fn layout_text_diagram(request: &DiagramRequest<'_>) -> Result<Vec<String>, MermaidFailure> {
    let width = request.width.max(1);
    let mut lines = request
        .source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("%%"));
    let _header = lines.next().ok_or_else(|| flow_failure(1))?;
    let title = format!("{} diagram", request.kind.as_str());
    let inner = width.saturating_sub(4).max(1);
    let mut rows = vec![format!("┌{}┐", boxed_fill(&title, inner))];
    for line in lines {
        rows.push(format!("│{}│", boxed_fill(line, inner)));
    }
    if rows.len() == 1 {
        rows.push(format!("│{}│", " ".repeat(inner)));
    }
    rows.push(format!("└{}┘", "─".repeat(inner)));
    Ok(rows)
}

fn boxed_fill(text: &str, width: usize) -> String {
    let mut value: String = text.chars().take(width).collect();
    value.push_str(&" ".repeat(width.saturating_sub(value.chars().count())));
    value
}

// ── Outcome ───────────────────────────────────────────────────────────

/// Rendered diagram footprint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagramFrame {
    pub kind: MermaidDiagramKind,
    /// Exact source this frame was produced from (prefix source for partial
    /// recovery, previous source for a retained frame).
    pub source: String,
    /// Canvas width in cells (`width: "100%"` is the viewport; the canvas may
    /// be wider and scroll horizontally, `markdown.ts:66-75`).
    pub width: usize,
    /// Canvas height in rows (`markdown.ts:71`).
    pub height: usize,
    /// Always [`DIAGRAM_MARGIN_TOP`] for diagrams (`markdown.ts:74`).
    pub margin_top: usize,
    /// True when the previous good frame for this block was reused
    /// (`markdown.ts:290-295`).
    pub retained: bool,
    /// True when the frame came from the `(source, width, theme)` cache.
    pub from_cache: bool,
}

impl DiagramFrame {
    /// Rows reserved in the transcript, including the diagram margin.
    #[must_use]
    pub fn footprint_rows(&self) -> usize {
        self.margin_top + self.height
    }

    /// Cells occupied by the canvas itself (margin excluded).
    #[must_use]
    pub fn canvas_cells(&self) -> usize {
        self.width.saturating_mul(self.height)
    }

    /// Footprint cells for a container of `container_width` columns: the
    /// renderable is `width: "100%"`, so the reserved width is the container
    /// width unless the canvas is wider (then it scrolls).
    #[must_use]
    pub fn footprint_cells(&self, container_width: usize) -> usize {
        container_width.max(self.width).saturating_mul(self.footprint_rows())
    }

    /// Viewport for drag/wheel scrolling (`markdown.ts:76-108`).
    #[must_use]
    pub fn viewport(&self, container_width: usize) -> DiagramViewport {
        DiagramViewport::new(self.width, container_width)
    }
}

/// Why the fence stays a plain code block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MermaidFallbackReason {
    /// `detectMermaidDiagram` returned `undefined` (`detect.ts:9-17`).
    UndetectedKind,
    /// Source exceeds [`MAX_MERMAID_SOURCE_BYTES`] (OC2 bound, fail-closed).
    SourceTooLarge { bytes: usize },
    /// Unsupported/invalid syntax at a 1-based line (`markdown.ts:298`).
    UnsupportedSyntax { line: usize },
    /// The canvas exceeds the upstream 250 000-cell limit (`canvas.ts:43,82`).
    OversizedCanvas { width: usize, height: usize },
    /// The painter produced a zero-height canvas (`markdown.ts:307`).
    EmptyCanvas,
    /// No painter configured.
    RendererUnavailable,
}

impl fmt::Display for MermaidFallbackReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndetectedKind => f.write_str("not a supported Mermaid diagram"),
            Self::SourceTooLarge { bytes } => {
                write!(f, "Mermaid source of {bytes} bytes exceeds the limit")
            }
            Self::UnsupportedSyntax { line } => {
                write!(f, "unsupported Mermaid syntax at line {line}")
            }
            Self::OversizedCanvas { width, height } => {
                write!(f, "diagram canvas {width}x{height} exceeds the cell limit")
            }
            Self::EmptyCanvas => f.write_str("diagram produced an empty canvas"),
            Self::RendererUnavailable => f.write_str("no Mermaid renderer configured"),
        }
    }
}

/// Plain code-block fallback. The source is preserved verbatim; the block
/// paints one row per source line (`Markdown.ts:1171-1186`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MermaidFallback {
    pub reason: MermaidFallbackReason,
    pub source: String,
    pub code_rows: usize,
}

impl MermaidFallback {
    /// Rows the fallback code block occupies (no diagram margin).
    #[must_use]
    pub fn rows(&self) -> usize {
        self.code_rows
    }
}

/// Renderer decision for one fence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MermaidOutcome {
    Rendered(DiagramFrame),
    Fallback(MermaidFallback),
}

impl MermaidOutcome {
    #[must_use]
    pub fn frame(&self) -> Option<&DiagramFrame> {
        match self {
            Self::Rendered(frame) => Some(frame),
            Self::Fallback(_) => None,
        }
    }

    #[must_use]
    pub fn fallback(&self) -> Option<&MermaidFallback> {
        match self {
            Self::Rendered(_) => None,
            Self::Fallback(fallback) => Some(fallback),
        }
    }
}

// ── Render cache ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CacheKey {
    source: String,
    width: usize,
    theme: String,
}

#[derive(Debug, Default)]
struct RenderCache {
    entries: BTreeMap<CacheKey, DiagramFrameSize>,
    order: VecDeque<CacheKey>,
}

impl RenderCache {
    fn get(&self, key: &CacheKey) -> Option<DiagramFrameSize> {
        self.entries.get(key).copied()
    }

    fn insert(&mut self, key: CacheKey, value: DiagramFrameSize) {
        if self.entries.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
        }
        while self.entries.len() > MAX_CACHE_ENTRIES {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

#[derive(Clone, Debug)]
struct LastGood {
    kind: MermaidDiagramKind,
    source: String,
    size: DiagramFrameSize,
}

// ── Renderer ──────────────────────────────────────────────────────────

/// Fence renderer. The lifecycle mirrors
/// `packages/merman/src/markdown.ts:268-336`: detect, bound, paint, recover
/// from a syntax error using the previous good frame of the same kind or a
/// non-blank prefix, and fall back to the plain code block otherwise.
#[derive(Debug)]
pub struct MermaidRenderer<P: DiagramPainter> {
    painter: P,
    theme: MermaidTheme,
    layout_max_width: usize,
    cache: RenderCache,
    last_good: BTreeMap<usize, LastGood>,
    last_good_order: Vec<usize>,
}

impl MermaidRenderer<NoDiagramPainter> {
    /// Fail-closed renderer: everything falls back to the code block.
    #[must_use]
    pub fn without_painter(theme: MermaidTheme) -> Self {
        Self::new(NoDiagramPainter, theme)
    }
}

impl<P: DiagramPainter> MermaidRenderer<P> {
    #[must_use]
    pub fn new(painter: P, theme: MermaidTheme) -> Self {
        Self {
            painter,
            theme,
            layout_max_width: DEFAULT_LAYOUT_MAX_WIDTH,
            cache: RenderCache::default(),
            last_good: BTreeMap::new(),
            last_good_order: Vec::new(),
        }
    }

    /// Replace the configured `layoutMaxWidth` (default 120,
    /// `markdown.ts:279-280`).
    #[must_use]
    pub fn with_layout_max_width(mut self, width: usize) -> Self {
        self.layout_max_width = width.max(1);
        self
    }

    #[must_use]
    pub fn theme(&self) -> &MermaidTheme {
        &self.theme
    }

    /// The configured painter. The live view reads glyph cells from it after
    /// a `Rendered` frame ([`CellDiagramPainter::cells`]).
    #[must_use]
    pub fn painter(&self) -> &P {
        &self.painter
    }

    /// Changing the theme changes the cache key; retained last-good frames are
    /// kept per block (upstream re-renders them with the new colors).
    pub fn set_theme(&mut self, theme: MermaidTheme) {
        self.theme = theme;
    }

    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    #[must_use]
    pub fn last_good_kind(&self, block_id: usize) -> Option<MermaidDiagramKind> {
        self.last_good.get(&block_id).map(|entry| entry.kind)
    }

    /// Drop retained state for a block whose renderable was destroyed
    /// (`markdown.ts:330-335`; the caller invokes this during reconciliation).
    pub fn forget_block(&mut self, block_id: usize) {
        if self.last_good.remove(&block_id).is_some() {
            self.last_good_order.retain(|id| *id != block_id);
        }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Effective layout width: `min(layoutMaxWidth, max(1, terminal))`
    /// (`markdown.ts:279-281`).
    #[must_use]
    pub fn effective_width(&self, terminal_width: usize) -> usize {
        self.layout_max_width.min(terminal_width.max(1))
    }

    /// Resolve one fence. Deterministic and side-effect free apart from the
    /// bounded caches.
    pub fn render(&mut self, fence: &MermaidFence, terminal_width: usize) -> MermaidOutcome {
        let fallback = |reason: MermaidFallbackReason| {
            MermaidOutcome::Fallback(MermaidFallback {
                reason,
                source: fence.source.clone(),
                code_rows: fence.source.split('\n').count(),
            })
        };

        if fence.truncated {
            // OC2 bound: never hand an unbounded document to a parser.
            return fallback(MermaidFallbackReason::SourceTooLarge {
                bytes: fence.total_bytes,
            });
        }
        let Some(kind) = detect_kind(&fence.source) else {
            return fallback(MermaidFallbackReason::UndetectedKind);
        };
        let width = self.effective_width(terminal_width);
        let key = CacheKey {
            source: fence.source.clone(),
            width,
            theme: self.theme.id(),
        };
        if let Some(size) = self.cache.get(&key) {
            self.claim(fence.block_id, kind, fence.source.clone(), size);
            return MermaidOutcome::Rendered(DiagramFrame {
                kind,
                source: fence.source.clone(),
                width: size.width,
                height: size.height,
                margin_top: DIAGRAM_MARGIN_TOP,
                retained: false,
                from_cache: true,
            });
        }

        let painted = {
            let request = DiagramRequest {
                kind,
                source: &fence.source,
                width,
                theme: &self.theme,
            };
            self.painter.paint(&request)
        };

        match painted {
            Ok(size) => match validate_size(size) {
                Ok(size) => {
                    self.store(key, fence.block_id, kind, &fence.source, size);
                    MermaidOutcome::Rendered(DiagramFrame {
                        kind,
                        source: fence.source.clone(),
                        width: size.width,
                        height: size.height,
                        margin_top: DIAGRAM_MARGIN_TOP,
                        retained: false,
                        from_cache: false,
                    })
                }
                Err(reason) => fallback(reason),
            },
            Err(MermaidFailure::UnsupportedSyntax { line }) => {
                self.recover_from_syntax(fence, kind, width, line)
            }
            Err(MermaidFailure::CanvasTooLarge { width, height }) => {
                fallback(MermaidFallbackReason::OversizedCanvas { width, height })
            }
            Err(MermaidFailure::Unavailable) => {
                fallback(MermaidFallbackReason::RendererUnavailable)
            }
        }
    }

    /// `markdown.ts:289-315`.
    fn recover_from_syntax(
        &mut self,
        fence: &MermaidFence,
        kind: MermaidDiagramKind,
        width: usize,
        line: usize,
    ) -> MermaidOutcome {
        let fallback = || {
            MermaidOutcome::Fallback(MermaidFallback {
                reason: MermaidFallbackReason::UnsupportedSyntax { line },
                source: fence.source.clone(),
                code_rows: fence.source.split('\n').count(),
            })
        };

        if let Some(previous) = self.last_good.get(&fence.block_id) {
            if previous.kind == kind {
                let frame = DiagramFrame {
                    kind,
                    source: previous.source.clone(),
                    width: previous.size.width,
                    height: previous.size.height,
                    margin_top: DIAGRAM_MARGIN_TOP,
                    retained: true,
                    from_cache: false,
                };
                let (source, size) = (previous.source.clone(), previous.size);
                self.claim(fence.block_id, kind, source, size);
                return MermaidOutcome::Rendered(frame);
            }
        }

        let lines: Vec<&str> = fence.source.split('\n').collect();
        let trailing_content = lines
            .iter()
            .skip(line)
            .any(|candidate| !candidate.trim().is_empty());
        if line <= 2 || trailing_content {
            return fallback();
        }
        let end = line.saturating_sub(1).min(lines.len());
        let prefix = lines[..end].join("\n");
        if prefix.is_empty() {
            return fallback();
        }

        let painted = {
            let request = DiagramRequest {
                kind,
                source: &prefix,
                width,
                theme: &self.theme,
            };
            self.painter.paint(&request)
        };
        match painted {
            Ok(size) => match validate_size(size) {
                Ok(size) if size.height > 0 => {
                    let key = CacheKey {
                        source: prefix.clone(),
                        width,
                        theme: self.theme.id(),
                    };
                    self.store(key, fence.block_id, kind, &prefix, size);
                    MermaidOutcome::Rendered(DiagramFrame {
                        kind,
                        source: prefix,
                        width: size.width,
                        height: size.height,
                        margin_top: DIAGRAM_MARGIN_TOP,
                        retained: false,
                        from_cache: false,
                    })
                }
                _ => fallback(),
            },
            Err(MermaidFailure::CanvasTooLarge { width, height }) => {
                MermaidOutcome::Fallback(MermaidFallback {
                    reason: MermaidFallbackReason::OversizedCanvas { width, height },
                    source: fence.source.clone(),
                    code_rows: fence.source.split('\n').count(),
                })
            }
            Err(_) => fallback(),
        }
    }

    fn store(
        &mut self,
        key: CacheKey,
        block_id: usize,
        kind: MermaidDiagramKind,
        source: &str,
        size: DiagramFrameSize,
    ) {
        self.cache.insert(key, size);
        self.claim(block_id, kind, source.to_owned(), size);
    }

    /// `claimLastGood` (`markdown.ts:322-336`), bounded per block.
    fn claim(&mut self, block_id: usize, kind: MermaidDiagramKind, source: String, size: DiagramFrameSize) {
        if !self.last_good.contains_key(&block_id) {
            self.last_good_order.push(block_id);
            while self.last_good_order.len() > MAX_LAST_GOOD_BLOCKS {
                let oldest = self.last_good_order.remove(0);
                self.last_good.remove(&oldest);
            }
        }
        self.last_good.insert(
            block_id,
            LastGood {
                kind,
                source,
                size,
            },
        );
    }
}

fn validate_size(size: DiagramFrameSize) -> Result<DiagramFrameSize, MermaidFallbackReason> {
    if size.height == 0 {
        // `markdown.ts:307` refuses a zero-height diagram.
        return Err(MermaidFallbackReason::EmptyCanvas);
    }
    if size.width.saturating_mul(size.height) > MAX_DIAGRAM_CELLS {
        // `core/canvas.ts:82`
        return Err(MermaidFallbackReason::OversizedCanvas {
            width: size.width,
            height: size.height,
        });
    }
    Ok(size)
}

// ── Horizontal viewport ───────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Result of a mouse event on the diagram renderable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseEventOutcome {
    /// Event was handled (`preventDefault`/`stopPropagation` upstream).
    pub consumed: bool,
    /// Left-button mouse-down clears the selection (`markdown.ts:78-79`).
    pub clear_selection: bool,
}

impl MouseEventOutcome {
    const PASSTHROUGH: Self = Self {
        consumed: false,
        clear_selection: false,
    };
    const CONSUMED: Self = Self {
        consumed: true,
        clear_selection: false,
    };
}

/// Horizontal-only viewport for a rendered diagram
/// (`packages/merman/src/markdown.ts:76-108`). Vertical wheel events are never
/// consumed, so the transcript keeps scrolling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagramViewport {
    content_width: usize,
    viewport_width: usize,
    scroll_x: usize,
    drag_x: Option<usize>,
}

impl DiagramViewport {
    #[must_use]
    pub fn new(content_width: usize, viewport_width: usize) -> Self {
        Self {
            content_width,
            viewport_width,
            scroll_x: 0,
            drag_x: None,
        }
    }

    #[must_use]
    pub fn scroll_x(&self) -> usize {
        self.scroll_x
    }

    /// Largest scroll offset; the canvas is clamped to its content
    /// (OC2 bound, see module docs).
    #[must_use]
    pub fn max_scroll_x(&self) -> usize {
        self.content_width.saturating_sub(self.viewport_width)
    }

    /// `onMouseDown` (`markdown.ts:77-83`): only the left button is handled.
    pub fn mouse_down(&mut self, button: MouseButton, x: usize) -> MouseEventOutcome {
        if button != MouseButton::Left {
            return MouseEventOutcome::PASSTHROUGH;
        }
        self.drag_x = Some(x);
        MouseEventOutcome {
            consumed: true,
            clear_selection: true,
        }
    }

    /// `onMouseDrag` (`markdown.ts:84-91`): `scrollX -= dx`.
    pub fn mouse_drag(&mut self, x: usize) -> bool {
        let Some(previous) = self.drag_x else {
            return true;
        };
        self.drag_x = Some(x);
        if x >= previous {
            self.scroll_x = self.scroll_x.saturating_sub(x - previous);
        } else {
            self.scroll_x = (self.scroll_x + (previous - x)).min(self.max_scroll_x());
        }
        true
    }

    /// `onMouseDragEnd` (`markdown.ts:92-96`).
    pub fn mouse_drag_end(&mut self) -> bool {
        self.drag_x = None;
        true
    }

    /// `onMouseUp` (`markdown.ts:97-102`).
    pub fn mouse_up(&mut self, button: MouseButton) -> bool {
        if button != MouseButton::Left {
            return false;
        }
        self.drag_x = None;
        true
    }

    /// `onMouseScroll` (`markdown.ts:103-108`): only left/right are consumed.
    pub fn mouse_scroll(&mut self, direction: ScrollDirection) -> bool {
        matches!(direction, ScrollDirection::Left | ScrollDirection::Right)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct StubPainter {
        responses: RefCell<Vec<Result<DiagramFrameSize, MermaidFailure>>>,
        calls: RefCell<Vec<(MermaidDiagramKind, String, usize)>>,
        default: DiagramFrameSize,
    }

    impl StubPainter {
        fn new(responses: Vec<Result<DiagramFrameSize, MermaidFailure>>) -> Self {
            Self {
                responses: RefCell::new(responses),
                calls: RefCell::new(Vec::new()),
                default: DiagramFrameSize::new(20, 4),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.borrow().len()
        }

        fn sources(&self) -> Vec<String> {
            self.calls
                .borrow()
                .iter()
                .map(|(_, source, _)| source.clone())
                .collect()
        }
    }

    impl DiagramPainter for StubPainter {
        fn paint(&self, request: &DiagramRequest<'_>) -> Result<DiagramFrameSize, MermaidFailure> {
            self.calls.borrow_mut().push((
                request.kind,
                request.source.to_owned(),
                request.width,
            ));
            let mut responses = self.responses.borrow_mut();
            if responses.is_empty() {
                return Ok(self.default);
            }
            responses.remove(0)
        }
    }

    fn fence(language: &str, source: &str, block_id: usize) -> MermaidFence {
        MermaidFence::from_code_block(Some(language), source, block_id).expect("mermaid fence")
    }

    fn renderer(painter: StubPainter) -> MermaidRenderer<StubPainter> {
        MermaidRenderer::new(painter, MermaidTheme::dark("opencode"))
    }

    /// MD-008. Upstream: `packages/merman/src/detect.ts:9-16` and the header
    /// matchers in `flowchart|gantt|gitgraph|sequence|state|timeline` parsers.
    #[test]
    fn md_008_each_supported_mermaid_kind_is_detected() {
        let cases = [
            ("graph TD\nA-->B", MermaidDiagramKind::Flowchart),
            ("flowchart LR\nA-->B", MermaidDiagramKind::Flowchart),
            ("%% comment\n\ngantt\ntitle x", MermaidDiagramKind::Gantt),
            ("gitGraph LR:\ncommit", MermaidDiagramKind::GitGraph),
            ("gitGraph:\ncommit", MermaidDiagramKind::GitGraph),
            ("sequenceDiagram\nA->>B: hi", MermaidDiagramKind::Sequence),
            ("stateDiagram-v2\n[*] --> S", MermaidDiagramKind::State),
            ("timeline TD\ntitle x", MermaidDiagramKind::Timeline),
        ];
        for (source, expected) in cases {
            assert_eq!(detect_kind(source), Some(expected), "{source:?}");
            assert_eq!(expected.as_str(), expected.as_str());
        }
        assert_eq!(MermaidDiagramKind::GitGraph.as_str(), "gitGraph");
        // Unknown kinds are not Mermaid diagrams here (`detect.ts:16`).
        for source in ["pie\n\"a\": 1", "classDiagram\nA <|-- B", "erDiagram\nA ||--o{ B", "", "  \n%% only\n"] {
            assert_eq!(detect_kind(source), None, "{source:?}");
        }
        // A direction suffix outside the accepted set is not a header.
        assert_eq!(detect_kind("graph XY\nA-->B"), None);
        assert_eq!(detect_kind("timeline BT\nx"), None);
        assert_eq!(detect_kind("gitGraph XX\ncommit"), None);
    }

    #[test]
    fn md_008_all_detected_kinds_replace_the_fence_with_diagram_rows() {
        let painter = CellDiagramPainter;
        for source in [
            "gantt\ntitle release",
            "gitGraph\ncommit",
            "sequenceDiagram\nA->>B: hello",
            "stateDiagram-v2\n[*] --> Ready",
            "timeline\n2024 : release",
        ] {
            let kind = detect_kind(source).expect("supported kind");
            let request = DiagramRequest {
                kind,
                source,
                width: 40,
                theme: &MermaidTheme::default(),
            };
            let rows = painter.cells(&request).expect("diagram rows");
            assert!(rows.len() >= 3, "{kind:?}: {rows:?}");
            assert!(rows.iter().all(|row| row.chars().count() <= 40));
            assert!(rows.iter().any(|row| row.contains(kind.as_str())));
        }
    }

    /// MD-008. Upstream: `packages/merman/src/markdown.ts:66-75` (width 100%,
    /// fixed height, marginTop 1) and `76-108` (drag/wheel handling).
    #[test]
    fn md_008_wide_diagram_drag_scrolls_and_clamps() {
        let frame = DiagramFrame {
            kind: MermaidDiagramKind::Flowchart,
            source: "graph TD\nA-->B".to_owned(),
            width: 200,
            height: 12,
            margin_top: DIAGRAM_MARGIN_TOP,
            retained: false,
            from_cache: false,
        };
        assert_eq!(frame.footprint_rows(), 13);
        assert_eq!(frame.canvas_cells(), 2400);
        assert_eq!(frame.footprint_cells(80), 200 * 13);

        let mut viewport = frame.viewport(80);
        assert_eq!(viewport.max_scroll_x(), 120);
        assert_eq!(viewport.scroll_x(), 0);

        let down = viewport.mouse_down(MouseButton::Left, 100);
        assert!(down.consumed);
        assert!(down.clear_selection);
        assert!(viewport.mouse_drag(60)); // drag left by 40 -> scroll right by 40
        assert_eq!(viewport.scroll_x(), 40);
        assert!(viewport.mouse_drag(0)); // drag further left by 60 -> 100
        assert_eq!(viewport.scroll_x(), 100);
        assert!(viewport.mouse_drag(10)); // drag right by 10
        assert_eq!(viewport.scroll_x(), 90);
        assert!(viewport.mouse_drag_end());
        assert!(viewport.mouse_drag(200)); // no drag in progress: consumed, no move
        assert_eq!(viewport.scroll_x(), 90);
        assert!(viewport.mouse_up(MouseButton::Left));

        // Right button is ignored; vertical wheel reaches the transcript.
        assert_eq!(
            viewport.mouse_down(MouseButton::Other, 10),
            MouseEventOutcome::PASSTHROUGH
        );
        assert!(!viewport.mouse_up(MouseButton::Other));
        assert!(viewport.mouse_scroll(ScrollDirection::Left));
        assert!(viewport.mouse_scroll(ScrollDirection::Right));
        assert!(!viewport.mouse_scroll(ScrollDirection::Up));
        assert!(!viewport.mouse_scroll(ScrollDirection::Down));
    }

    /// MD-008. Upstream: `packages/merman/src/markdown.ts:289-315` – a syntax
    /// error keeps the previous good frame of the same kind, otherwise a
    /// prefix is retried when every line after the error line is blank.
    #[test]
    fn md_008_streaming_partial_syntax_retains_prefix() {
        let source = "graph TD\nA-->B\nC-->D\nBOGUS\n";
        let painter = StubPainter::new(vec![
            Err(MermaidFailure::UnsupportedSyntax { line: 4 }),
            Ok(DiagramFrameSize::new(30, 10)),
            Err(MermaidFailure::UnsupportedSyntax { line: 3 }),
        ]);
        let mut renderer = renderer(painter);

        let first = renderer.render(&fence("mermaid", source, 7), 100);
        let frame = first.frame().expect("prefix diagram");
        assert_eq!(frame.source, "graph TD\nA-->B\nC-->D");
        assert_eq!(frame.height, 10);
        assert_eq!(frame.footprint_rows(), 11);
        assert!(!frame.retained);
        assert_eq!(renderer.last_good_kind(7), Some(MermaidDiagramKind::Flowchart));

        // Next streaming update errors again; the previous frame is retained
        // even though the error is on line 3 (previous check runs first).
        let second = renderer.render(&fence("mermaid", "graph TD\nA-->B\n??\n", 7), 100);
        let retained = second.frame().expect("retained diagram");
        assert!(retained.retained);
        assert_eq!(retained.source, "graph TD\nA-->B\nC-->D");
        assert_eq!(retained.height, 10);
        assert_eq!(retained.kind, MermaidDiagramKind::Flowchart);
    }

    /// MD-008. Upstream: `packages/merman/src/markdown.ts:298,312,316` – a
    /// syntax error on line <= 2, or with non-blank trailing lines, leaves the
    /// plain code block; the exact source is preserved.
    #[test]
    fn md_008_invalid_syntax_falls_back_to_plain_code() {
        let painter = StubPainter::new(vec![
            Err(MermaidFailure::UnsupportedSyntax { line: 2 }),
            Err(MermaidFailure::UnsupportedSyntax { line: 3 }),
        ]);
        let mut renderer = renderer(painter);

        let source = "graph TD\n???\nA-->B";
        let outcome = renderer.render(&fence("mermaid", source, 1), 80);
        let fallback = outcome.fallback().expect("line <= 2 fallback");
        assert_eq!(
            fallback.reason,
            MermaidFallbackReason::UnsupportedSyntax { line: 2 }
        );
        assert_eq!(fallback.source, source);
        assert_eq!(fallback.rows(), 3); // one row per source line, no margin

        // Error on line 3 but a non-blank line follows it -> fallback.
        let trailing = "graph TD\nA-->B\n???\nstill here";
        let outcome = renderer.render(&fence("mermaid", trailing, 1), 80);
        let fallback = outcome.fallback().expect("trailing content fallback");
        assert_eq!(
            fallback.reason,
            MermaidFallbackReason::UnsupportedSyntax { line: 3 }
        );
        assert_eq!(fallback.source, trailing);
        assert_eq!(fallback.rows(), 4);
    }

    /// MD-008. Upstream: `packages/merman/src/core/canvas.ts:43-56,79-82`
    /// (250 000-cell limit) and `markdown.ts:316-318` (size error falls back).
    #[test]
    fn md_008_oversized_canvas_and_input_fall_back() {
        let painter = StubPainter::new(vec![
            Err(MermaidFailure::CanvasTooLarge {
                width: 1000,
                height: 400,
            }),
            Ok(DiagramFrameSize::new(500_000, 1)),
            Ok(DiagramFrameSize::new(10, 4)), // must not be reached
        ]);
        let mut renderer = renderer(painter);

        let outcome = renderer.render(&fence("mermaid", "graph TD\nA-->B", 1), 80);
        assert_eq!(
            outcome.fallback().map(|fallback| fallback.reason),
            Some(MermaidFallbackReason::OversizedCanvas {
                width: 1000,
                height: 400
            })
        );

        // A painter that ignores the limit is still rejected by validation.
        let outcome = renderer.render(&fence("mermaid", "graph TD\nA-->B", 2), 80);
        assert_eq!(
            outcome.fallback().map(|fallback| fallback.reason),
            Some(MermaidFallbackReason::OversizedCanvas {
                width: 500_000,
                height: 1
            })
        );

        // Bounded input: an oversized fence is never handed to the painter.
        let huge = format!("graph TD\n{}", "A-->B\n".repeat(20_000));
        assert!(huge.len() > MAX_MERMAID_SOURCE_BYTES);
        let fence = fence("mermaid", &huge, 3);
        assert!(fence.truncated);
        let outcome = renderer.render(&fence, 80);
        assert_eq!(
            outcome.fallback().map(|fallback| fallback.reason),
            Some(MermaidFallbackReason::SourceTooLarge {
                bytes: huge.len()
            })
        );
    }

    /// MD-008. Fence/language recognition is case-insensitive and only the
    /// `mermaid` language key is registered upstream
    /// (`packages/merman/src/plugin.ts:8-13`); content is `token.text`
    /// (`Markdown.ts:1171-1186`).
    #[test]
    fn md_008_fence_info_and_language_recognition() {
        let info = parse_fence_info("```mermaid title=flow\n").expect("fence");
        assert_eq!(info.marker, '`');
        assert_eq!(info.marker_len, 3);
        assert_eq!(info.language.as_deref(), Some("mermaid"));
        assert_eq!(info.info, "mermaid title=flow");

        let tilde = parse_fence_info("   ~~~MERMAID\n").expect("tilde fence");
        assert_eq!(tilde.marker, '~');
        assert_eq!(tilde.marker_len, 3);
        assert_eq!(tilde.language.as_deref(), Some("mermaid"));

        assert!(parse_fence_info("    ```mermaid\n").is_none()); // 4 spaces
        assert!(parse_fence_info("``mermaid\n").is_none()); // 2 backticks
        assert!(parse_fence_info("plain text\n").is_none());

        assert!(accepts_language("Mermaid"));
        assert!(accepts_language("{.mermaid}"));
        assert!(!accepts_language("merman"));
        assert!(MermaidFence::from_code_block(Some("MERMAID"), "graph TD", 2).is_some());
        assert!(MermaidFence::from_code_block(Some("ts"), "graph TD", 2).is_none());
        assert!(MermaidFence::from_code_block(None, "graph TD", 2).is_none());
    }

    /// MD-008. Upstream cache identity is the block id
    /// (`packages/merman/src/markdown.ts:272-277,322-336`); OC2 additionally
    /// memoizes by `(source, width, theme)` so identical fences at the same
    /// width/theme reuse the render output.
    #[test]
    fn md_008_render_cache_is_keyed_by_source_width_and_theme() {
        let painter = StubPainter::new(vec![]);
        let mut renderer = renderer(painter);
        let fence = fence("mermaid", "graph TD\nA-->B", 1);

        assert!(matches!(
            renderer.render(&fence, 40),
            MermaidOutcome::Rendered(ref frame) if !frame.from_cache
        ));
        assert_eq!(renderer.cache_len(), 1);

        // Same source/width/theme -> cache hit, painter not called again.
        let again = renderer.render(&fence, 40);
        assert!(matches!(
            again,
            MermaidOutcome::Rendered(ref frame) if frame.from_cache
        ));
        assert_eq!(renderer_calls(&renderer), 1);

        // Width participates in the key.
        assert!(matches!(
            renderer.render(&fence, 60),
            MermaidOutcome::Rendered(ref frame) if !frame.from_cache
        ));
        assert_eq!(renderer_calls(&renderer), 2);
        assert_eq!(renderer.cache_len(), 2);

        // Theme participates in the key.
        renderer.set_theme(MermaidTheme::light("opencode"));
        assert!(matches!(
            renderer.render(&fence, 40),
            MermaidOutcome::Rendered(ref frame) if !frame.from_cache
        ));
        assert_eq!(renderer.cache_len(), 3);

        // Effective width is clamped by layoutMaxWidth (default 120).
        assert_eq!(renderer.effective_width(500), DEFAULT_LAYOUT_MAX_WIDTH);
        let narrow = MermaidRenderer::new(StubPainter::new(vec![]), MermaidTheme::default())
            .with_layout_max_width(60);
        assert_eq!(narrow.effective_width(500), 60);
        assert_eq!(narrow.effective_width(30), 30);
        assert_eq!(narrow.effective_width(0), 1);
    }

    fn renderer_calls(renderer: &MermaidRenderer<StubPainter>) -> usize {
        renderer.painter.call_count()
    }

    /// MD-008. An unconfigured painter must fail closed: the fence keeps its
    /// code block (`markdown.ts:316-318` rethrows unknown errors, so OC2
    /// refuses to render rather than guessing).
    #[test]
    fn md_008_renderer_without_painter_falls_back() {
        let mut renderer = MermaidRenderer::without_painter(MermaidTheme::dark("opencode"));
        let outcome = renderer.render(&fence("mermaid", "graph TD\nA-->B\n", 1), 80);
        let fallback = outcome.fallback().expect("fallback");
        assert_eq!(fallback.reason, MermaidFallbackReason::RendererUnavailable);
        assert_eq!(fallback.source, "graph TD\nA-->B\n");
        assert_eq!(fallback.rows(), 3);
        assert_eq!(renderer.cache_len(), 0);
    }

    /// MD-008. `claimLastGood` is released when the block is destroyed
    /// (`markdown.ts:330-335`); our bound is explicit and deterministic.
    #[test]
    fn md_008_forget_block_drops_retained_frame() {
        let painter = StubPainter::new(vec![
            Err(MermaidFailure::UnsupportedSyntax { line: 4 }),
            Ok(DiagramFrameSize::new(10, 3)),
            Err(MermaidFailure::UnsupportedSyntax { line: 5 }),
            Err(MermaidFailure::UnsupportedSyntax { line: 5 }),
        ]);
        let mut renderer = renderer(painter);
        let source = "graph TD\nA-->B\nC-->D\nBAD\n";
        assert!(renderer.render(&fence("mermaid", source, 9), 80).frame().is_some());
        assert_eq!(renderer.last_good_kind(9), Some(MermaidDiagramKind::Flowchart));
        renderer.forget_block(9);
        assert_eq!(renderer.last_good_kind(9), None);
        // Without a retained frame the same syntax error now falls back.
        let outcome = renderer.render(&fence("mermaid", "graph TD\nA-->B\nC-->D\n??\n", 9), 80);
        assert!(outcome.fallback().is_some());
    }
}
