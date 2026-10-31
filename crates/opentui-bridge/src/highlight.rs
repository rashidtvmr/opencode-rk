#![forbid(unsafe_code)]
//! Syntax highlighting model with generation-checked, non-flickering
//! completion (FFI-07).
//!
//! EVIDENCE / BOUNDARY
//! - `docs/ui-parity/opencode-ui-parity-kit/audit-opentui-boundary.md` §3
//!   "Highlighting spans native storage and a TypeScript worker" (ref [M3],
//!   `packages/core/src/renderables/Code.ts:47-170` at OpenTUI pin
//!   `8d6065a3e60039d16c6ee697692dcfbc46d3c786`) states the controller owns
//!   highlight dirtiness, content snapshot ids, pending promises, streaming
//!   state, concealment and conversion to styled chunks, and that updating
//!   content invalidates highlights and requests a frame *without first
//!   publishing raw unstyled text*. Refs [M4]/[M5] pin grammar/query assets
//!   and the worker lifecycle; [T5]
//!   (`packages/core/src/tests/renderer.native-scene-markdown.test.ts:16-100`)
//!   asserts that clearing/replacing a Markdown node while a highlight request
//!   is pending destroys the old node and cleans the registry.
//! - The vendored library exports text-buffer *storage* for spans
//!   (`textBufferAddHighlight`, `textBufferAddHighlightByCharRange`,
//!   `textBufferSetSyntaxStyle`, `createSyntaxStyle`, `syntaxStyleRegister`;
//!   see `nm -gU …/libopentui.dylib`), but nothing that owns a highlight
//!   *request*: no snapshot id, generation, pending result or completion
//!   callback is exported (no `highlightOnce`/worker symbol). The
//!   generation-checked completion state machine below is therefore a Rust
//!   model; writing the accepted spans into a native text buffer is a separate
//!   painter step (the checked ABI subset declared by this crate does not bind
//!   those text-buffer calls). It never claims grammar parity: grammars/queries
//!   are identified by [`GrammarKey`] and a result from a different grammar is
//!   refused, but the actual Tree-sitter parse is a separate component.
//! - OpenCode consumes the same controller through
//!   `CodeRenderable.onHighlight` and `getTreeSitterClient().highlightOnce`
//!   (`packages/tui/src/component/patch-diff.tsx:127-156`, pin `450e7b9`),
//!   which is why results must carry the exact content snapshot and grammar.

use crate::color::{indexed, RgbaU16, SLOT_ACCENT, SLOT_ERROR, SLOT_FG, SLOT_MUTED, SLOT_SUCCESS, SLOT_WARNING};
use crate::handle::{Arena, GenHandle, ResourceError};

/// Source byte cap for one highlighted text (fail closed).
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;
/// Spans per result (fail closed).
pub const MAX_SPANS: usize = 4096;
/// Link targets per result.
pub const MAX_LINKS: usize = 256;
/// Longest link target retained.
pub const MAX_LINK_TARGET_BYTES: usize = 1024;

/// Capture classes the theme maps to semantic colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CaptureClass {
    Keyword,
    String,
    Comment,
    Number,
    Function,
    Type,
    Variable,
    Operator,
    Punctuation,
    Property,
    Constant,
    Attribute,
    Tag,
    Error,
}

/// Number of [`CaptureClass`] variants (theme table length).
pub const CAPTURE_CLASSES: usize = 14;

impl CaptureClass {
    /// Every class, in theme-table order.
    pub const ALL: [CaptureClass; CAPTURE_CLASSES] = [
        Self::Keyword,
        Self::String,
        Self::Comment,
        Self::Number,
        Self::Function,
        Self::Type,
        Self::Variable,
        Self::Operator,
        Self::Punctuation,
        Self::Property,
        Self::Constant,
        Self::Attribute,
        Self::Tag,
        Self::Error,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Concealment of one span (secret input / concealed syntax).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Concealment {
    #[default]
    Visible,
    /// Range is removed from the visible text.
    Hidden,
    /// Range is replaced by exactly one marker character.
    Marker(char),
}

/// Semantic style resolved from the theme table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeStyle {
    pub fg: RgbaU16,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
}

impl Default for ThemeStyle {
    fn default() -> Self {
        Self {
            fg: indexed(SLOT_FG),
            bold: false,
            italic: false,
            underline: false,
            dim: false,
        }
    }
}

/// Capture-class to style table. One entry per class so a missing class is a
/// compile-time impossibility rather than a silent default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightTheme {
    styles: [ThemeStyle; CAPTURE_CLASSES],
}

impl Default for HighlightTheme {
    fn default() -> Self {
        let mut styles = [ThemeStyle::default(); CAPTURE_CLASSES];
        for (class, style) in CaptureClass::ALL.into_iter().zip(styles.iter_mut()) {
            *style = match class {
                CaptureClass::Keyword => ThemeStyle {
                    fg: indexed(SLOT_ACCENT),
                    bold: true,
                    ..ThemeStyle::default()
                },
                CaptureClass::String => ThemeStyle {
                    fg: indexed(SLOT_SUCCESS),
                    ..ThemeStyle::default()
                },
                CaptureClass::Comment => ThemeStyle {
                    fg: indexed(SLOT_MUTED),
                    italic: true,
                    dim: true,
                    ..ThemeStyle::default()
                },
                CaptureClass::Number => ThemeStyle {
                    fg: indexed(SLOT_WARNING),
                    ..ThemeStyle::default()
                },
                CaptureClass::Function => ThemeStyle {
                    bold: true,
                    ..ThemeStyle::default()
                },
                CaptureClass::Type => ThemeStyle {
                    fg: indexed(SLOT_WARNING),
                    ..ThemeStyle::default()
                },
                CaptureClass::Constant => ThemeStyle {
                    fg: indexed(SLOT_ACCENT),
                    ..ThemeStyle::default()
                },
                CaptureClass::Punctuation => ThemeStyle {
                    fg: indexed(SLOT_MUTED),
                    ..ThemeStyle::default()
                },
                CaptureClass::Attribute => ThemeStyle {
                    fg: indexed(SLOT_WARNING),
                    italic: true,
                    ..ThemeStyle::default()
                },
                CaptureClass::Tag => ThemeStyle {
                    fg: indexed(SLOT_ERROR),
                    ..ThemeStyle::default()
                },
                CaptureClass::Error => ThemeStyle {
                    fg: indexed(SLOT_ERROR),
                    underline: true,
                    ..ThemeStyle::default()
                },
                _ => ThemeStyle::default(),
            };
        }
        Self { styles }
    }
}

impl HighlightTheme {
    #[must_use]
    pub const fn style(&self, class: CaptureClass) -> ThemeStyle {
        self.styles[class.index()]
    }

    /// Replace one class without exposing the table layout.
    pub fn set_style(&mut self, class: CaptureClass, style: ThemeStyle) {
        self.styles[class.index()] = style;
    }
}

/// Which grammar/query produced a result, and at which version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GrammarKey {
    pub language: u16,
    pub query_version: u16,
}

/// A link target a span points at (terminal hyperlink / OSC 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightLink {
    id: u16,
    target: String,
}

impl HighlightLink {
    #[must_use]
    pub fn new(id: u16, target: &str) -> Self {
        Self {
            id,
            target: target.to_string(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> u16 {
        self.id
    }

    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }
}

/// One styled source range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightSpan {
    start: usize,
    end: usize,
    capture: CaptureClass,
    link: Option<u16>,
    conceal: Concealment,
}

impl HighlightSpan {
    #[must_use]
    pub const fn new(start: usize, end: usize, capture: CaptureClass) -> Self {
        Self {
            start,
            end,
            capture,
            link: None,
            conceal: Concealment::Visible,
        }
    }

    #[must_use]
    pub const fn with_link(mut self, id: u16) -> Self {
        self.link = Some(id);
        self
    }

    #[must_use]
    pub const fn with_conceal(mut self, conceal: Concealment) -> Self {
        self.conceal = conceal;
        self
    }

    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    #[must_use]
    pub const fn capture(&self) -> CaptureClass {
        self.capture
    }

    #[must_use]
    pub const fn link(&self) -> Option<u16> {
        self.link
    }

    #[must_use]
    pub const fn conceal(&self) -> Concealment {
        self.conceal
    }
}

/// Validation failures for a highlight result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightError {
    SourceTooLarge,
    TooManySpans,
    TooManyLinks,
    LinkTargetTooLarge,
    SpanOutOfBounds { start: usize, end: usize },
    SpanNotCharBoundary,
    EmptySpan,
    OverlappingConcealment,
    DuplicateLink { id: u16 },
    DanglingLink { span: usize, id: u16 },
    TooManyTargets,
    Destroyed,
    WrongThread,
    Full,
}

impl core::fmt::Display for HighlightError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SourceTooLarge => f.write_str("highlight source exceeds byte cap"),
            Self::TooManySpans => f.write_str("highlight result has too many spans"),
            Self::TooManyLinks => f.write_str("highlight result has too many links"),
            Self::LinkTargetTooLarge => f.write_str("highlight link target is too long"),
            Self::SpanOutOfBounds { start, end } => {
                write!(f, "highlight span {start}..{end} is out of bounds")
            }
            Self::SpanNotCharBoundary => f.write_str("highlight span is not on a char boundary"),
            Self::EmptySpan => f.write_str("highlight span is empty"),
            Self::OverlappingConcealment => f.write_str("concealment ranges overlap"),
            Self::DuplicateLink { id } => write!(f, "duplicate highlight link id {id}"),
            Self::DanglingLink { span, id } => {
                write!(f, "highlight span {span} references unknown link {id}")
            }
            Self::TooManyTargets => f.write_str("too many highlight targets"),
            Self::Destroyed => f.write_str("highlight target is destroyed"),
            Self::WrongThread => f.write_str("highlight target used off its owner thread"),
            Self::Full => f.write_str("highlight target arena is full"),
        }
    }
}

impl std::error::Error for HighlightError {}

impl From<ResourceError> for HighlightError {
    fn from(error: ResourceError) -> Self {
        match error {
            ResourceError::WrongThread => Self::WrongThread,
            ResourceError::Full => Self::Full,
            _ => Self::Destroyed,
        }
    }
}

/// Why a completed result was not applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleReason {
    /// The target handle no longer resolves (destroyed).
    NodeDestroyed,
    /// The node exists but its text view was replaced (generation bumped).
    NodeReplaced,
    /// Content changed after the request was issued.
    Superseded,
    /// No request with this snapshot is pending (already completed/cancelled).
    UnknownRequest,
    /// The result declares a different content snapshot than the request.
    SnapshotMismatch,
    /// The result came from a different grammar/query version.
    GrammarMismatch,
}

/// Result of [`HighlightTargets::complete`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    Applied { generation: u64 },
    Discarded(StaleReason),
}

/// A validated highlight result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedText {
    snapshot: u64,
    grammar: GrammarKey,
    source: String,
    spans: Vec<HighlightSpan>,
    links: Vec<HighlightLink>,
    concealments: Vec<(usize, usize)>,
}

impl HighlightedText {
    /// Validate and normalize (sort by start) a result.
    pub fn new(
        snapshot: u64,
        grammar: GrammarKey,
        source: &str,
        mut spans: Vec<HighlightSpan>,
        links: Vec<HighlightLink>,
    ) -> Result<Self, HighlightError> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(HighlightError::SourceTooLarge);
        }
        if spans.len() > MAX_SPANS {
            return Err(HighlightError::TooManySpans);
        }
        if links.len() > MAX_LINKS {
            return Err(HighlightError::TooManyLinks);
        }
        for link in &links {
            if link.target.is_empty() || link.target.len() > MAX_LINK_TARGET_BYTES {
                return Err(HighlightError::LinkTargetTooLarge);
            }
            if links.iter().filter(|other| other.id == link.id).count() > 1 {
                return Err(HighlightError::DuplicateLink { id: link.id });
            }
        }
        for (index, span) in spans.iter().enumerate() {
            if span.start >= span.end {
                return Err(if span.start == span.end {
                    HighlightError::EmptySpan
                } else {
                    HighlightError::SpanOutOfBounds {
                        start: span.start,
                        end: span.end,
                    }
                });
            }
            if span.end > source.len() {
                return Err(HighlightError::SpanOutOfBounds {
                    start: span.start,
                    end: span.end,
                });
            }
            if !source.is_char_boundary(span.start) || !source.is_char_boundary(span.end) {
                return Err(HighlightError::SpanNotCharBoundary);
            }
            if let Some(id) = span.link {
                if !links.iter().any(|link| link.id == id) {
                    return Err(HighlightError::DanglingLink { span: index, id });
                }
            }
        }
        spans.sort_by_key(|span| (span.start, span.end));

        // Concealment ranges must not overlap: the visible-text mapping has to
        // be a single deterministic rewrite of the source.
        let mut concealments: Vec<(usize, usize)> = spans
            .iter()
            .filter(|span| span.conceal != Concealment::Visible)
            .map(|span| (span.start, span.end))
            .collect();
        concealments.sort_unstable();
        if concealments
            .windows(2)
            .any(|pair| pair[0].1 > pair[1].0)
        {
            return Err(HighlightError::OverlappingConcealment);
        }

        Ok(Self {
            snapshot,
            grammar,
            source: source.to_string(),
            spans,
            links,
            concealments,
        })
    }

    #[must_use]
    pub const fn snapshot(&self) -> u64 {
        self.snapshot
    }

    #[must_use]
    pub const fn grammar(&self) -> GrammarKey {
        self.grammar
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn spans(&self) -> &[HighlightSpan] {
        &self.spans
    }

    #[must_use]
    pub fn links(&self) -> &[HighlightLink] {
        &self.links
    }

    /// Source text with concealment applied.
    #[must_use]
    pub fn visible_text(&self) -> String {
        let mut out = String::with_capacity(self.source.len());
        let mut cursor = 0;
        for span in &self.spans {
            if span.conceal == Concealment::Visible {
                continue;
            }
            out.push_str(&self.source[cursor..span.start]);
            match span.conceal {
                Concealment::Hidden => {}
                Concealment::Marker(marker) => out.push(marker),
                Concealment::Visible => unreachable!(),
            }
            cursor = span.end;
        }
        out.push_str(&self.source[cursor..]);
        out
    }

    /// Most specific span covering `byte`. Later starts win for nesting; on an
    /// identical start the narrower span (e.g. a keyword capture inside a
    /// longer string capture) wins.
    #[must_use]
    pub fn span_at(&self, byte: usize) -> Option<&HighlightSpan> {
        self.spans
            .iter()
            .filter(|span| span.start <= byte && byte < span.end)
            .max_by_key(|span| (span.start, core::cmp::Reverse(span.end)))
    }

    /// Resolved style for `byte`, or the default style when unstyled.
    #[must_use]
    pub fn style_at(&self, byte: usize, theme: &HighlightTheme) -> ThemeStyle {
        match self.span_at(byte) {
            Some(span) => theme.style(span.capture),
            None => ThemeStyle::default(),
        }
    }

    /// Map a source byte offset to its visible byte offset, or `None` when the
    /// offset lies inside hidden text.
    #[must_use]
    pub fn source_to_visible(&self, source_byte: usize) -> Option<usize> {
        if source_byte > self.source.len() {
            return None;
        }
        let mut visible = source_byte;
        for (start, end) in &self.concealments {
            if source_byte >= *end {
                visible -= end - start;
                // Subtract again for the replacement width below.
                visible += self.replacement_len(*start, *end);
            } else if source_byte >= *start {
                return None;
            }
        }
        Some(visible)
    }

    fn replacement_len(&self, start: usize, end: usize) -> usize {
        self.spans
            .iter()
            .find(|span| span.start == start && span.end == end)
            .map_or(0, |span| match span.conceal {
                Concealment::Marker(marker) => marker.len_utf8(),
                _ => 0,
            })
    }

    /// Visible byte length after concealment.
    #[must_use]
    pub fn visible_len(&self) -> usize {
        let mut len = self.source.len();
        for (start, end) in &self.concealments {
            len -= (end - start) - self.replacement_len(*start, *end);
        }
        len
    }

    /// Map a visible byte offset back to the source, or `None` when the
    /// offset is past the visible text. Offsets inside a concealed marker map
    /// to the start of the concealed source range.
    #[must_use]
    pub fn visible_to_source(&self, visible_byte: usize) -> Option<usize> {
        if visible_byte > self.visible_len() {
            return None;
        }
        // `delta` is the source-minus-visible shift accumulated so far.
        let mut delta = 0usize;
        for (start, end) in &self.concealments {
            let replacement = self.replacement_len(*start, *end);
            let visible_start = start - delta;
            let visible_end = visible_start + replacement;
            if visible_byte < visible_start {
                break;
            }
            if visible_byte < visible_end {
                return Some(*start);
            }
            if replacement == 0 && visible_byte == visible_start {
                // A hidden range has zero visible width, so its start boundary
                // is the byte immediately after it in the source.
                return Some(*end);
            }
            delta += (end - start) - replacement;
        }
        let source = visible_byte + delta;
        (source <= self.source.len()).then_some(source)
    }
}

#[derive(Debug)]
struct PendingRequest {
    generation: u64,
    snapshot: u64,
    grammar: GrammarKey,
}

#[derive(Debug)]
struct Target {
    generation: u64,
    snapshot: u64,
    source: String,
    /// Last applied result (may describe an older snapshot while pending).
    committed: Option<HighlightedText>,
    pending: Option<PendingRequest>,
    /// Whether raw source may be shown before a result lands. The pinned Code
    /// controller leaves this off while streaming.
    publish_raw_while_pending: bool,
}

impl Target {
    fn new() -> Self {
        Self {
            generation: 1,
            snapshot: 0,
            source: String::new(),
            committed: None,
            pending: None,
            publish_raw_while_pending: false,
        }
    }
}

/// What the host must run to fill a highlight request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightRequest {
    target: GenHandle,
    generation: u64,
    snapshot: u64,
    grammar: GrammarKey,
    content_bytes: usize,
}

impl HighlightRequest {
    #[must_use]
    pub const fn target(&self) -> GenHandle {
        self.target
    }

    /// Text-view generation captured at request time. A result for an older
    /// generation is discarded even if the handle still resolves.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn snapshot(&self) -> u64 {
        self.snapshot
    }

    #[must_use]
    pub const fn grammar(&self) -> GrammarKey {
        self.grammar
    }

    #[must_use]
    pub const fn content_bytes(&self) -> usize {
        self.content_bytes
    }
}

/// Owner-side registry of highlight targets. One instance per host thread.
#[derive(Debug, Default)]
pub struct HighlightTargets {
    arena: Arena<Target>,
}

impl HighlightTargets {
    #[must_use]
    pub fn new() -> Self {
        Self {
            arena: Arena::new(),
        }
    }

    /// Create a target (one per rendered Code view).
    pub fn create(&mut self) -> Result<(GenHandle, u64), HighlightError> {
        let handle = self.arena.insert(Target::new())?;
        Ok((handle, 1))
    }

    /// Destroy a target. Any request still in flight for it is discarded.
    pub fn destroy(&mut self, handle: GenHandle) -> Result<(), HighlightError> {
        self.arena.remove(handle)?;
        Ok(())
    }

    /// Replace the text view behind an existing node handle (the Code
    /// renderable recreating its native view). Bumps the generation so every
    /// request captured before the replacement is discarded.
    pub fn replace_view(&mut self, handle: GenHandle) -> Result<u64, HighlightError> {
        let target = self.arena.get_mut(handle)?;
        target.generation = target.generation.wrapping_add(1).max(1);
        target.pending = None;
        target.committed = None;
        Ok(target.generation)
    }

    /// Update source text. Returns the new snapshot id; a pending result for
    /// the previous snapshot is invalidated.
    pub fn set_source(&mut self, handle: GenHandle, source: &str) -> Result<u64, HighlightError> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(HighlightError::SourceTooLarge);
        }
        let target = self.arena.get_mut(handle)?;
        target.snapshot = target.snapshot.wrapping_add(1).max(1);
        target.source.clear();
        target.source.push_str(source);
        target.pending = None;
        Ok(target.snapshot)
    }

    /// Allow/disallow showing raw source before a result lands.
    pub fn set_publish_raw_while_pending(
        &mut self,
        handle: GenHandle,
        publish: bool,
    ) -> Result<(), HighlightError> {
        self.arena.get_mut(handle)?.publish_raw_while_pending = publish;
        Ok(())
    }

    /// The request the host must run for the current snapshot, or the already
    /// pending one when nothing changed (so repeated frames do not enqueue
    /// duplicate work).
    pub fn request(
        &mut self,
        handle: GenHandle,
        grammar: GrammarKey,
    ) -> Result<HighlightRequest, HighlightError> {
        let target = self.arena.get_mut(handle)?;
        if let Some(pending) = &target.pending {
            if pending.snapshot == target.snapshot && pending.grammar == grammar {
                return Ok(HighlightRequest {
                    target: handle,
                    generation: pending.generation,
                    snapshot: pending.snapshot,
                    grammar: pending.grammar,
                    content_bytes: target.source.len(),
                });
            }
        }
        let pending = PendingRequest {
            generation: target.generation,
            snapshot: target.snapshot,
            grammar,
        };
        let request = HighlightRequest {
            target: handle,
            generation: pending.generation,
            snapshot: pending.snapshot,
            grammar: pending.grammar,
            content_bytes: target.source.len(),
        };
        target.pending = Some(pending);
        Ok(request)
    }

    /// Apply a completed result. Every identity check happens here, on the
    /// completion edge: a late result for a destroyed target, a replaced
    /// view, or an outdated snapshot is discarded instead of written into the
    /// current node.
    pub fn complete(
        &mut self,
        request: HighlightRequest,
        result: HighlightedText,
    ) -> Completion {
        let target = match self.arena.get_mut(request.target) {
            Ok(target) => target,
            Err(_) => return Completion::Discarded(StaleReason::NodeDestroyed),
        };
        if target.generation != request.generation {
            return Completion::Discarded(StaleReason::NodeReplaced);
        }
        if target.snapshot != request.snapshot {
            return Completion::Discarded(StaleReason::Superseded);
        }
        match &target.pending {
            Some(pending)
                if pending.snapshot == request.snapshot
                    && pending.generation == request.generation
                    && pending.grammar == request.grammar => {}
            _ => return Completion::Discarded(StaleReason::UnknownRequest),
        }
        if result.snapshot != request.snapshot {
            return Completion::Discarded(StaleReason::SnapshotMismatch);
        }
        if result.grammar != request.grammar {
            return Completion::Discarded(StaleReason::GrammarMismatch);
        }
        let generation = target.generation;
        target.committed = Some(result);
        target.pending = None;
        Completion::Applied { generation }
    }

    /// Cancel any pending work (e.g. the node scrolled out of view).
    pub fn cancel(&mut self, handle: GenHandle) -> Result<(), HighlightError> {
        self.arena.get_mut(handle)?.pending = None;
        Ok(())
    }

    /// Text the host must paint for the next frame.
    ///
    /// - with a committed result for the current snapshot: its visible text;
    /// - with a committed result for an older snapshot: the last valid styled
    ///   text (upstream keeps it until the next valid result is ready);
    /// - otherwise: nothing, unless raw publishing was enabled, so an
    ///   unstyled frame never flashes raw source.
    #[must_use]
    pub fn frame_text(&self, handle: GenHandle) -> Option<String> {
        let target = self.arena.get(handle).ok()?;
        match &target.committed {
            Some(committed) if committed.snapshot == target.snapshot => {
                Some(committed.visible_text())
            }
            Some(committed) => Some(committed.visible_text()),
            None if target.publish_raw_while_pending => Some(target.source.clone()),
            None => Some(String::new()),
        }
    }

    /// Styled runs for the current frame (`start..end` are visible byte
    /// offsets). Adjacent runs with the same resolved style merge, so a
    /// painter never emits per-character attribute changes.
    #[must_use]
    pub fn styled_runs(&self, handle: GenHandle, theme: &HighlightTheme) -> Vec<StyledRun> {
        let Ok(target) = self.arena.get(handle) else {
            return Vec::new();
        };
        let Some(committed) = &target.committed else {
            return Vec::new();
        };
        let visible = committed.visible_text();
        let mut runs: Vec<StyledRun> = Vec::new();
        let mut cursor = 0;
        for span in committed.spans() {
            let (Some(start), Some(end)) = (
                committed.source_to_visible(span.start),
                committed.source_to_visible(span.end),
            ) else {
                continue;
            };
            // Overlapping spans share visible bytes; only paint what is not
            // already covered so runs stay contiguous.
            let start = start.max(cursor);
            if start >= end {
                continue;
            }
            if start > cursor {
                push_run(
                    &mut runs,
                    StyledRun {
                        start: cursor,
                        end: start,
                        style: ThemeStyle::default(),
                        link: None,
                        conceal: Concealment::Visible,
                    },
                );
            }
            push_run(
                &mut runs,
                StyledRun {
                    start,
                    end,
                    style: theme.style(span.capture()),
                    link: span.link(),
                    conceal: span.conceal(),
                },
            );
            cursor = end.max(cursor);
        }
        if cursor < visible.len() {
            push_run(
                &mut runs,
                StyledRun {
                    start: cursor,
                    end: visible.len(),
                    style: ThemeStyle::default(),
                    link: None,
                    conceal: Concealment::Visible,
                },
            );
        }
        runs
    }

    /// Snapshot currently associated with a target (for assertions/tests).
    #[must_use]
    pub fn snapshot(&self, handle: GenHandle) -> Option<u64> {
        self.arena.get(handle).ok().map(|target| target.snapshot)
    }

    /// True when a result is still expected for this target.
    #[must_use]
    pub fn has_pending(&self, handle: GenHandle) -> bool {
        self.arena
            .get(handle)
            .map(|target| target.pending.is_some())
            .unwrap_or(false)
    }
}

/// One contiguous visible run of styled text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyledRun {
    pub start: usize,
    pub end: usize,
    pub style: ThemeStyle,
    pub link: Option<u16>,
    pub conceal: Concealment,
}

fn push_run(runs: &mut Vec<StyledRun>, run: StyledRun) {
    if run.start >= run.end {
        return;
    }
    if let Some(last) = runs.last_mut() {
        if last.end == run.start
            && last.style == run.style
            && last.link == run.link
            && last.conceal == run.conceal
        {
            last.end = run.end;
            return;
        }
    }
    runs.push(run);
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUST: GrammarKey = GrammarKey {
        language: 7,
        query_version: 3,
    };

    fn span(start: usize, end: usize, class: CaptureClass) -> HighlightSpan {
        HighlightSpan::new(start, end, class)
    }

    fn doc(snapshot: u64, grammar: GrammarKey, source: &str) -> HighlightedText {
        HighlightedText::new(
            snapshot,
            grammar,
            source,
            vec![span(0, source.len(), CaptureClass::Keyword)],
            Vec::new(),
        )
        .expect("valid doc")
    }

    #[test]
    fn theme_covers_every_capture_class() {
        let theme = HighlightTheme::default();
        for class in CaptureClass::ALL {
            let style = theme.style(class);
            // At least one visual attribute distinguishes each class from the
            // plain default, except the classes that are deliberately plain.
            if matches!(
                class,
                CaptureClass::Variable | CaptureClass::Property | CaptureClass::Operator
            ) {
                assert_eq!(style, ThemeStyle::default());
            } else {
                assert_ne!(
                    style,
                    ThemeStyle::default(),
                    "{class:?} must carry a distinguishing attribute"
                );
            }
        }
    }

    #[test]
    fn result_validation_is_bounded_and_fail_closed() {
        assert_eq!(
            HighlightedText::new(1, RUST, "abc", vec![span(0, 4, CaptureClass::String)], vec![]),
            Err(HighlightError::SpanOutOfBounds { start: 0, end: 4 })
        );
        assert_eq!(
            HighlightedText::new(1, RUST, "abc", vec![span(2, 2, CaptureClass::String)], vec![]),
            Err(HighlightError::EmptySpan)
        );
        assert_eq!(
            HighlightedText::new(1, RUST, "abc", vec![span(1, 0, CaptureClass::String)], vec![]),
            Err(HighlightError::SpanOutOfBounds { start: 1, end: 0 })
        );
        // Multi-byte boundaries are rejected, not rounded.
        assert_eq!(
            HighlightedText::new(1, RUST, "é", vec![span(0, 1, CaptureClass::String)], vec![]),
            Err(HighlightError::SpanNotCharBoundary)
        );
        assert_eq!(
            HighlightedText::new(
                1,
                RUST,
                "abc",
                vec![span(0, 1, CaptureClass::String).with_link(9)],
                vec![]
            ),
            Err(HighlightError::DanglingLink { span: 0, id: 9 })
        );
        assert_eq!(
            HighlightedText::new(
                1,
                RUST,
                "abc",
                vec![],
                vec![HighlightLink::new(1, "https://a"), HighlightLink::new(1, "https://b")]
            ),
            Err(HighlightError::DuplicateLink { id: 1 })
        );
        let too_many = vec![span(0, 1, CaptureClass::String); MAX_SPANS + 1];
        assert_eq!(
            HighlightedText::new(1, RUST, "abc", too_many, vec![]),
            Err(HighlightError::TooManySpans)
        );
        let huge = "x".repeat(MAX_SOURCE_BYTES + 1);
        assert_eq!(
            HighlightedText::new(1, RUST, &huge, vec![], vec![]),
            Err(HighlightError::SourceTooLarge)
        );
    }

    #[test]
    fn spans_are_sorted_and_reported_per_byte() {
        let text = HighlightedText::new(
            4,
            RUST,
            "let x = 1;",
            vec![
                span(8, 9, CaptureClass::Number),
                span(0, 3, CaptureClass::Keyword),
                span(4, 5, CaptureClass::Variable),
                span(0, 9, CaptureClass::String),
            ],
            vec![],
        )
        .unwrap();
        let starts: Vec<usize> = text.spans().iter().map(HighlightSpan::start).collect();
        assert_eq!(starts, [0, 0, 4, 8]);
        assert_eq!(text.span_at(1).unwrap().capture(), CaptureClass::Keyword);
        // Nesting: the most specific (latest-starting) span wins.
        assert_eq!(text.span_at(4).unwrap().capture(), CaptureClass::Variable);
        assert_eq!(text.span_at(2).unwrap().capture(), CaptureClass::Keyword);
        assert!(text.span_at(9).is_none());
        let theme = HighlightTheme::default();
        assert_eq!(
            text.style_at(8, &theme),
            theme.style(CaptureClass::Number)
        );
    }

    #[test]
    fn concealment_hides_and_maps_offsets() {
        let source = "token=secret;next";
        let text = HighlightedText::new(
            1,
            RUST,
            source,
            vec![
                span(6, 12, CaptureClass::String).with_conceal(Concealment::Hidden),
                span(0, 5, CaptureClass::Property),
            ],
            vec![],
        )
        .unwrap();
        assert_eq!(text.visible_text(), "token=;next");
        assert_eq!(text.source_to_visible(6), None, "hidden start is unmapped");
        assert_eq!(text.source_to_visible(12), Some(6));
        assert_eq!(text.source_to_visible(13), Some(7));
        assert_eq!(text.visible_to_source(6), Some(12));
        assert_eq!(text.visible_to_source(7), Some(13));
        assert_eq!(text.visible_to_source(usize::MAX), None);
        assert_eq!(text.source_to_visible(source.len() + 1), None);
    }

    #[test]
    fn concealment_marker_counts_for_offsets() {
        let text = HighlightedText::new(
            1,
            RUST,
            "abc",
            vec![span(1, 2, CaptureClass::String).with_conceal(Concealment::Marker('•'))],
            vec![],
        )
        .unwrap();
        assert_eq!(text.visible_text(), "a•c");
        assert_eq!(text.source_to_visible(2), Some(4), "2 marker bytes");
    }

    #[test]
    fn overlapping_concealment_is_rejected() {
        assert_eq!(
            HighlightedText::new(
                1,
                RUST,
                "abcdef",
                vec![
                    span(0, 3, CaptureClass::String).with_conceal(Concealment::Hidden),
                    span(2, 5, CaptureClass::Comment).with_conceal(Concealment::Hidden),
                ],
                vec![]
            ),
            Err(HighlightError::OverlappingConcealment)
        );
    }

    #[test]
    fn late_result_is_discarded_after_destroy() {
        // FFI-07 fixture: highlight request pending, node destroyed.
        let mut targets = HighlightTargets::new();
        let (node, _) = targets.create().unwrap();
        targets.set_source(node, "let x = 1;").unwrap();
        let request = targets.request(node, RUST).unwrap();
        targets.destroy(node).unwrap();
        let completion = targets.complete(request, doc(request.snapshot(), RUST, "let x = 1;"));
        assert_eq!(
            completion,
            Completion::Discarded(StaleReason::NodeDestroyed)
        );
        assert_eq!(targets.frame_text(node), None);
    }

    #[test]
    fn late_result_is_discarded_after_view_replacement() {
        let mut targets = HighlightTargets::new();
        let (node, generation) = targets.create().unwrap();
        targets.set_source(node, "fn main() {}").unwrap();
        let request = targets.request(node, RUST).unwrap();
        assert_eq!(request.generation(), generation);
        // Same node handle, new native text view.
        let next = targets.replace_view(node).unwrap();
        assert_eq!(next, generation + 1);
        let completion = targets.complete(request, doc(request.snapshot(), RUST, "fn main() {}"));
        assert_eq!(
            completion,
            Completion::Discarded(StaleReason::NodeReplaced)
        );
        assert!(!targets.has_pending(node), "replacement clears pending work");
    }

    #[test]
    fn late_result_is_discarded_after_content_change() {
        let mut targets = HighlightTargets::new();
        let (node, _) = targets.create().unwrap();
        targets.set_source(node, "a").unwrap();
        let stale = targets.request(node, RUST).unwrap();
        targets.set_source(node, "ab").unwrap();
        let fresh = targets.request(node, RUST).unwrap();
        assert!(fresh.snapshot() > stale.snapshot());
        assert_eq!(
            targets.complete(stale, doc(stale.snapshot(), RUST, "a")),
            Completion::Discarded(StaleReason::Superseded)
        );
        assert_eq!(
            targets.complete(fresh, doc(fresh.snapshot(), RUST, "ab")),
            Completion::Applied { generation: 1 }
        );
    }

    #[test]
    fn mismatched_snapshot_or_grammar_is_rejected_on_completion() {
        let mut targets = HighlightTargets::new();
        let (a, _) = targets.create().unwrap();
        targets.set_source(a, "x").unwrap();
        let request = targets.request(a, RUST).unwrap();
        assert_eq!(
            targets.complete(request, doc(request.snapshot() + 1, RUST, "x")),
            Completion::Discarded(StaleReason::SnapshotMismatch)
        );
        let other = GrammarKey {
            language: 8,
            query_version: 1,
        };
        assert_eq!(
            targets.complete(request, doc(request.snapshot(), other, "x")),
            Completion::Discarded(StaleReason::GrammarMismatch)
        );
        // A second completion for the same request is no longer pending.
        assert_eq!(
            targets.complete(request, doc(request.snapshot(), RUST, "x")),
            Completion::Applied { generation: 1 }
        );
        assert_eq!(
            targets.complete(request, doc(request.snapshot(), RUST, "x")),
            Completion::Discarded(StaleReason::UnknownRequest)
        );
    }

    #[test]
    fn repeated_requests_do_not_enqueue_duplicate_work() {
        let mut targets = HighlightTargets::new();
        let (node, _) = targets.create().unwrap();
        targets.set_source(node, "abc").unwrap();
        let first = targets.request(node, RUST).unwrap();
        let second = targets.request(node, RUST).unwrap();
        assert_eq!(first, second);
        // A different grammar replaces the pending work.
        let other = GrammarKey {
            language: 2,
            query_version: 1,
        };
        let third = targets.request(node, other).unwrap();
        assert_eq!(third.grammar(), other);
        assert_eq!(
            targets.complete(first, doc(first.snapshot(), RUST, "abc")),
            Completion::Discarded(StaleReason::UnknownRequest)
        );
    }

    #[test]
    fn frame_text_never_flashes_raw_source() {
        // Fixture: a fresh node paints nothing rather than raw markers.
        let mut targets = HighlightTargets::new();
        let (fresh_node, _) = targets.create().unwrap();
        targets.set_source(fresh_node, "**raw**").unwrap();
        assert_eq!(targets.frame_text(fresh_node), Some(String::new()));
        // Fixture: previous valid styled frame survives a streaming update.
        let (node, _) = targets.create().unwrap();
        targets.set_source(node, "**raw**").unwrap();
        let request = targets.request(node, RUST).unwrap();
        targets.complete(request, doc(request.snapshot(), RUST, "**raw**"));
        targets.set_source(node, "**raw** plus").unwrap();
        assert_eq!(targets.frame_text(node), Some("**raw**".to_string()));
        assert!(targets.has_pending(node) == false);
        // Opting in publishes raw text (streaming editors), never partial
        // markers from an uncommitted parse.
        targets.set_publish_raw_while_pending(node, true).unwrap();
        let request = targets.request(node, RUST).unwrap();
        assert!(targets.has_pending(node));
        assert_eq!(
            targets.frame_text(node),
            Some("**raw**".to_string()),
            "a committed frame still wins over raw source"
        );
        assert_eq!(request.snapshot(), 2);
        // A destroyed node has no frame text at all.
        targets.destroy(node).unwrap();
        assert_eq!(targets.frame_text(node), None);
    }

    #[test]
    fn styled_runs_cover_visible_text_and_merge() {
        let mut targets = HighlightTargets::new();
        let (node, _) = targets.create().unwrap();
        targets.set_source(node, "let x = 1;").unwrap();
        let request = targets.request(node, RUST).unwrap();
        let text = HighlightedText::new(
            request.snapshot(),
            RUST,
            "let x = 1;",
            vec![
                span(0, 3, CaptureClass::Keyword),
                span(3, 4, CaptureClass::Keyword),
                span(4, 5, CaptureClass::String),
            ],
            vec![HighlightLink::new(1, "https://example.test")],
        )
        .unwrap();
        assert_eq!(
            targets.complete(request, text.clone()),
            Completion::Applied { generation: 1 }
        );
        let theme = HighlightTheme::default();
        let runs = targets.styled_runs(node, &theme);
        assert_eq!(runs.first().unwrap().start, 0);
        assert_eq!(runs.last().unwrap().end, "let x = 1;".len());
        for pair in runs.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "runs are contiguous");
        }
        // Adjacent keyword spans merge into one run of 4 bytes.
        assert_eq!(runs[0].end - runs[0].start, 4);
        assert_eq!(runs[0].style, theme.style(CaptureClass::Keyword));
        assert_eq!(runs[1].style, theme.style(CaptureClass::String));
        assert_eq!(runs[2].style, ThemeStyle::default());
        let link_span = HighlightSpan::new(4, 5, CaptureClass::Variable).with_link(1);
        assert_eq!(link_span.link(), Some(1));
    }

    #[test]
    fn styled_runs_are_empty_without_a_committed_result() {
        let mut targets = HighlightTargets::new();
        let (node, _) = targets.create().unwrap();
        targets.set_source(node, "abc").unwrap();
        assert!(targets.styled_runs(node, &HighlightTheme::default()).is_empty());
        assert!(targets.styled_runs(GenHandle::INVALID, &HighlightTheme::default()).is_empty());
    }

    #[test]
    fn cancel_discards_pending_work() {
        let mut targets = HighlightTargets::new();
        let (node, _) = targets.create().unwrap();
        targets.set_source(node, "x").unwrap();
        let request = targets.request(node, RUST).unwrap();
        targets.cancel(node).unwrap();
        assert!(!targets.has_pending(node));
        assert_eq!(
            targets.complete(request, doc(request.snapshot(), RUST, "x")),
            Completion::Discarded(StaleReason::UnknownRequest)
        );
    }

    #[test]
    fn owner_thread_is_enforced() {
        let mut targets = HighlightTargets::new();
        let (node, _) = targets.create().unwrap();
        targets.set_source(node, "x").unwrap();
        let completion = targets.request(node, RUST).unwrap();
        let (targets, observed) = std::thread::scope(|scope| {
            scope
                .spawn(move || {
                    let observed = targets.complete(completion, doc(1, RUST, "x"));
                    (targets, observed)
                })
                .join()
                .unwrap()
        });
        assert_eq!(
            observed,
            Completion::Discarded(StaleReason::NodeDestroyed),
            "off-thread completion cannot mutate the target"
        );
        assert_eq!(targets.snapshot(node), Some(1));
    }
}
