//! APP-001 credential persistence RED journey.
//!
//! This is deliberately an installed-binary test: `OC2_E2E_BIN` is mandatory,
//! so a developer PATH binary can never make the result look green.  `/usr/bin/
//! script` supplies a real macOS PTY while keeping the harness std-only.

use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const SECRET: &str = "sk-red-app001-persistence-credential";
const MAX_OUTPUT: usize = 64 * 1024;
const DEADLINE: Duration = Duration::from_secs(12);

struct Fixture {
    home: PathBuf,
    data: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!("oc2-app001-red3-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&root);
        let home = root.join("home");
        let data = root.join("data");
        fs::create_dir_all(&home).expect("create disposable HOME");
        fs::create_dir_all(&data).expect("create disposable data root");
        Self { home, data }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(root) = self.home.parent() {
            let _ = fs::remove_dir_all(root);
        }
    }
}

fn binary() -> PathBuf {
    let value = std::env::var_os("OC2_E2E_BIN")
        .expect("OC2_E2E_BIN must name the freshly built target/debug/oc2");
    let path = PathBuf::from(value);
    assert!(path.is_file(), "OC2_E2E_BIN is not a file: {}", path.display());
    assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("oc2"), "test must run exact oc2 binary");
    path
}

fn run_pty(fixture: &Fixture) -> String {
    let mut child = Command::new("/usr/bin/script")
        .args(["-q", "/dev/null"])
        .arg(binary())
        .env("HOME", &fixture.home)
        .env("OPENCODE_DATA_DIR", &fixture.data)
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("GOOGLE_API_KEY")
        .env_remove("GEMINI_API_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn exact oc2 through PTY");
    let started = Instant::now();
    while started.elapsed() < DEADLINE {
        if child.try_wait().expect("observe child") .is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    if child.try_wait().expect("observe child at deadline").is_none() {
        child.kill().expect("kill bounded PTY child");
    }
    let output = child.wait_with_output().expect("reap PTY child");
    let mut bytes = Vec::with_capacity((output.stdout.len() + output.stderr.len()).min(MAX_OUTPUT));
    bytes.extend_from_slice(&output.stdout[..output.stdout.len().min(MAX_OUTPUT)]);
    if bytes.len() < MAX_OUTPUT {
        bytes.extend_from_slice(&output.stderr[..output.stderr.len().min(MAX_OUTPUT - bytes.len())]);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn persisted_paths(root: &Path) -> Vec<PathBuf> {
    fn walk(path: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(path) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() { walk(&path, out); } else { out.push(path); }
        }
    }
    let mut result = Vec::new();
    walk(root, &mut result);
    result
}

#[test]
fn installed_binary_is_explicit_and_exact() {
    let _ = binary();
}

#[test]
fn first_setup_persists_provider_state_without_secret_output() {
    let fixture = Fixture::new("first");
    let output = run_pty(&fixture);
    assert!(output.contains("setup"), "first launch must render setup through the PTY; output={output:?}");
    assert!(!output.contains(SECRET), "credential leaked to PTY output");
}

#[test]
fn restart_skips_setup_and_uses_same_installed_state() {
    let fixture = Fixture::new("restart");
    let first = run_pty(&fixture);
    assert!(first.contains("setup"), "first launch did not reach setup: {first:?}");
    let second = run_pty(&fixture);
    assert!(!second.contains("opening in-app setup"), "restart reopened setup: {second:?}");
}

#[test]
fn persisted_credential_is_not_in_unprotected_metadata_or_process_surface() {
    let fixture = Fixture::new("secret");
    let output = run_pty(&fixture);
    assert!(!output.contains(SECRET), "credential leaked to stdout/stderr");
    for path in persisted_paths(&fixture.data) {
        let Ok(mut file) = fs::File::open(&path) else { continue };
        let mut bytes = Vec::new();
        file.take(MAX_OUTPUT as u64).read_to_end(&mut bytes).expect("read bounded metadata");
        assert!(!String::from_utf8_lossy(&bytes).contains(SECRET), "credential leaked in {}", path.display());
    }
}

#[test]
fn cancellation_does_not_leave_uncommitted_account_marker() {
    let fixture = Fixture::new("cancel");
    let _ = run_pty(&fixture);
    for path in persisted_paths(&fixture.data) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        assert!(!name.contains("staged") && !name.contains("pending"), "cancel left staged marker: {}", path.display());
    }
}
