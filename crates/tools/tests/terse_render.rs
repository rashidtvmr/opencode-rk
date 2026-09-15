#[path = "../src/terse_render.rs"]
mod terse_render;

use terse_render::{Intensity, TerseRenderer, MAX_INPUT_BYTES};

// TOOL-022-T01 (lite compresses): Lite strips filler, keeps content.
#[test]
fn tool_022_t01_lite_compresses() {
    let r = TerseRenderer::new();
    assert_eq!(r.intensity(), Intensity::Full);
    let mut lite = TerseRenderer::new();
    lite.set_intensity(Intensity::Lite);
    let input = "Sure! I'd be happy to help. Bug in auth middleware.";
    let out = lite.render(input);
    assert!(
        out.contains("Bug in auth middleware"),
        "lite keeps content, got {out:?}"
    );
    assert!(!out.contains("Sure!"), "lite strips filler, got {out:?}");
    assert!(
        !out.contains("I'd be happy"),
        "lite strips status phrase, got {out:?}"
    );
    assert!(out.len() <= input.len(), "output<=input");
    assert_eq!(r.render("x"), "x", "default Full keeps short input");
}

// TOOL-022-T02 (ultra minimal): ultra words < full words, len < input.
#[test]
fn tool_022_t02_ultra_minimal() {
    let input = "Token expiry check uses wrong operator";
    let snapshot = input.to_string();
    let mut full = TerseRenderer::new();
    full.set_intensity(Intensity::Full);
    let mut ultra = TerseRenderer::new();
    ultra.set_intensity(Intensity::Ultra);
    let full_out = full.render(input);
    let ultra_out = ultra.render(input);
    let full_wc = full_out.split_whitespace().count();
    let ultra_wc = ultra_out.split_whitespace().count();
    assert!(
        ultra_wc < full_wc,
        "ultra ({ultra_wc}) < full ({full_wc}): {ultra_out:?} vs {full_out:?}"
    );
    assert!(
        ultra_out.len() < input.len(),
        "ultra shorter than input, got {ultra_out:?}"
    );
    assert!(full_out.len() <= input.len(), "full output<=input");
    assert_eq!(input, snapshot, "renderer never mutates caller data");
}

// TOOL-022-T03 (identifiers verbatim): all levels keep code/paths/errors exact.
#[test]
fn tool_022_t03_identifiers_verbatim() {
    let input = "Fix `OutputStore::new` at crates/tools/src/output_store.rs:10 fails with E0599";
    for intensity in [Intensity::Lite, Intensity::Full, Intensity::Ultra] {
        let mut r = TerseRenderer::new();
        r.set_intensity(intensity);
        let out = r.render(input);
        assert!(
            out.contains("`OutputStore::new`"),
            "{intensity:?} keeps code span, got {out:?}"
        );
        assert!(
            out.contains("crates/tools/src/output_store.rs:10"),
            "{intensity:?} keeps path:line, got {out:?}"
        );
        assert!(
            out.contains("E0599"),
            "{intensity:?} keeps error code, got {out:?}"
        );
        assert!(out.len() <= input.len(), "{intensity:?} output<=input");
    }
}

// TOOL-022-T04 (numbers/paths exact + empty defined).
#[test]
fn tool_022_t04_numbers_paths_empty() {
    let r = TerseRenderer::new();
    assert_eq!(r.render(""), "", "empty input defined");
    let input = "retry after 24h, see SUBAGENT_COOLDOWN.md";
    let out = r.render(input);
    assert!(out.contains("24h"), "number exact, got {out:?}");
    assert!(
        out.contains("SUBAGENT_COOLDOWN.md"),
        "path exact, got {out:?}"
    );
    assert!(out.len() <= input.len(), "output<=input");
}

// TOOL-022-T05 (switching + opt-out + bypass/oversize negatives).
#[test]
fn tool_022_t05_switching_optout_bypass() {
    let input = "Sure! Of course! I'd be happy to help. Token expiry check uses wrong operator and more context here.";
    let mut r = TerseRenderer::new();
    r.set_intensity(Intensity::Ultra);
    let ultra_out = r.render(input);
    r.set_intensity(Intensity::Lite);
    let lite_out = r.render(input);
    assert_ne!(
        ultra_out, lite_out,
        "level switch changes output: {ultra_out:?} vs {lite_out:?}"
    );
    r.set_enabled(false);
    assert_eq!(r.render(input), input, "opt-out passthrough exact");
    assert_eq!(r.render(""), "", "disabled empty stays empty");
    // Bypass list renders verbatim (fail-closed presenter guard).
    r.set_enabled(true);
    let sec = "SECURITY: rotate the token now";
    assert_eq!(r.render(sec), sec, "SECURITY verbatim");
    let conf = "CONFIRM: drop database";
    assert_eq!(r.render(conf), conf, "CONFIRM verbatim");
    let seq = "1. stop\n2. snapshot\n3. restart";
    assert_eq!(r.render(seq), seq, "ordered sequence verbatim");
    let big = "a".repeat(MAX_INPUT_BYTES + 1);
    assert_eq!(r.render(&big), big, "oversize passthrough");
}
