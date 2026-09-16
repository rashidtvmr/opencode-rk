//! TOOL-023 auto-clarity guard RED suite (mirrors TOOL-022-T01..T05 / CV-CLR-T01..T05).
//! Compiles against crate modules; initially fails (no `clarity_guard` module).
#![forbid(unsafe_code)]

#[path = "../src/clarity_guard.rs"]
mod clarity_guard;

use clarity_guard::{select_mode, Mode, RenderCtx};

fn flagged_ctx(security: bool, irreversible: bool, ordered_steps: bool, repeat: bool) -> RenderCtx {
    RenderCtx::new(security, irreversible, ordered_steps, repeat, "en")
}

// TOOL-023-T01: security renders full.
#[test]
fn tool_023_t01_security_renders_full() {
    let ctx = flagged_ctx(true, false, false, false);
    let body = "Token expiry uses < not <=";
    assert_eq!(select_mode(&ctx), Mode::Full);
    let out = clarity_guard::render(&ctx, body);
    assert!(out.contains("Token expiry"), "missing subject: {out:?}");
    assert!(
        out.matches('.').count() >= 2,
        "need two complete sentences: {out:?}"
    );
    assert_no_banned(&out);
}

// TOOL-023-T02: irreversible requires confirmation.
#[test]
fn tool_023_t02_irreversible_requires_confirmation() {
    let ctx = flagged_ctx(false, true, false, false);
    let body = "drop database";
    assert_eq!(select_mode(&ctx), Mode::Full);
    let out = clarity_guard::render(&ctx, body);
    assert!(
        out.to_ascii_lowercase().contains("confirm"),
        "missing confirm copy: {out:?}"
    );
    assert!(out.contains("database"), "missing action noun: {out:?}");
    for line in out.lines().filter(|l| !l.trim().is_empty()) {
        assert!(has_verb(line), "terse-only shortening, no verb: {line:?}");
    }
    assert_no_banned(&out);
}

// TOOL-023-T03: ordered sequence numbered.
#[test]
fn tool_023_t03_ordered_sequence_numbered() {
    let ctx = flagged_ctx(false, false, true, false);
    let body = "check status\napply patch\nrun tests";
    assert_eq!(select_mode(&ctx), Mode::Full);
    let out = clarity_guard::render(&ctx, body);
    let p1 = out.find("1.").expect("missing 1.");
    let p2 = out.find("2.").expect("missing 2.");
    let p3 = out.find("3.").expect("missing 3.");
    assert!(p1 < p2 && p2 < p3, "numbered lines out of order: {out:?}");
    assert_no_banned(&out);
}

// TOOL-023-T04: repeat triggers full, releases when cleared.
#[test]
fn tool_023_t04_repeat_triggers_full() {
    let body = "routine status line";
    let repeat = flagged_ctx(false, false, false, true);
    assert_eq!(select_mode(&repeat), Mode::Full);
    let clear = flagged_ctx(false, false, false, false);
    assert_eq!(select_mode(&clear), Mode::Terse);
    let _ = body;
}

// TOOL-023-T05: language preserved, no self-reference.
#[test]
fn tool_023_t05_language_preserved_no_self_reference() {
    let mut ctx = flagged_ctx(false, false, false, true);
    ctx.lang = clarity_guard::LangTag::new("vi");
    let out = clarity_guard::render(&ctx, "xoa database");
    assert!(out.contains("xac nhan"), "missing VI confirm: {out:?}");
    assert!(
        !out.contains("Please confirm"),
        "English fallback leaked: {out:?}"
    );
    assert_no_banned(&out);
}

fn assert_no_banned(out: &str) {
    for banned in clarity_guard::BANNED.iter() {
        assert!(
            !out.to_ascii_lowercase().contains(banned),
            "banned self-reference {banned:?} in {out:?}"
        );
    }
}

fn has_verb(line: &str) -> bool {
    const VERBS: &[&str] = &[
        "is",
        "are",
        "uses",
        "use",
        "requires",
        "require",
        "confirms",
        "confirm",
        "run",
        "runs",
        "check",
        "checks",
        "apply",
        "applies",
        "complete",
        "completes",
        "xac",
        "hay",
        "vui",
    ];
    let lower = line.to_ascii_lowercase();
    VERBS
        .iter()
        .any(|v| lower.split(|c: char| !c.is_alphanumeric()).any(|w| w == *v))
}
