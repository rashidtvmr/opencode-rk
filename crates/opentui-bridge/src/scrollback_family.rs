#![forbid(unsafe_code)]
//! Typed renderables over the bounded scrollback contract.
//!
//! Mirrors `run_scrollback.rs` (CAP/freeze/evict) with typed rows:
//! text passes through, code is fenced, markdown stays raw.

/// Maximum bytes retained per renderable body.
pub const BODY_CAP: usize = 8 * 1024;

/// Maximum retained rows. Older rows are evicted first.
pub const CAP: usize = 2_000;

/// Alias matching `run_scrollback::MAX_ROWS`.
pub const MAX_ROWS: usize = CAP;

/// Kind of a single scrollback row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderableKind {
    Text,
    Code { lang: String },
    Markdown,
}

/// One typed scrollback row; body capped at [`BODY_CAP`] bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renderable {
    pub kind: RenderableKind,
    pub body: String,
}

impl Renderable {
    #[must_use]
    pub fn new(kind: RenderableKind, body: impl AsRef<str>) -> Self {
        Self {
            kind,
            body: truncate_body(body.as_ref()),
        }
    }
}

fn truncate_body(body: &str) -> String {
    if body.len() <= BODY_CAP {
        return body.to_owned();
    }
    let mut end = BODY_CAP;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    body[..end].to_owned()
}

/// Bounded, optionally frozen typed transcript.
#[derive(Debug, Clone, Default)]
pub struct ScrollbackWriter {
    pub rows: Vec<Renderable>,
    pub frozen: bool,
}

/// Back-compat alias: the typed surface of the scrollback family.
pub type Surface = ScrollbackWriter;

/// Back-compat alias: an owned snapshot of retained rows.
pub type Snapshot = Vec<Renderable>;

impl ScrollbackWriter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rows: Vec::new(),
            frozen: false,
        }
    }

    #[must_use]
    pub const fn cap() -> usize {
        CAP
    }

    fn push_renderable(&mut self, row: Renderable) {
        if self.frozen {
            return;
        }
        self.rows.push(row);
        if self.rows.len() > Self::cap() {
            let excess = self.rows.len() - Self::cap();
            self.rows.drain(..excess);
        }
    }

    /// Append one typed row, evicting oldest rows beyond [`CAP`].
    pub fn push(&mut self, kind: RenderableKind, body: impl AsRef<str>) {
        self.push_renderable(Renderable::new(kind, body));
    }

    /// Commit a completed row into retained scrollback.
    pub fn commit(&mut self, kind: RenderableKind, body: impl AsRef<str>) {
        self.push(kind, body);
    }

    /// Append the standard turn separator.
    pub fn separator(&mut self) {
        self.push_renderable(Renderable::new(RenderableKind::Text, "---"));
    }

    /// Freeze future writes while preserving the current snapshot.
    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    /// Return an owned copy of the retained rows.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Renderable> {
        self.rows.clone()
    }
}

/// Render typed rows to plain text: text/markdown raw, code fenced.
#[must_use]
pub fn writer_to_text(rows: &[Renderable]) -> Vec<String> {
    rows.iter()
        .map(|r| match &r.kind {
            RenderableKind::Text | RenderableKind::Markdown => r.body.clone(),
            RenderableKind::Code { lang } => format!("```{lang}\n{}\n```", r.body),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_passthrough() {
        let mut w = ScrollbackWriter::new();
        w.push(RenderableKind::Text, "hello");
        assert_eq!(writer_to_text(&w.snapshot()), vec!["hello"]);
    }

    #[test]
    fn markdown_stays_raw() {
        let mut w = ScrollbackWriter::new();
        w.push(RenderableKind::Markdown, "# hi *x*");
        assert_eq!(writer_to_text(&w.snapshot()), vec!["# hi *x*"]);
    }

    #[test]
    fn code_is_fenced() {
        let mut w = ScrollbackWriter::new();
        w.push(
            RenderableKind::Code {
                lang: "rust".to_owned(),
            },
            "let x = 1;",
        );
        assert_eq!(
            writer_to_text(&w.snapshot()),
            vec!["```rust\nlet x = 1;\n```"]
        );
    }

    #[test]
    fn body_capped_at_8kib() {
        let r = Renderable::new(RenderableKind::Text, "a".repeat(BODY_CAP + 10));
        assert_eq!(r.body.len(), BODY_CAP);
    }

    #[test]
    fn cap_evicts_oldest() {
        let mut w = ScrollbackWriter::new();
        for i in 0..=CAP {
            w.push(RenderableKind::Text, format!("row-{i}"));
        }
        assert_eq!(w.rows.len(), CAP);
        assert_eq!(w.rows.first().map(|r| r.body.as_str()), Some("row-1"));
    }

    #[test]
    fn freeze_blocks_writes() {
        let mut w = ScrollbackWriter::new();
        w.commit(RenderableKind::Text, "before");
        w.freeze();
        w.push(RenderableKind::Text, "after");
        w.commit(RenderableKind::Markdown, "also-after");
        w.separator();
        assert_eq!(w.snapshot().len(), 1);
        assert!(w.frozen);
    }

    #[test]
    fn snapshot_is_clone() {
        let mut w = ScrollbackWriter::new();
        w.push(RenderableKind::Text, "a");
        let snap = w.snapshot();
        w.push(RenderableKind::Text, "b");
        assert_eq!(snap.len(), 1);
        assert_eq!(w.snapshot().len(), 2);
    }
}
