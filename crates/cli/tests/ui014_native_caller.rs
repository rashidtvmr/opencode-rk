#![forbid(unsafe_code)]
//! UI-014 semantic RED at the real native caller boundary.
//!
//! This is deliberately not a leaf test of `native_composer.rs`. It inspects
//! the checked-in caller selected by `tui_entry::run_with_dir` and fails until
//! that caller routes terminal events through the real native composer.

const MAIN_RS: &str = include_str!("../src/main.rs");
const TUI_ENTRY_RS: &str = include_str!("../src/tui_entry.rs");

fn function_body<'a>(source: &'a str, name: &str) -> &'a str {
    let marker = format!("fn {name}(");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("missing native caller function {name}"));
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .expect("native caller function body");
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    for (offset, byte) in bytes.iter().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[start..=offset];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated native caller function {name}");
}

fn block_after<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("missing caller boundary marker {marker}"));
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .expect("caller boundary block");
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    for (offset, byte) in bytes.iter().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[start..=offset];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated caller boundary block {marker}");
}

fn native_loop() -> &'static str {
    function_body(TUI_ENTRY_RS, "native_interactive_loop")
}

fn require_any(source: &str, alternatives: &[&str], contract: &str) {
    assert!(
        alternatives.iter().any(|needle| source.contains(needle)),
        "native caller missing {contract}; expected one of {alternatives:?}"
    );
}

#[test]
fn t01_no_subcommand_reaches_real_native_tui_caller() {
    let run = function_body(MAIN_RS, "run");
    let no_subcommand = block_after(run, "None => {");
    assert!(
        no_subcommand.contains("tui_entry::run_with_dir(args, Some(&data))?"),
        "no-subcommand native launch must reach tui_entry::run_with_dir"
    );
    assert!(
        TUI_ENTRY_RS.contains("native_interactive_loop("),
        "tui_entry must expose the native loop selected by run_with_dir"
    );
}

#[test]
fn t02_native_caller_routes_enter_through_composer_submission() {
    let loop_body = native_loop();
    require_any(
        loop_body,
        &["native_composer::Composer", "crate::native_composer::Composer"],
        "the real native_composer type",
    );
    require_any(
        loop_body,
        &["Composer::with_keymap", "Composer::new"],
        "composer construction in native_interactive_loop",
    );
    require_any(
        loop_body,
        &["handle_key(Key::Enter)", "handle_key(crate::native_composer::Key::Enter)"],
        "Enter dispatch through Composer::handle_key",
    );
}

#[test]
fn t03_native_caller_routes_shift_enter_as_multiline_edit() {
    let loop_body = native_loop();
    require_any(
        loop_body,
        &[
            "Key::ShiftEnter",
            "crate::native_composer::Key::ShiftEnter",
            "ComposerKey::ShiftEnter",
        ],
        "Shift+Enter event decoding",
    );
    require_any(
        loop_body,
        &["handle_key", "decide_key"],
        "Shift+Enter through the composer key router",
    );
}

#[test]
fn t04_native_caller_routes_ctrl_j_with_configured_keymap() {
    let loop_body = native_loop();
    require_any(
        loop_body,
        &[
            "Key::CtrlJ",
            "crate::native_composer::Key::CtrlJ",
            "ComposerKey::CtrlJ",
        ],
        "Ctrl+J event decoding",
    );
    require_any(
        loop_body,
        &["set_keymap", "with_keymap", "resolve_keymap"],
        "configured submit keymap at the native caller boundary",
    );
}

#[test]
fn t05_native_caller_preserves_interrupt_and_bounded_busy_queue() {
    let loop_body = native_loop();
    require_any(
        loop_body,
        &["interrupt()", "Composer::interrupt"],
        "interrupt routed to Composer",
    );
    require_any(
        loop_body,
        &["finish_turn()", "Composer::finish_turn"],
        "turn completion routed to Composer",
    );
    require_any(
        loop_body,
        &["queue_len()", "queued()", "KeyHandled::Queued", "SubmitOutcome::Queued"],
        "bounded busy-queue state surfaced by the native caller",
    );
}
