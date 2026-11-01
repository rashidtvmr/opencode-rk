//! Golden capture envelope and canonical frame compare (PAR-01/03/04/05/06),
//! plus the PAR-22 provenance manifest that attaches source/fixture hashes,
//! exact test commands, failures and installed captures to a final Rust SHA.
//!
//! PAR-01/03/06 record the capture envelope (selected product/renderer/
//! runtime/theme pins, one identical deterministic fixture, the baseline
//! terminal/font environment) and reject mixed envelopes before any frame
//! comparison. PAR-04 compares text, positions, styles, clipping, cursor
//! and hit targets as structured mismatches; PAR-05 preserves wide-cell
//! continuation and grapheme semantics in the canonical serialization via
//! `native_view_layout::visit_cells`.
//!
//! Selected pins mirror
//! `docs/ui-parity/opencode-ui-parity-kit/baseline.lock.json:4-22`
//! (authoritative target commit 450e7b92... package 2.1.4, renderer OpenTUI
//! v0.6.8 8d6065a3..., bun 1.4.4 / solid 1.9.15, theme opencode/dark). The
//! comparison-only v2.2.0 pins are intentionally not the selected set.
#![allow(dead_code)] // harness API; consumed by the capture runner and tests

use crate::native_view_layout::visit_cells;

pub const SELECTED_PRODUCT_COMMIT: &str = "450e7b92412c780a7b5a75226bb8d49aaca24e22";
pub const SELECTED_PRODUCT_VERSION: &str = "2.1.4";
pub const SELECTED_RENDERER_TAG: &str = "v0.6.8";
pub const SELECTED_RENDERER_COMMIT: &str = "8d6065a3e60039d16c6ee697692dcfbc46d3c786";
pub const REFERENCE_BUN: &str = "1.4.4";
pub const REFERENCE_SOLID: &str = "1.9.15";
pub const SELECTED_THEME: &str = "opencode";
pub const SELECTED_THEME_MODE: &str = "dark";

/// Capture pins recorded for every golden (PAR-01).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturePins {
    pub product_commit: String,
    pub product_version: String,
    pub renderer_tag: String,
    pub renderer_commit: String,
    pub bun: String,
    pub solid: String,
    pub theme: String,
    pub theme_mode: String,
}

impl CapturePins {
    /// The selected authoritative pin set from `baseline.lock.json`.
    #[must_use]
    pub fn selected() -> Self {
        Self {
            product_commit: SELECTED_PRODUCT_COMMIT.to_string(),
            product_version: SELECTED_PRODUCT_VERSION.to_string(),
            renderer_tag: SELECTED_RENDERER_TAG.to_string(),
            renderer_commit: SELECTED_RENDERER_COMMIT.to_string(),
            bun: REFERENCE_BUN.to_string(),
            solid: REFERENCE_SOLID.to_string(),
            theme: SELECTED_THEME.to_string(),
            theme_mode: SELECTED_THEME_MODE.to_string(),
        }
    }

    #[must_use]
    pub fn matches_selected(&self) -> bool {
        *self == Self::selected()
    }
}

/// Deterministic fixture identity shared by both capture sides (PAR-03).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureRef {
    pub id: String,
    pub digest: u64,
}

/// Stable in-process digest of the deterministic fixture inputs.
#[must_use]
pub fn fixture_digest(parts: &[&str]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for part in parts {
        part.hash(&mut hasher);
    }
    parts.len().hash(&mut hasher);
    hasher.finish()
}

/// Baseline terminal/font environment for pixel captures (PAR-06). When the
/// environment is terminal-specific the capture must be isolated (`isolated`
/// on `Capture`) so terminal-driven regressions are diagnosed against that
/// terminal only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaselineEnvironment {
    pub terminal: String,
    pub font: String,
    pub cell_width_px: u16,
    pub cell_height_px: u16,
    pub terminal_specific: bool,
}

impl Default for BaselineEnvironment {
    fn default() -> Self {
        Self {
            terminal: "reference".to_string(),
            font: "monospace".to_string(),
            cell_width_px: 9,
            cell_height_px: 18,
            terminal_specific: false,
        }
    }
}

/// One painted cell of a canonical frame (PAR-05). A wide glyph (2 cells)
/// occupies its base cell plus one `continuation` cell; grapheme clusters
/// (combining marks, regional pairs, ZWJ joins) stay whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameCell {
    pub x: u16,
    pub grapheme: String,
    pub cells: u16,
    pub continuation: bool,
    pub style: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HitTarget {
    pub x: u16,
    pub y: u16,
    pub action: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    pub cols: u16,
    pub rows: u16,
    pub lines: Vec<Vec<FrameCell>>,
    pub cursor: Option<Cursor>,
    pub hit_targets: Vec<HitTarget>,
}

impl Frame {
    #[must_use]
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            lines: vec![Vec::new(); usize::from(rows)],
            cursor: None,
            hit_targets: Vec::new(),
        }
    }

    /// Paint one styled row, preserving grapheme clusters and wide-cell
    /// continuation exactly as the layout scanner segments them (PAR-05).
    pub fn paint(&mut self, y: u16, text: &str, style: &str) {
        if usize::from(y) >= self.lines.len() {
            return;
        }
        let row = &mut self.lines[usize::from(y)];
        let mut painter = |x: u16, glyph: &str, cells: u16| {
            row.push(FrameCell {
                x,
                grapheme: glyph.to_string(),
                cells,
                continuation: false,
                style: style.to_string(),
            });
            for step in 1..cells {
                if x.saturating_add(step) < self.cols {
                    row.push(FrameCell {
                        x: x.saturating_add(step),
                        grapheme: String::new(),
                        cells: 0,
                        continuation: true,
                        style: style.to_string(),
                    });
                }
            }
        };
        visit_cells(text, self.cols, &mut painter);
        row.sort_by_key(|cell| cell.x);
    }

    /// Canonical, style-aware serialization; the frame identity that goldens
    /// hash. Continuation cells serialize distinctly so a wide glyph can
    /// never be re-read as two narrow ones (PAR-05).
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut out = String::new();
        for (y, row) in self.lines.iter().enumerate() {
            out.push_str(&format!("{y}|"));
            for x in 0..self.cols {
                if x > 0 {
                out.push(' ');
            }
                match row.iter().find(|cell| cell.x == x) {
                    Some(cell) if cell.continuation => out.push_str("<cont>"),
                    Some(cell) => out.push_str(&format!("{}:{}", cell.style, cell.grapheme)),
                    None => out.push('~'),
                }
            }
            out.push('\n');
        }
        if let Some(cursor) = self.cursor {
            out.push_str(&format!(
                "cursor={},{},{}\n",
                cursor.x, cursor.y, cursor.visible
            ));
        }
        for target in &self.hit_targets {
            out.push_str(&format!(
                "hit={},{},{}\n",
                target.x, target.y, target.action
            ));
        }
        out
    }
}

/// Structured frame difference (PAR-04): text, positions, styles, clipping,
/// cursor and hit targets are each reported; text-only equality is not used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mismatch {
    Clipping {
        expected: (u16, u16),
        actual: (u16, u16),
    },
    Text {
        y: u16,
        expected: String,
        actual: String,
    },
    Position {
        y: u16,
        x: u16,
        expected: String,
        actual: String,
    },
    Style {
        y: u16,
        x: u16,
        expected: String,
        actual: String,
    },
    Cursor {
        expected: Option<Cursor>,
        actual: Option<Cursor>,
    },
    HitTarget {
        expected: Vec<HitTarget>,
        actual: Vec<HitTarget>,
    },
}

#[derive(Clone, Copy, Debug)]
enum Occupant<'a> {
    Gap,
    Continuation,
    Base { grapheme: &'a str, style: &'a str },
}

fn occupant<'a>(frame: &'a Frame, y: u16, x: u16) -> Occupant<'a> {
    let Some(row) = frame.lines.get(usize::from(y)) else {
        return Occupant::Gap;
    };
    let Some(cell) = row.iter().find(|cell| cell.x == x) else {
        return Occupant::Gap;
    };
    if cell.continuation {
        Occupant::Continuation
    } else {
        Occupant::Base {
            grapheme: &cell.grapheme,
            style: &cell.style,
        }
    }
}

/// Text-only projection of one row: base graphemes, gaps padded as spaces.
fn row_text(frame: &Frame, y: u16) -> String {
    let Some(row) = frame.lines.get(usize::from(y)) else {
        return String::new();
    };
    let mut text = String::new();
    for x in 0..frame.cols {
        match occupant(frame, y, x) {
            Occupant::Base { grapheme, .. } => text.push_str(grapheme),
            Occupant::Continuation => {}
            Occupant::Gap => text.push(' '),
        }
    }
    text
}

#[must_use]
pub fn compare(expected: &Frame, actual: &Frame) -> Vec<Mismatch> {
    let mut diffs = Vec::new();
    if (expected.cols, expected.rows) != (actual.cols, actual.rows) {
        diffs.push(Mismatch::Clipping {
            expected: (expected.cols, expected.rows),
            actual: (actual.cols, actual.rows),
        });
    }
    for y in 0..expected.rows.min(actual.rows) {
        let expected_text = row_text(expected, y);
        let actual_text = row_text(actual, y);
        if expected_text != actual_text {
            diffs.push(Mismatch::Text {
                y,
                expected: expected_text,
                actual: actual_text,
            });
            continue;
        }
        for x in 0..expected.cols {
            let e = occupant(expected, y, x);
            let a = occupant(actual, y, x);
            let describe = |o: &Occupant| match o {
                Occupant::Gap => "gap".to_string(),
                Occupant::Continuation => "continuation".to_string(),
                Occupant::Base { grapheme, .. } => (*grapheme).to_string(),
            };
            match (e, a) {
                (Occupant::Base { grapheme, .. }, Occupant::Base { .. })
                    if grapheme == " " => {}
                (Occupant::Gap, Occupant::Base { grapheme, .. })
                | (Occupant::Base { grapheme, .. }, Occupant::Gap)
                    if grapheme == " " => {}
                (Occupant::Base { style: es, .. }, Occupant::Base { style: as_, .. })
                    if es != as_ =>
                {
                    diffs.push(Mismatch::Style {
                        y,
                        x,
                        expected: es.to_string(),
                        actual: as_.to_string(),
                    });
                }
                (Occupant::Base { .. }, Occupant::Base { .. }) => {}
                (Occupant::Continuation, Occupant::Continuation) => {}
                (e, a) => diffs.push(Mismatch::Position {
                    y,
                    x,
                    expected: describe(&e),
                    actual: describe(&a),
                }),
            }
        }
    }
    if expected.cursor != actual.cursor {
        diffs.push(Mismatch::Cursor {
            expected: expected.cursor,
            actual: actual.cursor,
        });
    }
    let mut expected_hits = expected.hit_targets.clone();
    let mut actual_hits = actual.hit_targets.clone();
    let key = |t: &HitTarget| (t.y, t.x, t.action.clone());
    expected_hits.sort_by_key(key);
    actual_hits.sort_by_key(key);
    if expected_hits != actual_hits {
        diffs.push(Mismatch::HitTarget {
            expected: expected_hits,
            actual: actual_hits,
        });
    }
    diffs
}

/// One recorded golden capture: envelope plus frame (PAR-01/03/06).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub pins: CapturePins,
    pub fixture: FixtureRef,
    pub env: BaselineEnvironment,
    pub isolated: bool,
    pub source: &'static str,
    pub frame: Frame,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureError {
    /// PAR-01: every golden must carry the selected pins.
    MixedPins { index: usize },
    /// PAR-03: both sides must use one identical deterministic fixture.
    MixedFixture { index: usize },
    /// PAR-06: terminal-specific pixel captures must be isolated.
    UnisolatedPixelCapture { index: usize },
}

/// Reject mixed-version envelopes before any frame comparison.
pub fn validate_capture_set(captures: &[Capture]) -> Result<(), CaptureError> {
    let Some(first) = captures.first() else {
        return Ok(());
    };
    for (index, capture) in captures.iter().enumerate() {
        if capture.pins != first.pins {
            return Err(CaptureError::MixedPins { index });
        }
        if capture.fixture != first.fixture {
            return Err(CaptureError::MixedFixture { index });
        }
        if capture.env.terminal_specific && !capture.isolated {
            return Err(CaptureError::UnisolatedPixelCapture { index });
        }
    }
    Ok(())
}


/// Provenance bound to one final Rust SHA (PAR-22): the source/fixture
/// hashes, exact test commands, observed failures and installed captures
/// that a parity result was produced with. `canonical()` serializes every
/// field deterministically so the manifest itself can be hashed and
/// attached to the release, and `ok()` is false while any failure remains.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvenanceManifest {
    /// Final Rust build/release SHA this evidence is attached to.
    pub rust_sha: String,
    /// Label -> source commit hash (product, renderer, base SHA, ...).
    pub source_commits: Vec<(String, String)>,
    /// Fixture identities with their deterministic digests (PAR-03).
    pub fixtures: Vec<FixtureRef>,
    /// Exact test commands that were run.
    pub test_commands: Vec<String>,
    /// Observed failures; empty means the commands passed.
    pub failures: Vec<String>,
    /// Installed capture digests/paths reviewed at this SHA.
    pub installed_captures: Vec<String>,
}

impl ProvenanceManifest {
    #[must_use]
    pub fn ok(&self) -> bool {
        self.failures.is_empty()
    }

    /// Deterministic, hashable serialization of the whole manifest; equal
    /// manifests hash equal, and any bound field change (including the Rust
    /// SHA) changes the canonical bytes.
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut lines = vec![
            format!("rust_sha={}", self.rust_sha),
            "source:".to_string(),
        ];
        let mut sources = self.source_commits.clone();
        sources.sort();
        for (label, sha) in sources {
            lines.push(format!("  {label}={sha}"));
        }
        lines.push("fixtures:".to_string());
        let mut fixtures = self.fixtures.clone();
        fixtures.sort_by(|a, b| a.id.cmp(&b.id));
        for fixture in fixtures {
            lines.push(format!("  {}=0x{:016x}", fixture.id, fixture.digest));
        }
        lines.push("commands:".to_string());
        for command in &self.test_commands {
            lines.push(format!("  {command}"));
        }
        lines.push("failures:".to_string());
        for failure in &self.failures {
            lines.push(format!("  {failure}"));
        }
        lines.push("captures:".to_string());
        for capture in &self.installed_captures {
            lines.push(format!("  {capture}"));
        }
        lines.push(String::new());
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        compare, fixture_digest, validate_capture_set, BaselineEnvironment, Capture,
        CaptureError, CapturePins, Cursor, FixtureRef, Frame, HitTarget, Mismatch,
        ProvenanceManifest, SELECTED_PRODUCT_COMMIT, SELECTED_PRODUCT_VERSION,
        SELECTED_RENDERER_COMMIT, SELECTED_RENDERER_TAG,
    };

    fn base_frame() -> Frame {
        let mut frame = Frame::new(12, 1);
        frame.paint(0, "漢字X", "accent");
        frame
    }

    fn capture(index: u64) -> Capture {
        Capture {
            pins: CapturePins::selected(),
            fixture: FixtureRef {
                id: "idle-160x48".to_string(),
                digest: index,
            },
            env: BaselineEnvironment::default(),
            isolated: true,
            source: "rust",
            frame: base_frame(),
        }
    }

    #[test]
    fn selected_pins_match_the_kit_baseline_lock() {
        let pins = CapturePins::selected();
        assert!(pins.matches_selected());
        assert_eq!(SELECTED_PRODUCT_COMMIT, "450e7b92412c780a7b5a75226bb8d49aaca24e22");
        assert_eq!(SELECTED_PRODUCT_VERSION, "2.1.4");
        assert_eq!(SELECTED_RENDERER_TAG, "v0.6.8");
        assert_eq!(SELECTED_RENDERER_COMMIT, "8d6065a3e60039d16c6ee697692dcfbc46d3c786");
    }

    #[test]
    fn par01_mixed_capture_pins_are_rejected() {
        let mut drifted = capture(7);
        drifted.pins.product_commit = "1097e2668b1e08ad63bdf0b1c4860b9bc01e420c".to_string();
        let error = validate_capture_set(&[capture(7), drifted]).unwrap_err();
        assert_eq!(error, CaptureError::MixedPins { index: 1 });
        assert!(validate_capture_set(&[capture(7), capture(7)]).is_ok());
    }

    #[test]
    fn par03_captures_must_share_one_deterministic_fixture() {
        let shared = fixture_digest(&["idle-160x48", "theme=opencode", "mode=dark"]);
        let mut lhs = capture(shared);
        lhs.fixture.digest = shared;
        let mut rhs = capture(shared);
        rhs.fixture.digest = shared;
        assert!(validate_capture_set(&[lhs, rhs.clone()]).is_ok());
        rhs.fixture.id = "running-160x48".to_string();
        assert_eq!(
            validate_capture_set(&[capture(shared), rhs]).unwrap_err(),
            CaptureError::MixedFixture { index: 1 }
        );
    }

    #[test]
    fn par04_style_only_differences_do_not_hide_behind_text_equality() {
        let mut actual = base_frame();
        for cell in &mut actual.lines[0] {
            cell.style = "muted".to_string();
        }
        let diffs = compare(&base_frame(), &actual);
        assert!(!diffs.is_empty(), "style drift must surface");
        assert!(
            diffs
                .iter()
                .all(|d| matches!(d, Mismatch::Style { y: 0, .. })),
            "every difference is a style-only diff on the changed row: {diffs:?}"
        );
    }

    #[test]
    fn par04_cursor_and_hit_target_mismatches_are_reported() {
        let mut expected = base_frame();
        expected.cursor = Some(Cursor { x: 3, y: 0, visible: true });
        expected.hit_targets = vec![HitTarget {
            x: 1,
            y: 0,
            action: "tab.select".to_string(),
        }];
        let mut actual = base_frame();
        actual.cursor = Some(Cursor { x: 4, y: 0, visible: true });
        let diffs = compare(&expected, &actual);
        assert!(
            diffs.contains(&Mismatch::Cursor {
                expected: expected.cursor,
                actual: actual.cursor,
            }),
            "cursor drift must surface: {diffs:?}"
        );
        assert!(
            diffs.iter().any(|d| matches!(d, Mismatch::HitTarget { .. })),
            "missing hit target must surface: {diffs:?}"
        );
        assert!(!diffs.iter().any(|d| matches!(d, Mismatch::Text { .. })));
    }

    #[test]
    fn par04_clipping_reports_the_frame_boundary() {
        let mut actual = Frame::new(10, 2);
        actual.paint(0, "漢字X", "accent");
        actual.paint(1, "hidden", "accent");
        let diffs = compare(&base_frame(), &actual);
        assert!(
            diffs.contains(&Mismatch::Clipping {
                expected: (12, 1),
                actual: (10, 2),
            }),
            "clip rect drift must surface: {diffs:?}"
        );
    }

    #[test]
    fn par06_terminal_specific_pixel_captures_must_be_isolated() {
        let mut pixel = capture(7);
        pixel.env.terminal_specific = true;
        pixel.isolated = false;
        assert_eq!(
            validate_capture_set(&[capture(7), pixel.clone()]).unwrap_err(),
            CaptureError::UnisolatedPixelCapture { index: 1 }
        );
        pixel.isolated = true;
        assert!(validate_capture_set(&[capture(7), pixel]).is_ok());
    }

    #[test]
    fn par05_wide_cell_continuation_survives_serialization() {
        let mut frame = Frame::new(12, 1);
        frame.paint(0, "漢", "accent");
        let row = &frame.lines[0];
        assert_eq!(row[0].grapheme, "漢");
        assert_eq!(row[0].cells, 2);
        assert!(!row[0].continuation);
        assert!(row[1].continuation, "wide glyph paints a continuation cell");
        assert_ne!(frame.canonical(), Frame::new(12, 1).canonical());
        // Two narrow cells never equal one wide glyph in canonical form.
        let mut narrow = Frame::new(12, 1);
        narrow.paint(0, "aa", "accent");
        assert_ne!(frame.canonical(), narrow.canonical());
    }

    #[test]
    fn par05_graphemes_stay_whole_in_the_canonical_row() {
        let mut frame = Frame::new(24, 1);
        // combining accent, flag pair, ZWJ family.
        frame.paint(0, "e\u{301} 🇯🇵 👨\u{200D}👩\u{200D}👧", "body");
        let bases: Vec<_> = frame.lines[0]
            .iter()
            .filter(|cell| !cell.continuation)
            .collect();
        let glyphs: Vec<&str> = bases.iter().map(|cell| cell.grapheme.as_str()).collect();
        assert!(glyphs.contains(&"e\u{301}"), "combining cluster intact: {glyphs:?}");
        assert!(
            glyphs.iter().any(|g| {
                let ch: Vec<char> = g.chars().collect();
                ch.len() == 2
                    && ch.iter()
                        .all(|c| ('\u{1F1E6}'..='\u{1F1FF}').contains(c))
            }),
            "flag stays one cluster: {glyphs:?}"
        );
        let wide: Vec<u16> = bases.iter().map(|cell| cell.cells).collect();
        assert_eq!(wide.iter().filter(|w| **w >= 2).count(), 2);
    }

    #[test]
    fn par22_manifest_binds_sha_hashes_commands_failures_and_captures() {
        let manifest = ProvenanceManifest {
            rust_sha: "471c87d0".to_string(),
            source_commits: vec![
                ("product".to_string(), SELECTED_PRODUCT_COMMIT.to_string()),
                ("renderer".to_string(), SELECTED_RENDERER_COMMIT.to_string()),
            ],
            fixtures: vec![FixtureRef {
                id: "idle-160x48".to_string(),
                digest: fixture_digest(&["idle-160x48", "theme=opencode", "mode=dark"]),
            }],
            test_commands: vec!["cargo test -p oc2 native_capture".to_string()],
            failures: vec!["par22 probe: style drift at row 0".to_string()],
            installed_captures: vec!["captures/par22-installed.json".to_string()],
        };
        assert!(!manifest.ok(), "a recorded failure keeps the manifest not-ok");
        let canonical = manifest.canonical();
        for needle in [
            "rust_sha=471c87d0",
            "product=450e7b92412c780a7b5a75226bb8d49aaca24e22",
            "renderer=8d6065a3e60039d16c6ee697692dcfbc46d3c786",
            "idle-160x48=0x",
            "cargo test -p oc2 native_capture",
            "style drift at row 0",
            "captures/par22-installed.json",
        ] {
            assert!(canonical.contains(needle), "missing {needle:?} in:\n{canonical}");
        }
        let mut passed = manifest.clone();
        passed.failures.clear();
        assert!(passed.ok());
        assert_ne!(passed.canonical(), canonical, "failures must change the manifest");
        let mut other_sha = manifest.clone();
        other_sha.rust_sha = "deadbeef".to_string();
        assert_ne!(
            other_sha.canonical(),
            canonical,
            "manifest must be bound to its Rust SHA"
        );
        assert_eq!(manifest, manifest.clone(), "canonical source of equal manifests");
    }
}
