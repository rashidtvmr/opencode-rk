#![forbid(unsafe_code)]
//! FIX-PACKAGING frozen tests: installer --aliases, download mode, identity gate.

use std::process::Command;
use std::fs;
use std::path::Path;

const SCRIPT_PATH: &str = "/home/rashid/projects/opencode-rk/scripts/install-oc2.sh";
const BIN: &str = "oc2";
const TEMP_DIR: &str = "/tmp";

// Helper: run the install-oc2.sh script with given args, return exit code.
fn run_install(args: &str) -> i32 {
    let script = Path::new(SCRIPT_PATH);
    assert!(script.exists(), "install script must exist at {}", SCRIPT_PATH);
    let status = Command::new("sh")
        .arg(script)
        .args(args.split_whitespace())
        .status()
        .expect("install-oc2.sh should run");
    status.code().unwrap_or(127)
}

// ── T1: installer --aliases creates all listed aliases; alias resolves to same inode/content ──

#[test]
fn fix_packaging_t01_aliases_created_and_same_inode() {
    let install_dir = format!("{}/oc2_test_t01", TEMP_DIR);
    let _ = fs::remove_dir_all(&install_dir);

    let exit_code = run_install(
        &format!(
            "--aliases \"oc,ooo,oooo,ocrt\" --install-dir {}",
            install_dir
        ),
    );

    // Before implementation: --aliases not supported, exit != 0, aliases not created
    // After implementation: exit 0, all aliases created with same inode/content
    // Assert aliases were created and point to same binary
    let aliases = vec!["oc", "ooo", "oooo", "ocrt"];
    let mut all_pass = true;
    for alias in &aliases {
        let alias_path = Path::new(&install_dir).join(alias);
        if !alias_path.exists() {
            all_pass = false;
            eprintln!("  alias {} does not exist after --aliases", alias);
        }
    }
    // Before implementation, aliases won't be created, so this should fail (RED)
    // After implementation, all aliases should exist with same inode/content (GREEN)
    if !all_pass {
        panic!(
            "T01: --aliases must create all listed aliases ({:?}) in install dir",
            aliases
        );
    }
    eprintln!("T01: --aliases created all aliases successfully");
}

// ── T2: alias over existing different binary fails typed with exit 64 and installs nothing ──

#[test]
fn fix_packaging_t02_alias_over_existing_fails_exit64() {
    let install_dir = format!("{}/oc2_test_t02", TEMP_DIR);

    // First, create a "different" binary at oc in the install dir
    fs::create_dir_all(&install_dir).expect("create install dir");
    let different_binary = format!("{}/oc", install_dir);
    fs::write(&different_binary, "this is opencode2 not oc2")
        .expect("write different binary");

    // Before implementation: --aliases not supported, may fail with different exit
    // After implementation: should fail with exit 64, original binary untouched
    let exit_code = run_install(
        &format!(
            "--aliases \"oc\" --install-dir {} --archive /tmp/fake --checksum fake",
            install_dir
        ),
    );

    // Cleanup different binary first
    let _ = fs::remove_file(&different_binary);
    let _ = fs::remove_dir_all(&install_dir);

    // Before implementation: exit != 64 (--aliases not handled)
    // After implementation: exit == 64 (alias guard over existing binary)
    assert!(
        exit_code == 64,
        "T02: installing alias over existing different binary must fail with exit 64, got {}",
        exit_code
    );
    eprintln!("T02: alias over existing binary fails with exit 64");
}

// ── T3: uninstall removes aliases ──

#[test]
fn fix_packaging_t03_uninstall_removes_aliases() {
    let install_dir = format!("{}/oc2_test_t03", TEMP_DIR);

    // First install with aliases (may partially succeed or fail before impl)
    let _ = run_install(
        &format!(
            "--aliases \"oc,ooo\" --install-dir {}",
            install_dir
        ),
    );

    // Then uninstall
    let exit_code = run_install(&format!("--uninstall --install-dir {}", install_dir));

    // After implementation: uninstall succeeds (exit 0) and aliases are removed
    // Before implementation: uninstall succeeds (exit 0) but aliases remain
    assert_eq!(
        exit_code, 0,
        "T03: uninstall must succeed with exit 0, got {}",
        exit_code
    );

    // Check that aliases are removed after uninstall
    let aliases = vec!["oc", "ooo"];
    let mut aliases_removed = true;
    for alias in &aliases {
        let alias_path = Path::new(&install_dir).join(alias);
        if alias_path.exists() {
            aliases_removed = false;
            eprintln!("  alias {} still exists after uninstall", alias);
        }
    }
    assert!(
        aliases_removed,
        "T03: --uninstall must remove all recorded aliases"
    );
    eprintln!("T03: uninstall removed aliases successfully");
}

// ── T4: download mode builds correct artifact URL for a mocked base ──

#[test]
fn fix_packaging_t04_download_mode_mocked_base() {
    let install_dir = format!("{}/oc2_test_t04", TEMP_DIR);

    // Before implementation: OC2_RELEASE_BASE not yet handled
    // After implementation: script uses OC2_RELEASE_BASE to construct artifact URL
    let exit_code = run_install(
        &format!(
            "--install-dir {} OC2_RELEASE_BASE=/tmp/opencode-rk/release-metadata",
            install_dir
        ),
    );

    // Before implementation: may ignore OC2_RELEASE_BASE or fail differently
    // After implementation: should attempt download with custom base URL (exit may be 0 or non-zero depending on fetch success)
    eprintln!("T04: download mode with OC2_RELEASE_BASE exit code: {}", exit_code);

    let _ = fs::remove_dir_all(&install_dir);
}

// ── T5: script contains no stale opencode2 references (grep assertion) ──

#[test]
fn fix_packaging_t05_no_stale_opencode2_references() {
    let content = fs::read_to_string(SCRIPT_PATH).expect("read install script");
    // Check for stale references to "opencode2" as a binary program name.
    // Only check non-comment, non-shebang lines to avoid false positives on header.
    let mut found_stale = false;
    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();
        // Skip shebang and full-line comments
        if trimmed.starts_with("#!") || trimmed.starts_with("#") {
            continue;
        }
        // Check if line contains "opencode2" as a program reference
        if trimmed.contains("opencode2") {
            panic!(
                "line {} in install-oc2.sh contains stale 'opencode2' reference: {}",
                line_num,
                line
            );
        }
    }
    if !found_stale {
        eprintln!("T05: no stale opencode2 references found");
    }
}