#![forbid(unsafe_code)]
//! Bracketed-paste normalization, paste folding/expansion, draft stash, prompt
//! stash, tracked prompt parts and serialized async paste admission.
//!
//! Upstream reference: pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`.
//! The TUI source at this pin lives under `packages/tui/src/`:
//!
//! - `component/prompt/index.tsx:1429-1477` (`pasteInputText`): pasted text is
//!   ANSI-stripped, CR/CRLF normalized to `\n`, then trimmed for the folding
//!   decision; a paste folds when it has at least three lines **or** more than
//!   150 JavaScript string units (UTF-16 code units) and `config.prompt.paste`
//!   is not `"full"`. An unfolded paste inserts the normalized (untrimmed)
//!   text. A folded paste inserts `[Pasted ~N lines] ` plus a tracked part.
//!   Repeating an identical paste immediately after an existing folded part
//!   expands that part in place instead of folding again
//!   (`index.tsx:1455-1468`, extmark end at cursor or cursor-1).
//! - `component/prompt/index.tsx:1441-1462` (`expandPastedText`): expansion
//!   replaces the placeholder range with the stored text and drops the part.
//! - `component/prompt/index.tsx:1526-1538` (`clearPrompt`): a cleared draft is
//!   appended to prompt history only when the trimmed text is at least
//!   `DRAFT_RETENTION_MIN_CHARS` (20) or tracked pasted parts/attachments exist
//!   (`index.tsx:112-113`).
//! - `component/prompt/draft-stash.ts:1-18`: at most one in-progress draft per
//!   tab, consumed by `takeDraft`; `index.tsx:717-733` saves only a non-empty
//!   draft on cleanup and restores only into an empty composer, clamping the
//!   cursor to the measured text.
//! - `prompt/stash.tsx:9-91`: explicit prompt stash, FIFO-ish list capped at
//!   `MAX_STASH_ENTRIES` (50), `pop` removes the newest entry.
//! - `prompt/history.tsx:60-141`: prompt history capped at
//!   `MAX_HISTORY_ENTRIES` (50), duplicate-last detection, `move` walks back
//!   from index 0 and returns `emptyPrompt()` at index 0.
//! - `prompt/mention.ts:20-48,92-135`: mention realignment by identical text,
//!   ordered duplicate assignment, and pasted-placeholder expansion that shifts
//!   later mentions (`index.tsx:1134-1148`).
//! - `component/prompt/index.tsx:376-392` (`enqueuePaste`) and `:1141-1150`:
//!   pastes are serialized, and a paste that resolves after a session/mode/text
//!   change (or after unmount/disable) is dropped instead of inserting stale
//!   text.
//! - `component/prompt/index.tsx:398-402,1680-1698` and
//!   `component/prompt/local-attachment.ts:29-59,107-158`: image attachment
//!   previews are bounded by terminal height, at most three are painted, and
//!   pasted file paths are resolved only when they name local files.
//!
//! Offset note: upstream textarea/extmark offsets are *measured* cell offsets
//! (via `measureEditText`). The native port keeps part ranges as UTF-8 byte
//! offsets because the draft store is a Rust `String`; renderer math converts
//! them with [`measure_cells`] so painting never assumes byte offsets equal
//! cells (see CP-41). This deviation is recorded in the P10 handoff.

use std::collections::BTreeMap;

/// Bound on filesystem candidates parsed from one pasted path list.
pub const MAX_PASTED_FILEPATHS: usize = 32;
/// Upstream per-paste attachment byte budget (`local-attachment.ts:5`).
pub const MAX_LOCAL_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
/// Upstream explicit stash cap (`prompt/stash.tsx:14`).
pub const MAX_STASH_ENTRIES: usize = 50;
/// Upstream prompt history cap (`prompt/history.tsx:60`).
pub const MAX_HISTORY_ENTRIES: usize = 50;
/// Upstream retention threshold for a cleared draft (`index.tsx:112`).
pub const DRAFT_RETENTION_MIN_CHARS: usize = 20;
/// Upstream folding threshold beyond which a single-line paste folds
/// (`index.tsx:1448`).
pub const FOLD_MAX_UNITS: usize = 150;
/// Upstream folding threshold in lines (`index.tsx:1448`).
pub const FOLD_MIN_LINES: usize = 3;
/// Upstream maximum painted image previews (`index.tsx:402`).
pub const MAX_VISIBLE_PREVIEWS: usize = 3;

/// Prompt mode carried by drafts, stash and history entries
/// (`prompt/history.tsx:22`, `index.tsx:340-351`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptMode {
    Normal,
    Shell,
}

impl Default for PromptMode {
    fn default() -> Self {
        Self::Normal
    }
}

/// Strip terminal escape sequences the way `<textarea>` paste expects before
/// any text becomes draft content (`index.tsx:1430`).
///
/// CSI, OSC/DCS/SOS/PM/APC strings, two-byte escapes and a dangling ESC are
/// removed. Incomplete sequences at end of input are dropped rather than
/// leaked. C0 controls other than the stripped ones are preserved; the native
/// renderer maps them to a space cell.
pub fn strip_ansi_sequences(input: &str) -> String {
    #[derive(Clone, Copy)]
    enum Escape {
        Normal,
        Esc,
        Csi,
        Str,
    }
    let mut out = String::with_capacity(input.len());
    let mut state = Escape::Normal;
    for c in input.chars() {
        match state {
            Escape::Normal => {
                if c == '\x1b' {
                    state = Escape::Esc;
                } else {
                    out.push(c);
                }
            }
            Escape::Esc => match c {
                '[' => state = Escape::Csi,
                ']' | 'P' | 'X' | '^' | '_' => state = Escape::Str,
                // ESC + one character is a two-byte terminal escape.
                _ => state = Escape::Normal,
            },
            Escape::Csi => {
                if ('\x40'..='\x7e').contains(&c) {
                    state = Escape::Normal;
                }
            }
            Escape::Str => match c {
                '\x07' => state = Escape::Normal,
                '\x1b' => state = Escape::Esc,
                _ => {}
            },
        }
    }
    out
}

/// Normalize a pasted body: strip ANSI sequences, then fold CRLF and lone CR
/// into `\n` (`index.tsx:1430`).
pub fn normalize_paste(input: &str) -> String {
    strip_ansi_sequences(input)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

/// ECMAScript `String.prototype.trim` removes WhiteSpace (TAB, VT, FF, SP,
/// NBSP, ZWNBSP/U+FEFF, and every `Zs` space separator) plus LineTerminator
/// (LF, CR, LS, PS). Rust's `char::is_whitespace` follows Unicode White_Space,
/// which additionally includes U+0085 NEL and lacks U+FEFF; exclude NEL and add
/// the BOM so the trimming set matches ECMAScript exactly (upstream
/// `index.tsx:1431` `normalizedText.trim()`).
fn is_js_whitespace(c: char) -> bool {
    c == '\u{feff}' || (c != '\u{85}' && c.is_whitespace())
}

/// Upstream trim for the folding decision (`index.tsx:1431`).
pub fn js_trim(input: &str) -> &str {
    input.trim_matches(is_js_whitespace)
}

/// JavaScript `string.length`: UTF-16 code units, so an astral character
/// counts twice (`index.tsx:1448`). This is deliberately *not* a byte length.
pub fn utf16_len(input: &str) -> usize {
    input.encode_utf16().count()
}

/// Lines in a pasted body, mirroring `(match(/\n/g)?.length ?? 0) + 1`.
pub fn line_count(input: &str) -> usize {
    1 + input.bytes().filter(|byte| *byte == b'\n').count()
}

/// Display cells of `text` under the native renderer's cluster segmentation.
///
/// Bounded to 4096 cells; longer text is measured only up to that budget,
/// which is far beyond any prompt row width.
pub fn measure_cells(text: &str) -> usize {
    let mut total = 0usize;
    crate::native_view_layout::visit_unpadded_cells(text, 4096, |_, width| total += width);
    total
}

/// `config.prompt.paste` (`index.tsx:1448`). `full` never folds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PasteMode {
    /// Default: long or multi-line pastes collapse into a tracked placeholder.
    Fold,
    /// `config.prompt.paste === "full"`: always insert the literal text.
    Full,
}

impl Default for PasteMode {
    fn default() -> Self {
        Self::Fold
    }
}

impl PasteMode {
    /// Parse the configuration value. Upstream compares the raw value against
    /// the exact literal `"full"` (`config.prompt?.paste !== "full"`,
    /// `prompt/index.tsx:1463`), so only `Some("full")` selects [`PasteMode::Full`];
    /// any other value folds.
    #[must_use]
    pub fn from_config(value: Option<&str>) -> Self {
        match value {
            Some("full") => Self::Full,
            _ => Self::Fold,
        }
    }
}

/// A folded paste tracked in the draft. `start`/`end` are UTF-8 byte offsets
/// of the visible placeholder in the draft text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PastedText {
    /// Full pasted content, restored on expansion.
    pub text: String,
    /// Visible placeholder, e.g. `[Pasted ~3 lines]`.
    pub placeholder: String,
    pub start: usize,
    pub end: usize,
}

impl PastedText {
    /// Range length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// What a pasted body does to the draft (`index.tsx:1429-1477`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PastePlan {
    /// Insert the literal normalized text (short/simple paste, or `full`).
    Insert(String),
    /// Insert a placeholder plus its tracked part.
    Fold(PastedText),
    /// Expand an existing folded part at `index` because the same content was
    /// pasted again next to it.
    Expand(usize),
}

/// Rejections leave the draft untouched.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PasteError {
    /// The insertion would exceed the caller's draft byte budget.
    TooLarge { bytes: usize, limit: usize },
    /// The cursor is not a character boundary of the draft.
    NotBoundary,
}

/// Decide how a raw pasted body enters the draft at `cursor`.
#[must_use]
pub fn plan_paste(
    body: &str,
    mode: PasteMode,
    cursor: usize,
    parts: &[PastedText],
) -> PastePlan {
    let normalized = normalize_paste(body);
    let content = js_trim(&normalized);
    let lines = line_count(content);
    let folds = mode == PasteMode::Fold
        && (lines >= FOLD_MIN_LINES || utf16_len(content) > FOLD_MAX_UNITS);
    if !folds {
        return PastePlan::Insert(normalized);
    }
    if let Some(index) = parts.iter().position(|part| {
        (part.end == cursor || part.end + 1 == cursor) && part.text == content
    }) {
        return PastePlan::Expand(index);
    }
    let placeholder = format!("[Pasted ~{lines} lines]");
    PastePlan::Fold(PastedText {
        text: content.to_owned(),
        start: cursor,
        end: cursor + placeholder.len(),
        placeholder,
    })
}

/// Shift every tracked part for an insertion of `len` bytes at `at`.
///
/// An insertion strictly inside a part extends it; an insertion exactly at a
/// part's end leaves it unchanged, matching textarea extmark behavior.
pub fn shift_for_insert(parts: &mut [PastedText], at: usize, len: usize) {
    if len == 0 {
        return;
    }
    for part in parts {
        if part.start >= at {
            part.start += len;
            part.end += len;
        } else if part.end > at {
            part.end += len;
        }
    }
}

/// Re-anchor tracked folded parts to the CURRENT draft text after an arbitrary
/// edit (character insert/delete, line-binding, drain). The live composer draft
/// is still a bare `String` (the `TrackedDraft` owner is a follow-up), so a
/// tracked range can go stale between an edit and the next paste.
///
/// Each part's visible `placeholder` is located in the draft from the running
/// search cursor forward (so identical placeholders keep document order); a
/// part whose placeholder no longer appears is dropped, exactly as upstream
/// stops tracking a deleted placeholder (`prompt/index.tsx:1440-1443`,
/// `1522-1535`). Ranges whose bytes are not a character boundary are dropped
/// rather than sliced.
pub fn reconcile_parts(draft: &str, parts: &mut Vec<PastedText>) {
    let mut search_from = 0usize;
    parts.retain_mut(|part| {
        // Trust the recorded range only while it is a valid slice that still
        // holds this part's placeholder. A stale range may legitimately point
        // past the (shrunk) draft, so bounds/boundary failures do NOT drop the
        // part by themselves — they only force a relocation search below.
        if part.start <= part.end
            && part.end <= draft.len()
            && draft.is_char_boundary(part.start)
            && draft.is_char_boundary(part.end)
            && draft[part.start..part.end] == part.placeholder
        {
            search_from = part.end;
            return true;
        }
        let from = search_from.min(draft.len());
        match draft[from..].find(&part.placeholder) {
            Some(relative) => {
                let start = from + relative;
                part.start = start;
                part.end = start + part.placeholder.len();
                search_from = part.end;
                true
            }
            None => false,
        }
    });
}

/// Apply a [`PastePlan`] to a plain draft.
///
/// `limit` bounds the resulting draft length in bytes. On `Err` the draft,
/// cursor and parts are unchanged.
pub fn apply_paste_plan(
    draft: &mut String,
    cursor: &mut usize,
    parts: &mut Vec<PastedText>,
    plan: PastePlan,
    limit: usize,
) -> Result<(), PasteError> {
    if *cursor > draft.len() {
        return Err(PasteError::NotBoundary);
    }
    if !draft.is_char_boundary(*cursor) {
        return Err(PasteError::NotBoundary);
    }
    match plan {
        PastePlan::Insert(text) => {
            let bytes = draft.len() + text.len();
            if bytes > limit {
                return Err(PasteError::TooLarge { bytes, limit });
            }
            shift_for_insert(parts, *cursor, text.len());
            draft.insert_str(*cursor, &text);
            *cursor += text.len();
        }
        PastePlan::Fold(part) => {
            // The upstream placeholder is followed by one separating space
            // while the tracked range covers only the placeholder text.
            let inserted = part.placeholder.len() + 1;
            let bytes = draft.len() + inserted;
            if bytes > limit {
                return Err(PasteError::TooLarge { bytes, limit });
            }
            shift_for_insert(parts, *cursor, inserted);
            let mut insert = part.placeholder.clone();
            insert.push(' ');
            draft.insert_str(*cursor, &insert);
            *cursor += inserted;
            parts.push(part);
            parts.sort_by_key(|part| part.start);
        }
        PastePlan::Expand(index) => {
            if !expand_pasted(draft, cursor, parts, index) {
                return Err(PasteError::NotBoundary);
            }
        }
    }
    Ok(())
}

/// Replace a folded placeholder with its full text (`index.tsx:1441-1455`).
///
/// The cursor lands at the end of the restored text, as `setSelection` +
/// `insertText` does upstream. Later parts and mentions that the caller tracks
/// separately are shifted by [`expansion_delta`]. Returns `false` when the
/// index or range is invalid.
pub fn expand_pasted(
    draft: &mut String,
    cursor: &mut usize,
    parts: &mut Vec<PastedText>,
    index: usize,
) -> bool {
    let Some(part) = parts.get(index).cloned() else {
        return false;
    };
    if part.start > part.end
        || part.end > draft.len()
        || !draft.is_char_boundary(part.start)
        || !draft.is_char_boundary(part.end)
    {
        return false;
    }
    let replaced = part.len();
    let delta = part.text.len() as isize - replaced as isize;
    draft.replace_range(part.start..part.end, &part.text);
    *cursor = part.start + part.text.len();
    parts.remove(index);
    for other in parts.iter_mut() {
        if other.start >= part.end {
            other.start = (other.start as isize + delta) as usize;
            other.end = (other.end as isize + delta) as usize;
        } else if other.end > part.end {
            other.end = (other.end as isize + delta) as usize;
        }
    }
    true
}

/// Build the body to SEND for one submit dispatch: every tracked folded
/// placeholder is replaced by its full pasted text while the displayed draft
/// and its tracked ranges stay untouched.
///
/// Upstream computes `inputText` once from the store text plus the pasted
/// extmarks and hands it to `session.prompt` (`prompt/index.tsx:1129` ->
/// `:1368`); a folded placeholder is only ever a display affordance
/// (`prompt/part.ts:11-27`). Ranges expand right-to-left so byte offsets
/// taken from the unexpanded text remain valid.
#[must_use]
pub fn expand_for_submit(draft: &str, parts: &[PastedText]) -> String {
    if parts.is_empty() {
        return draft.to_owned();
    }
    let mut result = draft.to_owned();
    let mut ranges: Vec<&PastedText> = parts.iter().collect();
    ranges.sort_by_key(|part| std::cmp::Reverse(part.start));
    for part in ranges {
        if part.start > part.end
            || part.end > draft.len()
            || !draft.is_char_boundary(part.start)
            || !draft.is_char_boundary(part.end)
        {
            continue;
        }
        result.replace_range(part.start..part.end, &part.text);
    }
    result
}

/// Byte count added by expanding `part` in place, for shifting mention ranges.
#[must_use]
pub fn expansion_delta(part: &PastedText, full_text: &str) -> isize {
    full_text.len() as isize - part.len() as isize
}

/// Saved composer state for one tab (`index.tsx:717-733`).
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct DraftEntry {
    pub text: String,
    pub cursor: usize,
    pub mode: PromptMode,
    pub parts: Vec<PastedText>,
}

impl DraftEntry {
    /// Empty drafts are never stashed upstream (`onCleanup` test).
    #[must_use]
    pub fn is_stashable(&self) -> bool {
        !self.text.is_empty()
    }
}

/// One draft per tab across prompt remounts (`draft-stash.ts`).
///
/// `None` keys the home prompt, matching upstream's
/// `Map<string | undefined, DraftEntry>`.
#[derive(Clone, Debug, Default)]
pub struct DraftStash {
    by_tab: BTreeMap<Option<String>, DraftEntry>,
}

impl DraftStash {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Store (or replace) the draft for `tab`.
    pub fn save(&mut self, tab: Option<&str>, entry: DraftEntry) {
        self.by_tab.insert(tab.map(str::to_owned), entry);
    }

    /// Consume the draft for `tab`; a stale copy can never shadow newer input.
    pub fn take(&mut self, tab: Option<&str>) -> Option<DraftEntry> {
        self.by_tab.remove(&tab.map(str::to_owned))
    }

    /// Observe a draft without consuming it (tests/diagnostics only).
    #[must_use]
    pub fn peek(&self, tab: Option<&str>) -> Option<&DraftEntry> {
        self.by_tab.get(&tab.map(str::to_owned))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_tab.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_tab.is_empty()
    }

    /// Restore a saved draft into a prompt that has just mounted
    /// (`index.tsx:721-731`): nothing happens when the live draft already has
    /// text, and the cursor is clamped to the stored text.
    pub fn restore(
        &mut self,
        tab: Option<&str>,
        text: &mut String,
        cursor: &mut usize,
        mode: &mut PromptMode,
        parts: &mut Vec<PastedText>,
    ) -> bool {
        if !text.is_empty() {
            return false;
        }
        let Some(entry) = self.take(tab) else {
            return false;
        };
        if entry.text.is_empty() {
            return false;
        }
        *text = entry.text;
        *cursor = entry.cursor.min(text.len());
        while *cursor > 0 && !text.is_char_boundary(*cursor) {
            *cursor -= 1;
        }
        *mode = entry.mode;
        *parts = entry.parts;
        true
    }
}

/// An explicit stashed prompt (`prompt/stash.tsx:9-12`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StashEntry {
    pub prompt: DraftEntry,
    pub timestamp: u64,
}

/// Bound the entry list the way upstream rewrites the stash file.
fn trim_entries<T>(entries: &mut Vec<T>, cap: usize) -> bool {
    if entries.len() <= cap {
        return false;
    }
    let excess = entries.len() - cap;
    entries.drain(..excess);
    true
}

/// Explicit prompt stash: last in, first out, capped at
/// [`MAX_STASH_ENTRIES`] (`prompt/stash.tsx:36-91`).
#[derive(Clone, Debug, Default)]
pub struct PromptStash {
    entries: Vec<StashEntry>,
}

impl PromptStash {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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

    /// Push with a caller-supplied timestamp (`Date.now()` upstream).
    pub fn push(&mut self, prompt: DraftEntry, timestamp: u64) {
        self.entries.push(StashEntry { prompt, timestamp });
        trim_entries(&mut self.entries, MAX_STASH_ENTRIES);
    }

    /// Remove and return the newest entry.
    pub fn pop(&mut self) -> Option<StashEntry> {
        self.entries.pop()
    }

    /// Remove one entry by index; out-of-range indexes are ignored.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        self.entries.remove(index);
        true
    }

    /// Replace the whole list from persisted entries, keeping the newest
    /// [`MAX_STASH_ENTRIES`] (`prompt/stash.tsx:16-34`).
    pub fn load(&mut self, entries: Vec<StashEntry>) {
        self.entries = entries;
        trim_entries(&mut self.entries, MAX_STASH_ENTRIES);
    }
}

/// One prompt-history entry; `mode` is part of the entry so history restores
/// mode and parts, not just text (`index.tsx:1048-1050`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryEntry {
    pub prompt: DraftEntry,
}

/// Prompt history with upstream move/append semantics
/// (`prompt/history.tsx:103-141`).
#[derive(Clone, Debug, Default)]
pub struct PromptHistory {
    entries: Vec<HistoryEntry>,
    /// Mirrors upstream's numeric `index`: `0` is the empty prompt, `-1` is
    /// the newest entry, `-len` the oldest (`history.tsx:106-116`).
    index: isize,
}

impl PromptHistory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    #[must_use]
    pub fn index(&self) -> isize {
        self.index
    }

    /// `Array.prototype.at` semantics: negative indexes count from the end.
    fn at(&self, index: isize) -> Option<&HistoryEntry> {
        let len = self.entries.len() as isize;
        let resolved = if index < 0 { len + index } else { index };
        if resolved < 0 || resolved >= len {
            return None;
        }
        self.entries.get(resolved as usize)
    }

    /// Append a submitted entry, skipping an exact duplicate of the newest
    /// entry and keeping the newest [`MAX_HISTORY_ENTRIES`].
    pub fn append(&mut self, entry: HistoryEntry) {
        if self.entries.last() == Some(&entry) {
            self.index = 0;
            return;
        }
        self.entries.push(entry);
        trim_entries(&mut self.entries, MAX_HISTORY_ENTRIES);
        self.index = 0;
    }

    /// Move `direction` (-1 back, 1 forward) relative to the current index.
    ///
    /// Editing the draft (`current_text`) invalidates walking unless the
    /// current entry still matches, exactly as upstream bails out when the
    /// text differs and is non-empty. Index 0 means "empty prompt"; walking
    /// forward to 0 yields an empty prompt, and moving past either end is a
    /// no-op (`history.tsx:106-116`).
    pub fn move_through(
        &mut self,
        direction: i8,
        current_text: &str,
    ) -> Option<HistoryEntry> {
        if self.entries.is_empty() {
            return None;
        }
        let current = self.at(self.index)?;
        if current.prompt.text != current_text && !current_text.is_empty() {
            return None;
        }
        let next = self.index + direction as isize;
        if next.unsigned_abs() > self.entries.len() || next > 0 {
            return None;
        }
        self.index = next;
        if next == 0 {
            return Some(HistoryEntry {
                prompt: DraftEntry::default(),
            });
        }
        self.at(next).cloned()
    }
}

/// Whether a cleared draft is worth retaining in history
/// (`index.tsx:1526-1538`).
#[must_use]
pub fn should_retain_cleared_draft(
    text: &str,
    pasted: usize,
    files: usize,
    agents: usize,
) -> bool {
    js_trim(text).chars().count() >= DRAFT_RETENTION_MIN_CHARS
        || pasted > 0
        || files > 0
        || agents > 0
}

/// A tracked mention range in the draft, in UTF-8 byte offsets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mention {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// An attachment part (`files` upstream), optionally with a tracked mention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilePart {
    pub uri: String,
    pub name: Option<String>,
    pub mention: Option<Mention>,
}

/// A labelled part (`agents`/`skills` upstream).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LabelPart {
    pub id: String,
    pub mention: Option<Mention>,
}

/// Native counterpart of `PromptInfo` for part bookkeeping.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrackedPrompt {
    pub text: String,
    pub files: Vec<FilePart>,
    pub agents: Vec<LabelPart>,
    pub skills: Vec<LabelPart>,
    pub pasted: Vec<PastedText>,
}

fn overlaps(left: (usize, usize), right: (usize, usize)) -> bool {
    left.0 < right.1 && left.1 > right.0
}

/// Find every non-overlapping occurrence of `text` as `(start, end)` bytes.
fn occurrences(content: &str, text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if text.is_empty() {
        return out;
    }
    let mut from = 0usize;
    while let Some(found) = content[from..].find(text) {
        let start = from + found;
        let end = start + text.len();
        out.push((start, end));
        from = end;
        if from >= content.len() {
            break;
        }
    }
    out
}

/// Assign ordered items to ordered candidates: maximize matched items, then
/// minimize total distance to the previous offset
/// (`prompt/mention.ts:109-135`).
fn assign_mentions(
    items: &[(usize, usize)], // (previous start, order index)
    candidates: &[(usize, usize)],
) -> Vec<(usize, usize)> {
    let m = items.len();
    let n = candidates.len();
    if m == 0 || n == 0 {
        return Vec::new();
    }
    // best[i][j] = (matches, cost) assigning items[i..] to candidates[j..].
    let mut best = vec![vec![(0usize, 0i64); n + 1]; m + 1];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            let matched = {
                let (matches, cost) = best[i + 1][j + 1];
                (
                    matches + 1,
                    cost + (candidates[j].0 as i64 - items[i].0 as i64).abs(),
                )
            };
            let skip_item = best[i + 1][j];
            let skip_candidate = best[i][j + 1];
            let mut winner = matched;
            for candidate in [skip_item, skip_candidate] {
                if candidate.0 > winner.0 || (candidate.0 == winner.0 && candidate.1 < winner.1) {
                    winner = candidate;
                }
            }
            best[i][j] = winner;
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < m && j < n {
        let matched = {
            let (matches, cost) = best[i + 1][j + 1];
            (
                matches + 1,
                cost + (candidates[j].0 as i64 - items[i].0 as i64).abs(),
            )
        };
        let chosen = {
            let mut winner = matched;
            for candidate in [best[i + 1][j], best[i][j + 1]] {
                if candidate.0 > winner.0 || (candidate.0 == winner.0 && candidate.1 < winner.1) {
                    winner = candidate;
                }
            }
            winner
        };
        if chosen == matched {
            pairs.push((items[i].1, j));
            i += 1;
            j += 1;
        } else if best[i + 1][j] == chosen {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

impl TrackedPrompt {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    fn mentions(&self) -> Vec<(Mention, usize)> {
        let mut out = Vec::new();
        for (index, part) in self.files.iter().enumerate() {
            if let Some(mention) = &part.mention {
                out.push((mention.clone(), index));
            }
        }
        let base = self.files.len();
        for (index, part) in self.agents.iter().enumerate() {
            if let Some(mention) = &part.mention {
                out.push((mention.clone(), base + index));
            }
        }
        let base = base + self.agents.len();
        for (index, part) in self.skills.iter().enumerate() {
            if let Some(mention) = &part.mention {
                out.push((mention.clone(), base + index));
            }
        }
        out
    }

    fn set_mention(&mut self, slot: usize, mention: Option<Mention>) {
        if slot < self.files.len() {
            self.files[slot].mention = mention;
            return;
        }
        let slot = slot - self.files.len();
        if slot < self.agents.len() {
            self.agents[slot].mention = mention;
            return;
        }
        let slot = slot - self.agents.len();
        if let Some(part) = self.skills.get_mut(slot) {
            part.mention = mention;
        }
    }

    /// Re-derive mention ranges from the current text
    /// (`prompt/mention.ts:20-48`).
    ///
    /// Identical mention texts are grouped; each group is assigned in order to
    /// non-overlapping occurrences, preferring the previous offset. Mentions
    /// whose text no longer occurs are dropped; mentions without text are kept
    /// verbatim. This covers reordered duplicates, duplicate mentions and
    /// prefix collisions such as `@src/a` inside `@src/a/b`.
    pub fn realign(&mut self) {
        let mut protected: Vec<(usize, usize)> = Vec::new();
        let mentions = self.mentions();
        let slots: Vec<usize> = mentions.iter().map(|(_, slot)| *slot).collect();
        let mut aligned: Vec<Option<Mention>> = mentions
            .iter()
            .map(|(mention, _)| {
                if mention.text.is_empty() {
                    Some(mention.clone())
                } else {
                    None
                }
            })
            .collect();
        let mut groups: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
        for (slot, (mention, _)) in mentions.iter().enumerate() {
            if mention.text.is_empty() {
                continue;
            }
            groups
                .entry(mention.text.clone())
                .or_default()
                .push((mention.start, slot));
        }
        let mut ordered_groups: Vec<(String, Vec<(usize, usize)>)> = groups.into_iter().collect();
        // Upstream sorts the `[text, items]` entries by the *text* length
        // descending, then lexicographically, so longer mentions claim their
        // occurrences before a prefix mention can steal them.
        ordered_groups.sort_by(|left, right| {
            utf16_len(&right.0)
                .cmp(&utf16_len(&left.0))
                .then_with(|| left.0.cmp(&right.0))
        });
        for (text, mut items) in ordered_groups {
            let candidates = occurrences(&self.text, &text);
            let available: Vec<(usize, usize)> = candidates
                .iter()
                .copied()
                .filter(|candidate| !protected.iter().any(|range| overlaps(*candidate, *range)))
                .collect();
            items.sort_by_key(|(start, slot)| (*start, *slot));
            for (slot_position, candidate_index) in assign_mentions(&items, &available) {
                let (start, end) = available[candidate_index];
                aligned[slot_position] = Some(Mention {
                    text: text.clone(),
                    start,
                    end,
                });
            }
            protected.extend(candidates);
        }
        for (position, mention) in aligned.into_iter().enumerate() {
            if let Some(slot) = slots.get(position).copied() {
                self.set_mention(slot, mention);
            }
        }
        // Parts whose mention disappeared are dropped for agents/skills; files
        // without a mention are preserved (`index.tsx:868-878`).
        self.agents.retain(|part| part.mention.is_some());
        self.skills.retain(|part| part.mention.is_some());
    }

    /// Replace every folded placeholder with its full text and shift later
    /// mentions (`prompt/mention.ts:92-108`, `prompt/part.ts:11-27`).
    ///
    /// Expansion happens right-to-left so earlier ranges stay valid.
    pub fn expand_pasted(&mut self) -> bool {
        if self.pasted.is_empty() {
            return false;
        }
        let mut order: Vec<usize> = (0..self.pasted.len()).collect();
        order.sort_by_key(|index| std::cmp::Reverse(self.pasted[*index].start));
        for index in order {
            let Some(part) = self.pasted.get(index).cloned() else {
                continue;
            };
            if part.end > self.text.len()
                || !self.text.is_char_boundary(part.start)
                || !self.text.is_char_boundary(part.end)
            {
                continue;
            }
            let delta = expansion_delta(&part, &part.text);
            self.text.replace_range(part.start..part.end, &part.text);
            self.shift_mentions_after(part.end, delta);
            self.pasted.remove(index);
        }
        self.pasted.clear();
        true
    }

    fn shift_mentions_after(&mut self, after: usize, delta: isize) {
        if delta == 0 {
            return;
        }
        let mut shift = |mention: &mut Option<Mention>| {
            if let Some(mention) = mention {
                if mention.start >= after {
                    mention.start = (mention.start as isize + delta).max(0) as usize;
                    mention.end = (mention.end as isize + delta).max(0) as usize;
                }
            }
        };
        for part in &mut self.files {
            shift(&mut part.mention);
        }
        for part in &mut self.agents {
            shift(&mut part.mention);
        }
        for part in &mut self.skills {
            shift(&mut part.mention);
        }
    }

    /// Insert text at a byte offset, shifting tracked ranges the way textarea
    /// extmarks follow edits. Returns `false` on a non-boundary offset.
    pub fn insert_text(&mut self, at: usize, text: &str) -> bool {
        if at > self.text.len() || !self.text.is_char_boundary(at) {
            return false;
        }
        self.text.insert_str(at, text);
        for part in &mut self.pasted {
            if part.start >= at {
                part.start += text.len();
                part.end += text.len();
            } else if part.end > at {
                part.end += text.len();
            }
        }
        self.shift_mentions_at_insert(at, text.len());
        true
    }

    fn shift_mentions_at_insert(&mut self, at: usize, len: usize) {
        let mut shift = |mention: &mut Option<Mention>| {
            if let Some(mention) = mention {
                if mention.start >= at {
                    mention.start += len;
                    mention.end += len;
                } else if mention.end > at {
                    mention.end += len;
                }
            }
        };
        for part in &mut self.files {
            shift(&mut part.mention);
        }
        for part in &mut self.agents {
            shift(&mut part.mention);
        }
        for part in &mut self.skills {
            shift(&mut part.mention);
        }
    }

    /// Delete `[start, end)` from the draft.
    ///
    /// Mentioned parts that the deletion removed follow
    /// `syncExtmarksWithPromptParts` (`index.tsx:809-878`): file attachments
    /// survive as mentionless entries, while agents, skills and pasted parts
    /// disappear. Ranges after the edit shift left; ranges overlapping the edit
    /// are removed.
    pub fn delete_range(&mut self, start: usize, end: usize) -> bool {
        if start > end
            || end > self.text.len()
            || !self.text.is_char_boundary(start)
            || !self.text.is_char_boundary(end)
        {
            return false;
        }
        let len = end - start;
        self.text.replace_range(start..end, "");
        let edited = (start, end);
        let mut adjust = |mention: &mut Option<Mention>| {
            let Some(current) = mention.as_mut() else { return };
            if overlaps((current.start, current.end), edited) {
                *mention = None;
            } else if current.start >= end {
                current.start -= len;
                current.end -= len;
            }
        };
        for part in &mut self.files {
            adjust(&mut part.mention);
        }
        for part in &mut self.agents {
            adjust(&mut part.mention);
        }
        for part in &mut self.skills {
            adjust(&mut part.mention);
        }
        self.pasted.retain(|part| {
            let range = (part.start, part.end);
            if overlaps(range, edited) {
                return false;
            }
            true
        });
        for part in &mut self.pasted {
            if part.start >= end {
                part.start -= len;
                part.end -= len;
            }
        }
        self.agents.retain(|part| part.mention.is_some());
        self.skills.retain(|part| part.mention.is_some());
        true
    }
}

/// Keep attachments that never had a mention, and re-associate mentioned ones
/// in order with the mentions still present (`prompt/attachment.ts:67-78`).
#[must_use]
pub fn preserve_mentionless_attachments(
    files: &[FilePart],
    mentioned: Vec<FilePart>,
) -> Vec<FilePart> {
    let mut tracked = mentioned.into_iter();
    let mut out = Vec::new();
    for file in files {
        if file.mention.is_none() {
            out.push(file.clone());
            continue;
        }
        match tracked.next() {
            Some(next) => out.push(next),
            // Upstream `flatMap` skips the leftover mentioned file and keeps
            // processing the remaining input (`index.tsx:868-878`).
            None => continue,
        }
    }
    out
}

/// The live composer draft: the visible text, the caret offset in bytes, and
/// the tracked input parts (mentions, attachments, folded pastes) that must
/// survive edits (CP-35, `index.tsx:809-878`).
///
/// `text` is what the user sees and is the single source of truth; the embedded
/// [`TrackedPrompt`] carries the part anchors. Call [`Self::realign_after_edit`]
/// after any text mutation so the anchors follow the edit (re-anchored, reordered
/// or dropped) exactly like upstream `syncExtmarksWithPromptParts`.
///
/// This is the owning state the production edit pipeline must migrate to: the
/// live draft is currently a bare `String` + `usize` cursor in `tui_entry.rs`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrackedDraft {
    pub text: String,
    pub cursor: usize,
    pub parts: TrackedPrompt,
}

impl TrackedDraft {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.len();
        let parts = TrackedPrompt::new(text.clone());
        Self { text, cursor, parts }
    }

    /// Replace the visible text (the caret is clamped to a char boundary)
    /// without touching the tracked parts. Call [`Self::realign_after_edit`]
    /// afterwards.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.clamp_cursor();
    }

    /// Insert `text` at byte offset `at`, moving the caret to the end of the
    /// insertion. All tracked ranges shift the way textarea extmarks do.
    pub fn insert_text(&mut self, at: usize, text: &str) -> bool {
        self.sync_parts_text();
        if !self.parts.insert_text(at, text) {
            return false;
        }
        self.text = self.parts.text.clone();
        self.cursor = at + text.len();
        true
    }

    /// Delete `[start, end)` from the draft. Mentioned parts that the deletion
    /// removed follow [`TrackedPrompt::delete_range`]: files survive mentionless,
    /// agents/skills/pastes disappear.
    pub fn delete_range(&mut self, start: usize, end: usize) -> bool {
        self.sync_parts_text();
        if !self.parts.delete_range(start, end) {
            return false;
        }
        self.text = self.parts.text.clone();
        self.cursor = start;
        true
    }

    /// Re-derive every tracked part from the current `text`
    /// (`prompt/mention.ts:20-48`, `index.tsx:809-878`). Mentions whose text no
    /// longer occurs are dropped; reordered duplicates are reassigned to the
    /// nearest occurrence. This is the seam the production edit pipeline calls
    /// after it mutates the draft.
    pub fn realign_after_edit(&mut self) {
        self.clamp_cursor();
        self.sync_parts_text();
        self.parts.realign();
        // CP-35: folded pastes are re-anchored against the (possibly edited)
        // text so a later expansion/submit uses the current placeholder range.
        // This is the seam the live composer previously reached through the
        // free `reconcile_parts` helper.
        let mut pasted = std::mem::take(&mut self.parts.pasted);
        reconcile_parts(&self.text, &mut pasted);
        self.parts.pasted = pasted;
    }

    fn sync_parts_text(&mut self) {
        if self.parts.text != self.text {
            self.parts.text = self.text.clone();
        }
    }

    fn clamp_cursor(&mut self) {
        if self.cursor > self.text.len() {
            self.cursor = self.text.len();
        }
        while self.cursor > 0 && !self.text.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
    }
}

/// Snapshot of the paste-relevant state before an asynchronous paste starts
/// (`index.tsx:376-392`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasteTicket {
    pub session: Option<String>,
    pub mode: PromptMode,
    pub text: String,
}

impl PasteTicket {
    #[must_use]
    pub fn capture(session: Option<&str>, mode: PromptMode, text: &str) -> Self {
        Self {
            session: session.map(str::to_owned),
            mode,
            text: text.to_owned(),
        }
    }
}

/// Reasons a resolved paste is dropped instead of inserted
/// (`index.tsx:382-389`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PasteStale {
    Disposed,
    Disabled,
    SessionChanged,
    ModeChanged,
    TextChanged,
}

/// Whether a paste may still write into the composer.
///
/// The checks mirror upstream's `changed()` predicate exactly: disposed,
/// destroyed, disabled, session switched, mode switched, or the draft text
/// changed while the paste was resolving.
#[must_use]
pub fn paste_admission(
    before: &PasteTicket,
    after: &PasteTicket,
    disposed: bool,
    destroyed: bool,
    disabled: bool,
) -> Result<(), PasteStale> {
    if disposed {
        return Err(PasteStale::Disposed);
    }
    if destroyed {
        return Err(PasteStale::Disposed);
    }
    if disabled {
        return Err(PasteStale::Disabled);
    }
    if before.session != after.session {
        return Err(PasteStale::SessionChanged);
    }
    if before.mode != after.mode {
        return Err(PasteStale::ModeChanged);
    }
    if before.text != after.text {
        return Err(PasteStale::TextChanged);
    }
    Ok(())
}

/// Serializes pastes: only the oldest admitted paste may resolve, and each
/// resolution releases the next one (`index.tsx:376-392` `pasteQueue`).
#[derive(Clone, Debug, Default)]
pub struct PasteQueue {
    next_id: u64,
    pending: Vec<u64>,
}

impl PasteQueue {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Admit a paste; returns its ticket id.
    pub fn enqueue(&mut self, before: PasteTicket) -> (u64, PasteRequest) {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.push(id);
        (id, PasteRequest { before })
    }

    /// True when `id` is the oldest outstanding paste.
    #[must_use]
    pub fn is_head(&self, id: u64) -> bool {
        self.pending.first() == Some(&id)
    }

    /// Resolve `id`, dropping it from the queue even when stale.
    pub fn resolve(&mut self, id: u64) -> bool {
        if !self.is_head(id) {
            return false;
        }
        self.pending.remove(0);
        true
    }

    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

/// A queued paste request: the pre-paste snapshot plus its position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasteRequest {
    pub before: PasteTicket,
}

/// Image preview geometry for the prompt surface
/// (`index.tsx:397-402,1680-1698`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentPreview {
    /// Per-image height in cells.
    pub height: u16,
    /// Per-image width in cells.
    pub width: u16,
    /// Painted images (max [`MAX_VISIBLE_PREVIEWS`]).
    pub visible: usize,
    /// True when more images exist than are painted; the strip is clickable.
    pub overflow: bool,
    /// Row count reserved by the preview strip, including its bottom padding.
    pub container_height: u16,
}

/// One preview cell: a decoded image or a failed decode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewItem {
    pub uri: String,
    pub failed: bool,
}

/// Preview geometry: `height = max(4, min(8, floor(terminal_height / 4)))`,
/// `width = height * 2`, at most three images, plus one padding row.
#[must_use]
pub fn image_preview(terminal_height: u16, image_count: usize) -> AttachmentPreview {
    let height = (terminal_height / 4).clamp(4, 8);
    AttachmentPreview {
        height,
        width: height * 2,
        visible: image_count.min(MAX_VISIBLE_PREVIEWS),
        overflow: image_count > MAX_VISIBLE_PREVIEWS,
        container_height: if image_count == 0 { 0 } else { height + 1 },
    }
}

/// Images painted by the strip; failed decodes stay visible as error cells
/// (`index.tsx:1690-1712`).
#[must_use]
pub fn visible_previews(items: &[PreviewItem]) -> &[PreviewItem] {
    let end = items.len().min(MAX_VISIBLE_PREVIEWS);
    &items[..end]
}

/// Index opened by clicking the strip or its overflow indicator.
#[must_use]
pub fn preview_click_index(items: &[PreviewItem], overflow: bool) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    if overflow {
        return Some(MAX_VISIBLE_PREVIEWS);
    }
    Some(0)
}

/// Strip quotes and decode a `file://` URL from a pasted path
/// (`local-attachment.ts:81-105`).
#[must_use]
pub fn normalize_pasted_filepath(value: &str, windows: bool) -> String {
    let raw = value.trim_matches(|c| c == '\'' || c == '"');
    if let Some(path) = decode_file_url(raw, windows) {
        return path;
    }
    if windows {
        return raw.to_owned();
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn decode_file_url(value: &str, windows: bool) -> Option<String> {
    let rest = value.strip_prefix("file://")?;
    let (authority, pathname) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => ("", ""),
    };
    if pathname.is_empty() {
        return None;
    }
    let pathname = percent_decode(pathname)?;
    if pathname.contains("/%2f") || pathname.contains("/%5c") {
        return None;
    }
    if !windows {
        if !authority.is_empty() && authority != "localhost" {
            return None;
        }
        return Some(pathname);
    }
    let local = pathname
        .strip_prefix('/')
        .filter(|path| path.chars().nth(1) == Some(':'))
        .map_or_else(|| pathname.clone(), |path| path.to_owned())
        .replace('/', "\\");
    if authority.is_empty() || authority == "localhost" {
        Some(local)
    } else {
        Some(format!("\\\\{authority}{local}"))
    }
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = value.get(index + 1..index + 3)?;
            let byte = u8::from_str_radix(hex, 16).ok()?;
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Split a pasted path list, honoring single/double quotes and backslash
/// escapes (`local-attachment.ts:107-158`). More than
/// [`MAX_PASTED_FILEPATHS`] entries yield no candidates.
#[must_use]
pub fn parse_pasted_filepaths(value: &str, windows: bool) -> Vec<String> {
    let filtered;
    let input = if value.contains("file://") {
        filtered = value
            .split('\n')
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        filtered.as_str()
    } else {
        value
    };
    let mut result: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = input.chars();
    while let Some(character) = chars.next() {
        if let Some(active) = quote {
            if character == active {
                quote = None;
                continue;
            }
            if character == '\\' && !windows && active == '"' {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
                continue;
            }
            current.push(character);
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '\\' if !windows => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    result.push(decode_file_url(&current, windows).unwrap_or_else(|| current.clone()));
                    current.clear();
                    if result.len() > MAX_PASTED_FILEPATHS {
                        return Vec::new();
                    }
                }
            }
            other => current.push(other),
        }
    }
    if quote.is_some() {
        return Vec::new();
    }
    if !current.is_empty() {
        result.push(decode_file_url(&current, windows).unwrap_or(current));
    }
    if result.len() > MAX_PASTED_FILEPATHS {
        return Vec::new();
    }
    result
}

/// Attachment kind for a resolved mime type; unknown types are not attached
/// (`local-attachment.ts:160-176`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentKind {
    SvgText,
    Binary,
}

/// Classify a decoded local attachment, or `None` when it is unsupported,
/// empty or over budget.
#[must_use]
pub fn classify_attachment(mime: &str, bytes: usize) -> Option<AttachmentKind> {
    if !mime.starts_with("image/") && mime != "application/pdf" {
        return None;
    }
    if bytes == 0 || bytes > MAX_LOCAL_ATTACHMENT_BYTES {
        return None;
    }
    if mime == "image/svg+xml" {
        Some(AttachmentKind::SvgText)
    } else {
        Some(AttachmentKind::Binary)
    }
}

/// Placeholder label inserted for an inline image/PDF attachment
/// (`index.tsx:1470-1477`, `prompt/attachment.ts:80-96`).
#[must_use]
pub fn attachment_label(files: &[FilePart], uri: &str, name: Option<&str>) -> Option<String> {
    let kind = if uri.starts_with("data:image/") {
        "Image"
    } else if uri.starts_with("data:application/pdf;") {
        "PDF"
    } else {
        return None;
    };
    if kind == "Image" {
        if let Some(existing) = files.iter().find(|file| {
            file.uri == uri && file.mention.as_ref().is_some_and(|mention| !mention.text.is_empty())
        }) {
            if let Some(mention) = &existing.mention {
                return Some(mention.text.clone());
            }
        }
    }
    let mut highest = 0usize;
    for file in files {
        let Some(mention) = &file.mention else { continue };
        let prefix = format!("[{kind} ");
        let Some(rest) = mention.text.strip_prefix(&prefix) else {
            continue;
        };
        let Some(number) = rest.strip_suffix(']') else {
            continue;
        };
        if let Ok(value) = number.parse::<usize>() {
            highest = highest.max(value);
        }
    }
    let _ = name;
    Some(format!("[{kind} {}]", highest + 1))
}

/// Label for an SVG pasted as text (`index.tsx:1470-1474`).
#[must_use]
pub fn svg_placeholder_label(filename: Option<&str>) -> String {
    format!("[SVG: {}]", filename.unwrap_or("image"))
}

#[cfg(test)]
mod paste_normalization_tests {
    //! CP-36: normalization and folding boundaries
    //! (`component/prompt/index.tsx:1429-1477`).

    use super::*;

    #[test]
    fn cp36_strips_ansi_and_normalizes_crlf_and_cr() {
        assert_eq!(
            normalize_paste("\x1b[31mred\x1b[0m\r\nnext\rlast"),
            "red\nnext\nlast"
        );
        assert_eq!(
            normalize_paste("\x1b]0;title\x07text\x1bPpayload\x1b\\tail"),
            "texttail"
        );
        assert_eq!(normalize_paste("a\x1b"), "a", "dangling ESC is dropped");
        assert_eq!(normalize_paste("kept\ttab"), "kept\ttab");
    }

    #[test]
    fn cp36_two_lines_never_fold_and_three_lines_always_fold() {
        let two = plan_paste("one\ntwo", PasteMode::Fold, 0, &[]);
        assert_eq!(two, PastePlan::Insert("one\ntwo".to_owned()));
        let three = plan_paste("one\ntwo\nthree", PasteMode::Fold, 0, &[]);
        match three {
            PastePlan::Fold(part) => {
                assert_eq!(part.placeholder, "[Pasted ~3 lines]");
                assert_eq!(part.text, "one\ntwo\nthree");
            }
            other => panic!("expected fold, got {other:?}"),
        }
    }

    #[test]
    fn cp36_hundred_fifty_units_do_not_fold_but_one_hundred_fifty_one_do() {
        let exactly = "x".repeat(FOLD_MAX_UNITS);
        assert_eq!(
            plan_paste(&exactly, PasteMode::Fold, 0, &[]),
            PastePlan::Insert(exactly.clone())
        );
        let over = "x".repeat(FOLD_MAX_UNITS + 1);
        assert!(matches!(
            plan_paste(&over, PasteMode::Fold, 0, &[]),
            PastePlan::Fold(_)
        ));
    }

    #[test]
    fn cp36_fold_threshold_counts_utf16_units_not_bytes_or_chars() {
        // 80 astral emoji = 80 chars, 320 UTF-8 bytes, 160 UTF-16 units.
        let emoji = "😀".repeat(80);
        assert_eq!(utf16_len(&emoji), 160);
        assert!(matches!(
            plan_paste(&emoji, PasteMode::Fold, 0, &[]),
            PastePlan::Fold(_)
        ));
        // 76 emoji = 152 UTF-16 units: still folds; 75 = 150 does not.
        assert!(matches!(
            plan_paste(&"😀".repeat(76), PasteMode::Fold, 0, &[]),
            PastePlan::Fold(_)
        ));
        assert!(matches!(
            plan_paste(&"😀".repeat(75), PasteMode::Fold, 0, &[]),
            PastePlan::Insert(_)
        ));
    }

    #[test]
    fn cp36_full_paste_setting_never_folds() {
        let body = "1\n2\n3\n4";
        assert_eq!(
            plan_paste(body, PasteMode::Full, 0, &[]),
            PastePlan::Insert(body.to_owned())
        );
        assert_eq!(PasteMode::from_config(Some("full")), PasteMode::Full);
        // CP-36: upstream compares the raw value against the exact literal
        // `"full"` (`prompt/index.tsx:1463`); no trim, no case folding.
        assert_eq!(PasteMode::from_config(Some("FULL")), PasteMode::Fold);
        assert_eq!(PasteMode::from_config(Some(" full ")), PasteMode::Fold);
        assert_eq!(PasteMode::from_config(Some("Full")), PasteMode::Fold);
        assert_eq!(PasteMode::from_config(None), PasteMode::Fold);
        assert_eq!(PasteMode::from_config(Some("summary")), PasteMode::Fold);
    }

    #[test]
    fn cp36_folding_uses_trimmed_content_but_inserts_untrimmed_text() {
        let padded = "\n  one\ntwo\nthree  \n";
        match plan_paste(padded, PasteMode::Fold, 0, &[]) {
            PastePlan::Fold(part) => {
                assert_eq!(part.text, "one\ntwo\nthree");
                assert_eq!(part.end - part.start, "[Pasted ~3 lines]".len());
            }
            other => panic!("expected fold, got {other:?}"),
        }
        // A short paste inserts the normalized text *untrimmed*.
        assert_eq!(
            plan_paste("  hi  ", PasteMode::Fold, 0, &[]),
            PastePlan::Insert("  hi  ".to_owned())
        );
        assert!(js_trim("\u{feff}  x  \u{feff}") == "x", "JS trim drops BOM");
    }

    #[test]
    fn cp36_repeating_a_folded_paste_expands_it_in_place() {
        let mut draft = String::new();
        let mut cursor = 0usize;
        let mut parts = Vec::new();
        let body = "alpha\nbeta\ngamma";
        let plan = plan_paste(body, PasteMode::Fold, cursor, &parts);
        apply_paste_plan(&mut draft, &mut cursor, &mut parts, plan, 4096).unwrap();
        assert_eq!(draft, "[Pasted ~3 lines] ");
        assert_eq!(cursor, draft.len());
        // Same content pasted immediately after the placeholder expands it.
        let repeat = plan_paste(body, PasteMode::Fold, cursor, &parts);
        assert_eq!(repeat, PastePlan::Expand(0));
        apply_paste_plan(&mut draft, &mut cursor, &mut parts, repeat, 4096).unwrap();
        // `expandPastedText` selects only the placeholder extmark, so the
        // separating space `pasteText` appended stays in the draft.
        assert_eq!(draft, format!("{body} "));
        assert_eq!(cursor, body.len());
        assert!(parts.is_empty());
        // A third identical paste now folds again (nothing to expand).
        let third = plan_paste(body, PasteMode::Fold, cursor, &parts);
        assert!(matches!(third, PastePlan::Fold(_)));
    }

    #[test]
    fn cp36_expanding_shifts_later_folded_parts() {
        let mut draft = String::new();
        let mut cursor = 0usize;
        let mut parts = Vec::new();
        apply_paste_plan(
            &mut draft,
            &mut cursor,
            &mut parts,
            plan_paste("a\nb\nc", PasteMode::Fold, 0, &[]),
            4096,
        )
        .unwrap();
        let second_start = draft.len();
        let second = plan_paste("x\ny\nz\nw", PasteMode::Fold, cursor, &parts);
        apply_paste_plan(&mut draft, &mut cursor, &mut parts, second, 4096).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1].start, second_start);
        assert!(expand_pasted(&mut draft, &mut cursor, &mut parts, 0));
        assert_eq!(parts.len(), 1);
        assert_eq!(&draft[parts[0].start..parts[0].end], "[Pasted ~4 lines]");
    }

    #[test]
    fn cp36_paste_respects_draft_budget_and_boundaries() {
        let mut draft = "abc".to_owned();
        let mut cursor = draft.len();
        let mut parts = Vec::new();
        let error = apply_paste_plan(
            &mut draft,
            &mut cursor,
            &mut parts,
            PastePlan::Insert("xy".to_owned()),
            4,
        )
        .unwrap_err();
        assert_eq!(error, PasteError::TooLarge { bytes: 5, limit: 4 });
        assert_eq!(draft, "abc");
        assert_eq!(cursor, 3);
        let mut broken = "aé".to_owned();
        let mut offset = 2; // inside 'é'
        assert_eq!(
            apply_paste_plan(
                &mut broken,
                &mut offset,
                &mut parts,
                PastePlan::Insert("x".to_owned()),
                64
            ),
            Err(PasteError::NotBoundary)
        );
    }

    #[test]
    fn cp36_expand_restores_multiline_text_with_unicode_boundaries() {
        let mut draft = String::from("prefix [Pasted ~3 lines] suffix 世界");
        let start = "prefix ".len();
        let marker = "[Pasted ~3 lines]";
        let mut parts = vec![PastedText {
            text: "一\n二\n三 🦀".to_owned(),
            placeholder: marker.to_owned(),
            start,
            end: start + marker.len(),
        }];
        let mut cursor = draft.len();
        assert!(expand_pasted(&mut draft, &mut cursor, &mut parts, 0));
        assert_eq!(draft, "prefix 一\n二\n三 🦀 suffix 世界");
        assert_eq!(cursor, "prefix 一\n二\n三 🦀".len());
        assert!(draft.is_char_boundary(cursor));
    }

    #[test]
    fn cp36_js_trim_matches_ecmascript_not_unicode_whitespace() {
        // ECMAScript `trim` does not strip U+0085 NEL: it is a control, not
        // WhiteSpace (`index.tsx:1431`). Unicode White_Space does include it,
        // so `char::is_whitespace` alone over-trims.
        assert_eq!(js_trim("\u{85}x\u{85}"), "\u{85}x\u{85}");
        // The Zs separators, BOM and line terminators ARE stripped.
        assert_eq!(js_trim("\u{3000}x\u{3000}"), "x");
        assert_eq!(js_trim("\u{feff}x\u{feff}"), "x");
        assert_eq!(js_trim("\u{2028}x\u{2029}"), "x");
        // Folding boundary: 150 "x" plus a trailing NEL is 151 UTF-16 units for
        // ECMAScript (NEL survives trim), so the paste folds. Over-trimming NEL
        // would drop it to 150 and wrongly insert literally.
        let nel_tail = format!("{}\u{85}", "x".repeat(FOLD_MAX_UNITS));
        assert!(matches!(
            plan_paste(&nel_tail, PasteMode::Fold, 0, &[]),
            PastePlan::Fold(_)
        ));
        // A NEL-only wrapper is not blank for the clear-retention check either.
        assert!(!js_trim("\u{85}").is_empty());
    }

    #[test]
    fn cp08_submit_expands_folded_placeholder_without_touching_display() {
        // A draft holding one folded part submits the EXPANDED body while the
        // displayed draft and its tracked range stay unchanged
        // (`prompt/index.tsx:1129`, `prompt/part.ts:11-27`).
        let body = "l1\nl2\nl3";
        let mut draft = String::from("see ");
        let mut cursor = draft.len();
        let mut parts = Vec::new();
        let at = draft.len();
        let plan = plan_paste(body, PasteMode::Fold, at, &[]);
        apply_paste_plan(&mut draft, &mut cursor, &mut parts, plan, 4096).unwrap();
        let folded = draft.clone();
        let sent = expand_for_submit(&draft, &parts);
        assert_eq!(sent, format!("see {body} "));
        assert_eq!(draft, folded, "displayed draft must not change");
        assert_eq!(parts.len(), 1, "tracked ranges must not change");
    }
}

#[cfg(test)]
mod draft_stash_tests {
    //! CP-34: per-tab draft ownership and explicit stash
    //! (`draft-stash.ts`, `prompt/stash.tsx`).

    use super::*;

    fn entry(text: &str, cursor: usize) -> DraftEntry {
        DraftEntry {
            text: text.to_owned(),
            cursor,
            ..DraftEntry::default()
        }
    }

    #[test]
    fn cp34_each_tab_keeps_its_own_text_cursor_and_parts() {
        let mut stash = DraftStash::new();
        stash.save(Some("tab-a"), exp("a", 1));
        stash.save(Some("tab-b"), exp("b", 1));
        stash.save(None, exp("home", 4));
        assert_eq!(stash.len(), 3);
        let a = stash.take(Some("tab-a")).unwrap();
        assert_eq!(a.text, "a");
        assert_eq!(a.cursor, 1);
        // Taking consumes: the same tab has no stale copy afterwards.
        assert!(stash.take(Some("tab-a")).is_none());
        // Home is a distinct key from any session id.
        assert!(stash.take(None).is_some());
        assert_eq!(stash.take(Some("tab-b")).unwrap().text, "b");
        assert!(stash.is_empty());
    }

    fn exp(text: &str, cursor: usize) -> DraftEntry {
        DraftEntry {
            text: text.to_owned(),
            cursor,
            mode: PromptMode::Normal,
            parts: Vec::new(),
        }
    }

    #[test]
    fn cp34_restore_only_into_an_empty_composer_and_clamp_cursor() {
        let mut stash = DraftStash::new();
        let mut saved = entry("draft 世界", 6);
        saved.mode = PromptMode::Shell;
        saved.parts = vec![PastedText {
            text: "a\nb\nc".to_owned(),
            placeholder: "[Pasted ~3 lines]".to_owned(),
            start: 0,
            end: 17,
        }];
        stash.save(Some("s1"), saved);
        let mut text = "typed".to_owned();
        let mut cursor = 5;
        let mut mode = PromptMode::Normal;
        let mut parts = Vec::new();
        assert!(!stash.restore(Some("s1"), &mut text, &mut cursor, &mut mode, &mut parts));
        assert_eq!(text, "typed");
        assert_eq!(stash.len(), 1, "restore keeps the draft when it cannot use it");

        let mut empty = String::new();
        let mut cursor = 0;
        assert!(stash.restore(Some("s1"), &mut empty, &mut cursor, &mut mode, &mut parts));
        assert_eq!(empty, "draft 世界");
        assert_eq!(cursor, 6);
        assert_eq!(mode, PromptMode::Shell);
        assert_eq!(parts.len(), 1);

        // A cursor beyond the text clamps to the end.
        let mut stash = DraftStash::new();
        stash.save(Some("s2"), entry("ab", 99));
        let mut text = String::new();
        let mut cursor = 0;
        let mut mode = PromptMode::Normal;
        let mut parts = Vec::new();
        assert!(stash.restore(Some("s2"), &mut text, &mut cursor, &mut mode, &mut parts));
        assert_eq!(cursor, 2);
    }

    #[test]
    fn cp34_empty_drafts_are_not_stashable() {
        assert!(!DraftEntry::default().is_stashable());
        assert!(entry("x", 1).is_stashable());
    }

    #[test]
    fn cp34_explicit_stash_pops_newest_and_caps_at_fifty() {
        let mut stash = PromptStash::new();
        assert!(stash.pop().is_none());
        stash.push(entry("first", 5), 1);
        stash.push(entry("second", 6), 2);
        assert_eq!(stash.list().len(), 2);
        let popped = stash.pop().unwrap();
        assert_eq!(popped.prompt.text, "second");
        assert_eq!(popped.timestamp, 2);
        assert_eq!(stash.list().len(), 1);
        for index in 0..(MAX_STASH_ENTRIES + 5) {
            stash.push(entry(&format!("e{index}"), 0), index as u64);
        }
        assert_eq!(stash.len(), MAX_STASH_ENTRIES);
        assert_eq!(
            stash.list().first().unwrap().prompt.text,
            format!("e{}", 5),
            "oldest entries are trimmed"
        );
        let mut removable = PromptStash::new();
        removable.push(entry("a", 1), 1);
        removable.push(entry("b", 1), 2);
        assert!(removable.remove(0));
        assert_eq!(removable.list()[0].prompt.text, "b");
        assert!(!removable.remove(9));
    }

    #[test]
    fn cp34_cleared_draft_history_retention_uses_twenty_char_threshold() {
        assert!(!should_retain_cleared_draft("short", 0, 0, 0));
        assert!(should_retain_cleared_draft(
            &"x".repeat(DRAFT_RETENTION_MIN_CHARS),
            0,
            0,
            0
        ));
        assert!(
            should_retain_cleared_draft("   nineteen chars xx   ", 0, 0, 0) == false,
            "trimmed length decides"
        );
        assert!(should_retain_cleared_draft("hi", 1, 0, 0), "pasted part");
        assert!(should_retain_cleared_draft("hi", 0, 1, 0), "attachment");
        assert!(should_retain_cleared_draft("hi", 0, 0, 1), "agent part");
    }

    #[test]
    fn cp34_history_walks_back_to_empty_and_restores_mode_cursor_parts() {
        let mut history = PromptHistory::new();
        let mut first = entry("older", 5);
        first.mode = PromptMode::Shell;
        history.append(HistoryEntry { prompt: first });
        let mut second = entry("newer", 5);
        second.parts = vec![PastedText {
            text: "1\n2\n3".to_owned(),
            placeholder: "[Pasted ~3 lines]".to_owned(),
            start: 0,
            end: 17,
        }];
        history.append(HistoryEntry { prompt: second });
        assert_eq!(history.index(), 0);
        let previous = history.move_through(-1, "").unwrap();
        assert_eq!(previous.prompt.text, "newer");
        assert_eq!(previous.prompt.parts.len(), 1);
        assert_eq!(history.index(), -1);
        let older = history.move_through(-1, "newer").unwrap();
        assert_eq!(older.prompt.text, "older");
        assert_eq!(older.prompt.mode, PromptMode::Shell);
        assert_eq!(history.index(), -2);
        // Walking past the oldest entry is a no-op.
        assert!(history.move_through(-1, "older").is_none());
        assert_eq!(history.index(), -2);
        // Moving forward reaches index 0, which upstream renders as an empty
        // prompt; moving forward again is refused.
        let newest_again = history.move_through(1, "older").unwrap();
        assert_eq!(newest_again.prompt.text, "newer");
        let empty = history.move_through(1, "newer").unwrap();
        assert_eq!(empty.prompt, DraftEntry::default());
        assert_eq!(history.index(), 0);
        assert!(history.move_through(1, "").is_none());
        // Editing the draft stops history walking.
        let mut edited = PromptHistory::new();
        edited.append(HistoryEntry {
            prompt: entry("only", 4),
        });
        assert!(edited.move_through(-1, "different").is_none());
    }

    #[test]
    fn cp34_history_dedupes_last_entry_and_caps_at_fifty() {
        let mut history = PromptHistory::new();
        history.append(HistoryEntry {
            prompt: entry("same", 4),
        });
        history.append(HistoryEntry {
            prompt: entry("same", 4),
        });
        assert_eq!(history.entries().len(), 1);
        for index in 0..(MAX_HISTORY_ENTRIES + 3) {
            history.append(HistoryEntry {
                prompt: entry(&format!("e{index}"), 0),
            });
        }
        assert_eq!(history.entries().len(), MAX_HISTORY_ENTRIES);
        assert_eq!(
            history.entries().first().unwrap().prompt.text,
            "e3".to_owned()
        );
    }
}

#[cfg(test)]
mod tracked_parts_tests {
    //! CP-35: tracked input parts through edits (`prompt/mention.ts`,
    //! `index.tsx:809-878,1134-1148`, `prompt/attachment.ts:67-78`).

    use super::*;

    fn mention(text: &str, start: usize) -> Mention {
        Mention {
            text: text.to_owned(),
            start,
            end: start + text.len(),
        }
    }

    fn file(uri: &str, mention: Option<Mention>) -> FilePart {
        FilePart {
            uri: uri.to_owned(),
            name: None,
            mention,
        }
    }

    #[test]
    fn cp35_reordered_and_duplicate_mentions_are_reassigned_in_order() {
        let text = "explain @src/a then @src/a again";
        let mut prompt = TrackedPrompt::new(text);
        prompt.files = vec![
            // Intentionally reversed previous offsets.
            file("file:///second", Some(mention("@src/a", 24))),
            file("file:///first", Some(mention("@src/a", 8))),
        ];
        prompt.realign();
        // `"explain @src/a then @src/a again"` has occurrences at byte 8 and
        // 20. Items are assigned to the nearest occurrence in order, so the
        // part that previously sat at 8 keeps it and the other moves to 20.
        assert_eq!(prompt.files[0].mention.as_ref().unwrap().start, 20);
        assert_eq!(prompt.files[1].mention.as_ref().unwrap().start, 8);
    }

    #[test]
    fn cp35_prefix_mentions_do_not_steal_longer_occurrences() {
        // "@src/a" is a prefix of "@src/a/b"; the longer mention is grouped
        // first so the prefix mention cannot claim its occurrence.
        let text = "see @src/a/b and @src/a";
        let mut prompt = TrackedPrompt::new(text);
        prompt.files = vec![
            file("file:///long", Some(mention("@src/a/b", 4))),
            file("file:///short", Some(mention("@src/a", 17))),
        ];
        prompt.realign();
        assert_eq!(prompt.files[0].mention.as_ref().unwrap().text, "@src/a/b");
        assert_eq!(prompt.files[0].mention.as_ref().unwrap().start, 4);
        // `"see @src/a/b and @src/a"`: the standalone mention starts at 17.
        assert_eq!(prompt.files[1].mention.as_ref().unwrap().start, 17);
        assert_eq!(
            &prompt.text[prompt.files[1].mention.as_ref().unwrap().start
                ..prompt.files[1].mention.as_ref().unwrap().end],
            "@src/a"
        );
    }

    #[test]
    fn cp35_deleted_mentions_drop_agents_and_skills_but_keep_mentionless_files() {
        let mut prompt = TrackedPrompt::new("keep @gone end");
        prompt.files = vec![file("file:///kept", Some(mention("@gone", 5)))];
        prompt.agents = vec![LabelPart {
            id: "agent".to_owned(),
            mention: Some(mention("@gone", 5)),
        }];
        prompt.skills = vec![LabelPart {
            id: "skill".to_owned(),
            mention: Some(mention("@gone", 5)),
        }];
        assert!(prompt.delete_range(5, 10));
        assert_eq!(prompt.text, "keep  end");
        assert!(prompt.agents.is_empty());
        assert!(prompt.skills.is_empty());
        assert_eq!(prompt.files.len(), 1, "file attachment survives");
        assert!(prompt.files[0].mention.is_none());
    }

    #[test]
    fn cp35_deletion_shifts_later_mentions_left() {
        let mut prompt = TrackedPrompt::new("aaa @keep");
        prompt.files = vec![file("file:///keep", Some(mention("@keep", 4)))];
        assert!(prompt.delete_range(0, 4));
        assert_eq!(prompt.text, "@keep");
        assert_eq!(prompt.files[0].mention.as_ref().unwrap().start, 0);
    }

    #[test]
    fn cp35_expanding_a_paste_shifts_mentions_after_it() {
        let mut prompt = TrackedPrompt::new("[Pasted ~3 lines] @tail");
        prompt.pasted = vec![PastedText {
            text: "1\n2\n3".to_owned(),
            placeholder: "[Pasted ~3 lines]".to_owned(),
            start: 0,
            end: 17,
        }];
        prompt.files = vec![file(
            "file:///tail",
            Some(Mention {
                text: "@tail".to_owned(),
                start: 18,
                end: 23,
            }),
        )];
        assert!(prompt.expand_pasted());
        assert_eq!(prompt.text, "1\n2\n3 @tail");
        let shifted = prompt.files[0].mention.as_ref().unwrap();
        assert_eq!(&prompt.text[shifted.start..shifted.end], "@tail");
        assert!(prompt.pasted.is_empty());
    }

    #[test]
    fn cp35_insertion_inside_a_mention_extends_it_and_after_it_shifts() {
        let mut prompt = TrackedPrompt::new("@src/a");
        prompt.files = vec![file("file:///a", Some(mention("@src/a", 0)))];
        // Inserting strictly inside the range extends it.
        assert!(prompt.insert_text(3, "x"));
        assert_eq!(prompt.text, "@srxc/a");
        assert_eq!(prompt.files[0].mention.as_ref().unwrap().start, 0);
        assert_eq!(prompt.files[0].mention.as_ref().unwrap().end, 7);
        // Inserting exactly at the end does not extend the extmark.
        assert!(prompt.insert_text(7, " tail"));
        assert_eq!(prompt.text, "@srxc/a tail");
        assert_eq!(prompt.files[0].mention.as_ref().unwrap().start, 0);
        assert_eq!(prompt.files[0].mention.as_ref().unwrap().end, 7);
    }

    #[test]
    fn cp35_mentionless_attachments_are_preserved_in_place() {
        let original = vec![
            file("file:///plain", None),
            file("file:///mentioned", Some(mention("@a", 0))),
            file("file:///plain2", None),
        ];
        let mentioned = vec![file("file:///mentioned", Some(mention("@a", 4)))];
        let kept = preserve_mentionless_attachments(&original, mentioned);
        assert_eq!(kept.len(), 3);
        assert_eq!(kept[0].uri, "file:///plain");
        assert_eq!(kept[1].mention.as_ref().unwrap().start, 4);
        assert_eq!(kept[2].uri, "file:///plain2");
        // A mentioned file whose extmark is gone is dropped.
        let kept = preserve_mentionless_attachments(&original, Vec::new());
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().all(|file| file.uri != "file:///mentioned"));
    }

    #[test]
    fn cp35_realign_drops_mentions_whose_text_disappeared() {
        let mut prompt = TrackedPrompt::new("no mentions left");
        prompt.files = vec![file("file:///a", Some(mention("@a", 0)))];
        prompt.realign();
        assert!(prompt.files[0].mention.is_none());
        prompt.agents = vec![LabelPart {
            id: "agent".to_owned(),
            mention: Some(mention("@a", 0)),
        }];
        prompt.realign();
        assert!(prompt.agents.is_empty());
    }

    #[test]
    fn cp35_tracked_draft_realigns_reordered_and_deleted_mentions() {
        // "@src/a" (file) and "@agent/x" (agent) are swapped by the user.
        let mut draft = TrackedDraft::new("@src/a and @agent/x");
        draft.parts.files = vec![file("file:///a", Some(mention("@src/a", 0)))];
        draft.parts.agents = vec![LabelPart {
            id: "a".to_owned(),
            mention: Some(mention("@agent/x", 11)),
        }];
        draft.set_text("@agent/x and @src/a");
        draft.realign_after_edit();
        // "@agent/x and " is 13 bytes, so "@src/a" now starts at 13 and the
        // agent mention moved back to 0.
        assert_eq!(draft.parts.files[0].mention.as_ref().unwrap().start, 13);
        assert_eq!(draft.parts.agents[0].mention.as_ref().unwrap().start, 0);
        // The user deletes "@src/a": its file survives mentionless and the
        // caret clamps to the new length ("index.tsx:868-878").
        draft.set_text("@agent/x and ");
        draft.realign_after_edit();
        assert_eq!(draft.cursor, 13);
        assert!(draft.parts.files[0].mention.is_none());
        assert_eq!(draft.parts.agents[0].mention.as_ref().unwrap().start, 0);
    }
}

#[cfg(test)]
mod paste_async_tests {
    //! CP-37: serialized asynchronous paste (`index.tsx:376-392,1141-1150`).

    use super::*;

    fn ticket(session: Option<&str>, mode: PromptMode, text: &str) -> PasteTicket {
        PasteTicket::capture(session, mode, text)
    }

    #[test]
    fn cp37_pastes_resolve_in_submission_order() {
        let mut queue = PasteQueue::new();
        let (first, _) = queue.enqueue(ticket(Some("s"), PromptMode::Normal, ""));
        let (second, _) = queue.enqueue(ticket(Some("s"), PromptMode::Normal, ""));
        assert!(queue.is_head(first));
        assert!(!queue.is_head(second));
        assert!(!queue.resolve(second), "later paste cannot overtake");
        assert!(queue.resolve(first));
        assert!(queue.is_head(second));
        assert!(queue.resolve(second));
        assert_eq!(queue.pending(), 0);
    }

    #[test]
    fn cp37_stale_pastes_are_dropped_for_every_change() {
        let before = ticket(Some("s1"), PromptMode::Normal, "draft");
        assert_eq!(
            paste_admission(
                &before,
                &ticket(Some("s1"), PromptMode::Normal, "draft"),
                true,
                false,
                false
            ),
            Err(PasteStale::Disposed)
        );
        assert_eq!(
            paste_admission(
                &before,
                &ticket(Some("s1"), PromptMode::Normal, "draft"),
                false,
                true,
                false
            ),
            Err(PasteStale::Disposed)
        );
        assert_eq!(
            paste_admission(
                &before,
                &ticket(Some("s1"), PromptMode::Normal, "draft"),
                false,
                false,
                true
            ),
            Err(PasteStale::Disabled)
        );
        assert_eq!(
            paste_admission(
                &before,
                &ticket(Some("s2"), PromptMode::Normal, "draft"),
                false,
                false,
                false
            ),
            Err(PasteStale::SessionChanged)
        );
        assert_eq!(
            paste_admission(
                &before,
                &ticket(Some("s1"), PromptMode::Shell, "draft"),
                false,
                false,
                false
            ),
            Err(PasteStale::ModeChanged)
        );
        assert_eq!(
            paste_admission(
                &before,
                &ticket(Some("s1"), PromptMode::Normal, "draft typed"),
                false,
                false,
                false
            ),
            Err(PasteStale::TextChanged)
        );
        assert_eq!(
            paste_admission(
                &before,
                &ticket(Some("s1"), PromptMode::Normal, "draft"),
                false,
                false,
                false
            ),
            Ok(())
        );
    }
}

#[cfg(test)]
mod attachment_tests {
    //! CP-38: image preview geometry and local attachment failure states
    //! (`index.tsx:397-402,1680-1698`, `local-attachment.ts`).

    use super::*;

    #[test]
    fn cp38_preview_geometry_tracks_terminal_height_and_caps_at_eight() {
        assert_eq!(image_preview(12, 1).height, 4);
        assert_eq!(image_preview(12, 1).width, 8);
        assert_eq!(image_preview(24, 1).height, 6);
        assert_eq!(image_preview(24, 1).width, 12);
        assert_eq!(image_preview(40, 1).height, 8);
        assert_eq!(image_preview(40, 1).width, 16);
        // Very short terminals still reserve the 4-row floor.
        assert_eq!(image_preview(5, 1).height, 4);
        assert_eq!(image_preview(12, 1).container_height, 5);
        assert_eq!(image_preview(12, 0).container_height, 0);
        assert_eq!(image_preview(12, 0).visible, 0);
    }

    #[test]
    fn cp38_zero_one_three_and_four_images_paint_at_most_three() {
        let items = |count: usize| {
            (0..count)
                .map(|index| PreviewItem {
                    uri: format!("data:image/png;base64,{index}"),
                    failed: false,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(visible_previews(&items(0)).len(), 0);
        assert_eq!(visible_previews(&items(1)).len(), 1);
        assert_eq!(visible_previews(&items(3)).len(), 3);
        assert_eq!(visible_previews(&items(4)).len(), 3);
        assert!(image_preview(24, 3).overflow == false);
        assert!(image_preview(24, 4).overflow);
        assert_eq!(preview_click_index(&items(4), true), Some(3));
        assert_eq!(preview_click_index(&items(2), false), Some(0));
        assert_eq!(preview_click_index(&items(0), false), None);
    }

    #[test]
    fn cp38_decode_failure_stays_visible_and_unsupported_mimes_are_skipped() {
        let items = vec![
            PreviewItem {
                uri: "data:image/png;base64,ok".to_owned(),
                failed: false,
            },
            PreviewItem {
                uri: "data:image/png;base64,broken".to_owned(),
                failed: true,
            },
        ];
        let painted = visible_previews(&items);
        assert_eq!(painted.len(), 2);
        assert!(painted[1].failed, "failed decode keeps its error cell");
        assert_eq!(classify_attachment("image/png", 10), Some(AttachmentKind::Binary));
        assert_eq!(
            classify_attachment("image/svg+xml", 10),
            Some(AttachmentKind::SvgText)
        );
        assert_eq!(
            classify_attachment("application/pdf", 10),
            Some(AttachmentKind::Binary)
        );
        assert_eq!(classify_attachment("text/plain", 10), None);
        assert_eq!(classify_attachment("image/png", 0), None);
        assert_eq!(
            classify_attachment("image/png", MAX_LOCAL_ATTACHMENT_BYTES + 1),
            None
        );
    }

    #[test]
    fn cp38_pasted_paths_are_parsed_with_quotes_escapes_and_file_urls() {
        assert_eq!(
            parse_pasted_filepaths("/tmp/a.png '/tmp/b c.png'", false),
            vec!["/tmp/a.png".to_owned(), "/tmp/b c.png".to_owned()]
        );
        assert_eq!(
            parse_pasted_filepaths("/tmp/with\\ space.png", false),
            vec!["/tmp/with space.png".to_owned()]
        );
        assert_eq!(
            parse_pasted_filepaths("file:///tmp/a%20b.png", false),
            vec!["/tmp/a b.png".to_owned()]
        );
        assert!(parse_pasted_filepaths("'/tmp/unterminated", false).is_empty());
        assert_eq!(
            normalize_pasted_filepath("\"/tmp/x.png\"", false),
            "/tmp/x.png"
        );
        assert_eq!(
            normalize_pasted_filepath("C:\\\\tmp\\\\x.png", true),
            "C:\\\\tmp\\\\x.png",
            "windows keeps backslashes"
        );
        let many = (0..=MAX_PASTED_FILEPATHS)
            .map(|index| format!("/tmp/{index}.png"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(parse_pasted_filepaths(&many, false).is_empty());
    }

    #[test]
    fn cp38_attachment_labels_number_images_and_name_svgs() {
        let files = vec![
            file("data:image/png;base64,1", Some(mention("[Image 1]", 0))),
            file("data:image/png;base64,2", Some(mention("[Image 2]", 0))),
        ];
        assert_eq!(
            attachment_label(&files, "data:image/png;base64,3", Some("c")),
            Some("[Image 3]".to_owned())
        );
        // Re-pasting the identical image reuses its label.
        assert_eq!(
            attachment_label(&files, "data:image/png;base64,1", Some("a")),
            Some("[Image 1]".to_owned())
        );
        assert_eq!(attachment_label(&[], "text/plain", None), None);
        assert_eq!(
            attachment_label(&files, "data:application/pdf;base64,9", None),
            Some("[PDF 1]".to_owned())
        );
        assert_eq!(svg_placeholder_label(Some("logo.svg")), "[SVG: logo.svg]");
        assert_eq!(svg_placeholder_label(None), "[SVG: image]");
    }

    fn file(uri: &str, mention: Option<Mention>) -> FilePart {
        FilePart {
            uri: uri.to_owned(),
            name: None,
            mention,
        }
    }

    fn mention(text: &str, start: usize) -> Mention {
        Mention {
            text: text.to_owned(),
            start,
            end: start + text.len(),
        }
    }
}

#[cfg(test)]
mod measurement_tests {
    //! CP-41: renderer-compatible cell measurement for paste/part math.

    use super::*;

    #[test]
    fn cp41_measure_cells_matches_wide_combining_zwj_tamil_and_tab_cells() {
        assert_eq!(measure_cells("abc"), 3);
        assert_eq!(measure_cells("日本語"), 6, "CJK is two cells each");
        assert_eq!(measure_cells("e\u{301}"), 1, "combining mark is zero width");
        assert_eq!(measure_cells("👨‍👩‍👧‍👦"), 2, "ZWJ family renders as one cluster");
        assert_eq!(measure_cells("\t"), 1, "control becomes a space cell");
        assert_eq!(measure_cells("\x1b[31mred\x1b[0m"), 3, "escapes are not cells");
        assert_eq!(utf16_len("日本語"), 3);
        assert_eq!(measure_cells("நி"), 1, "Tamil cluster measured once");
        assert_ne!(utf16_len("日本語"), measure_cells("日本語"));
    }
}

#[cfg(test)]
mod reconcile_parts_tests {
    //! F2 review fix: tracked folded ranges are re-anchored to the CURRENT
    //! draft after arbitrary edits, and a deleted placeholder stops being
    //! tracked (`prompt/index.tsx:1440-1443,1522-1535`).
    use super::*;

    fn part(text: &str, placeholder: &str, start: usize) -> PastedText {
        PastedText {
            text: text.to_owned(),
            placeholder: placeholder.to_owned(),
            start,
            end: start + placeholder.len(),
        }
    }

    #[test]
    fn reconcile_reanchors_shifted_and_drops_deleted_placeholders() {
        // Two parts; an insertion before both shifts them right.
        let ph_first = "[Pasted ~2 lines]";
        let ph_second = "[Pasted ~4 lines]";
        let mut draft = format!("ab{ph_first} cd{ph_second} ");
        let first_at = draft.find(ph_first).unwrap();
        let second_at = draft.find(ph_second).unwrap();
        let mut parts = vec![
            part("l1\nl2", ph_first, first_at),
            part("x1\nx2\nx3\nx4", ph_second, second_at),
        ];
        reconcile_parts(&draft, &mut parts);
        assert_eq!(parts[0].start, first_at);
        assert_eq!(parts[1].start, second_at);

        // Insert a character at the head: both ranges shift by one.
        draft.insert(0, 'X');
        reconcile_parts(&draft, &mut parts);
        assert_eq!(parts[0].start, first_at + 1);
        assert_eq!(parts[1].start, second_at + 1);
        assert_eq!(&draft[parts[0].start..parts[0].end], ph_first);
        assert_eq!(&draft[parts[1].start..parts[1].end], ph_second);

        // Delete the FIRST placeholder text: only the second part survives.
        let (start, end) = (parts[0].start, parts[0].end);
        draft.replace_range(start..end, "");
        reconcile_parts(&draft, &mut parts);
        assert_eq!(parts.len(), 1, "a deleted placeholder is no longer tracked");
        assert_eq!(&draft[parts[0].start..parts[0].end], ph_second);
    }

    #[test]
    fn reconcile_keeps_identical_placeholders_in_document_order() {
        let ph = "[Pasted ~3 lines]";
        let mut draft = format!("{ph} {ph} rest");
        let second_occurrence = ph.len() + 1;
        // The second part's recorded range is stale (points at the first
        // occurrence's prefix, not at a real placeholder); reconcile must
        // relocate it to the NEXT occurrence after the first part, never
        // collapsing the two onto the same range.
        let mut parts = vec![part("a\nb\nc", ph, 0), part("d\ne\nf", ph, 1)];
        reconcile_parts(&draft, &mut parts);
        assert_eq!(parts[0].start, 0);
        assert_eq!(parts[1].start, second_occurrence);
        assert_eq!(&draft[parts[1].start..parts[1].end], ph);
        draft.push('!');
        reconcile_parts(&draft, &mut parts);
        assert_eq!(parts[0].start, 0);
        assert_eq!(parts[1].start, second_occurrence);
    }
}
