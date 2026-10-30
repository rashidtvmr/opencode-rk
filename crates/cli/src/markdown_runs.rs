#![forbid(unsafe_code)]
//! Bounded, additive Markdown styling runs over canonical sanitized source rows.
//!
//! Text and row layout remain owned by native_view_chat. Runs never parse a
//! painted rail/frame, never alter blank rows, and reset fence state per part.
//! Cluster, code and URL spans are lossless: run byte boundaries snap to the
//! shared display-cluster edges (mid-cluster starts advance, mid-cluster ends
//! retreat), fenced rows carry a full-row code run, and the raw URL byte span
//! is preserved even when the URL bound makes a link non-clickable.

use crate::native_view_layout::visit_unpadded_cells;
use crate::native_transcript::strip_ansi;

pub const MAX_INPUT_BYTES: usize = 32 * 1024;
pub const MAX_RUNS_PER_ROW: usize = 64;
pub const MAX_URL_BYTES: usize = 2 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    Prose,
    Plain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Bold,
    Italic,
    Code,
    Link,
}

pub const BOLD: u8 = 1;
pub const ITALIC: u8 = 2;
pub const CODE: u8 = 4;
pub const LINK: u8 = 8;
/// Inline raw code receives upstream base background; fenced/block code does not.
pub const CODE_BLOCK: u8 = 16;

/// A style interval on an undecorated, sanitized source row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownRun {
    pub start: usize,
    pub end: usize,
    pub start_cell: usize,
    pub end_cell: usize,
    pub kind: RunKind,
    /// Oversized URLs are deliberately non-clickable (`None`), never truncated.
    pub url: Option<String>,
    /// Raw URL byte span in the sanitized source row; preserved losslessly
    /// even when [`Self::url`] is `None`.
    pub url_span: Option<(usize, usize)>,
    /// Combined OpenTUI TextAttributes-compatible flags.
    pub flags: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyledSourceRow {
    /// Sanitized source content, without caller decoration.
    pub text: String,
    /// True for fenced-code rows under the part-local strict fence state.
    pub code: bool,
    pub runs: Vec<MarkdownRun>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownPart {
    pub kind: PartKind,
    pub rows: Vec<StyledSourceRow>,
    pub truncated: bool,
}

/// Sanitize the whole bounded message before splitting lines, matching the
/// canonical ANSI-state-across-newlines behavior. `Plain` preserves every
/// blank row but never assigns Markdown styles. Fence state is part-local.
pub fn parse_part(kind: PartKind, input: &str) -> MarkdownPart {
    let bounded = clip_utf8_prefix(input, MAX_INPUT_BYTES);
    let truncated = input.len() > bounded.len();
    let sanitized = strip_ansi(bounded);
    let mut fence: Option<(char, usize)> = None;
    let mut rows = Vec::new();
    for line in sanitized.split('\n') {
        let mut runs = Vec::new();
        if kind == PartKind::Prose {
            let was_fenced = fence.is_some();
            let mut code = was_fenced;
            if let Some((marker, count, trailing)) = fence_marker(line) {
                match fence {
                    Some((open, minimum))
                        if marker == open
                            && count >= minimum
                            && trailing.bytes().all(|b| matches!(b, b' ' | b'\t')) =>
                    {
                        fence = None;
                        code = true;
                    }
                    None => {
                        fence = Some((marker, count));
                        code = true;
                    }
                    _ => {}
                }
            }
            let visible = if code {
                sanitize_code_line(line)
            } else {
                sanitize_prose_line(line)
            };
            if code {
                push_full(&visible, RunKind::Code, &mut runs);
            } else {
                let bounds = cluster_boundaries(&visible);
                scan_inline(&visible, 0, visible.len(), 0, &bounds, &mut runs);
                runs.sort_by_key(|run| (run.start, std::cmp::Reverse(run.end)));
                runs.truncate(MAX_RUNS_PER_ROW);
            }
            rows.push(StyledSourceRow { text: visible, code, runs });
        } else {
            rows.push(StyledSourceRow { text: line.to_owned(), code: false, runs });
        }
    }
    MarkdownPart { kind, rows, truncated }
}

/// Rebase source-cell runs through an explicit caller prefix and clipping
/// window. The caller supplies the undecorated source row, not a painted row.
pub fn project_row(
    source_row: &str,
    runs: &[MarkdownRun],
    source_start_cell: usize,
    prefix_cells: usize,
    clip_cells: usize,
) -> Vec<MarkdownRun> {
    let source_end_cell = source_start_cell.saturating_add(clip_cells);
    let mut projected = Vec::new();
    for run in runs.iter().take(MAX_RUNS_PER_ROW) {
        let start = run.start_cell.max(source_start_cell);
        let end = run.end_cell.min(source_end_cell);
        if start >= end || run.start >= source_row.len() || run.end > source_row.len() {
            continue;
        }
        let mut copy = run.clone();
        copy.start_cell = prefix_cells.saturating_add(start - source_start_cell);
        copy.end_cell = prefix_cells.saturating_add(end - source_start_cell);
        projected.push(copy);
    }
    projected
}

fn sanitize_prose_line(line: &str) -> String {
    line.chars().map(|ch| if ch.is_control() { ' ' } else { ch }).collect()
}

fn sanitize_code_line(line: &str) -> String {
    let safe: String = line.chars().map(|ch| if ch.is_control() && ch != '\t' { ' ' } else { ch }).collect();
    let mut output = String::with_capacity(safe.len());
    let mut column = 0usize;
    for segment in safe.split_inclusive('\t') {
        let (text, tab) = segment.strip_suffix('\t').map_or((segment, false), |part| (part, true));
        visit_unpadded_cells(text, MAX_INPUT_BYTES, |glyph, cells| {
            output.push_str(glyph);
            column = column.saturating_add(cells);
        });
        if tab {
            let spaces = 8 - column % 8;
            output.extend(std::iter::repeat_n(' ', spaces));
            column = column.saturating_add(spaces);
        }
    }
    output
}

fn clip_utf8_prefix(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn fence_marker(line: &str) -> Option<(char, usize, &str)> {
    let indent = line.bytes().take_while(|b| *b == b' ').count();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let marker = rest.chars().next()?;
    if !matches!(marker, '`' | '~') {
        return None;
    }
    let count = rest.bytes().take_while(|b| *b == marker as u8).count();
    if count < 3 {
        return None;
    }
    let trailing = &rest[count..];
    if marker == '`' && trailing.contains('`') {
        return None;
    }
    Some((marker, count, trailing))
}

fn push_full(line: &str, kind: RunKind, out: &mut Vec<MarkdownRun>) {
    if line.is_empty() {
        return;
    }
    let cells = cell_width(line);
    out.push(MarkdownRun {
        start: 0,
        end: line.len(),
        start_cell: 0,
        end_cell: cells,
        kind,
        url: None,
        url_span: None,
        flags: CODE | CODE_BLOCK,
    });
}

fn cell_width(text: &str) -> usize {
    visit_unpadded_cells(text, text.len(), |_, _| {})
}

/// Shared display-cluster byte edges for the sanitized row: 0, each cluster
/// end, and the end sentinel. Walks the same segmentation as cell measurement
/// so a style span can never split a displayed cluster. Assumes sanitized
/// input (glyph bytes equal source bytes).
fn cluster_boundaries(text: &str) -> Vec<usize> {
    let mut bounds = Vec::with_capacity(16);
    bounds.push(0usize);
    let mut consumed = 0usize;
    visit_unpadded_cells(text, text.len(), |glyph, _| {
        consumed += glyph.len();
        bounds.push(consumed);
    });
    bounds
}

/// Conservative start snap: advance to the first cluster edge at or after
/// `byte`, never styling half a cluster from its middle.
fn snap_start(text: &str, byte: usize, bounds: &[usize]) -> usize {
    bounds.iter().copied().find(|b| *b >= byte).unwrap_or(text.len())
}

/// Conservative end snap: retreat to the last cluster edge at or before `byte`.
fn snap_end(text: &str, byte: usize, bounds: &[usize]) -> usize {
    bounds.iter().rev().copied().find(|b| *b <= byte).unwrap_or(0)
}

fn byte_cell_offset(text: &str, byte: usize) -> usize {
    if byte == 0 {
        return 0;
    }
    let byte = byte.min(text.len());
    let mut consumed = 0usize;
    let mut cells = 0usize;
    visit_unpadded_cells(text, text.len(), |glyph, width| {
        if consumed < byte && consumed + glyph.len() <= byte {
            consumed += glyph.len();
            cells += width;
        }
    });
    cells
}

fn next_char(text: &str, byte: usize) -> usize {
    text[byte..]
        .chars()
        .next()
        .map(|ch| byte + ch.len_utf8())
        .unwrap_or(text.len())
}

fn escaped(text: &str, at: usize) -> bool {
    let bytes = text.as_bytes();
    let mut i = at;
    let mut slashes = 0;
    while i > 0 && bytes[i - 1] == b'\\' {
        slashes += 1;
        i -= 1;
    }
    slashes % 2 == 1
}

fn find_unescaped(text: &str, delimiter: &str, start: usize, end: usize) -> Option<usize> {
    let mut at = start;
    while at + delimiter.len() <= end {
        if text[at..end].starts_with(delimiter) && !escaped(text, at) {
            return Some(at);
        }
        at = next_char(text, at);
    }
    None
}

/// A closer `*`/`**` must be right-flanking: the preceding character exists
/// and is not whitespace. Candidates that fail are skipped, not accepted.
fn right_flanking(text: &str, at: usize) -> bool {
    match text[..at].chars().next_back() {
        Some(ch) => !ch.is_whitespace(),
        None => false,
    }
}

/// An opener `*`/`**` must be left-flanking: the first content character
/// exists and is not whitespace, so a leftover delimiter from a longer run
/// (for example the stray `*` after `\**literal**`) cannot open emphasis.
fn opener_flanking(text: &str, content_start: usize, end: usize) -> bool {
    match text[content_start..end].chars().next() {
        Some(ch) => !ch.is_whitespace(),
        None => false,
    }
}

fn find_closer(text: &str, delimiter: &str, start: usize, end: usize) -> Option<usize> {
    let mut at = start;
    while at + delimiter.len() <= end {
        if text[at..end].starts_with(delimiter)
            && !escaped(text, at)
            && right_flanking(text, at)
        {
            return Some(at);
        }
        at = next_char(text, at);
    }
    None
}

fn scan_inline(
    text: &str,
    start: usize,
    end: usize,
    parent: u8,
    bounds: &[usize],
    out: &mut Vec<MarkdownRun>,
) {
    let mut at = start;
    while at < end && out.len() < MAX_RUNS_PER_ROW {
        if text.as_bytes()[at] == b'\\' {
            at = next_char(text, at);
            if at < end {
                at = next_char(text, at);
            }
            continue;
        }
        let (delimiter, kind, flags) = if text[at..end].starts_with("**") {
            ("**", RunKind::Bold, BOLD)
        } else if text.as_bytes()[at] == b'`' {
            ("`", RunKind::Code, CODE)
        } else if text.as_bytes()[at] == b'*' {
            ("*", RunKind::Italic, ITALIC)
        } else {
            if text.as_bytes()[at] == b'[' {
                if let Some(close) = find_unescaped(text, "](", at + 1, end) {
                    if let Some(finish) = find_unescaped(text, ")", close + 2, end) {
                        if close > at + 1 {
                            let raw_url = &text[close + 2..finish];
                            let url = (raw_url.len() <= MAX_URL_BYTES).then(|| raw_url.to_owned());
                            push_run(
                                text,
                                at + 1,
                                close,
                                RunKind::Link,
                                LINK | parent,
                                url,
                                Some((close + 2, finish)),
                                bounds,
                                out,
                            );
                            scan_inline(text, at + 1, close, parent | LINK, bounds, out);
                            at = finish + 1;
                            continue;
                        }
                    }
                }
            }
            at = next_char(text, at);
            continue;
        };
        if delimiter != "`" && !opener_flanking(text, at + delimiter.len(), end) {
            at = next_char(text, at);
            continue;
        }
        let content_start = at + delimiter.len();
        if content_start < end {
            let close = if delimiter == "`" {
                find_unescaped(text, delimiter, content_start, end)
            } else {
                find_closer(text, delimiter, content_start, end)
            };
            if let Some(close) = close {
                if close > content_start {
                    push_run(text, content_start, close, kind, parent | flags, None, None, bounds, out);
                    if kind != RunKind::Code {
                        scan_inline(text, content_start, close, parent | flags, bounds, out);
                    }
                    at = close + delimiter.len();
                    continue;
                }
            }
        }
        at = next_char(text, at);
    }
}

#[allow(clippy::too_many_arguments)]
fn push_run(
    text: &str,
    start: usize,
    end: usize,
    kind: RunKind,
    flags: u8,
    url: Option<String>,
    url_span: Option<(usize, usize)>,
    bounds: &[usize],
    out: &mut Vec<MarkdownRun>,
) {
    if out.len() >= MAX_RUNS_PER_ROW {
        return;
    }
    let start = snap_start(text, start, bounds);
    let end = snap_end(text, end, bounds);
    if start >= end {
        return;
    }
    out.push(MarkdownRun {
        start,
        end,
        start_cell: byte_cell_offset(text, start),
        end_cell: byte_cell_offset(text, end),
        kind,
        url,
        url_span,
        flags,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_rows_and_part_local_fence_state_are_preserved() {
        let parsed = parse_part(PartKind::Prose, "before\n\n~~~rust\n**literal**\n~~~~\nafter");
        // Six source lines split into exactly six rows; the blank row stays.
        assert_eq!(parsed.rows.len(), 6);
        assert!(parsed.rows[1].runs.is_empty());
        assert_eq!(parsed.rows[3].runs[0].kind, RunKind::Code);
        let next = parse_part(PartKind::Prose, "**bold**");
        assert_eq!(next.rows[0].runs[0].kind, RunKind::Bold);
    }

    #[test]
    fn strict_fence_closer_and_plain_content() {
        let parsed = parse_part(PartKind::Prose, "~~~~lang\n~~~\ntext\n~~~~  \n**bold**");
        assert_eq!(parsed.rows[2].runs[0].kind, RunKind::Code);
        assert_eq!(parsed.rows[4].runs[0].kind, RunKind::Bold);
        let plain = parse_part(PartKind::Plain, "**not styled**\n\n");
        assert_eq!(plain.rows.len(), 3);
        assert!(plain.rows.iter().all(|row| row.runs.is_empty()));
    }

    #[test]
    fn complete_message_ansi_sanitization_and_unicode_cells() {
        let parsed = parse_part(PartKind::Prose, "\x1b]8;;bad\ncontinued\x07é **界**");
        assert_eq!(parsed.rows.len(), 1);
        let run = &parsed.rows[0].runs[0];
        assert_eq!(run.kind, RunKind::Bold);
        // é (1 cell) + space (1) + ** (2) place 界 at source cells 4..6.
        assert_eq!(run.start_cell, 4);
        assert_eq!(run.end_cell, 6);
    }

    #[test]
    fn escapes_and_url_bound_are_safe() {
        let parsed = parse_part(PartKind::Prose, r"\**literal** **bold** [x](https://example.test)");
        assert_eq!(parsed.rows[0].runs.iter().filter(|run| run.kind == RunKind::Bold).count(), 1);
        assert_eq!(parsed.rows[0].runs.iter().find(|run| run.kind == RunKind::Link).unwrap().url.as_deref(), Some("https://example.test"));
        let huge = format!("[x]({})", "u".repeat(MAX_URL_BYTES + 1));
        assert!(parse_part(PartKind::Prose, &huge).rows[0].runs[0].url.is_none());
    }

    #[test]
    fn projected_runs_clip_prefix_without_paint_inference() {
        let parsed = parse_part(PartKind::Prose, "**界**");
        // The bold run covers 界 at source cells 2..4; the clip window starts
        // at that actual source position and shows one cell behind 3 prefix cells.
        let runs = project_row(&parsed.rows[0].text, &parsed.rows[0].runs, 2, 3, 1);
        assert_eq!(runs[0].start_cell, 3);
        assert_eq!(runs[0].end_cell, 4);
        // A marker-only window (source cells 0..1) carries no style run.
        let none = project_row(&parsed.rows[0].text, &parsed.rows[0].runs, 0, 3, 1);
        assert!(none.is_empty());
    }

    #[test]
    fn cluster_boundaries_and_conservative_snapping_are_lossless() {
        assert_eq!(cluster_boundaries("**界**"), vec![0, 1, 2, 5, 6, 7]);
        let bounds = cluster_boundaries("界x");
        assert_eq!(snap_start("界x", 1, &bounds), 3);
        assert_eq!(snap_end("界x", 2, &bounds), 0);
    }

    #[test]
    fn url_span_is_lossless_even_when_url_is_not_clickable() {
        let raw = format!("[label]({})", "u".repeat(MAX_URL_BYTES + 1));
        let parsed = parse_part(PartKind::Prose, &raw);
        let row = &parsed.rows[0];
        let run = &row.runs[0];
        assert_eq!(run.kind, RunKind::Link);
        assert!(run.url.is_none());
        let (span_start, span_end) = run.url_span.expect("url span preserved");
        assert_eq!(&row.text[span_start..span_end], "u".repeat(MAX_URL_BYTES + 1).as_str());
    }

    #[test]
    fn fenced_code_runs_cover_the_full_visible_row_losslessly() {
        let parsed = parse_part(PartKind::Prose, "~~~\n\tab\n~~~");
        let row = &parsed.rows[1];
        assert_eq!(row.text, "        ab");
        let run = &row.runs[0];
        assert_eq!(run.kind, RunKind::Code);
        assert_eq!(run.start, 0);
        assert_eq!(run.end, row.text.len());
        assert_eq!(run.start_cell, 0);
        assert_eq!(run.end_cell, cell_width(&row.text));
    }
}
