// Thin cargo wrapper around the existing Python parity harness
// (tools/ui_parity/ui_parity.py). This file is NOT a second implementation:
// rendering, goldens and cell comparison stay in the Python harness exactly
// as described in crates/cli/tests/parity/README.md.

use std::path::Path;
use std::process::Command;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/cli must sit two levels below the repository root")
}

// Proves the comparator and recording pipeline themselves.
#[test]
fn ui_parity_comparator_self_test() {
    let status = Command::new("python3")
        .arg("tools/ui_parity/selftest.py")
        .current_dir(repo_root())
        .status()
        .expect("python3 must be available for the parity harness");
    assert!(status.success(), "ui parity self-test failed");
}

// Real comparison: re-renders every recorded par-static fixture with the
// binary cargo just built and diffs every styled cell against the recorded
// golden. Fails on any cell mismatch.
//
// rider: ponytail — --allow-pin-drift exists because `cargo test` links a debug
// binary while goldens are recorded from the release binary, so product
// version/sha256 pins drift by profile; cells, theme, viewport and class
// comparisons stay strict. Add when the test profile equals the record profile.
#[test]
fn ui_parity_goldens_match_rendered_cells() {
    let output = Command::new("python3")
        .args([
            "tools/ui_parity/ui_parity.py",
            "--check",
            "--filter",
            "par-static-",
            "--bin",
            env!("CARGO_BIN_EXE_oc2"),
            "--allow-pin-drift",
        ])
        .current_dir(repo_root())
        .output()
        .expect("python3 must be available for the parity harness");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "ui parity check failed ({status}):\n{stdout}\n{stderr}",
        status = output.status,
        stdout = stdout,
        stderr = stderr,
    );
    // Guard against a vacuous green: the run must actually have compared the
    // goldens ("summary: ... checked=N ..."), not skipped every fixture.
    let summary = stdout
        .lines()
        .find(|line| line.starts_with("summary: "))
        .unwrap_or_else(|| panic!("harness produced no summary line:\n{stdout}"));
    let checked: u32 = summary
        .split_whitespace()
        .find_map(|item| item.strip_prefix("checked="))
        .and_then(|value| value.parse().ok())
        .expect("harness summary must report checked=N");
    assert!(checked > 0, "harness checked no fixtures:\n{stdout}");
}

// Item 13 pinned-OpenCode comparison. Six par-static goldens were stale: their
// composer metadata row still carried the `unset` placeholder, while the
// renderer paints the upstream empty state. The row is not a local invention --
// pinned upstream OpenCode 450e7b92412c780a7b5a75226bb8d49aaca24e22 renders
// exactly `No provider selected` + `Connect a provider` when no model selection
// exists:
//
//   packages/tui/src/context/local.tsx:398-417
//     parsed: createMemo(() => {
//       const value = currentSelection()
//       if (!value) return { provider: "Connect a provider",
//                            model: "No provider selected", reasoning: false }
//       ...
//   packages/tui/src/component/prompt/metadata.tsx:100-175
//     text(): [...model, ...provider].join(" ")
//             -> "No provider selected Connect a provider"
//
// So the renderer is correct and the goldens -- not the renderer -- were stale.
// The six files were re-recorded with the repository's own recorder
// (`tools/ui_parity/ui_parity.py --record --filter par-static-`); this test
// pins the recorded row to the upstream contract so a future renderer
// regression fails here as well as in `ui_parity_goldens_match_rendered_cells`.
#[test]
fn ui_parity_composer_metadata_matches_pinned_upstream() {
    const COMPOSER_ROW: &str = "No provider selected Connect a provider";
    // Exactly the six par-static fixtures that render the composer metadata row.
    let fixtures = [
        "par-static-dialog-model-120x40-dark",
        "par-static-home-100x30-dark",
        "par-static-home-60x20-dark",
        "par-static-home-80x10-dark",
        "par-static-home-80x24-dark",
        "par-static-home-80x24-light",
    ];
    for id in fixtures {
        let path = repo_root()
            .join("tests/fixtures/ui-parity/goldens")
            .join(format!("{id}.golden.json"));
        let golden = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        assert!(
            golden.contains(COMPOSER_ROW),
            "{id}: composer metadata row must match pinned upstream \
             `{COMPOSER_ROW}` (context/local.tsx:398-417 + prompt/metadata.tsx:100-175)"
        );
        assert!(
            !golden.contains("┃  unset"),
            "{id}: the stale `unset` placeholder must not remain in the recorded golden"
        );
    }
}
