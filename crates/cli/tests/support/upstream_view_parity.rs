#![forbid(unsafe_code)]
//! Independent rendered-view checks for OBS-NATIVE-UPSTREAM-LAYOUT-PARITY.
//!
//! The frozen geometry proposal in `upstream_layout_parity.rs` is intentionally
//! not modified here.  This target covers the adjacent observable surfaces
//! required by the pinned upstream implementation:
//!
//! * `packages/tui/src/ui/dialog.tsx` at upstream pin
//!   `95daf90670b7c039c436c85537da5fbfe2205b41`: medium/large/xlarge widths
//!   (`60/88/116`), `maxWidth = terminal width - 2`, `paddingTop = height / 4`,
//!   backdrop alpha 150, and one inner top row;
//! * `packages/tui/src/routes/home.tsx:33-36`: Home prompt max width defaults
//!   to 75, supports `auto = max(75, floor(width * .7))`, and is constrained by
//!   the Home box's two-cell left/right padding;
//! * `packages/tui/src/routes/session/index.tsx:255-278`: the optional sidebar
//!   is strict `width > 120`, fixed 42 columns when wide, and the narrow
//!   overlay uses alpha 70 while the wide inline sidebar is not dimmed.
//!
//! This test is compiled as a child of the real CLI test crate, so it uses the
//! production native view modules and their real `tui_entry` dependencies.
//! Assertions stay focused on public view behavior and returned frame rows.

use crate::native_view::{DialogSize, NativeView, PromptWidth};
use crate::native_view_dialogs::{DialogKind, DialogRequest};

fn open_modal(view: &mut NativeView, title: &str) {
    view.dialogs
        .open_request(DialogRequest {
            id: "request-1".to_owned(),
            session_id: "session-1".to_owned(),
            kind: DialogKind::Confirm,
            title: title.to_owned(),
            choices: vec!["continue".to_owned()],
            message: None,
            disabled: Vec::new(),
            error: None,
        })
        .expect("test modal request is valid");
}

#[test]
fn modal_heading_uses_upstream_top_padding_and_inner_two_cell_inset() {
    let mut view = NativeView::new(120, 32);
    let title = "Confirm the upstream layout";
    open_modal(&mut view, title);

    let modal = view.modal_presentation();
    assert_eq!(modal.width, 60);
    assert_eq!(modal.x, 30);
    assert_eq!(modal.y, 8, "dialog backdrop starts at height / 4");
    assert_eq!(modal.content_y, 9, "panel has one inner top-padding row");
    assert_eq!(modal.content_width, 56, "panel content is inset by two cells");
    assert_eq!(modal.backdrop_alpha, 150);

    let frame = view.render("draft", &[], false, &[]);
    assert_eq!(frame.len(), 32);
    assert!(
        frame[usize::from(modal.y)].trim().is_empty(),
        "the panel's top padding is an empty rendered row"
    );
    let heading_row = &frame[usize::from(modal.content_y)];
    assert_eq!(
        heading_row.find(title),
        Some(usize::from(modal.x) + 2),
        "heading begins at the panel's two-cell content inset"
    );
}

#[test]
fn dialog_sizes_match_upstream_and_cap_to_terminal_minus_two() {
    let mut view = NativeView::new(200, 40);
    for (size, expected) in [
        (DialogSize::Medium, 60),
        (DialogSize::Large, 88),
        (DialogSize::XLarge, 116),
    ] {
        view.set_dialog_size(size);
        let modal = view.modal_presentation();
        assert_eq!(modal.width, expected, "dialog width for {size:?}");
        assert_eq!(modal.x, (200 - expected) / 2);
        assert_eq!(modal.content_width, expected - 4);
    }

    // The xlarge panel is still capped when the terminal is narrower than its
    // configured width.  This also checks that the centered x calculation
    // remains valid at the cap boundary.
    view.resize(64, 20);
    view.set_dialog_size(DialogSize::XLarge);
    let modal = view.modal_presentation();
    assert_eq!(modal.width, 62);
    assert_eq!(modal.x, 1);
    assert_eq!(modal.content_width, 58);
}

#[test]
fn home_prompt_width_modes_follow_config_and_appear_in_real_rows() {
    let mut view = NativeView::new(160, 24);
    view.set_session_route(false);

    assert_eq!(view.home_prompt_width(), 75, "default Home max width");
    view.set_prompt_width(PromptWidth::Auto);
    assert_eq!(view.home_prompt_width(), 112, "floor(160 * .7) wins over 75");
    view.set_prompt_width(PromptWidth::Fixed(200));
    assert_eq!(view.home_prompt_width(), 156, "Home padding leaves cols - 4");

    // Use a fixed value so the externally returned row has a deterministic
    // content start: outer prompt left + the view's two-cell inner padding.
    view.set_prompt_width(PromptWidth::Fixed(30));
    let frame = view.render("home draft", &[], false, &[]);
    let row = frame
        .iter()
        .find(|line| line.contains("home draft"))
        .expect("Home draft is present in an actual rendered row");
    assert_eq!(row.find("home draft"), Some(65 + 2));

    view.resize(40, 24);
    view.set_prompt_width(PromptWidth::Auto);
    assert_eq!(view.home_prompt_width(), 36, "auto width is constrained by cols - 4");
}

#[test]
fn sidebar_dim_matches_narrow_overlay_and_wide_inline_layout() {
    let mut narrow = NativeView::new(120, 24);
    narrow.set_session_route(true);
    narrow.toggle_sidebar();
    assert_eq!(narrow.sidebar_backdrop_alpha(), 70);
    let narrow_frame = narrow.render("draft", &[], false, &[]);
    assert_eq!(narrow_frame.len(), 24);
    assert!(narrow_frame.iter().all(|row| row.chars().count() == 120));

    let mut wide = NativeView::new(121, 24);
    assert_eq!(wide.presentation().sidebar_width, 42);
    assert_eq!(wide.presentation().sidebar_offset, Some(79));
    assert_eq!(wide.sidebar_backdrop_alpha(), 0, "wide sidebar is inline, not dimmed");
    let wide_frame = wide.render("draft", &[], false, &[]);
    assert_eq!(wide_frame.len(), 24);
    assert!(wide_frame.iter().all(|row| row.chars().count() == 121));
}

#[test]
fn extreme_and_tiny_viewports_render_without_panic_and_stay_bounded() {
    let mut huge = NativeView::new(u16::MAX, 256);
    let huge_frame = huge.render("draft", &[], false, &["overlay".to_owned()]);
    let row_bound = (1usize << 20) / usize::from(u16::MAX);
    assert!(!huge_frame.is_empty(), "the bounded huge viewport still paints");
    assert!(huge_frame.len() <= row_bound);
    assert!(huge_frame.len() <= 256);
    assert_eq!(
        huge_frame.first().and_then(|row| row.chars().next()),
        Some(' '),
        "the first cell remains empty rather than receiving overlay content"
    );
    for (index, row) in huge_frame.iter().enumerate() {
        assert!(
            row.chars().count() <= usize::from(u16::MAX),
            "huge row {index} exceeds the terminal column bound"
        );
        assert!(
            row.len() <= usize::from(u16::MAX) * 4,
            "huge row {index} exceeds the UTF-8 byte bound"
        );
    }

    for (cols, rows) in [(0u16, 0u16), (0, 1), (1, 0), (1, 1)] {
        let mut view = NativeView::new(cols, rows);
        let frame = view.render("draft", &[], false, &["overlay".to_owned()]);
        assert!(
            frame.len() <= usize::from(rows),
            "{cols}x{rows} must never emit more rows than requested"
        );
    }
}

#[test]
fn home_route_preserves_workspace_and_model_identity_without_missing_agent_claim() {
    let mut view = NativeView::new(100, 24);
    view.set_session_route(false);
    view.set_workspace("/real/workspace".to_owned());
    view.set_model(Some("provider/model".to_owned()));

    let frame = view.render("", &[], false, &[]);
    let rendered = frame.join("\n");
    assert!(rendered.contains("/real/workspace"), "Home footer keeps workspace identity");
    assert!(
        rendered.contains(env!("CARGO_PKG_VERSION")),
        "Home footer keeps the upstream version component"
    );
    assert!(rendered.contains("provider/model"), "Home prompt keeps model identity");
    assert!(
        !rendered.to_ascii_lowercase().contains("agent"),
        "Home must not claim a missing agent when no agent fact was observed"
    );
}
