//! Measure the real loader in an isolated process, rather than retaining an
//! oversized instruction file before applying its advertised byte limit.
#[cfg(target_os = "macos")]
#[test]
fn oversized_instruction_read_has_bounded_peak_allocation() {
    const CHILD_WORKSPACE: &str = "OPENCODE_TEST_RULES_ALLOCATION_WORKSPACE";
    if let Some(workspace) = std::env::var_os(CHILD_WORKSPACE) {
        let snapshot = opencode_rk_server::rules_loader::load_rules(std::path::Path::new(&workspace)).unwrap();
        assert!(snapshot.entries.is_empty(), "oversized instructions must remain excluded");
        return;
    }
    let workspace = tempfile::tempdir().unwrap();
    // Sparse, disposable fixture: reading it eagerly still touches 128 MiB of
    // resident memory, while a bounded reader only reads MAX_FILE_BYTES + 1.
    std::fs::File::create(workspace.path().join("AGENTS.md")).unwrap()
        .set_len(128 * 1024 * 1024).unwrap();
    let output = std::process::Command::new("/usr/bin/time")
        .args(["-l"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "oversized_instruction_read_has_bounded_peak_allocation", "--nocapture"])
        .env(CHILD_WORKSPACE, workspace.path())
        .output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let timing = String::from_utf8(output.stderr).unwrap();
    let resident_bytes: u64 = timing.lines().find_map(|line| {
        line.contains("maximum resident set size").then(|| line.split_whitespace().next().unwrap().parse().unwrap())
    }).expect("macOS time must report child peak resident bytes");
    // A fresh fixture process has ample baseline headroom here; a full-file
    // 128 MiB allocation cannot pass. No daemon or user's database is opened.
    assert!(resident_bytes < 96 * 1024 * 1024,
        "instruction loading exceeded the bounded-read memory budget: {resident_bytes} bytes");
}
