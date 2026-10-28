#![forbid(unsafe_code)]
//! Native diff view model v3: multi-file unified-diff classification.
//!
//! V2 fixes plus:
//! - No-newline marker (`\`) retained after exact hunk counts via pre-check
//! - G4 count validation at end of every hunk (malformed indicator, not false success)
//! - Distinct `malformed` flag separate from resource `truncated`
//!
//! Pure state: no rendering, no IO, no threads, no FFI. Std only.

pub const MAX_DIFF_PATCH_BYTES: usize = 64 * 1024;
pub const MAX_DIFF_LINES: usize = 4096;
pub const MAX_HUNKS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineRole {
    HunkHeader,
    Added,
    Removed,
    Context,
    NoNewline,
    FileHeader,
    DiffHeader,
    Blank,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub role: DiffLineRole,
    pub content: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub hunk_index: usize,
    pub line_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassifiedPatch {
    pub lines: Vec<DiffLine>,
    pub truncated: bool,
    pub malformed: bool,
    pub consumed_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffPaintRow {
    pub role: DiffLineRole,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaintResult {
    pub rows: Vec<DiffPaintRow>,
    pub truncated: bool,
    pub malformed: bool,
}

#[must_use]
pub fn paint_rows(patch: &str, max_rows: usize) -> PaintResult {
    let classified = classify_patch(patch);
    let total_input_lines = classified.lines.len();
    let mut rows = Vec::with_capacity(classified.lines.len().min(max_rows));
    let mut row_count = 0usize;
    for line in classified.lines.into_iter() {
        if row_count >= max_rows { break; }
        let text = match line.role {
            DiffLineRole::Added => format!("+{}", line.content),
            DiffLineRole::Removed => format!("-{}", line.content),
            DiffLineRole::Context => format!(" {}", line.content),
            DiffLineRole::NoNewline => format!("\\{}", line.content),
            DiffLineRole::FileHeader => line.content,
            DiffLineRole::DiffHeader => line.content,
            _ => line.content,
        };
        rows.push(DiffPaintRow { role: line.role, old_line: line.old_line, new_line: line.new_line, text });
        row_count += 1;
    }
    PaintResult {
        rows,
        truncated: classified.truncated || row_count < total_input_lines,
        malformed: classified.malformed,
    }
}

#[must_use]
pub fn paint_rows_with_gutter(patch: &str, max_rows: usize) -> PaintResult {
    let classified = classify_patch(patch);
    let total_input_lines = classified.lines.len();
    let (padding, digits) = gutter_width(&classified.lines, 0);
    let mut rows = Vec::with_capacity(classified.lines.len().min(max_rows));
    let mut row_count = 0usize;
    for line in classified.lines.into_iter() {
        if row_count >= max_rows { break; }
        let text = match line.role {
            DiffLineRole::Added => format!("+{}", line.content),
            DiffLineRole::Removed => format!("-{}", line.content),
            DiffLineRole::Context => format!(" {}", line.content),
            DiffLineRole::NoNewline => format!("\\{}", line.content),
            DiffLineRole::FileHeader => line.content,
            DiffLineRole::DiffHeader => line.content,
            _ => line.content,
        };
        let old = line.old_line.map(|n| n.to_string()).unwrap_or_default();
        let new = line.new_line.map(|n| n.to_string()).unwrap_or_default();
        let gutter = format!("{:padding$} {:>digits$} {:>digits$} ", "", old, new, padding = padding, digits = digits);
        rows.push(DiffPaintRow { role: line.role, old_line: line.old_line, new_line: line.new_line, text: format!("{gutter}{text}") });
        row_count += 1;
    }
    PaintResult {
        rows,
        truncated: classified.truncated || row_count < total_input_lines,
        malformed: classified.malformed,
    }
}

#[must_use]
pub fn paint_rows_simple(patch: &str, max_rows: usize) -> Vec<DiffPaintRow> {
    paint_rows(patch, max_rows).rows
}

pub fn classify_patch(patch: &str) -> ClassifiedPatch {
    let bytes = patch.as_bytes();
    let byte_cap_reached = bytes.len() > MAX_DIFF_PATCH_BYTES;
    let end = if byte_cap_reached { MAX_DIFF_PATCH_BYTES } else { bytes.len() };
    let end = snap_to_char_boundary(patch, end);
    let input = &patch[..end];

    let hunk_starts = find_hunk_starts(input);
    let mut lines: Vec<DiffLine> = Vec::with_capacity(hunk_starts.len().min(MAX_HUNKS).saturating_mul(8));
    let mut truncated = byte_cap_reached;
    let mut malformed = false;

    // Prefix region: before first @@ — classify ---/+++ as FileHeader,
    // diff --git/index as DiffHeader.
    let prefix_end = hunk_starts.first().copied().unwrap_or(input.len());
    for raw in input[..prefix_end].split('\n').filter(|l| !l.is_empty()) {
        if lines.len() >= MAX_DIFF_LINES { truncated = true; break; }
        let role = classify_prefix_line(raw);
        lines.push(DiffLine {
            role,
            content: raw.to_string(),
            old_line: None,
            new_line: None,
            hunk_index: 0,
            line_index: lines.len(),
        });
    }

    for (hunk_idx, &start) in hunk_starts.iter().enumerate() {
        if hunk_idx >= MAX_HUNKS {
            truncated = true;
            break;
        }
        let hunk_end = hunk_starts.get(hunk_idx + 1).copied().unwrap_or(input.len());
        let hunk_text = &input[start..hunk_end];
        if let Some(ranges) = parse_hunk_header(hunk_text) {
            let (old_start, old_count, new_start, new_count) = ranges;
            let mut old_line = old_start;
            let mut new_line = new_start;
            let mut old_consumed: u32 = 0;
            let mut new_consumed: u32 = 0;

            let header_end = hunk_text.find('\n').unwrap_or(hunk_text.len());
            let header_line = &hunk_text[..header_end];

            if lines.len() < MAX_DIFF_LINES {
                lines.push(DiffLine {
                    role: DiffLineRole::HunkHeader,
                    content: header_line.to_string(),
                    old_line: None,
                    new_line: None,
                    hunk_index: hunk_idx,
                    line_index: lines.len(),
                });
            }

            let body = if header_end < hunk_text.len() { &hunk_text[header_end + 1..] } else { "" };
            let body_lines: Vec<&str> = split_diff_lines(body);
            let mut b_line_index = 1usize;
            let mut inter_hunk = false;

            for &raw_line in &body_lines {
                if lines.len() >= MAX_DIFF_LINES {
                    truncated = true;
                    break;
                }

                // No-newline marker is ALWAYS part of hunk body, checked first
                // (V2 fix for t03/e06: marker retained after exact counts).
                let is_no_newline = !raw_line.is_empty() && raw_line.as_bytes()[0] == b'\\';

                // Exhausted counts alone do not turn an extra body line into
                // metadata. Preserve its visible role/number, but flag the
                // invalid count below. An actual header ends the body after
                // the declared range; later header rows stay outside it.
                let header_role = classify_prefix_line(raw_line);
                if old_consumed >= old_count && new_consumed >= new_count
                    && matches!(header_role, DiffLineRole::FileHeader | DiffLineRole::DiffHeader)
                {
                    inter_hunk = true;
                }
                let in_body = is_no_newline || !inter_hunk;

                let (role, content, inc_old, inc_new) = if in_body {
                    classify_hunk_body_line(raw_line)
                } else {
                    // Inter-hunk: --- / +++ are FileHeader, not Removed/Added.
                    (classify_prefix_line(raw_line), raw_line, false, false)
                };

                lines.push(DiffLine {
                    role,
                    content: content.to_string(),
                    old_line: if inc_old { Some(old_line) } else { None },
                    new_line: if inc_new { Some(new_line) } else { None },
                    hunk_index: hunk_idx,
                    line_index: b_line_index,
                });

                if inc_old {
                    if old_consumed >= old_count { malformed = true; }
                    old_consumed = old_consumed.saturating_add(1);
                    old_line = old_line.saturating_add(1);
                }
                if inc_new {
                    if new_consumed >= new_count { malformed = true; }
                    new_consumed = new_consumed.saturating_add(1);
                    new_line = new_line.saturating_add(1);
                }
                b_line_index += 1;
            }

            // G4: validate hunk body line counts against declared ranges.
            // Mismatched counts indicate malformed diff, not resource truncation.
            if old_consumed != old_count || new_consumed != new_count {
                malformed = true;
            }
        }
        if truncated { break; }
    }

    ClassifiedPatch {
        lines,
        truncated,
        malformed,
        consumed_bytes: end,
    }
}

fn split_diff_lines(s: &str) -> Vec<&str> {
    let ends_with_newline = s.ends_with('\n');
    s.split('\n')
        .filter(|l| !l.is_empty() || !ends_with_newline)
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect()
}

fn classify_prefix_line(line: &str) -> DiffLineRole {
    if line.starts_with("---") {
        DiffLineRole::FileHeader
    } else if line.starts_with("+++") {
        DiffLineRole::FileHeader
    } else if line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("new file")
        || line.starts_with("deleted file")
        || line.starts_with("old mode")
        || line.starts_with("new mode")
        || line.starts_with("similarity index")
        || line.starts_with("dissimilarity index")
        || line.starts_with("rename from")
        || line.starts_with("rename to")
        || line.starts_with("copy from")
        || line.starts_with("copy to") {
        DiffLineRole::DiffHeader
    } else {
        DiffLineRole::Blank
    }
}

fn classify_hunk_body_line(line: &str) -> (DiffLineRole, &str, bool, bool) {
    if line.is_empty() {
        return (DiffLineRole::Context, "", true, true);
    }
    let first = line.as_bytes()[0];
    match first {
        b'+' => (DiffLineRole::Added, &line[1..], false, true),
        b'-' => (DiffLineRole::Removed, &line[1..], true, false),
        b'\\' => (DiffLineRole::NoNewline, &line[1..], false, false),
        b' ' => (DiffLineRole::Context, &line[1..], true, true),
        _ => (DiffLineRole::Blank, line, false, false),
    }
}

fn find_hunk_starts(input: &str) -> Vec<usize> {
    let mut starts = Vec::with_capacity(MAX_HUNKS.saturating_add(1));
    let bytes = input.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'@' && bytes[i + 1] == b'@'
            && (i == 0 || bytes[i - 1] == b'\n') {
            if is_hunk_header_at(&bytes[i..]) {
                if starts.len() >= MAX_HUNKS.saturating_add(1) { break; }
                starts.push(i);
                while i < bytes.len() && bytes[i] != b'\n' { i += 1; }
                if i < bytes.len() { i += 1; }
            } else { i += 1; }
        } else { i += 1; }
    }
    starts
}

fn is_hunk_header_at(bytes: &[u8]) -> bool {
    let end = bytes.iter().position(|&b| b == b'\n').unwrap_or(bytes.len());
    let line = match std::str::from_utf8(&bytes[..end]) { Ok(s) => s, Err(_) => return false };
    if !line.starts_with("@@") { return false; }
    let mut parts = line.split_whitespace();
    if parts.next() != Some("@@") { return false; }
    let Some(old) = parts.next() else { return false };
    let Some(new) = parts.next() else { return false };
    if !old.starts_with('-') || !new.starts_with('+') { return false; }
    old[1..].split(',').next().is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        && new[1..].split(',').next().is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        && parts.next().is_some_and(|token| token == "@@")
}

fn parse_hunk_header(hunk_text: &str) -> Option<(u32, u32, u32, u32)> {
    let first_line = hunk_text.split('\n').next()?;
    let mut parts = first_line.split_whitespace();
    if parts.next()? != "@@" { return None; }
    let old_part = parts.next()?;
    if !old_part.starts_with('-') { return None; }
    let old_start = old_part[1..].split(',').next()?.parse::<u32>().ok()?;
    let old_count = old_part[1..].split(',').nth(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let new_part = parts.next()?;
    if !new_part.starts_with('+') { return None; }
    let new_start = new_part[1..].split(',').next()?.parse::<u32>().ok()?;
    let new_count = new_part[1..].split(',').nth(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    Some((old_start, old_count, new_start, new_count))
}

fn snap_to_char_boundary(s: &str, byte_pos: usize) -> usize {
    let mut pos = byte_pos.min(s.len());
    while pos > 0 && !s.is_char_boundary(pos) { pos -= 1; }
    pos
}

fn gutter_width(lines: &[DiffLine], gutter_padding: usize) -> (usize, usize) {
    let padding_left = 1 + gutter_padding;
    let mut max_line: u32 = 0;
    for line in lines {
        if let Some(n) = line.old_line { max_line = max_line.max(n); }
        if let Some(n) = line.new_line { max_line = max_line.max(n); }
    }
    for line in lines {
        if line.role == DiffLineRole::HunkHeader {
            if let Some((old_last, new_last)) = hunk_header_max_lines(&line.content) {
                max_line = max_line.max(old_last).max(new_last);
            }
        }
    }
    let min_digits = if max_line == 0 { 1 } else { num_digits(max_line) };
    (padding_left, min_digits)
}

fn hunk_header_max_lines(header: &str) -> Option<(u32, u32)> {
    let inner = header.strip_prefix("@@")?;
    let inner = inner.find("@@").map(|end| &inner[..end])?;
    let inner = inner.trim();
    let mut parts = inner.split_whitespace();
    let old_part = parts.next()?.strip_prefix('-')?;
    let new_part = parts.next()?.strip_prefix('+')?;

    let old_start = old_part.split(',').next()?.parse::<u32>().ok()?;
    let old_count = old_part.split(',').nth(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let new_start = new_part.split(',').next()?.parse::<u32>().ok()?;
    let new_count = new_part.split(',').nth(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);

    let old_last = old_start.saturating_add(old_count.saturating_sub(1));
    let new_last = new_start.saturating_add(new_count.saturating_sub(1));
    Some((old_last, new_last))
}

fn num_digits(n: u32) -> usize {
    if n == 0 { 1 } else {
        let mut d = 0;
        let mut v = n;
        while v > 0 { v /= 10; d += 1; }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T01: basic add/remove/context with line numbers
    #[test]
    fn t01_basic_classification() {
        let patch = "@@ -1,3 +1,4 @@\n line1\n-old\n+new\n line3\n";
        let result = classify_patch(patch);
        assert!(!result.truncated);
        assert_eq!(result.lines.len(), 5);
        assert_eq!(result.lines[0].role, DiffLineRole::HunkHeader);
        assert_eq!(result.lines[1].role, DiffLineRole::Context);
        assert_eq!(result.lines[1].old_line, Some(1));
        assert_eq!(result.lines[1].new_line, Some(1));
        assert_eq!(result.lines[2].role, DiffLineRole::Removed);
        assert_eq!(result.lines[2].old_line, Some(2));
        assert_eq!(result.lines[2].new_line, None);
        assert_eq!(result.lines[3].role, DiffLineRole::Added);
        assert_eq!(result.lines[3].old_line, None);
        assert_eq!(result.lines[3].new_line, Some(2));
        assert_eq!(result.lines[4].role, DiffLineRole::Context);
        assert_eq!(result.lines[4].old_line, Some(3));
        assert_eq!(result.lines[4].new_line, Some(3));
    }

    // T02: file headers classified as FileHeader (before hunks)
    #[test]
    fn t02_file_headers() {
        let patch = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,2 +1,3 @@\n-old\n+new\n+extra\n";
        let result = classify_patch(patch);
        let file_headers: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::FileHeader).collect();
        assert_eq!(file_headers.len(), 2);
        assert_eq!(file_headers[0].content, "--- a/src/lib.rs");
        assert_eq!(file_headers[1].content, "+++ b/src/lib.rs");
    }

    // T03: no-newline marker
    #[test]
    fn t03_no_newline_marker() {
        let patch = "@@ -1 +1 @@\n-old\n+new\n\\ No newline at end of file\n";
        let result = classify_patch(patch);
        let no_nl: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::NoNewline).collect();
        assert_eq!(no_nl.len(), 1);
        assert_eq!(no_nl[0].content, " No newline at end of file");
    }

    // T04: count omitted in hunk header defaults to 1
    #[test]
    fn t04_hunk_header_count_default() {
        let patch = "@@ -5 +5 @@\n context\n-new\n+new_val\n";
        let result = classify_patch(patch);
        assert_eq!(result.lines[1].old_line, Some(5));
        assert_eq!(result.lines[1].new_line, Some(5));
        assert_eq!(result.lines[2].old_line, Some(6));
        assert_eq!(result.lines[2].new_line, None);
        assert_eq!(result.lines[2].role, DiffLineRole::Removed);
        assert_eq!(result.lines[3].old_line, None);
        assert_eq!(result.lines[3].new_line, Some(6));
        assert_eq!(result.lines[3].role, DiffLineRole::Added);
    }

    // T05: byte cap truncation
    #[test]
    fn t05_byte_cap_truncation() {
        let long_line = "+".to_string() + &"x".repeat(MAX_DIFF_PATCH_BYTES + 100);
        let patch = format!("@@ -1 +1 @@\n{}", long_line);
        let result = classify_patch(&patch);
        assert!(result.truncated);
        assert_eq!(result.consumed_bytes, MAX_DIFF_PATCH_BYTES);
    }

    // T06: max lines cap
    #[test]
    fn t06_max_lines_cap() {
        let mut patch = String::from("@@ -1 +1 @@\n");
        for i in 0..(MAX_DIFF_LINES + 50) {
            patch.push_str(&format!("+line{}\n", i));
        }
        let result = classify_patch(&patch);
        assert!(result.truncated);
        assert_eq!(result.lines.len(), MAX_DIFF_LINES);
    }

    // T07: gutter width
    #[test]
    fn t07_gutter_width() {
        let patch = "@@ -1,3 +1,10 @@\n old1\n-old2\n+new1\n+new2\n+new3\n+new4\n+new5\n+new6\n+new7\n+new8\n+new9\n+new10\n";
        let result = classify_patch(patch);
        let (padding, digits) = gutter_width(&result.lines, 0);
        assert_eq!(padding, 1);
        assert_eq!(digits, 2);
    }

    // T08: Unicode content preserved
    #[test]
    fn t08_unicode_content() {
        let patch = "@@ -1 +1 @@\n-hello\n+\u{7ba1}\u{8d85}\n";
        let result = classify_patch(patch);
        let added: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::Added).collect();
        assert_eq!(added[0].content, "\u{7ba1}\u{8d85}");
    }

    // T09: empty patch
    #[test]
    fn t09_empty_patch() {
        let result = classify_patch("");
        assert!(!result.truncated);
        assert!(result.lines.is_empty());
    }

    // T10: no hunks (only file headers)
    #[test]
    fn t10_no_hunks() {
        let patch = "--- a/file\n+++ b/file\n";
        let result = classify_patch(&patch);
        assert!(!result.truncated);
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].role, DiffLineRole::FileHeader);
        assert_eq!(result.lines[1].role, DiffLineRole::FileHeader);
    }

    // T11: multiple hunks single file
    #[test]
    fn t11_multiple_hunks() {
        let patch = "@@ -1,2 +1,2 @@\n a\n-b\n+c\n@@ -10,2 +10,3 @@\n x\n-y\n+z\n+w\n";
        let result = classify_patch(patch);
        assert_eq!(result.lines.len(), 9);
        assert_eq!(result.lines[0].hunk_index, 0);
        let second_header = result.lines.iter().find(|l| l.hunk_index == 1 && l.role == DiffLineRole::HunkHeader);
        assert!(second_header.is_some());
    }

    // A01: +++ inside hunk is Added, not FileHeader (from v1)
    #[test]
    fn a01_plus_plus_inside_hunk() {
        let patch = "@@ -1,1 +1,1 @@\n-old\n+++new\n";
        let result = classify_patch(patch);
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[1].role, DiffLineRole::Removed);
        assert_eq!(result.lines[1].content, "old");
        assert_eq!(result.lines[2].role, DiffLineRole::Added);
        assert_eq!(result.lines[2].content, "++new");
    }

    // A02: --- inside hunk is Removed, not FileHeader (from v1)
    #[test]
    fn a02_minus_minus_inside_hunk() {
        let patch = "@@ -1,1 +1,1 @@\n---old\n+new\n";
        let result = classify_patch(patch);
        assert_eq!(result.lines[1].role, DiffLineRole::Removed);
        assert_eq!(result.lines[1].content, "--old");
        assert_eq!(result.lines[2].role, DiffLineRole::Added);
        assert_eq!(result.lines[2].content, "new");
    }

    // A03: hostile u32 range never panics
    #[test]
    fn a03_hostile_range_no_panic() {
        let result = std::panic::catch_unwind(|| {
            classify_patch("@@ -4294967295,2 +1,2 @@\n context\n context\n")
        });
        assert!(result.is_ok());
    }

    // A04: paint truncation visible
    #[test]
    fn a04_paint_truncation_visible() {
        let patch = "@@ -1,5 +1,5 @@\n+a\n+b\n+c\n+d\n+e\n";
        let result = paint_rows(patch, 3);
        assert!(result.truncated);
        assert_eq!(result.rows.len(), 3);
    }

    // A05: hostile u32 in paint with gutter never panics
    #[test]
    fn a05_hostile_gutter_no_panic() {
        let result = std::panic::catch_unwind(|| {
            paint_rows_with_gutter("@@ -4294967295,2 +1,2 @@\n+added\n", 10)
        });
        assert!(result.is_ok());
    }

    // D1: --- a/f2 / +++ b/f2 between hunks classified as FileHeader, not Removed/Added
    #[test]
    fn d1_multi_file_headers_between_hunks() {
        let patch = "\
--- a/f1
+++ b/f1
@@ -1,3 +1,3 @@
 ctx
-old
+new
 ctx
--- a/f2
+++ b/f2
@@ -1,2 +1,2 @@
 x
-y
+z
";
        let result = classify_patch(patch);
        assert!(!result.truncated);
        let file_headers: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::FileHeader).collect();
        assert_eq!(file_headers.len(), 4, "must have 4 file headers: ---f1,+++f1,---f2,+++f2");
        // Verify f2 headers are NOT Removed/Added
        for h in &file_headers {
            assert!(
                h.content == "--- a/f2" || h.content == "+++ b/f2" || h.content == "--- a/f1" || h.content == "+++ b/f1",
                "unexpected file header: {}", h.content
            );
        }
    }

    // D2: file headers after no-newline marker classified as FileHeader
    #[test]
    fn d2_headers_after_no_newline() {
        let patch = "\
@@ -1 +1 @@
-old
+new
\\ No newline at end of file
--- a/f2
+++ b/f2
@@ -1 +1 @@
-x
+y
";
        let result = classify_patch(patch);
        assert!(!result.truncated);
        let no_nl: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::NoNewline).collect();
        assert_eq!(no_nl.len(), 1, "no-newline marker must be retained");
        let f2_old = result.lines.iter().find(|l| l.content == "--- a/f2");
        assert!(f2_old.is_some(), "--- a/f2 must be present");
        assert_eq!(f2_old.unwrap().role, DiffLineRole::FileHeader);
        let f2_new = result.lines.iter().find(|l| l.content == "+++ b/f2");
        assert!(f2_new.is_some(), "+++ b/f2 must be present");
        assert_eq!(f2_new.unwrap().role, DiffLineRole::FileHeader);
    }

    // G3: diff --git / index headers classified as DiffHeader
    #[test]
    fn g3_diff_git_headers() {
        let patch = "\
diff --git a/f.rs b/f.rs
index abc123..def456 100644
--- a/f.rs
+++ b/f.rs
@@ -1 +1 @@
-old
+new
";
        let result = classify_patch(patch);
        let diff_headers: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::DiffHeader).collect();
        assert!(diff_headers.iter().any(|l| l.content == "diff --git a/f.rs b/f.rs"));
        assert!(diff_headers.iter().any(|l| l.content == "index abc123..def456 100644"));
    }

    // G4: hunk count mismatch flagged as malformed
    #[test]
    fn g4_count_mismatch_malformed() {
        // @@ -1,2 +1,2 @@ declares 2 old, 2 new lines, but body has only 1 old, 1 new
        let patch = "@@ -1,2 +1,2 @@\n-old\n+new\n";
        let result = classify_patch(patch);
        assert!(result.malformed, "count mismatch must set malformed flag");
    }

    // G4b: matching counts NOT malformed
    #[test]
    fn g4b_matching_counts_ok() {
        let patch = "@@ -1,2 +1,2 @@\n ctx\n-old\n+new\n";
        let result = classify_patch(patch);
        assert!(!result.malformed, "matching counts must not set malformed");
    }

    // E01: CRLF line endings handled
    #[test]
    fn e01_crlf_endings() {
        let patch = "@@ -1,3 +1,3 @@\r\n ctx\r\n-old\r\n+new\r\n ctx\r\n";
        let result = classify_patch(patch);
        assert_eq!(result.lines.len(), 5);
        assert_eq!(result.lines[1].content, "ctx");
        assert_eq!(result.lines[2].content, "old");
        assert_eq!(result.lines[3].content, "new");
    }

    // E02: body content starting with -- is Removed, not FileHeader
    #[test]
    fn e02_body_double_dash_is_removed() {
        let patch = "@@ -1 +1 @@\n---source line\n+added\n";
        let result = classify_patch(patch);
        assert_eq!(result.lines[1].role, DiffLineRole::Removed);
        assert_eq!(result.lines[1].content, "--source line");
    }

    // E03: body content starting with ++ is Added, not FileHeader
    // NOTE: @@ -1,1 +1,2 @@ because ctx + added = 2 new lines
    #[test]
    fn e03_body_double_plus_is_added() {
        let patch = "@@ -1,1 +1,2 @@\n ctx\n+++source line\n";
        let result = classify_patch(patch);
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[1].role, DiffLineRole::Context);
        assert_eq!(result.lines[2].role, DiffLineRole::Added);
        assert_eq!(result.lines[2].content, "++source line");
    }

    // E04: multi-file full diff
    #[test]
    fn e04_multi_file_full() {
        let patch = "\
--- a/f1.rs
+++ b/f1.rs
@@ -1,2 +1,3 @@
 a
-b
+c
+d
--- a/f2.rs
+++ b/f2.rs
@@ -10,2 +10,3 @@
 x
-y
+z
+w";
        let result = classify_patch(patch);
        let file_headers: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::FileHeader).collect();
        assert_eq!(file_headers.len(), 4, "4 file headers across 2 files");
        let added: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::Added).collect();
        assert_eq!(added.len(), 4, "4 added lines");
        let removed: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::Removed).collect();
        assert_eq!(removed.len(), 2, "2 removed lines");
    }

    // E05: EOF without trailing newline
    #[test]
    fn e05_eof_no_trailing_newline() {
        let patch = "@@ -1 +1 @@\n-old\n+new";
        let result = classify_patch(patch);
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[2].role, DiffLineRole::Added);
        assert_eq!(result.lines[2].content, "new");
    }

    // E06: no-newline marker then file headers (combined case)
    #[test]
    fn e06_no_newline_then_headers() {
        let patch = "@@ -1 +1 @@\n-old\n+new\n\\ No newline at end of file\n--- a/f2\n+++ b/f2\n@@ -1 +1 @@\n-a\n+b\n";
        let result = classify_patch(patch);
        let no_nl: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::NoNewline).collect();
        assert_eq!(no_nl.len(), 1, "no-newline marker must be preserved");
        let f2_old = result.lines.iter().find(|l| l.content == "--- a/f2");
        assert!(f2_old.is_some());
        assert_eq!(f2_old.unwrap().role, DiffLineRole::FileHeader);
    }

    // E07: huge input bounded
    #[test]
    fn e07_huge_input_bounded() {
        let mut patch = String::from("@@ -1 +1 @@\n");
        patch.push_str(&format!("+{}\n", "x".repeat(MAX_DIFF_PATCH_BYTES + 100)));
        let result = classify_patch(&patch);
        assert!(result.truncated);
        assert_eq!(result.consumed_bytes, MAX_DIFF_PATCH_BYTES);
    }

    // E08: u32 overflow in new_start saturates
    #[test]
    fn e08_u32_max_new_start() {
        let result = std::panic::catch_unwind(|| {
            classify_patch("@@ -1 +4294967295 @@\n-old\n+new\n")
        });
        assert!(result.is_ok());
        let parsed = result.unwrap();
        let removed: Vec<_> = parsed.lines.iter().filter(|l| l.role == DiffLineRole::Removed).collect();
        assert_eq!(removed[0].old_line, Some(1));
    }

    // E09: no-newline marker preserves correct line metadata
    #[test]
    fn e09_no_newline_preserves_line_numbers() {
        let patch = "@@ -1,2 +1,2 @@\n ctx\n-old\n+new\n\\ No newline at end of file\n";
        let result = classify_patch(patch);
        let no_nl: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::NoNewline).collect();
        assert_eq!(no_nl.len(), 1);
    }

    // E10: body ++++ (four pluses) is three added content chars
    #[test]
    fn e10_quad_plus_in_body() {
        let patch = "@@ -1 +2 @@\n-old\n++++content\n";
        let result = classify_patch(patch);
        let added: Vec<_> = result.lines.iter().filter(|l| l.role == DiffLineRole::Added).collect();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].content, "+++content");
    }

    // F01: paint_rows with max_rows=0
    #[test]
    fn f01_paint_zero_rows() {
        let patch = "@@ -1 +1 @@\n-old\n+new\n";
        let result = paint_rows(patch, 0);
        assert!(result.rows.is_empty());
        assert!(result.truncated);
    }

    // F02: paint_rows_with_gutter truncation visible
    #[test]
    fn f02_paint_gutter_truncation() {
        let patch = "@@ -1,5 +1,5 @@\n+a\n+b\n+c\n+d\n+e\n";
        let result = paint_rows_with_gutter(patch, 2);
        assert!(result.truncated);
    }
}
