#![forbid(unsafe_code)]
//! Markdown block model with stable identities across streaming updates
//! (FFI-06).
//!
//! EVIDENCE / BOUNDARY
//! - `docs/ui-parity/opencode-ui-parity-kit/audit-opentui-boundary.md` §3
//!   "Markdown is a parser plus a retained composition engine" (refs [M1]
//!   `renderables/Markdown.ts:1-165`, [M2] `markdown-parser.ts:1-79`) states the
//!   contract this module models: the incremental parser reuses matching
//!   `token.raw` prefixes, keeps the **final two tokens unstable** so an
//!   unfinished token can still change type, falls back to a complete parse when
//!   incremental parsing fails, and must not flash raw markers. Ref [T1]
//!   (`Markdown.test.ts:1553-1639`) asserts object reuse while appending list
//!   items.
//! - The vendored library exports **no** Markdown entry point
//!   (`nm -gU …/libopentui.dylib` lists no `markdown*` symbol), so this is a
//!   Rust model of the document layer. It produces block identity and
//!   marker-free display text only; inline styling, tables-in-cells layout,
//!   syntax highlighting (FFI-07) and Mermaid/math fences belong to the
//!   consumers of this model and are not claimed here.
//! - OpenCode drives Markdown through `createMarkdownRenderer`
//!   (`packages/tui/src/plugin/markdown.ts:6-21`, pin 450e7b9); that seam is the
//!   reason block identity, not inline styling, is the parity-critical part of
//!   this module.

/// Blocks kept in the unstable tail (audit [M2]).
pub const UNSTABLE_TAIL: usize = 2;
/// Block cap per document (fail closed).
pub const MAX_BLOCKS: usize = 4096;
/// Source byte cap (fail closed).
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;

/// Block kind. Ordered-list items carry their marker so they are distinct
/// blocks (object reuse while appending list items, ref [T1]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Heading(u8),
    Paragraph,
    ListItem { ordered: bool, marker: String },
    Code { lang: Option<String>, closed: bool },
    Quote,
    Table,
    Rule,
    /// Content that could not be classified (still a stable block).
    Other,
}

/// One block with a stable identity across [`MarkdownDoc::set_source`] calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    id: u64,
    kind: BlockKind,
    raw: String,
    start_line: usize,
    unstable: bool,
}

impl Block {
    /// Stable id: identical for a block whose kind and raw text were reused.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn kind(&self) -> &BlockKind {
        &self.kind
    }

    /// Source text of the block, including markers.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    #[must_use]
    pub const fn start_line(&self) -> usize {
        self.start_line
    }

    /// True while the block may still change type (last [`UNSTABLE_TAIL`]).
    #[must_use]
    pub const fn is_unstable(&self) -> bool {
        self.unstable
    }

    /// Marker-free text for painting: concealment of `#`, list markers, quote
    /// markers and code-fence lines (no raw-marker flashes in a settled block).
    #[must_use]
    pub fn display_text(&self) -> String {
        match &self.kind {
            BlockKind::Heading(_) => self.raw.trim_start_matches('#').trim().to_owned(),
            BlockKind::ListItem { marker, .. } => {
                let rest = self.raw.strip_prefix(marker.as_str()).unwrap_or(&self.raw);
                rest.strip_prefix(' ').unwrap_or(rest).to_owned()
            }
            BlockKind::Code { .. } => self
                .raw
                .lines()
                .filter(|line| !line.trim_start().starts_with("```"))
                .collect::<Vec<_>>()
                .join("\n"),
            BlockKind::Quote => self
                .raw
                .lines()
                .map(|line| line.strip_prefix("> ").unwrap_or_else(|| line.strip_prefix('>').unwrap_or(line)))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => self.raw.clone(),
        }
    }

    /// Display rows wrapped to `width` cells.
    #[must_use]
    pub fn rows(&self, width: u32) -> Vec<String> {
        crate::text::wrap_cells(&self.display_text(), width as usize)
    }
}

/// What one [`MarkdownDoc::set_source`] call changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChangeSet {
    /// Leading blocks reused from the previous parse (same id).
    pub reused: usize,
    /// Index of the first replaced block (`blocks.len()` when purely appended).
    pub replaced_from: usize,
    /// Number of blocks in the new document.
    pub total: usize,
}

/// Fail-closed document errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownError {
    SourceTooLarge,
    TooManyBlocks,
}

impl core::fmt::Display for MarkdownError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::SourceTooLarge => "markdown source too large",
            Self::TooManyBlocks => "markdown block limit",
        })
    }
}

impl std::error::Error for MarkdownError {}

/// Streaming Markdown document.
#[derive(Debug, Clone, Default)]
pub struct MarkdownDoc {
    blocks: Vec<Block>,
    source: String,
    next_id: u64,
    revision: u64,
}

impl MarkdownDoc {
    #[must_use]
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            source: String::new(),
            next_id: 0,
            revision: 0,
        }
    }

    #[must_use]
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Monotonic revision, bumped on every accepted update.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Blocks that can still change type.
    #[must_use]
    pub fn unstable_count(&self) -> usize {
        self.blocks.iter().filter(|b| b.unstable).count()
    }

    /// Replace the source, reusing the matching prefix.
    ///
    /// Mirrors the audited incremental parser: a leading run of blocks whose
    /// kind **and** raw text are unchanged keeps its id, everything after the
    /// first difference is rebuilt, and the final [`UNSTABLE_TAIL`] blocks are
    /// flagged unstable. Previous block ids are never resurrected out of order,
    /// so a late renderer cannot update a replaced block.
    pub fn set_source(&mut self, source: &str) -> Result<ChangeSet, MarkdownError> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(MarkdownError::SourceTooLarge);
        }
        let parsed = parse_blocks(source)?;
        let previous = core::mem::take(&mut self.blocks);
        let mut reused = 0usize;
        while reused < parsed.len()
            && reused < previous.len()
            && parsed[reused].kind == previous[reused].kind
            && parsed[reused].raw == previous[reused].raw
        {
            reused += 1;
        }
        let mut blocks: Vec<Block> = Vec::with_capacity(parsed.len());
        for (index, mut block) in parsed.into_iter().enumerate() {
            block.id = if index < reused {
                previous[index].id
            } else {
                self.alloc_id()
            };
            blocks.push(block);
        }
        // Ids of replaced blocks are dropped, so a late consumer holding one
        // cannot update a block that no longer exists.
        drop(previous);
        let tail = blocks.len().saturating_sub(UNSTABLE_TAIL);
        for (index, block) in blocks.iter_mut().enumerate() {
            block.unstable = index >= tail;
        }
        let total = blocks.len();
        self.blocks = blocks;
        self.source = source.to_owned();
        self.revision += 1;
        Ok(ChangeSet {
            reused,
            replaced_from: reused,
            total,
        })
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Full marker-free text in block order.
    #[must_use]
    pub fn display_text(&self) -> String {
        self.blocks
            .iter()
            .map(Block::display_text)
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

fn parse_blocks(source: &str) -> Result<Vec<Block>, MarkdownError> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut lines = source.split('\n').enumerate().peekable();
    let mut push = |kind: BlockKind, raw: String, start: usize| -> Result<(), MarkdownError> {
        if blocks.len() >= MAX_BLOCKS {
            return Err(MarkdownError::TooManyBlocks);
        }
        blocks.push(Block {
            id: 0,
            kind,
            raw,
            start_line: start,
            unstable: false,
        });
        Ok(())
    };
    while let Some((index, line)) = lines.next() {
        let trimmed = line.trim_end();
        let leading = trimmed.trim_start();
        if leading.is_empty() {
            continue;
        }
        // Fenced code: consume until a closing fence or EOF.
        if leading.starts_with("```") {
            let lang = leading.trim_start_matches('`').trim();
            let lang = if lang.is_empty() {
                None
            } else {
                Some(lang.to_owned())
            };
            let mut raw = String::from(line);
            let mut closed = false;
            for (_, body) in lines.by_ref() {
                raw.push('\n');
                raw.push_str(body);
                if body.trim_start().starts_with("```") {
                    closed = true;
                    break;
                }
            }
            push(BlockKind::Code { lang, closed }, raw, index)?;
            continue;
        }
        if let Some(level) = heading_level(leading) {
            push(BlockKind::Heading(level), trimmed.to_owned(), index)?;
            continue;
        }
        if is_rule(leading) {
            push(BlockKind::Rule, trimmed.to_owned(), index)?;
            continue;
        }
        if leading.starts_with('>') {
            let mut raw = String::from(line);
            loop {
                let Some((_, next)) = lines.peek() else { break };
                if !next.trim_start().starts_with('>') {
                    break;
                }
                let next = next.trim_end().to_owned();
                raw.push('\n');
                raw.push_str(&next);
                lines.next();
            }
            push(BlockKind::Quote, raw, index)?;
            continue;
        }
        if let Some((ordered, marker)) = list_marker(leading) {
            // One block per item: appending an item reuses the earlier items.
            push(
                BlockKind::ListItem { ordered, marker },
                trimmed.to_owned(),
                index,
            )?;
            continue;
        }
        // Table: a header row with pipes followed by a delimiter row.
        if leading.contains('|') {
            let delimiter = lines
                .peek()
                .map(|(_, next)| next.trim())
                .filter(|next| is_table_delimiter(next))
                .map(str::to_owned);
            if let Some(delimiter) = delimiter {
                let mut raw = format!("{trimmed}\n{delimiter}");
                lines.next();
                loop {
                    let Some((_, body)) = lines.peek() else { break };
                    if !(body.contains('|') && !body.trim().is_empty()) {
                        break;
                    }
                    let body = body.trim_end().to_owned();
                    raw.push('\n');
                    raw.push_str(&body);
                    lines.next();
                }
                push(BlockKind::Table, raw, index)?;
                continue;
            }
        }
        // Paragraph: consume following plain lines.
        let mut raw = String::from(trimmed);
        loop {
            let Some((_, next)) = lines.peek() else { break };
            let next_trimmed = next.trim();
            if next_trimmed.is_empty()
                || heading_level(next_trimmed).is_some()
                || next_trimmed.starts_with("```")
                || next_trimmed.starts_with('>')
                || list_marker(next_trimmed).is_some()
                || is_rule(next_trimmed)
            {
                break;
            }
            let next = next.trim_end().to_owned();
            raw.push('\n');
            raw.push_str(&next);
            lines.next();
        }
        push(BlockKind::Paragraph, raw, index)?;
    }
    Ok(blocks)
}

fn heading_level(line: &str) -> Option<u8> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && line.chars().nth(hashes) == Some(' ') {
        Some(hashes as u8)
    } else {
        None
    }
}

fn is_rule(line: &str) -> bool {
    let compact: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && (compact.iter().all(|c| *c == '-')
            || compact.iter().all(|c| *c == '*')
            || compact.iter().all(|c| *c == '_'))
}

fn list_marker(line: &str) -> Option<(bool, String)> {
    for marker in ["- ", "* ", "+ "] {
        if line.starts_with(marker) {
            return Some((false, marker.trim_end().to_owned()));
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        let rest = &line[digits..];
        if rest.starts_with(". ") {
            return Some((true, format!("{}.", &line[..digits])));
        }
    }
    None
}

fn is_table_delimiter(line: &str) -> bool {
    line.contains('|')
        && line
            .chars()
            .all(|c| matches!(c, '|' | '-' | ':' | ' '))
        && line.chars().any(|c| c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi06_parse_block_kinds() {
        let source = "# Title\n\npara line\n\n- one\n- two\n\n1. first\n\n> quoted\n\n```rust\nfn x() {}\n```\n\n---\n\n| a | b |\n| --- | --- |\n| 1 | 2 |";
        let mut doc = MarkdownDoc::new();
        doc.set_source(source).unwrap();
        let kinds: Vec<_> = doc.blocks().iter().map(|b| b.kind().clone()).collect();
        assert_eq!(kinds[0], BlockKind::Heading(1));
        assert_eq!(kinds[1], BlockKind::Paragraph);
        assert!(matches!(kinds[2], BlockKind::ListItem { ordered: false, .. }));
        assert!(matches!(kinds[3], BlockKind::ListItem { ordered: false, .. }));
        assert!(matches!(kinds[4], BlockKind::ListItem { ordered: true, .. }));
        assert_eq!(kinds[5], BlockKind::Quote);
        assert!(matches!(
            kinds[6],
            BlockKind::Code {
                lang: Some(_),
                closed: true
            }
        ));
        assert_eq!(kinds[7], BlockKind::Rule);
        assert_eq!(kinds[8], BlockKind::Table);
        assert_eq!(doc.blocks().len(), 9);
    }

    #[test]
    fn ffi06_stable_prefix_reused_on_append() {
        let mut doc = MarkdownDoc::new();
        doc.set_source("# Title\n\nintro\n\n- one").unwrap();
        let first: Vec<u64> = doc.blocks().iter().map(Block::id).collect();
        assert_eq!(first.len(), 3);

        // Append a second list item: earlier blocks are reused, not rebuilt
        // (audit ref [T1]: object reuse while appending list items).
        let change = doc.set_source("# Title\n\nintro\n\n- one\n- two").unwrap();
        assert_eq!(change.reused, 3, "all previous blocks reused");
        let ids: Vec<u64> = doc.blocks().iter().map(Block::id).collect();
        assert_eq!(&ids[..3], &first[..], "ids survive the append");
        assert_eq!(ids.len(), 4);
        assert_ne!(ids[3], first[2], "the appended item is a new block");

        // Editing the first block invalidates everything after it.
        let change = doc.set_source("# Changed\n\nintro\n\n- one\n- two").unwrap();
        assert_eq!(change.reused, 0);
        assert!(doc
            .blocks()
            .iter()
            .all(|block| !first.contains(&block.id())));
        assert_eq!(doc.revision(), 3);
    }

    #[test]
    fn ffi06_last_two_blocks_unstable() {
        let mut doc = MarkdownDoc::new();
        doc.set_source("# a\n\nb\n\nc").unwrap();
        let stability: Vec<bool> = doc.blocks().iter().map(Block::is_unstable).collect();
        assert_eq!(stability, vec![false, true, true]);
        assert_eq!(doc.unstable_count(), 2);
        // Short documents mark every block unstable but never panic.
        doc.set_source("only").unwrap();
        assert_eq!(doc.blocks().len(), 1);
        assert!(doc.blocks()[0].is_unstable());
        assert_eq!(doc.unstable_count(), 1);
    }

    #[test]
    fn ffi06_display_text_conceals_markers() {
        let source = "# Head\n\n- item\n\n> quote\n\n```js\nlet a = 1;\n```";
        let mut doc = MarkdownDoc::new();
        doc.set_source(source).unwrap();
        let texts: Vec<String> = doc.blocks().iter().map(Block::display_text).collect();
        assert_eq!(texts[0], "Head");
        assert_eq!(texts[1], "item");
        assert_eq!(texts[2], "quote");
        assert_eq!(texts[3], "let a = 1;");
        for text in &texts {
            assert!(!text.contains("```"), "raw fence marker leaked: {text:?}");
            assert!(!text.starts_with("# "), "raw heading marker leaked");
        }
        // Wrapped rows respect the requested cell width.
        let rows = doc.blocks()[1].rows(3);
        assert_eq!(rows, vec!["ite".to_owned(), "m".to_owned()]);
    }

    #[test]
    fn ffi06_unclosed_fence_stays_a_code_block() {
        let mut doc = MarkdownDoc::new();
        doc.set_source("```python\nprint(1)").unwrap();
        let block = &doc.blocks()[0];
        assert!(matches!(
            block.kind(),
            BlockKind::Code {
                lang: Some(_),
                closed: false
            }
        ));
        assert_eq!(block.display_text(), "print(1)");
        assert!(block.is_unstable(), "the open fence can still change");
        // Completing the fence changes the raw text and yields a new block.
        let before = block.id();
        doc.set_source("```python\nprint(1)\n```").unwrap();
        assert_ne!(doc.blocks()[0].id(), before);
        assert!(matches!(
            doc.blocks()[0].kind(),
            BlockKind::Code { closed: true, .. }
        ));
    }

    #[test]
    fn ffi06_fail_closed_limits() {
        let mut doc = MarkdownDoc::new();
        assert_eq!(
            doc.set_source(&"x".repeat(MAX_SOURCE_BYTES + 1)),
            Err(MarkdownError::SourceTooLarge)
        );
        assert!(doc.blocks().is_empty(), "rejected source changes nothing");
        let many = (0..MAX_BLOCKS + 1)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        assert_eq!(doc.set_source(&many), Err(MarkdownError::TooManyBlocks));
        assert!(doc.blocks().is_empty());
    }

    #[test]
    fn ffi06_table_and_quote_grouping() {
        let mut doc = MarkdownDoc::new();
        doc.set_source("| h |\n| - |\n| v |\n\n> a\n> b").unwrap();
        assert!(matches!(doc.blocks()[0].kind(), BlockKind::Table));
        assert_eq!(doc.blocks()[0].raw(), "| h |\n| - |\n| v |");
        assert!(matches!(doc.blocks()[1].kind(), BlockKind::Quote));
        assert_eq!(doc.blocks()[1].display_text(), "a\nb");
    }
}
