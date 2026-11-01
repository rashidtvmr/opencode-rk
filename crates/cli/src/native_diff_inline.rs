#![forbid(unsafe_code)]
//! P15 inline diff presentation: the `Edit` / `ApplyPatch` tool bodies.
//!
//! Upstream evidence (pin 450e7b92412c780a7b5a75226bb8d49aaca24e22):
//! - Edit tool: `packages/tui/src/routes/session/index.tsx:3448-3501` —
//!   `parseApplyPatchFiles(metadata.files)[0]`, `← Edit` label, content width
//!   split rule at `:3459`, `paddingLeft={1}` diff box at `:3470`, and
//!   `Diagnostics` after the diff at `:3492`.
//! - ApplyPatch tool: `index.tsx:3512-3606` — per-file `# Created` /
//!   `# Deleted` / `← Patched` labels, the `-N line(s)` deleted fallback at
//!   `:3555-3557`, gap 1 between files at `:3514`, split rule at `:3536`, and
//!   the applied-resource fallback at `:3588-3606`.
//! - Wrap mode: `index.tsx:245` (`config.diffs?.wrap ?? "word"`) and
//!   `:3477,3567`; permission bodies always use `wrapMode="word"`
//!   (`routes/session/permission.tsx:63`).
//! - Split thresholds and split row pairing live in `native_diff_view.rs`
//!   (DIFF-001/DIFF-002 evidence is recorded there).
//! - DIFF-001/DIFF-003 arithmetic gaps are fixed in `native_diff_view.rs`:
//!   `paint_rows_split` 50/50 rounding and `relative_scroll_top` feeding the
//!   root-relative `visible_chunk_range` scroll contract.
//! - NOT WIRED: `build_edit_diff` / `build_patch_diff` /
//!   `build_permission_diff` are still unconsumed by `native_view_chat.rs`;
//!   routing Edit/ApplyPatch/permission chat rows through them is a separate
//!   consumer step outside this model module.
//!
//! Pure row model: no rendering, no IO.

use crate::native_diff_view::{
    applied_label, diagnostics_for_file, format_diagnostic, paint_rows_split,
    paint_rows_with_gutter, paint_rows_with_gutter_wrapped, parse_applied, parse_patch_files,
    resolve_view_mode, DiffPaintRow, DiffSurface, DiffViewMode, DiffViewPreference, DiffWrapMode,
    SplitRow, WireApplied, WireDiagnostic, WirePatchFile, MAX_DIFF_LINES,
};

/// `index.tsx:3514`: `gap={1}` between patch files.
pub const INLINE_BLOCK_GAP_ROWS: usize = 1;
/// One coherent tool body never paints more rows than the shared diff cap.
pub const INLINE_DIFF_MAX_ROWS: usize = MAX_DIFF_LINES;

#[derive(Clone, Debug, PartialEq)]
pub enum InlineDiffRow {
    Unified(DiffPaintRow),
    Split(SplitRow),
    /// `index.tsx:3555-3557`: a deleted file shows its deletion count and no
    /// diff body.
    DeletedSummary(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlineDiffBlock {
    /// `# Created`, `# Deleted`, `← Patched` (`index.tsx:3547-3549`) or
    /// `← Edit` (`index.tsx:3470`).
    pub path_label: &'static str,
    pub path_value: String,
    pub rows: Vec<InlineDiffRow>,
    pub diagnostics: Vec<String>,
    /// True when this block is an applied-resource fallback rather than a
    /// patch (`index.tsx:3588-3606`).
    pub applied_fallback: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlineDiffRender {
    pub blocks: Vec<InlineDiffBlock>,
    pub gap_rows: usize,
    pub applied_only: bool,
}

impl InlineDiffRender {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            blocks: Vec::new(),
            gap_rows: INLINE_BLOCK_GAP_ROWS,
            applied_only: false,
        }
    }
}

/// Paints one file patch for the resolved view. Word wrap retains
/// continuation metadata; `None` wrap keeps one row per diff line.
#[must_use]
pub fn paint_inline_patch(
    patch: &str,
    view: DiffViewMode,
    wrap: DiffWrapMode,
    measured_width: usize,
    max_rows: usize,
) -> Vec<InlineDiffRow> {
    match view {
        DiffViewMode::Split => paint_rows_split(patch, max_rows, measured_width)
            .rows
            .into_iter()
            .map(InlineDiffRow::Split)
            .collect(),
        DiffViewMode::Unified => match wrap {
            DiffWrapMode::Word => paint_rows_with_gutter_wrapped(patch, max_rows, measured_width)
                .rows
                .into_iter()
                .map(InlineDiffRow::Unified)
                .collect(),
            DiffWrapMode::None => paint_rows_with_gutter(patch, max_rows)
                .rows
                .into_iter()
                .map(InlineDiffRow::Unified)
                .collect(),
        },
    }
}

/// Builds the multi-file `ApplyPatch` body. Invalid wire entries are dropped
/// by `parse_patch_files`, so malformed metadata cannot fabricate rows.
#[must_use]
pub fn build_patch_diff(
    files: &[WirePatchFile],
    surface: DiffSurface,
    preference: DiffViewPreference,
    measured_width: usize,
    wrap: DiffWrapMode,
) -> InlineDiffRender {
    let parsed = parse_patch_files(files);
    if parsed.is_empty() {
        return InlineDiffRender::empty();
    }
    let view = resolve_view_mode(surface, preference, measured_width);
    let blocks = parsed
        .into_iter()
        .map(|file| {
            let rows = if file.is_delete() {
                vec![InlineDiffRow::DeletedSummary(file.deleted_summary())]
            } else {
                paint_inline_patch(
                    &file.patch,
                    view,
                    wrap,
                    measured_width,
                    INLINE_DIFF_MAX_ROWS,
                )
            };
            InlineDiffBlock {
                path_label: file.path_label(),
                path_value: file.relative_path,
                rows,
                diagnostics: Vec::new(),
                applied_fallback: false,
            }
        })
        .collect();
    InlineDiffRender {
        blocks,
        gap_rows: INLINE_BLOCK_GAP_ROWS,
        applied_only: false,
    }
}

/// Builds the `Edit` tool body: the first parseable file, the `← Edit` label,
/// and severity-1 diagnostics keyed by the tool's input path.
#[must_use]
pub fn build_edit_diff(
    file_path: &str,
    files: &[WirePatchFile],
    diagnostics: &[(String, Vec<WireDiagnostic>)],
    content_width: usize,
    preference: DiffViewPreference,
    wrap: DiffWrapMode,
    platform: &str,
) -> Option<InlineDiffBlock> {
    let file = parse_patch_files(files).into_iter().next()?;
    let view = resolve_view_mode(DiffSurface::Inline, preference, content_width);
    let rows = paint_inline_patch(
        &file.patch,
        view,
        wrap,
        content_width,
        INLINE_DIFF_MAX_ROWS,
    );
    let diagnostics = diagnostics_for_file(diagnostics, file_path, platform)
        .iter()
        .map(format_diagnostic)
        .collect();
    Some(InlineDiffBlock {
        path_label: "← Edit",
        path_value: file_path.to_string(),
        rows,
        diagnostics,
        applied_fallback: false,
    })
}

/// `permission.tsx:36,55-74`: the permission body measures the terminal width,
/// always wraps words, and has no diagnostics row.
#[must_use]
pub fn build_permission_diff(
    patch: &str,
    terminal_width: usize,
    preference: DiffViewPreference,
) -> Vec<InlineDiffRow> {
    let view = resolve_view_mode(DiffSurface::Permission, preference, terminal_width);
    paint_inline_patch(
        patch,
        view,
        DiffWrapMode::Word,
        terminal_width,
        INLINE_DIFF_MAX_ROWS,
    )
}

/// `index.tsx:3588-3606`: when no file patch survives parsing but applied
/// resources do, render only their path rows — never invented diff content.
#[must_use]
pub fn build_applied_fallback(applied: &[WireApplied]) -> Option<InlineDiffRender> {
    let resources = parse_applied(applied);
    if resources.is_empty() {
        return None;
    }
    Some(InlineDiffRender {
        blocks: resources
            .into_iter()
            .map(|resource| InlineDiffBlock {
                path_label: applied_label(&resource.kind),
                path_value: resource.resource,
                rows: Vec::new(),
                diagnostics: Vec::new(),
                applied_fallback: true,
            })
            .collect(),
        gap_rows: INLINE_BLOCK_GAP_ROWS,
        applied_only: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_diff_view::WireDiagnostic;

    fn add_file(path: &str, patch: &str) -> WirePatchFile {
        WirePatchFile {
            file_type: Some("add".into()),
            file: Some(path.into()),
            file_path: Some(path.into()),
            patch: Some(patch.into()),
            additions: Some(2.0),
            deletions: Some(0.0),
            ..WirePatchFile::default()
        }
    }

    fn update_file(path: &str, patch: &str) -> WirePatchFile {
        WirePatchFile {
            file_type: Some("update".into()),
            file: Some(path.into()),
            patch: Some(patch.into()),
            additions: Some(1.0),
            deletions: Some(1.0),
            ..WirePatchFile::default()
        }
    }

    fn delete_file(path: &str, deletions: f64) -> WirePatchFile {
        WirePatchFile {
            file_type: Some("delete".into()),
            file: Some(path.into()),
            file_path: Some(path.into()),
            patch: Some(String::new()),
            additions: Some(0.0),
            deletions: Some(deletions),
            ..WirePatchFile::default()
        }
    }

    // DIFF-001: the inline surface splits on content width 121, not at 120.
    #[test]
    fn inline_surface_uses_content_width_121_boundary() {
        let patch = "@@ -1,1 +1,1 @@\n-old\n+new\n";
        let files = vec![update_file("src/a.rs", patch)];
        let narrow = build_patch_diff(
            &files,
            DiffSurface::Inline,
            DiffViewPreference::Auto,
            120,
            DiffWrapMode::Word,
        );
        let wide = build_patch_diff(
            &files,
            DiffSurface::Inline,
            DiffViewPreference::Auto,
            121,
            DiffWrapMode::Word,
        );
        assert!(matches!(
            narrow.blocks[0].rows[1],
            InlineDiffRow::Unified(_)
        ));
        assert!(matches!(wide.blocks[0].rows[1], InlineDiffRow::Split(_)));

        let forced = build_patch_diff(
            &files,
            DiffSurface::Inline,
            DiffViewPreference::Split,
            80,
            DiffWrapMode::Word,
        );
        assert!(matches!(
            forced.blocks[0].rows[1],
            InlineDiffRow::Split(_)
        ));
        let forced_unified = build_patch_diff(
            &files,
            DiffSurface::Inline,
            DiffViewPreference::Unified,
            200,
            DiffWrapMode::Word,
        );
        assert!(matches!(
            forced_unified.blocks[0].rows[1],
            InlineDiffRow::Unified(_)
        ));
    }

    // DIFF-001 (permission surface): terminal width 121 enables split.
    #[test]
    fn permission_surface_uses_terminal_width_121_boundary() {
        let patch = "@@ -1,1 +1,1 @@\n-old\n+new\n";
        let narrow = build_permission_diff(patch, 120, DiffViewPreference::Auto);
        let wide = build_permission_diff(patch, 121, DiffViewPreference::Auto);
        assert!(matches!(narrow[1], InlineDiffRow::Unified(_)));
        assert!(matches!(wide[1], InlineDiffRow::Split(_)));
    }

    // DIFF-004: deleted files render only their deletion count; applied
    // resources render only path rows.
    #[test]
    fn delete_and_applied_fallbacks_have_no_diff_content() {
        let render = build_patch_diff(
            &[delete_file("src/gone.rs", 2.0), delete_file("src/one.rs", 1.0)],
            DiffSurface::Inline,
            DiffViewPreference::Auto,
            80,
            DiffWrapMode::Word,
        );
        assert_eq!(render.blocks.len(), 2);
        assert_eq!(
            render.blocks[0].rows,
            vec![InlineDiffRow::DeletedSummary("-2 lines".into())]
        );
        assert_eq!(render.blocks[1].path_label, "# Deleted");
        assert_eq!(
            render.blocks[1].rows,
            vec![InlineDiffRow::DeletedSummary("-1 line".into())]
        );

        let applied = build_applied_fallback(&[
            WireApplied {
                kind: Some("add".into()),
                resource: Some("src/new.rs".into()),
            },
            WireApplied {
                kind: None,
                resource: Some("malformed".into()),
            },
        ])
        .expect("one applied resource survives");
        assert!(applied.applied_only);
        assert_eq!(applied.blocks.len(), 1);
        assert_eq!(applied.blocks[0].path_label, "# Created");
        assert_eq!(applied.blocks[0].path_value, "src/new.rs");
        assert!(applied.blocks[0].rows.is_empty());
        assert!(applied.blocks[0].applied_fallback);
        assert!(build_applied_fallback(&[]).is_none());
    }

    // DIFF-002: labels, gap, and per-file order match the ApplyPatch body.
    #[test]
    fn patch_blocks_keep_labels_gap_and_order() {
        let render = build_patch_diff(
            &[
                add_file("a/very/long/path/that/stays/intact.rs", "@@ -0,0 +1,1 @@\n+x\n"),
                update_file("b.rs", "@@ -1 +1 @@\n-y\n+z\n"),
                delete_file("c.rs", 3.0),
            ],
            DiffSurface::Inline,
            DiffViewPreference::Auto,
            200,
            DiffWrapMode::Word,
        );
        assert_eq!(render.gap_rows, 1);
        assert_eq!(render.blocks.len(), 3);
        assert_eq!(render.blocks[0].path_label, "# Created");
        assert_eq!(
            render.blocks[0].path_value,
            "a/very/long/path/that/stays/intact.rs"
        );
        assert_eq!(render.blocks[1].path_label, "← Patched");
        assert_eq!(render.blocks[2].path_label, "# Deleted");
        assert!(!render.applied_only);
    }

    // DIFF-005: Edit attaches only severity-1 diagnostics, one-based, at most
    // three, looked up by the tool's input path.
    #[test]
    fn edit_block_attaches_severity_one_diagnostics() {
        let files = vec![update_file(
            "src/a.rs",
            "@@ -1 +1 @@\n-old\n+new\n",
        )];
        let diagnostics = vec![(
            "src/a.rs".to_string(),
            vec![
                WireDiagnostic {
                    severity: Some(1.0),
                    line: Some(0.0),
                    character: Some(2.0),
                    message: Some("first".into()),
                },
                WireDiagnostic {
                    severity: Some(2.0),
                    line: Some(5.0),
                    character: Some(0.0),
                    message: Some("warning".into()),
                },
                WireDiagnostic {
                    severity: Some(1.0),
                    line: Some(9.0),
                    character: Some(9.0),
                    message: Some("second".into()),
                },
                WireDiagnostic {
                    severity: Some(1.0),
                    line: Some(10.0),
                    character: Some(0.0),
                    message: Some("third".into()),
                },
                WireDiagnostic {
                    severity: Some(1.0),
                    line: Some(11.0),
                    character: Some(0.0),
                    message: Some("fourth".into()),
                },
            ],
        )];
        let block = build_edit_diff(
            "src/a.rs",
            &files,
            &diagnostics,
            100,
            DiffViewPreference::Auto,
            DiffWrapMode::Word,
            "linux",
        )
        .expect("first patch file");
        assert_eq!(block.path_label, "← Edit");
        assert_eq!(
            block.diagnostics,
            vec![
                "Error [1:3] first".to_string(),
                "Error [10:10] second".to_string(),
                "Error [11:1] third".to_string(),
            ]
        );
    }

    // DIFF-002 wrap mode: `none` yields no continuation rows, `word` wraps.
    #[test]
    fn wrap_mode_controls_continuation_rows() {
        assert_eq!(
            DiffWrapMode::from_config(Some("none")),
            DiffWrapMode::None
        );
        assert_eq!(DiffWrapMode::from_config(None), DiffWrapMode::Word);
        let patch = "@@ -1 +1 @@\n+added content that is long enough to wrap in a narrow pane\n";
        let files = vec![add_file("a.rs", patch)];
        let wrapped = build_patch_diff(
            &files,
            DiffSurface::Inline,
            DiffViewPreference::Auto,
            24,
            DiffWrapMode::Word,
        );
        let unwrapped = build_patch_diff(
            &files,
            DiffSurface::Inline,
            DiffViewPreference::Auto,
            24,
            DiffWrapMode::None,
        );
        let wrapped_rows = wrapped.blocks[0]
            .rows
            .iter()
            .filter(|row| matches!(row, InlineDiffRow::Unified(_)))
            .count();
        let unwrapped_rows = unwrapped.blocks[0].rows.len();
        assert!(wrapped_rows > unwrapped_rows);
        assert_eq!(unwrapped_rows, 2, "hunk header plus one added row");
    }
}
