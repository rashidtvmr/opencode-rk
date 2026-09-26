#![forbid(unsafe_code)]
//! Prompt state stores (std-only).
//!
//! Evidence (TS checkout a0d9b6c):
//! - frecency `packages/tui/src/prompt/frecency.tsx:8-36`
//!   score `frequency / (1 + (Date.now() - lastOpen) / 86400000)` (:35),
//!   cap `MAX_FRECENCY_ENTRIES = 1000` (:10), evict oldest-lastOpen (:64-67).
//!   Re-export shim `packages/tui/src/component/prompt/frecency.tsx:1` (identical pair).
//! - history `packages/tui/src/prompt/history.tsx:27-47,69-108`
//!   cap `MAX_HISTORY_ENTRIES = 50` (:27), negative-index cursor clamped
//!   `|next| > len return`, `next > 0 return` (:77-78), duplicate skip (:87-90).
//!   Shim `component/prompt/history.tsx:1`.
//! - stash `packages/tui/src/prompt/stash.tsx:9-30,50-86`
//!   cap `MAX_STASH_ENTRIES = 50` (:15), overflow drops oldest via
//!   `slice(-MAX)` (:56-58), `pop` empty -> undefined (:70). Shim `component/prompt/stash.tsx:1`.
//! - cwd `component/prompt/cwd.ts` is 0 bytes (no portable state); `Cwd`
//!   mirrors `paths.cwd` use in `component/prompt/index.tsx:445`.
//! - workspace `component/prompt/workspace.tsx:16-137` holds transient
//!   selection/creating signals only; no portable store (not ported).
//! - move `component/prompt/move.tsx:14-16` reminder text + transient
//!   progress signals only; no portable store (not ported).
//! - attachment `component/prompt/local-attachment.ts:25-48` mime table
//!   (:25-34) + accept rule: svg text, `image/*`, `application/pdf` (:39-44).
//!
//! Bounds below follow the lane contract (256/500/16), NOT the TS 1000/50/50.

/// Frecency entries retained.
pub const MAX_ITEMS: usize = 256;
/// Prompt history entries retained.
pub const MAX_HISTORY: usize = 500;
/// Stash slots retained.
pub const MAX_STASH: usize = 16;
/// Max attachment path length in bytes.
pub const MAX_ATTACHMENT_PATH_LEN: usize = 4096;
/// Max attachments held in a store.
pub const MAX_ATTACHMENTS: usize = 16;

/// Millis per day; divisor in the TS decay formula.
pub const MS_PER_DAY: f64 = 86_400_000.0;

/// Exact TS decay formula: `frequency / (1 + (now_ms - last_open) / 86400000)`.
#[must_use]
pub fn frecency_score(frequency: u64, last_open_ms: i64, now_ms: i64) -> f64 {
    frequency as f64 / (1.0 + (now_ms - last_open_ms) as f64 / MS_PER_DAY)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrecencyEntry {
    pub path: String,
    pub frequency: u64,
    pub last_open_ms: i64,
}

#[derive(Clone, Debug, Default)]
pub struct Frecency {
    entries: Vec<FrecencyEntry>,
}

impl Frecency {
    #[must_use]
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Record an access; evicts oldest `last_open_ms` beyond [`MAX_ITEMS`].
    pub fn record(&mut self, path: &str, now_ms: i64) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.path == path) {
            e.frequency = e.frequency.saturating_add(1);
            e.last_open_ms = now_ms;
        } else {
            self.entries.push(FrecencyEntry { path: path.to_string(), frequency: 1, last_open_ms: now_ms });
        }
        if self.entries.len() > MAX_ITEMS {
            self.entries.sort_by_key(|e| e.last_open_ms);
            let drop = self.entries.len() - MAX_ITEMS;
            self.entries.drain(..drop);
        }
    }

    #[must_use]
    pub fn score(&self, path: &str, now_ms: i64) -> f64 {
        self.entries
            .iter()
            .find(|e| e.path == path)
            .map_or(0.0, |e| frecency_score(e.frequency, e.last_open_ms, now_ms))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub struct History {
    entries: Vec<String>,
    /// Cursor: `entries.len()` means fresh draft (== TS index 0).
    cursor: usize,
}

impl History {
    #[must_use]
    pub fn new() -> Self {
        Self { entries: Vec::new(), cursor: 0 }
    }

    /// Push entry; consecutive duplicates reset cursor without appending (TS :87-90).
    pub fn push(&mut self, entry: &str) {
        if self.entries.last().is_some_and(|last| last == entry) {
            self.cursor = self.entries.len();
            return;
        }
        self.entries.push(entry.to_string());
        if self.entries.len() > MAX_HISTORY {
            self.entries.drain(..self.entries.len() - MAX_HISTORY);
        }
        self.cursor = self.entries.len();
    }

    /// Older entry; clamps at oldest (TS :77 `|next| > len return`).
    #[must_use]
    pub fn prev(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        self.cursor = self.cursor.saturating_sub(1);
        self.entries.get(self.cursor).map(String::as_str)
    }

    /// Newer entry; `None` at fresh draft, clamps (TS :78 `next > 0 return`).
    #[must_use]
    pub fn next(&mut self) -> Option<&str> {
        if self.cursor >= self.entries.len() {
            return None;
        }
        self.cursor += 1;
        if self.cursor >= self.entries.len() {
            return None;
        }
        self.entries.get(self.cursor).map(String::as_str)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StashEntry {
    pub input: String,
    pub timestamp_ms: i64,
}

#[derive(Clone, Debug, Default)]
pub struct Stash {
    entries: Vec<StashEntry>,
}

impl Stash {
    #[must_use]
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Push slot; overflow drops oldest (TS `slice(-MAX_STASH_ENTRIES)`).
    pub fn push(&mut self, input: &str, now_ms: i64) {
        self.entries.push(StashEntry { input: input.to_string(), timestamp_ms: now_ms });
        if self.entries.len() > MAX_STASH {
            self.entries.drain(..self.entries.len() - MAX_STASH);
        }
    }

    pub fn pop(&mut self) -> Option<StashEntry> {
        self.entries.pop()
    }

    pub fn remove(&mut self, index: usize) -> Option<StashEntry> {
        if index < self.entries.len() {
            Some(self.entries.remove(index))
        } else {
            None
        }
    }

    #[must_use]
    pub fn list(&self) -> &[StashEntry] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cwd {
    pub path: String,
    pub is_trusted: bool,
}

impl Cwd {
    #[must_use]
    pub fn new(path: &str, is_trusted: bool) -> Self {
        Self { path: path.to_string(), is_trusted }
    }

    pub fn set_trusted(&mut self, trusted: bool) {
        self.is_trusted = trusted;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttachmentKind {
    SvgText,
    Image { mime: &'static str },
    Pdf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub path: String,
    pub kind: AttachmentKind,
}

impl Attachment {
    /// Validate path bound + kind from extension (TS mime table
    /// `local-attachment.ts:25-34`, accept rule :39-44).
    pub fn validate(path: &str) -> Option<Self> {
        if path.is_empty() || path.len() > MAX_ATTACHMENT_PATH_LEN {
            return None;
        }
        let ext = path.rsplit('.').next()?.to_ascii_lowercase();
        let kind = match ext.as_str() {
            "svg" => AttachmentKind::SvgText,
            "avif" => AttachmentKind::Image { mime: "image/avif" },
            "gif" => AttachmentKind::Image { mime: "image/gif" },
            "jpg" | "jpeg" => AttachmentKind::Image { mime: "image/jpeg" },
            "png" => AttachmentKind::Image { mime: "image/png" },
            "webp" => AttachmentKind::Image { mime: "image/webp" },
            "pdf" => AttachmentKind::Pdf,
            _ => return None,
        };
        Some(Self { path: path.to_string(), kind })
    }
}

#[derive(Clone, Debug, Default)]
pub struct AttachmentStore {
    items: Vec<Attachment>,
}

impl AttachmentStore {
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// `false` when over cap ([`MAX_ATTACHMENTS`]) or invalid path.
    pub fn push(&mut self, path: &str) -> bool {
        if self.items.len() >= MAX_ATTACHMENTS {
            return false;
        }
        match Attachment::validate(path) {
            Some(a) => {
                self.items.push(a);
                true
            }
            None => false,
        }
    }

    #[must_use]
    pub fn list(&self) -> &[Attachment] {
        &self.items
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Rank cap for autocomplete; TS `limit: 10` (:511) + `slice(0, 10)` (:524).
pub const RANK_LIMIT: usize = 10;
/// Alias kept for the task contract (`LIMIT` == `RANK_LIMIT`).
pub const LIMIT: usize = RANK_LIMIT;
/// fuzzysort `threshold` for `@` mode (:510). `@` = 0.5, `/` = 0.
/// DIVERGENCE: no fuzzysort in std-only; substring/prefix match replaces it.
pub const FUZZY_THRESHOLD: f64 = 0.5;

/// Parse trailing `#N[-M]` line range (TS `extractLineRange` :32-57).
/// Returns `(start, Option<end>)`; end kept only when `start < end` (:47).
#[must_use]
pub fn parse_line_range(s: &str) -> Option<(u32, Option<u32>)> {
    let hash = s.rfind('#')?;
    let part = &s[hash + 1..];
    if part.is_empty() {
        return None;
    }
    let (start_s, end_s) = match part.split_once('-') {
        Some((a, b)) => (a, Some(b)),
        None => (part, None),
    };
    if start_s.is_empty() || !start_s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let start: u32 = start_s.parse().ok()?;
    let end = match end_s {
        None => None,
        Some("") => None, // open-ended `N-`
        Some(e) if !e.is_empty() && e.bytes().all(|b| b.is_ascii_digit()) => {
            let n: u32 = e.parse().ok()?;
            (start < n).then_some(n)
        }
        _ => return None, // `N-x` invalid -> whole match invalid (TS :42)
    };
    Some((start, end))
}

/// Strip trailing `#...` range from a query (TS `removeLineRange` :27-30).
#[must_use]
pub fn remove_line_range(s: &str) -> &str {
    match s.rfind('#') {
        Some(i) => &s[..i],
        None => s,
    }
}

/// Weight base score with frecency + prefix-x2
/// (TS `scoreFn` :512-520: prefix `score *= 2`, then `score * (1 + frecency)`).
/// NOTE: `frecency_score` (f64 decay) feeds the `frecency: f32` arg; f32
/// suffices here (ranking weight, not timestamp math).
#[must_use]
pub fn score_with_frecency(base: u8, frecency: f32, is_prefix: bool) -> f32 {
    let s = f32::from(base) * if is_prefix { 2.0 } else { 1.0 };
    s * (1.0 + frecency)
}

/// Hide rules (TS :678-687): cursor<=index, ws in range, `/cmd arg` pattern.
#[must_use]
pub fn should_hide_autocomplete(cursor: usize, index: usize, in_ws: bool, cmd_arg: bool) -> bool {
    cursor <= index || in_ws || cmd_arg
}

/// Mention trigger (TS `display.ts:38-48` via `:703`):
/// at start or after ws, and no ws inside the `@query`.
#[must_use]
pub fn mention_triggered(at_start: bool, prev_ws: bool, no_ws_query: bool) -> bool {
    (at_start || prev_ws) && no_ws_query
}

/// Space needed after insert when char after cursor is not a space
/// (TS `insertPart` :176-178).
#[must_use]
pub fn needs_space(before: Option<char>) -> bool {
    before.map_or(true, |c| c != ' ')
}

/// Merge commands (TS :447-474): skip `skill` source, `:mcp` tag for mcp
/// source, `localeCompare` sort, `padEnd(max + 2)` width.
#[must_use]
pub fn merge_commands(items: Vec<(String, bool, bool)>) -> Vec<String> {
    // (name, is_skill, is_mcp)
    let mut out: Vec<String> = items
        .into_iter()
        .filter(|(_, skill, _)| !skill)
        .map(|(name, _, mcp)| format!("/{name}{}", if mcp { ":mcp" } else { "" }))
        .collect();
    out.sort();
    let max = out.iter().map(|s| s.len()).max().unwrap_or(0);
    out.into_iter().map(|s| format!("{s:<width$}", width = max + 2)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decay_math_exact() {
        // freq 2, age exactly 1 day -> 2 / (1 + 1) = 1.0
        assert!((frecency_score(2, 0, 86_400_000) - 1.0).abs() < 1e-12);
        // fresh access -> frequency unchanged
        assert!((frecency_score(3, 1000, 1000) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn frecency_missing_scores_zero() {
        let f = Frecency::new();
        assert_eq!(f.score("nope", 0), 0.0);
    }

    #[test]
    fn frecency_record_counts_and_bound() {
        let mut f = Frecency::new();
        f.record("a", 1);
        f.record("a", 2);
        assert!((f.score("a", 2) - 2.0).abs() < 1e-12);
        for i in 0..MAX_ITEMS {
            f.record(&format!("f{i}"), 10 + i as i64);
        }
        assert_eq!(f.len(), MAX_ITEMS);
    }

    #[test]
    fn history_cursor_clamps() {
        let mut h = History::new();
        assert_eq!(h.prev(), None);
        assert_eq!(h.next(), None);
        h.push("one");
        h.push("two");
        assert_eq!(h.prev(), Some("two"));
        assert_eq!(h.prev(), Some("one"));
        assert_eq!(h.prev(), Some("one")); // clamped at oldest
        assert_eq!(h.next(), Some("two"));
        assert_eq!(h.next(), None); // fresh draft
        assert_eq!(h.next(), None); // stays clamped
    }

    #[test]
    fn history_duplicate_push_skips() {
        let mut h = History::new();
        h.push("x");
        h.push("x");
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn stash_overflow_drops_oldest() {
        let mut s = Stash::new();
        for i in 0..(MAX_STASH + 1) {
            s.push(&format!("e{i}"), i as i64);
        }
        assert_eq!(s.len(), MAX_STASH);
        assert_eq!(s.list()[0].input, "e1");
        assert!(Stash::new().pop().is_none());
    }

    #[test]
    fn cwd_trust_flag() {
        let mut c = Cwd::new("/repo", false);
        assert!(!c.is_trusted);
        c.set_trusted(true);
        assert!(c.is_trusted);
    }

    #[test]
    fn attachment_kinds_and_over_cap() {
        assert_eq!(Attachment::validate("a.svg").unwrap().kind, AttachmentKind::SvgText);
        assert_eq!(
            Attachment::validate("a.png").unwrap().kind,
            AttachmentKind::Image { mime: "image/png" }
        );
        assert_eq!(Attachment::validate("a.pdf").unwrap().kind, AttachmentKind::Pdf);
        assert!(Attachment::validate("a.txt").is_none());
        assert!(Attachment::validate("").is_none());
        let long = "x".repeat(MAX_ATTACHMENT_PATH_LEN + 1) + ".png";
        assert!(Attachment::validate(&long).is_none()); // path over-cap
        let mut store = AttachmentStore::new();
        for i in 0..MAX_ATTACHMENTS {
            assert!(store.push(&format!("f{i}.png")));
        }
        assert!(!store.push("extra.png")); // count over-cap
        assert_eq!(store.len(), MAX_ATTACHMENTS);
    }

    #[test]
    fn range_parse_cases() {
        assert_eq!(parse_line_range("a.ts#12"), Some((12, None)));
        assert_eq!(parse_line_range("a.ts#12-20"), Some((12, Some(20))));
        assert_eq!(parse_line_range("a.ts#20-12"), Some((20, None)));
        assert_eq!(parse_line_range("a.ts#12-"), Some((12, None)));
        assert_eq!(parse_line_range("a.ts#x"), None);
        assert_eq!(parse_line_range("plain"), None);
    }

    #[test]
    fn range_strip_query() {
        assert_eq!(remove_line_range("a.ts#12"), "a.ts");
        assert_eq!(remove_line_range("a.ts#12-20"), "a.ts");
        assert_eq!(remove_line_range("plain"), "plain");
    }

    #[test]
    fn score_prefix_only() {
        assert!((score_with_frecency(10, 0.0, true) - 20.0).abs() < 1e-6);
    }

    #[test]
    fn score_frecency_only() {
        assert!((score_with_frecency(10, 1.0, false) - 20.0).abs() < 1e-6);
    }

    #[test]
    fn score_combined() {
        assert!((score_with_frecency(10, 1.0, true) - 40.0).abs() < 1e-6);
    }

    #[test]
    fn hide_all_rules() {
        assert!(should_hide_autocomplete(0, 2, false, false));
        assert!(should_hide_autocomplete(5, 2, true, false));
        assert!(should_hide_autocomplete(5, 2, false, true));
        assert!(!should_hide_autocomplete(5, 2, false, false));
    }

    #[test]
    fn mention_and_space() {
        assert!(mention_triggered(true, false, true));
        assert!(mention_triggered(false, true, true));
        assert!(!mention_triggered(false, false, true));
        assert!(!mention_triggered(true, false, false));
        assert!(!needs_space(Some(' ')));
        assert!(needs_space(Some('x')));
        assert!(needs_space(None));
    }

    #[test]
    fn rank_consts() {
        assert_eq!(RANK_LIMIT, 10);
        assert_eq!(LIMIT, 10);
        assert!((FUZZY_THRESHOLD - 0.5).abs() < 1e-12);
    }

    #[test]
    fn merge_skips_skill_tags_mcp() {
        let out = merge_commands(vec![
            ("b".into(), false, true),
            ("a".into(), false, false),
            ("s".into(), true, false),
        ]);
        assert_eq!(out.len(), 2);
        assert!(out[0].starts_with("/a"));
        assert!(out[1].starts_with("/b:mcp"));
        assert_eq!(out[0].len(), out[1].len()); // padEnd(max+2)
    }
}
