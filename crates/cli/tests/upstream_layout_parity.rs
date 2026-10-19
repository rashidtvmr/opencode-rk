#![forbid(unsafe_code)]
//! Frozen independent-RED regression tests for OBS-NATIVE-UPSTREAM-LAYOUT-PARITY.
//!
//! Contract source (higher authority than the obsolete inline unit contract in
//! `native_view_layout.rs`, which is superseded pending independent
//! adjudication and must not be edited by the implementer):
//!
//! * SPEC `upstream-tui-layout-parity-v1`, upstream pin
//!   `95daf90670b7c039c436c85537da5fbfe2205b41`,
//!   `packages/tui/src/routes/session/index.tsx:255-278`:
//!   `wide = terminal_width > 120` (strict), the sidebar is an inline box of
//!   exactly 42 columns only when wide, and
//!   `contentWidth = cols - (sidebarVisible ? 42 : 0) - 4`, where the `- 4`
//!   is the view's own left/right padding of 2 and must NOT be folded into
//!   the layout geometry (`center_width == cols - right_width`).
//! * `session/index.tsx:1157-1361`: the session main area is one growing
//!   column — there is no permanent left all-sessions pane and no permanent
//!   right status/MCP pane at any width.
//!
//! These tests exercise only the current public API
//! (`Layout::new`, `compose_frame`) of the pure std-only module
//! `crates/cli/src/native_view_layout.rs`, included directly so the geometry
//! contract compiles and fails (RED) before any product change. No frozen
//! existing test is modified.

#[path = "../src/native_view_layout.rs"]
mod layout;

use layout::{compose_frame, Layout};

/// Vertical pane border from the frozen harness set; a main-only column
/// must never draw one.
const BORDER: char = '│';

fn s(text: &str) -> String {
    text.to_owned()
}

/// Display cells of a composed row using a local width table (the module's
/// `char_width` is private, so bounded unicode is checked through this
/// independent observer).
fn cells(row: &str) -> usize {
    row.chars().map(cell_width).sum()
}

fn cell_width(c: char) -> usize {
    let cp = c as u32;
    if c.is_control() {
        0
    } else if (0x1100..=0x115F).contains(&cp)
        || (0x2E80..=0x303E).contains(&cp)
        || (0x3041..=0x33FF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        || (0x4E00..=0x9FFF).contains(&cp)
        || (0xAC00..=0xD7A3).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE30..=0xFE6F).contains(&cp)
        || (0xFF00..=0xFF60).contains(&cp)
        || (0xFFE0..=0xFFE6).contains(&cp)
        || (0x1F300..=0x1FAFF).contains(&cp)
        || (0x20000..=0x3FFFD).contains(&cp)
    {
        2
    } else {
        1
    }
}

/// Boundary 120: the sidebar predicate is a strict `> 120`, so exactly 120
/// columns is a single main column — no left sessions pane, no 42-column
/// sidebar, and the main logical content width after the view's 2+2 padding
/// is the SPEC matrix value 116.
#[test]
fn upstream_parity_boundary_120_is_main_only_without_sidebar() {
    let layout = Layout::new(120, 30);
    assert!(!layout.compact, "120 columns is the normal main shell");
    assert_eq!(layout.left_width, 0, "no permanent left sessions pane");
    assert_eq!(
        layout.right_width, 0,
        "sidebar hidden at exactly 120 (wide is strictly > 120)"
    );
    assert_eq!(
        layout.center_width, 120,
        "main column spans the full viewport"
    );
    assert_eq!(
        usize::from(layout.center_width) - 4,
        116,
        "logical content width after the view's 2+2 padding (SPEC matrix)"
    );
}

/// Above 120: the sidebar is a FIXED 42-column inline box (never `cols/3`)
/// on the right, the left width stays 0, and the layout center is the outer
/// `cols - 42` — the view's `- 4` padding is applied in the view, not folded
/// into the layout (SPEC logical content widths 75 at 121 and 114 at 160).
#[test]
fn upstream_parity_boundary_121_and_wider_use_fixed_42_sidebar_not_thirds() {
    for (cols, center) in [(121u16, 79u16), (122, 80), (132, 90), (160, 118), (200, 158)] {
        let layout = Layout::new(cols, 30);
        assert!(!layout.compact, "{cols} columns is the normal main shell");
        assert_eq!(layout.left_width, 0, "{cols}: no permanent left pane");
        assert_eq!(
            layout.right_width, 42,
            "{cols}: sidebar is a fixed 42-column box, never thirds"
        );
        assert_eq!(
            layout.center_width, center,
            "{cols}: center is the outer cols - 42 (padding stays in the view)"
        );
        assert_eq!(
            layout.left_width + layout.center_width + layout.right_width,
            layout.cols,
            "{cols}: main + sidebar fill the viewport with no separator cells"
        );
    }
    // SPEC logical contentWidth = cols - 42 - 4 (padding done by the view).
    assert_eq!(usize::from(Layout::new(121, 30).center_width) - 4, 75);
    assert_eq!(usize::from(Layout::new(160, 30).center_width) - 4, 114);
}

/// Old 80-column thirds regression: the current shell splits 80 columns
/// into left 26 / center 27 / right 25 with separator borders. Upstream has
/// no width-based thirds shell at all — 80 columns is one main column with
/// no separators.
#[test]
fn upstream_parity_80_columns_drops_old_thirds_shell() {
    let layout = Layout::new(80, 24);
    assert!(!layout.compact, "80 columns is the normal main shell");
    assert_eq!(layout.left_width, 0, "no permanent left pane at 80");
    assert_eq!(layout.right_width, 0, "no sidebar at 80");
    assert_eq!(layout.center_width, 80, "main column spans the viewport");
    let frame = compose_frame(
        &layout,
        &[s("Sessions"), s("chat one")],
        &[s("workspace alpha"), s("> help")],
        &[s("Status"), s("MCP")],
    );
    assert_eq!(frame.len(), 24);
    for (index, row) in frame.iter().enumerate() {
        assert_eq!(cells(row), 80, "row {index} spans 80 cells");
        assert!(
            !row.contains(BORDER),
            "row {index} must draw no pane separators"
        );
    }
}

/// Tiny boundaries including zero: safe, bounded, main-only geometry at
/// every width below the sidebar threshold (the old shell turned 80 columns
/// into thirds with borders — that must not survive anywhere).
#[test]
fn upstream_parity_tiny_viewports_including_zero_stay_safe_and_main_only() {
    for (cols, rows) in [
        (0u16, 0u16),
        (0, 30),
        (120, 0),
        (1, 1),
        (10, 4),
        (40, 10),
        (60, 8),
        (79, 24),
        (80, 2),
        (80, 24),
    ] {
        let layout = Layout::new(cols, rows);
        assert_eq!(layout.left_width, 0, "{cols}x{rows}: no left pane");
        assert_eq!(layout.right_width, 0, "{cols}x{rows}: no sidebar");
        assert_eq!(
            layout.center_width, cols,
            "{cols}x{rows}: main column spans the viewport"
        );
        let frame = compose_frame(
            &layout,
            &[s("Sessions")],
            &[s("main"), s("> x")],
            &[s("MCP")],
        );
        assert!(
            frame.len() <= usize::from(rows),
            "{cols}x{rows}: never more rows than the viewport"
        );
        for row in &frame {
            assert_eq!(
                cells(row),
                usize::from(cols),
                "{cols}x{rows}: row width"
            );
            assert!(!row.contains(BORDER), "{cols}x{rows}: no separators");
        }
    }
}

/// The obsolete permanent three-pane shell is gone at normal width: a
/// frame composed at 120x30 contains no left `Sessions` heading, no right
/// `Status`/`MCP` headings, no separator borders, and retains every center
/// body line row-for-row, including wide glyphs at exact cell width.
#[test]
fn upstream_parity_no_obsolete_left_sessions_or_right_status_pane_at_120() {
    let layout = Layout::new(120, 30);
    let left = vec![
        s("OpenCode RK (native)"),
        s("Sessions"),
        s("chat one"),
        s("chat two"),
    ];
    let center = vec![
        s("workspace line alpha"),
        s("workspace line beta"),
        s(&"漢".repeat(60)),
        s("> prompt"),
    ];
    let right = vec![s("Status"), s("MCP"), s("idle")];
    let frame = compose_frame(&layout, &left, &center, &right);
    assert_eq!(frame.len(), 30, "every viewport row is emitted");
    for (index, row) in frame.iter().enumerate() {
        assert_eq!(cells(row), 120, "row {index} spans exactly 120 cells");
        assert!(!row.contains("Sessions"), "row {index}: no left sessions pane");
        assert!(!row.contains("OpenCode RK (native)"), "row {index}: no left banner");
        assert!(!row.contains("Status"), "row {index}: no right status pane");
        assert!(!row.contains("MCP"), "row {index}: no right MCP pane");
        assert!(!row.contains(BORDER), "row {index}: no pane separators");
    }
    for (index, line) in center.iter().enumerate() {
        assert!(
            frame[index].contains(line.as_str()),
            "center row {index} body line retained in the main column"
        );
    }
    // Bounded unicode: the 60 wide glyphs truncate on a cell boundary at
    // exactly the viewport width, never a sliced glyph.
    assert!(frame[2].contains("漢"), "wide glyphs survive truncation");
    assert_eq!(
        frame[2].chars().filter(|c| *c == '漢').count(),
        60,
        "all 60 wide glyphs fit the 120-cell main column"
    );
}
