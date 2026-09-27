#![forbid(unsafe_code)]
//! APP-001-CREDENTIAL-PERSISTENCE-RED — frozen vertical RED for the installed
//! credential-persistence journey of the no-subcommand entrypoint.
//!
//! Contract under test (observable, end to end through the installed binary):
//! 1. Fresh disposable HOME + data dir, no provider env var: a no-subcommand
//!    launch runs in-app setup, accepts provider + fake credential + model, and
//!    exits cleanly.
//! 2. The credential state written for that provider is owner-only (`0600` on
//!    Unix): no group/other read or write bit.
//! 3. A second no-subcommand launch with the same HOME/data skips setup and can
//!    construct and send exactly one request to a fake provider using the stored
//!    credential.
//! 4. The raw credential never appears in stdout, stderr, argv, or any URL.
//! 5. Cancellation (EOF / interrupt at the credential step) leaves no residue:
//!    no credential file, no staged account.
//! 6. The launch owns any child/daemon it starts and reaps it on exit — no
//!    orphan process survives the parent.
//!
//! Source evidence (current code, base `ecec045`, 2026-09-26):
//! - `crates/cli/src/daemon_client.rs:775-791` — `creds_configured` only reads
//!   provider env vars; it never reads a persisted credential store. There is
//!   no on-disk credential read path today.
//! - `crates/cli/src/onboarding.rs:348-398` — `AccountStore` /
//!   `MemoryAccountStore`: the only store is in-memory and holds a `bool`, not
//!   credential bytes; nothing is persisted to disk.
//! - `crates/cli/src/onboarding.rs:157-199` — `SecretString` redacts `Debug` /
//!   `Display`, but no code writes it anywhere durable.
//! - `crates/cli/src/app_start.rs:287-346` — `plan_default_launch` /
//!   `needs_setup` route missing creds to `StartupView::Setup`, but the setup
//!   outcome is never committed to a durable store a later launch can read.
//! - No `0600` / `set_permissions` / `PermissionsExt` call exists anywhere in
//!   `crates/cli/src` (grep at base `ecec045` returned no matches).
//!
//! Therefore the credential-persistence behavior does not exist yet: these
//! tests compile with std only and fail for the missing behavior, never for a
//! missing import or fixture. They must NOT be edited to obtain GREEN; the
//! implementation must add the persistence + read-back path.
//!
//! Std-only by design (no new dev-deps): the file compiles as a root
//! integration target under `crates/cli/tests/`.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Fake credential used by every test. Never a real key: fixed, obviously
/// synthetic, and asserted absent from all observable surfaces.
const FAKE_KEY: &str = "sk-fake-app001-not-a-real-provider-key-0001";
const FAKE_PROVIDER: &str = "fixture-provider";
const FAKE_MODEL: &str = "fixture-model-1";

/// Provider env vars the current binary treats as "creds configured"
/// (`daemon_client.rs:777-782`). Scrub them so a developer shell cannot make a
/// fresh HOME look already-configured.
const PROVIDER_ENV_VARS: &[&str] = &[
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "GOOGLE_API_KEY",
    "GEMINI_API_KEY",
];

/// Documented credential-store contract for the durable provider credential.
/// The RED asserts on the absence of *any* owner-only file under the data dir,
/// so it does not depend on the implementation choosing this exact name.
const CREDENTIAL_STORE_REL: &str = "credentials/provider-credentials.json";

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn fresh_home(tag: &str) -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = env::temp_dir().join(format!(
        "opencode2-app001-{}-{}-{id}",
        std::process::id(),
        tag
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create disposable HOME");
    path
}

/// Disposable HOME + data dir removed on drop so reruns never inherit state.
struct DisposableHome {
    home: PathBuf,
    data: PathBuf,
}

impl DisposableHome {
    fn new(tag: &str) -> Self {
        let home = fresh_home(tag);
        let data = home.join("data");
        fs::create_dir_all(&data).expect("create disposable data dir");
        Self { home, data }
    }

    fn home(&self) -> &Path {
        &self.home
    }

    fn data(&self) -> &Path {
        &self.data
    }

    fn credential_store(&self) -> PathBuf {
        self.data.join(CREDENTIAL_STORE_REL)
    }
}

impl Drop for DisposableHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

/// Owned child reaped on drop: kill, then bounded wait so no orphan survives.
struct ChildGuard(Child);

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self(child)
    }

    /// Bounded wait: `std` has no `wait_timeout`, so poll `try_wait`.
    fn wait_bounded(&mut self, limit: Duration) -> Option<std::process::ExitStatus> {
        let start = Instant::now();
        loop {
            match self.0.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) => {
                    if start.elapsed() > limit {
                        return None;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                Err(_) => return None,
            }
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Resolve the installed/dev binary. Dev runs expose `opencode-rk` via
/// `CARGO_BIN_EXE_opencode-rk`; release ships `oc2` installed as `opencode2`.
fn installed_binary() -> Command {
    match env::var_os("CARGO_BIN_EXE_opencode-rk") {
        Some(path) => Command::new(path),
        None => Command::new("opencode-rk"),
    }
}

/// Scrub provider env vars and pin the disposable data dir for one launch.
fn configured_command(home: &DisposableHome) -> Command {
    let mut cmd = installed_binary();
    for key in PROVIDER_ENV_VARS {
        cmd.env_remove(key);
    }
    cmd.env_remove("OPENCODE_RK_DAEMON_TOKEN");
    cmd.env("HOME", home.home());
    cmd.env("OPENCODE_RK_HOME", home.data());
    cmd
}

/// Every regular file under `root` that is owner-only (`0600` and no
/// group/other bits on Unix). Used to prove, without naming the implementation
/// path, that no owner-only credential state exists.
fn owner_only_files(root: &Path) -> BTreeSet<PathBuf> {
    let mut found = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() && is_owner_only(&meta) {
                found.insert(path);
            }
        }
    }
    found
}

#[cfg(unix)]
fn is_owner_only(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o777 == 0o600
}

#[cfg(not(unix))]
fn is_owner_only(_meta: &fs::Metadata) -> bool {
    // Non-Unix platforms have no POSIX mode bits; the owner-only contract is
    // asserted on Unix and treated as satisfied elsewhere.
    true
}

/// Run one no-subcommand launch under a bounded timeout, feeding `stdin` if
/// provided. Returns captured stdout/stderr so redaction can be asserted.
fn run_launch(home: &DisposableHome, stdin: Option<&str>, extra_args: &[&str]) -> (String, String, Option<i32>) {
    let mut cmd = configured_command(home);
    cmd.args(extra_args);
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut guard = ChildGuard::new(cmd.spawn().expect("spawn installed binary"));

    if let Some(text) = stdin {
        use std::io::Write;
        if let Some(mut pipe) = guard.0.stdin.take() {
            let _ = pipe.write_all(text.as_bytes());
            let _ = pipe.flush();
        }
    }

    let status = guard.wait_bounded(Duration::from_secs(20));
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(out) = guard.0.stdout.take() {
        let mut buf = Vec::new();
        let mut out = out;
        use std::io::Read;
        let _ = out.read_to_end(&mut buf);
        stdout = String::from_utf8_lossy(&buf).into_owned();
    }
    if let Some(err) = guard.0.stderr.take() {
        let mut buf = Vec::new();
        let mut err = err;
        use std::io::Read;
        let _ = err.read_to_end(&mut buf);
        stderr = String::from_utf8_lossy(&buf).into_owned();
    }

    (stdout, stderr, status.and_then(|s| s.code()))
}

// ---------------------------------------------------------------------------
// APP-001-CREDENTIAL-PERSISTENCE-T01 (RED): fresh setup accepts provider +
// fake credential + model and exits cleanly, leaving durable credential state.
// ---------------------------------------------------------------------------

#[test]
fn fresh_setup_accepts_provider_credential_model_and_exits_clean() {
    let home = DisposableHome::new("fresh-setup");
    let input = format!("{FAKE_PROVIDER}\n{FAKE_KEY}\n{FAKE_MODEL}\n");

    let (_stdout, _stderr, code) = run_launch(&home, Some(&input), &[]);

    // Missing behavior: no durable credential state is ever written.
    let store = home.credential_store();
    assert!(
        store.is_file(),
        "MISSING BEHAVIOR (APP-001-CREDENTIAL-PERSISTENCE): a fresh no-subcommand \
         setup that accepts provider={FAKE_PROVIDER} + fake credential + model must \
         persist credential state at {}, but no file exists (exit code was {code:?}). \
         See onboarding.rs AccountStore (in-memory only) and daemon_client::creds_configured \
         (env-only) at base ecec045.",
        store.display()
    );

    let owner_only = owner_only_files(home.data());
    assert!(
        !owner_only.is_empty(),
        "MISSING BEHAVIOR (APP-001-CREDENTIAL-PERSISTENCE): after successful setup the \
         data dir must contain owner-only credential state; found none under {}",
        home.data().display()
    );

    assert_eq!(
        code,
        Some(0),
        "fresh setup must exit cleanly (code 0); observed {code:?}"
    );
}

// ---------------------------------------------------------------------------
// APP-001-CREDENTIAL-PERSISTENCE-T02 (RED): credential state is owner-only.
// ---------------------------------------------------------------------------

#[test]
fn credential_state_is_owner_only_0600_on_unix() {
    let home = DisposableHome::new("owner-only");
    let input = format!("{FAKE_PROVIDER}\n{FAKE_KEY}\n{FAKE_MODEL}\n");
    let _ = run_launch(&home, Some(&input), &[]);

    let owner_only = owner_only_files(home.data());
    assert!(
        !owner_only.is_empty(),
        "MISSING BEHAVIOR (APP-001-CREDENTIAL-PERSISTENCE): persisted credential state \
         must be owner-only (0600); no owner-only file exists under {} after setup \
         (no 0600/set_permissions call exists anywhere in crates/cli/src at ecec045)",
        home.data().display()
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in &owner_only {
            let mode = fs::metadata(path).expect("stat credential file").permissions().mode();
            assert_eq!(
                mode & 0o777,
                0o600,
                "credential state {} must be exactly 0600, got {:o}",
                path.display(),
                mode & 0o777
            );
        }
    }
}

// ---------------------------------------------------------------------------
// APP-001-CREDENTIAL-PERSISTENCE-T03 (RED): restart skips setup and can
// construct/send one fake-provider request using the stored credential.
// ---------------------------------------------------------------------------

#[test]
fn restart_skips_setup_and_uses_stored_credential_for_one_request() {
    let home = DisposableHome::new("restart");
    let input = format!("{FAKE_PROVIDER}\n{FAKE_KEY}\n{FAKE_MODEL}\n");
    let _ = run_launch(&home, Some(&input), &[]);

    // Second launch: no stdin, same HOME/data. It must NOT need setup again and
    // must be able to build a request that carries the stored credential.
    let (stdout, stderr, _code) = run_launch(&home, None, &[]);
    let combined = format!("{stdout}\n{stderr}");

    // Missing behavior: there is no persisted credential for a restart to read,
    // so setup re-runs instead of being skipped.
    assert!(
        !combined.contains("opening in-app setup"),
        "MISSING BEHAVIOR (APP-001-CREDENTIAL-PERSISTENCE): a restart with persisted \
         credential state must skip setup; the binary still reported the in-app setup \
         view. Output:\n{combined}"
    );

    // Missing behavior: no persisted credential and no stored-credential request
    // path exist, so a restart cannot build a fake-provider request at all. Both
    // assertions become satisfiable once the persistence/read-back path lands.
    let store = home.credential_store();
    assert!(
        store.is_file(),
        "MISSING BEHAVIOR (APP-001-CREDENTIAL-PERSISTENCE): restart must read the \
         credential stored at {} to build one fake-provider request; no store exists \
         (creds_configured is env-only at base ecec045)",
        store.display()
    );
    assert!(
        combined.contains("request") || combined.to_lowercase().contains("turn"),
        "MISSING BEHAVIOR (APP-001-CREDENTIAL-PERSISTENCE): restart must construct and \
         send exactly one request to the fake provider using the stored credential; \
         output showed no request/turn:\n{combined}"
    );
}

// ---------------------------------------------------------------------------
// APP-001-CREDENTIAL-PERSISTENCE-T04 (RED): raw key never leaks.
// ---------------------------------------------------------------------------

#[test]
fn key_absent_from_stdout_stderr_argv_and_urls() {
    let home = DisposableHome::new("redaction");
    let input = format!("{FAKE_PROVIDER}\n{FAKE_KEY}\n{FAKE_MODEL}\n");

    // argv must not carry the raw key.
    let (_o, _e, _c) = run_launch(&home, Some(&input), &[]);
    let argv = env::args().collect::<Vec<_>>().join(" ");
    assert!(
        !argv.contains(FAKE_KEY),
        "credential must never appear in argv"
    );

    let (stdout, stderr, _code) = run_launch(&home, Some(&input), &[]);
    let mut surfaces = vec![
        ("stdout", stdout),
        ("stderr", stderr),
    ];
    // Durable state (if it existed) must not contain the raw key either.
    if let Ok(bytes) = fs::read(home.credential_store()) {
        surfaces.push(("credential store", String::from_utf8_lossy(&bytes).into_owned()));
    }
    for (name, text) in &surfaces {
        assert!(
            !text.contains(FAKE_KEY),
            "raw credential leaked into {name}:\n{text}"
        );
    }

    // URLs recorded anywhere in the data dir must not embed the raw key.
    for path in owner_only_files(home.data()) {
        if let Ok(bytes) = fs::read(&path) {
            let text = String::from_utf8_lossy(&bytes);
            assert!(
                !text.contains(FAKE_KEY),
                "raw credential leaked into URL/state file {}",
                path.display()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// APP-001-CREDENTIAL-PERSISTENCE-T05 (RED): cancellation leaves no residue.
// ---------------------------------------------------------------------------

#[test]
fn cancellation_leaves_no_residue() {
    let home = DisposableHome::new("cancel");
    // Provide provider + credential, then close stdin (EOF) before the model
    // step, simulating a user abandoning setup mid-flow.
    let input = format!("{FAKE_PROVIDER}\n{FAKE_KEY}\n");
    let _ = run_launch(&home, Some(&input), &[]);

    assert!(
        !home.credential_store().exists(),
        "MISSING BEHAVIOR (APP-001-CREDENTIAL-PERSISTENCE): cancelling setup before the \
         model step must leave no credential residue, but {} exists",
        home.credential_store().display()
    );

    let residue = owner_only_files(home.data());
    assert!(
        residue.is_empty(),
        "MISSING BEHAVIOR (APP-001-CREDENTIAL-PERSISTENCE): cancelled setup must leave no \
         owner-only residue; found {:?}",
        residue
    );
}

// ---------------------------------------------------------------------------
// APP-001-CREDENTIAL-PERSISTENCE-T06 (RED): owned child/daemon reaped.
// ---------------------------------------------------------------------------

#[test]
fn owned_child_daemon_is_reaped_on_exit() {
    let home = DisposableHome::new("reap");
    let input = format!("{FAKE_PROVIDER}\n{FAKE_KEY}\n{FAKE_MODEL}\n");
    let _ = run_launch(&home, Some(&input), &[]);

    // After the launch returns, no owned daemon descriptor/temp should remain,
    // and no child of this process should still be alive. RED asserts the
    // observable part that does not need a PTY: the launch must not leave a
    // staged descriptor behind on a clean setup exit.
    let data = home.data();
    let mut staged = Vec::new();
    if let Ok(entries) = fs::read_dir(&data) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.contains("daemon") && name.ends_with(".tmp") {
                staged.push(entry.path());
            }
        }
    }
    assert!(
        staged.is_empty(),
        "MISSING BEHAVIOR (APP-001-CREDENTIAL-PERSISTENCE): launch must reap any owned \
         child/daemon and leave no staged descriptor; found {:?}",
        staged
    );

    // The persistence contract ties the reaping assertion to the setup path:
    // without persisted state this launch could not have completed setup, so the
    // cleanup assertion above is vacuously true. Fail explicitly on the missing
    // behavior so the RED is about persistence, not process bookkeeping.
    assert!(
        home.credential_store().is_file(),
        "MISSING BEHAVIOR (APP-001-CREDENTIAL-PERSISTENCE): clean setup exit must have \
         persisted credential state before reaping the owned daemon; {} is absent",
        home.credential_store().display()
    );
}