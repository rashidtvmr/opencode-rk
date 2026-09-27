//! APP-001-FILE0600-RED: installed first-run setup Atomic File0600 contract.
//!
//! Source evidence at base 798cbff99f4e2a2ad21cc937df428eb6bd871cca:
//! - `crates/cli/src/main.rs:256-262` routes a missing/unknown provider credential
//!   to `run_setup` and states no credentials are written to disk.
//! - `crates/cli/src/main.rs:359-371` documents the disposable in-memory
//!   `MemoryAccountStore`; `data` is accepted but unused for persistence.
//! - `crates/cli/src/main.rs:387-449` collects provider, key and model via stdin
//!   and completes the account only in memory.
//!
//! Approved new persistence contract (user-approved stable Phase 1):
//! - credential directory is exactly `<data_dir>/credentials`, mode 0700;
//! - provider file is exactly `<data_dir>/credentials/<provider>.json`, mode 0600;
//! - the file is plaintext JSON and MAY contain provider, model and the credential
//!   (no encryption and no secure-erase claim is made);
//! - the raw credential must never appear on the PTY transcript, in argv, in a
//!   URL, or in any file other than the exact protected provider file;
//! - commit is atomic: no `.tmp` (or equivalent) artifact survives success;
//! - a partial/cancelled setup creates no credential file;
//! - a pre-created symlink at the exact provider path is not followed and the
//!   setup fails without replacing the symlink or writing through it;
//! - an existing valid provider file is not replaced by invalid/partial input.
//!
//! This test invokes the exact installed binary named by `OC2_E2E_BIN` through
//! macOS `script` to obtain a real PTY. Std-only: no external crates. All
//! filesystem and output bounds are confined to a fresh disposable data
//! directory and a capped capture.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Raw credential value; must never be observable outside the protected file.
const KEY: &str = "rk-e2e-secret-never-log-0600";
const PROVIDER: &str = "openai";
const MODEL: &str = "openai/gpt-5.6";

fn root(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("oc2-file0600-{}-{tag}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).expect("create disposable test root");
    p
}

fn cred_dir(data: &Path) -> PathBuf {
    data.join("credentials")
}

fn cred_file(data: &Path, provider: &str) -> PathBuf {
    cred_dir(data).join(format!("{provider}.json"))
}

/// Run the exact installed binary under a real PTY with `input` on stdin.
/// Bounded to 20s and 64KiB; the child is killed and reaped on timeout.
fn run_setup(data: &Path, input: &str) -> (i32, String) {
    let bin = std::env::var_os("OC2_E2E_BIN")
        .expect("OC2_E2E_BIN must name the exact installed binary");
    let mut child = Command::new("/usr/bin/script")
        .args(["-q", "/dev/null"])
        .arg(&bin)
        .arg("--data-dir")
        .arg(data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn installed binary through PTY");
    {
        let mut stdin = child.stdin.take().expect("PTY input handle");
        stdin.write_all(input.as_bytes()).expect("send setup input");
    } // dropping the handle closes stdin and signals EOF

    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().expect("poll child") {
            let mut output = child.wait_with_output().expect("collect bounded setup output");
            let mut bytes = std::mem::take(&mut output.stdout);
            bytes.extend_from_slice(&output.stderr);
            bytes.truncate(64 * 1024);
            return (status.code().unwrap_or(128), String::from_utf8_lossy(&bytes).into_owned());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("setup timed out after 20s; child killed and reaped");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// All regular files beneath `dir` (bounded walk); never follows a symlink into
/// another tree because `file_type()` does not traverse links.
fn all_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(rd) = fs::read_dir(&p) else { continue };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file() {
                found.push(entry.path());
            }
        }
    }
    found
}

#[cfg(unix)]
fn mode(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::symlink_metadata(p).expect("metadata").permissions().mode() & 0o777
}

fn complete_input() -> String {
    format!("{PROVIDER}\n{KEY}\n{MODEL}\n")
}

#[test]
fn t01_successful_setup_writes_exact_protected_file_atomically() {
    let home = root("t01");
    let data = home.join("data");
    fs::create_dir_all(&data).unwrap();

    let (status, output) = run_setup(&data, &complete_input());
    assert_eq!(status, 0, "completed setup must succeed; got {status}: {output}");

    // The real PTY echoes our typed stdin once, so the raw key appears at most
    // once as that echo. The program itself must never re-print it.
    let key_occurrences = output.matches(KEY).count();
    assert!(
        key_occurrences <= 1,
        "program re-emitted the credential to PTY output ({key_occurrences} occurrences)"
    );

    let dir = cred_dir(&data);
    assert!(dir.is_dir(), "credential directory <data_dir>/credentials missing");
    let file = cred_file(&data, PROVIDER);
    assert!(
        file.is_file(),
        "exact credential file <data_dir>/credentials/{PROVIDER}.json missing"
    );
    let meta = fs::symlink_metadata(&file).unwrap();
    assert!(meta.file_type().is_file(), "credential path must be a regular file");

    // Plaintext JSON may carry provider, model and the credential here.
    let body = fs::read_to_string(&file).unwrap();
    assert!(body.contains(PROVIDER), "provider id absent from credential file: {body}");
    assert!(body.contains(MODEL), "model id absent from credential file: {body}");
    assert!(body.contains(KEY), "credential absent from protected credential file");

    // Atomic commit leaves no temporary artifact behind.
    for entry in fs::read_dir(&dir).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            !name.ends_with(".tmp") && !name.contains(".tmp."),
            "atomic write left a temp artifact: {name}"
        );
    }

    // The raw credential exists nowhere except the exact protected file.
    for path in all_files(&data) {
        if path == file {
            continue;
        }
        let bytes = fs::read(&path).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains(KEY), "credential leaked into {}", path.display());
    }

    fs::remove_dir_all(home).unwrap();
}

#[cfg(unix)]
#[test]
fn t02_credential_directory_and_file_modes_are_restrictive() {
    let home = root("t02");
    let data = home.join("data");
    fs::create_dir_all(&data).unwrap();

    let (status, output) = run_setup(&data, &complete_input());
    assert_eq!(status, 0, "completed setup must succeed: {output}");

    let dir = cred_dir(&data);
    assert!(dir.is_dir(), "credential directory missing");
    assert_eq!(mode(&dir), 0o700, "credential directory must be mode 0700");

    let file = cred_file(&data, PROVIDER);
    assert!(file.is_file(), "credential file missing");
    assert_eq!(mode(&file), 0o600, "credential file must be mode 0600");

    fs::remove_dir_all(home).unwrap();
}

#[test]
fn t03_partial_setup_creates_no_credential_file() {
    let home = root("t03");
    let data = home.join("data");
    fs::create_dir_all(&data).unwrap();

    // Provider only, then EOF before the credential/model steps complete.
    let (_status, output) = run_setup(&data, &format!("{PROVIDER}\n"));

    assert!(
        !cred_file(&data, PROVIDER).exists(),
        "partial setup created a credential file: {output}"
    );
    // Nothing anywhere in the disposable tree may hold the raw credential.
    for path in all_files(&data) {
        let bytes = fs::read(&path).unwrap_or_default();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(KEY),
            "partial setup leaked credential into {}",
            path.display()
        );
    }

    fs::remove_dir_all(home).unwrap();
}

#[cfg(unix)]
#[test]
fn t04_precreated_symlink_is_not_followed_and_setup_fails() {
    let home = root("t04");
    let data = home.join("data");
    let outside = home.join("outside.json");
    fs::create_dir_all(&data).unwrap();
    fs::write(&outside, b"ORIGINAL-OUTSIDE-CONTENT").unwrap();

    let dir = cred_dir(&data);
    fs::create_dir_all(&dir).unwrap();
    let link = cred_file(&data, PROVIDER);
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    let (status, output) = run_setup(&data, &complete_input());

    // The outside target must remain byte-identical: no write through the link.
    assert_eq!(
        fs::read(&outside).unwrap(),
        b"ORIGINAL-OUTSIDE-CONTENT",
        "setup wrote through the pre-created symlink into the outside target"
    );
    assert!(
        !String::from_utf8_lossy(&fs::read(&outside).unwrap()).contains(KEY),
        "credential leaked through symlink to the outside file"
    );

    // The exact path must not be silently replaced by a regular secret file.
    let meta = fs::symlink_metadata(&link).expect("credential path metadata");
    assert!(
        meta.file_type().is_symlink(),
        "setup replaced the pre-created symlink with a regular file"
    );

    // Contract: setup fails rather than silently ignoring the rejection.
    assert_ne!(status, 0, "setup must fail on a pre-created symlink: {output}");

    fs::remove_dir_all(home).unwrap();
}

#[test]
fn t05_existing_valid_file_is_not_replaced_by_partial_input() {
    let home = root("t05");
    let data = home.join("data");
    fs::create_dir_all(&data).unwrap();

    let dir = cred_dir(&data);
    fs::create_dir_all(&dir).unwrap();
    let file = cred_file(&data, PROVIDER);
    let original =
        br#"{"provider":"openai","model":"openai/gpt-5.6","credential":"pre-existing-valid"}"#;
    fs::write(&file, original).unwrap();

    // Invalid/partial input: provider and key but no model line before EOF.
    let (_status, _output) = run_setup(&data, &format!("{PROVIDER}\n{KEY}\n"));

    assert_eq!(
        fs::read(&file).unwrap(),
        original,
        "partial/invalid input replaced the existing valid credential file"
    );

    fs::remove_dir_all(home).unwrap();
}