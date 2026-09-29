#![cfg(unix)]

//! Packaging-level contract tests for the real POSIX installer scripts.
//!
//! The tiny `oc2`/`opencode2` shell executable in these tests is only an archive
//! byte fixture so the installers can exercise their copy/identity paths. It is
//! not the native application and proves nothing about ELF loading or rpath.
//! Linkage-dependent scenarios are intentionally deferred; see the companion
//! APP-010-LINUX-NATIVE-ARCHIVE-CONTRACT worklog.

use std::ffi::{OsStr, OsString};
use std::fs::{self, Metadata};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const PATH_VALUE: &str = "/usr/bin:/bin:/sbin";
const MAX_CAPTURE: usize = 64 * 1024;
const MAX_FIXTURE_ARCHIVE: u64 = 256 * 1024;
const CHILD_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_TAR_MEMBERS: usize = 16;
const MAX_TAR_MEMBER_BYTES: usize = 32 * 1024;

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug)]
struct Installer {
    script: &'static str,
    binary: &'static str,
    install_env: &'static str,
    version_env: &'static str,
    identity_gate: bool,
}

const INSTALLERS: [Installer; 2] = [
    Installer {
        script: "install-oc2.sh",
        binary: "oc2",
        install_env: "OC2_INSTALL_DIR",
        version_env: "OC2_VERSION",
        identity_gate: true,
    },
    Installer {
        script: "install-opencode2.sh",
        binary: "opencode2",
        install_env: "OPENCODE2_INSTALL_DIR",
        version_env: "OPENCODE2_VERSION",
        identity_gate: false,
    },
];

#[derive(Debug)]
struct Captured {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    timed_out: bool,
    output_truncated: bool,
}

impl Captured {
    fn assert_bounded(&self, context: &str) {
        assert!(!self.timed_out, "{context} exceeded {CHILD_TIMEOUT:?}");
        assert!(
            !self.output_truncated,
            "{context} exceeded the {MAX_CAPTURE}-byte output budget"
        );
    }

    fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let base = std::env::temp_dir();
        for _ in 0..32 {
            let seq = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let root = base.join(format!(
                "oc2-installer-contract-{label}-{}-{seq}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    fs::create_dir(root.join("home")).expect("create disposable HOME");
                    fs::create_dir(root.join("tmp")).expect("create disposable TMPDIR");
                    return Self { root };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create disposable fixture root: {error}"),
            }
        }
        panic!("could not allocate unique disposable fixture root");
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn temp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    fn archive(&self, label: &str, entries: &[TarEntry]) -> PathBuf {
        assert!(entries.len() <= MAX_TAR_MEMBERS, "fixture member bound exceeded");
        let tar = write_tar(entries);
        let archive = self.root.join(format!("{label}.tar.gz"));
        let compressed = run_child(
            "gzip",
            &[OsStr::new("-n"), OsStr::new("-c")],
            &self.root,
            &self.home(),
            &self.temp(),
            Some(&tar),
        )
        .unwrap_or_else(|error| panic!("start gzip fixture builder: {error}"));
        compressed.assert_bounded("gzip fixture builder");
        assert!(
            compressed.status.success(),
            "gzip fixture builder failed: {}",
            compressed.stderr_text()
        );
        assert!(
            (compressed.stdout.len() as u64) <= MAX_FIXTURE_ARCHIVE,
            "compressed fixture exceeds {MAX_FIXTURE_ARCHIVE} bytes"
        );
        fs::write(&archive, &compressed.stdout).expect("write fixture archive");
        archive
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // Only remove the uniquely allocated test root; never clean a shared
        // temp directory or any user-owned path.
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone, Copy)]
enum TarKind {
    Regular,
    Symlink,
    Fifo,
}

struct TarEntry {
    name: String,
    kind: TarKind,
    data: Vec<u8>,
    link: String,
    mode: u32,
}

impl TarEntry {
    fn file(name: impl Into<String>, data: Vec<u8>, mode: u32) -> Self {
        Self {
            name: name.into(),
            kind: TarKind::Regular,
            data,
            link: String::new(),
            mode,
        }
    }

    fn symlink(name: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: TarKind::Symlink,
            data: Vec::new(),
            link: target.into(),
            mode: 0o777,
        }
    }

    fn fifo(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: TarKind::Fifo,
            data: Vec::new(),
            link: String::new(),
            mode: 0o600,
        }
    }
}

fn fixture_binary(installer: Installer, version: &str) -> Vec<u8> {
    let name = installer.binary;
    let version_line = format!("{name} {version} fixture\n");
    let help_line = format!("{name} - fixture usage\n");
    format!(
        "#!/bin/sh\ncase \"${{1:-}}\" in\n  --version) printf '%s' '{version_line}' ;;\n  --help) printf '%s' '{help_line}' ;;\n  *) exit 0 ;;\nesac\n"
    )
    .into_bytes()
}

fn script_path(installer: Installer) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts")
        .join(installer.script)
}

fn run_installer(
    sandbox: &Sandbox,
    installer: Installer,
    install_dir: &Path,
    args: &[OsString],
) -> Captured {
    let script = script_path(installer);
    assert!(script.is_file(), "registered installer script missing: {script:?}");
    let mut command_args = vec![script.as_os_str().to_os_string()];
    command_args.extend_from_slice(args);
    command_args.push(OsString::from("--install-dir"));
    command_args.push(install_dir.as_os_str().to_os_string());
    let env = [
        (OsString::from("TERM"), OsString::from("dumb")),
        (
            OsString::from(installer.install_env),
            OsString::from(""),
        ),
        (
            OsString::from(installer.version_env),
            OsString::from(""),
        ),
    ];
    run_child_with_env(
        "sh",
        &command_args,
        &sandbox.root,
        &sandbox.home(),
        &sandbox.temp(),
        &env,
        None,
    )
    .unwrap_or_else(|error| panic!("start real {} installer: {error}", installer.script))
}

fn run_child(
    program: &str,
    args: &[&OsStr],
    cwd: &Path,
    home: &Path,
    tmpdir: &Path,
    stdin_bytes: Option<&[u8]>,
) -> std::io::Result<Captured> {
    let args: Vec<OsString> = args.iter().map(|arg| (*arg).to_os_string()).collect();
    run_child_with_env(
        program,
        &args,
        cwd,
        home,
        tmpdir,
        &[],
        stdin_bytes,
    )
}

fn run_child_with_env(
    program: &str,
    args: &[OsString],
    cwd: &Path,
    home: &Path,
    tmpdir: &Path,
    extra_env: &[(OsString, OsString)],
    stdin_bytes: Option<&[u8]>,
) -> std::io::Result<Captured> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", PATH_VALUE)
        .env("HOME", home)
        .env("TMPDIR", tmpdir)
        .stdin(if stdin_bytes.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra_env {
        command.env(key, value);
    }

    let mut child = command.spawn()?;
    let stdout = child.stdout.take().expect("stdout pipe requested");
    let stderr = child.stderr.take().expect("stderr pipe requested");
    let stdout_reader = thread::spawn(move || read_capped(stdout, MAX_CAPTURE));
    let stderr_reader = thread::spawn(move || read_capped(stderr, MAX_CAPTURE));
    if let Some(input) = stdin_bytes {
        let write_result = child
            .stdin
            .take()
            .expect("stdin pipe requested")
            .write_all(input);
        if let Err(error) = write_result {
            terminate_and_reap(&mut child);
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(error);
        }
    }

    let deadline = Instant::now() + CHILD_TIMEOUT;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                terminate_and_reap(&mut child);
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(error);
            }
        }
        if Instant::now() >= deadline {
            timed_out = true;
            break terminate_and_reap(&mut child);
        }
        thread::sleep(Duration::from_millis(10));
    };
    let (stdout, stdout_truncated) = stdout_reader
        .join()
        .expect("join owned stdout reader");
    let (stderr, stderr_truncated) = stderr_reader
        .join()
        .expect("join owned stderr reader");
    Ok(Captured {
        status,
        stdout,
        stderr,
        timed_out,
        output_truncated: stdout_truncated || stderr_truncated,
    })
}

fn terminate_and_reap(child: &mut Child) -> ExitStatus {
    let _ = child.kill();
    child.wait().expect("reap owned fixture child")
}

fn read_capped(mut reader: impl Read, cap: usize) -> (Vec<u8>, bool) {
    let mut kept = Vec::with_capacity(cap.min(4096));
    let mut truncated = false;
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(count) => {
                let remaining = cap.saturating_sub(kept.len());
                let retain = remaining.min(count);
                kept.extend_from_slice(&buf[..retain]);
                truncated |= retain != count;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    (kept, truncated)
}

fn checksum(path: &Path, sandbox: &Sandbox) -> String {
    let (program, args): (&str, Vec<OsString>) = if executable_in_path("sha256sum") {
        ("sha256sum", vec![path.as_os_str().to_os_string()])
    } else {
        (
            "shasum",
            vec![OsString::from("-a"), OsString::from("256"), path.as_os_str().to_os_string()],
        )
    };
    let result = run_child_with_env(
        program,
        &args,
        &sandbox.root,
        &sandbox.home(),
        &sandbox.temp(),
        &[],
        None,
    )
    .unwrap_or_else(|error| panic!("start {program} for fixture checksum: {error}"));
    result.assert_bounded("fixture checksum command");
    assert!(
        result.status.success(),
        "checksum command failed: {}",
        result.stderr_text()
    );
    let text = result.stdout_text();
    let digest = text.split_whitespace().next().expect("checksum output has digest");
    assert_eq!(digest.len(), 64, "checksum utility returned malformed digest");
    digest.to_owned()
}

fn executable_in_path(name: &str) -> bool {
    PATH_VALUE.split(':').any(|dir| {
        let path = Path::new(dir).join(name);
        fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    })
}

fn archive_args(path: &Path, digest: &str) -> Vec<OsString> {
    vec![
        OsString::from("--archive"),
        path.as_os_str().to_os_string(),
        OsString::from("--checksum"),
        OsString::from(digest),
    ]
}

fn snapshot_tree(path: &Path) -> Vec<(String, String, Vec<u8>, u32)> {
    if !path.exists() && !path.is_symlink() {
        return Vec::new();
    }
    let mut entries = Vec::new();
    snapshot_into(path, path, &mut entries);
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

fn snapshot_into(root: &Path, path: &Path, entries: &mut Vec<(String, String, Vec<u8>, u32)>) {
    let metadata = fs::symlink_metadata(path).expect("metadata for fixture snapshot");
    let relative = path
        .strip_prefix(root)
        .expect("snapshot path inside root")
        .to_string_lossy()
        .into_owned();
    let (kind, bytes) = if metadata.file_type().is_symlink() {
        (
            "symlink".to_owned(),
            fs::read_link(path)
                .expect("read fixture symlink")
                .to_string_lossy()
                .as_bytes()
                .to_vec(),
        )
    } else if metadata.is_dir() {
        ("dir".to_owned(), Vec::new())
    } else if metadata.is_file() {
        ("file".to_owned(), fs::read(path).expect("read fixture file"))
    } else {
        ("other".to_owned(), Vec::new())
    };
    entries.push((relative, kind, bytes, metadata.permissions().mode()));
    if metadata.is_dir() {
        let children = fs::read_dir(path).expect("read fixture directory");
        for child in children {
            snapshot_into(root, &child.expect("read fixture entry").path(), entries);
        }
    }
}

fn metadata_mode(metadata: &Metadata) -> u32 {
    metadata.permissions().mode() & 0o777
}

fn assert_child_ok(result: &Captured, context: &str) {
    result.assert_bounded(context);
    assert!(
        result.status.success(),
        "{context} failed ({}):\nstdout:\n{}\nstderr:\n{}",
        result.status,
        result.stdout_text(),
        result.stderr_text()
    );
}

fn octal(value: u64, width: usize) -> Vec<u8> {
    let text = format!("{value:0width$o}", width = width - 1);
    assert!(text.len() < width, "tar numeric field overflow");
    let mut out = vec![b'0'; width];
    out[..text.len()].copy_from_slice(text.as_bytes());
    out[width - 1] = 0;
    out
}

fn put_field(header: &mut [u8], start: usize, width: usize, value: &[u8]) {
    assert!(value.len() <= width, "tar field exceeds header slot");
    header[start..start + value.len()].copy_from_slice(value);
}

fn split_tar_name(name: &str) -> (String, String) {
    if name.as_bytes().len() <= 100 {
        return (name.to_owned(), String::new());
    }
    for (index, byte) in name.bytes().enumerate().rev() {
        if byte == b'/' {
            let prefix = &name[..index];
            let base = &name[index + 1..];
            if prefix.len() <= 155 && base.len() <= 100 {
                return (base.to_owned(), prefix.to_owned());
            }
        }
    }
    panic!("fixture tar name exceeds ustar limits: {name:?}");
}

fn write_tar(entries: &[TarEntry]) -> Vec<u8> {
    let mut tar = Vec::new();
    for entry in entries {
        assert!(entry.data.len() <= MAX_TAR_MEMBER_BYTES, "tar member byte bound exceeded");
        let (name, prefix) = split_tar_name(&entry.name);
        let mut header = [0u8; 512];
        put_field(&mut header, 0, 100, name.as_bytes());
        put_field(&mut header, 100, 8, &octal(entry.mode as u64, 8));
        put_field(&mut header, 108, 8, &octal(0, 8));
        put_field(&mut header, 116, 8, &octal(0, 8));
        put_field(&mut header, 124, 12, &octal(entry.data.len() as u64, 12));
        put_field(&mut header, 136, 12, &octal(0, 12));
        header[148..156].fill(b' ');
        header[156] = match entry.kind {
            TarKind::Regular => b'0',
            TarKind::Symlink => b'2',
            TarKind::Fifo => b'6',
        };
        put_field(&mut header, 157, 100, entry.link.as_bytes());
        put_field(&mut header, 257, 6, b"ustar\0");
        put_field(&mut header, 263, 2, b"00");
        put_field(&mut header, 345, 155, prefix.as_bytes());
        let checksum: u64 = header.iter().map(|byte| u64::from(*byte)).sum();
        let checksum_text = format!("{checksum:06o}\0 ");
        assert_eq!(checksum_text.len(), 8);
        put_field(&mut header, 148, 8, checksum_text.as_bytes());
        tar.extend_from_slice(&header);
        tar.extend_from_slice(&entry.data);
        let padding = (512 - entry.data.len() % 512) % 512;
        tar.resize(tar.len() + padding, 0);
    }
    tar.resize(tar.len() + 1024, 0);
    tar
}

fn run_valid_install(
    sandbox: &Sandbox,
    installer: Installer,
    install_dir: &Path,
    version: &str,
) -> (PathBuf, Vec<u8>, Captured) {
    let binary = fixture_binary(installer, version);
    let archive = sandbox.archive(
        &format!("{}-{version}", installer.binary),
        &[TarEntry::file(installer.binary, binary.clone(), 0o755)],
    );
    let digest = checksum(&archive, sandbox);
    let args = archive_args(&archive, &digest);
    let result = run_installer(sandbox, installer, install_dir, &args);
    (archive, binary, result)
}

#[test]
fn checksum_mismatch_preserves_existing_destination_for_both_installers() {
    for installer in INSTALLERS {
        let sandbox = Sandbox::new("checksum");
        let install_dir = sandbox.path().join("install path 日本語/bin");
        fs::create_dir_all(&install_dir).expect("seed disposable install dir");
        fs::write(install_dir.join("keep.marker"), b"pre-existing install marker\0").unwrap();
        let before = snapshot_tree(&install_dir);
        let archive = sandbox.archive(
            installer.binary,
            &[TarEntry::file(
                installer.binary,
                fixture_binary(installer, "checksum"),
                0o755,
            )],
        );
        let actual = checksum(&archive, &sandbox);
        let replacement = if actual.starts_with('0') { '1' } else { '0' };
        let wrong = format!("{replacement}{}", &actual[1..]);
        let args = archive_args(&archive, &wrong);
        let result = run_installer(&sandbox, installer, &install_dir, &args);
        result.assert_bounded("checksum rejection installer");
        assert_eq!(result.status.code(), Some(65), "checksum rejection status from {}", installer.script);
        assert!(
            result.stderr_text().contains("checksum mismatch"),
            "{} should diagnose the rejected outer archive digest",
            installer.script
        );
        assert_eq!(snapshot_tree(&install_dir), before, "{} mutated existing destination", installer.script);
        assert_eq!(fs::read_dir(sandbox.temp()).unwrap().count(), 0, "{} left staging residue", installer.script);
    }
}

#[test]
fn positive_packaging_install_preserves_existing_data_and_handles_unicode_paths() {
    for installer in INSTALLERS {
        let sandbox = Sandbox::new("positive");
        let install_dir = sandbox.path().join("install path 日本語/bin");
        let legacy = sandbox.path().join("legacy-path/opencode");
        let user_marker = sandbox.home().join(".local/share/opencode-rk/user.marker");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::create_dir_all(user_marker.parent().unwrap()).unwrap();
        fs::write(&legacy, b"legacy opencode bytes\0").unwrap();
        fs::write(&user_marker, b"user data marker\0\xff").unwrap();
        let legacy_before = fs::read(&legacy).unwrap();
        let marker_before = fs::read(&user_marker).unwrap();

        let (_archive, expected_binary, result) =
            run_valid_install(&sandbox, installer, &install_dir, "9.9.9");
        assert_child_ok(&result, "positive packaging install");
        let installed = install_dir.join(installer.binary);
        assert_eq!(fs::read(&installed).expect("installed fixture executable"), expected_binary);
        assert_eq!(metadata_mode(&fs::metadata(&installed).unwrap()), 0o755);
        assert_eq!(fs::read(&legacy).unwrap(), legacy_before);
        assert_eq!(fs::read(&user_marker).unwrap(), marker_before);
        if installer.identity_gate {
            assert!(result.stdout_text().contains("oc2 9.9.9 fixture"));
            assert!(!result.stdout_text().contains("opencode-rk"));
        } else {
            assert!(result.stdout_text().contains("opencode2 9.9.9 fixture"));
        }
        assert_eq!(fs::read_dir(sandbox.temp()).unwrap().count(), 0, "{} left staging residue", installer.script);
    }
}

#[test]
fn upgrade_replaces_only_packaged_executable_and_preserves_user_markers() {
    for installer in INSTALLERS {
        let sandbox = Sandbox::new("upgrade");
        let install_dir = sandbox.path().join("upgrade path/bin");
        fs::create_dir_all(&install_dir).unwrap();
        let marker = install_dir.join("user.marker");
        fs::write(&marker, b"keep across upgrade\n").unwrap();
        let user_data = sandbox.home().join("history/marker");
        fs::create_dir_all(user_data.parent().unwrap()).unwrap();
        fs::write(&user_data, b"durable user data\n").unwrap();

        let (_, first_binary, first) = run_valid_install(&sandbox, installer, &install_dir, "1.0");
        assert_child_ok(&first, "first packaging install");
        let installed = install_dir.join(installer.binary);
        assert_eq!(fs::read(&installed).unwrap(), first_binary);

        let (_, second_binary, second) = run_valid_install(&sandbox, installer, &install_dir, "2.0");
        assert_child_ok(&second, "packaging upgrade");
        assert_eq!(fs::read(&installed).unwrap(), second_binary);
        assert_ne!(first_binary, second_binary);
        assert_eq!(fs::read(&marker).unwrap(), b"keep across upgrade\n");
        assert_eq!(fs::read(&user_data).unwrap(), b"durable user data\n");
    }
}

#[test]
fn uninstall_removes_only_installed_binary_and_preserves_other_files() {
    for installer in INSTALLERS {
        let sandbox = Sandbox::new("uninstall");
        let install_dir = sandbox.path().join("uninstall path/bin");
        fs::create_dir_all(&install_dir).unwrap();
        let installed = install_dir.join(installer.binary);
        fs::write(&installed, b"packaged fixture bytes").unwrap();
        let sibling = install_dir.join("unrelated.tool");
        fs::write(&sibling, b"unrelated sibling bytes\0").unwrap();
        let user_marker = sandbox.home().join("history/keep.marker");
        fs::create_dir_all(user_marker.parent().unwrap()).unwrap();
        fs::write(&user_marker, b"user marker\0").unwrap();
        let before = snapshot_tree(&install_dir);
        let args = vec![OsString::from("--uninstall")];

        let result = run_installer(&sandbox, installer, &install_dir, &args);
        assert_child_ok(&result, "uninstall");
        assert!(!installed.exists(), "{} binary still exists", installer.binary);
        assert_eq!(fs::read(&sibling).unwrap(), b"unrelated sibling bytes\0");
        assert_eq!(fs::read(&user_marker).unwrap(), b"user marker\0");
        let after = snapshot_tree(&install_dir);
        let expected: Vec<_> = before
            .into_iter()
            .filter(|entry| entry.0 != installer.binary)
            .collect();
        assert_eq!(after, expected, "{} uninstall changed unrelated install files", installer.script);
    }
}

#[test]
fn legacy_opencode_refusal_preserves_destination_for_both_installers() {
    for installer in INSTALLERS {
        let sandbox = Sandbox::new("legacy");
        let install_dir = sandbox.path().join("legacy install/bin");
        fs::create_dir_all(&install_dir).unwrap();
        fs::write(install_dir.join("opencode"), b"legacy executable sentinel\0").unwrap();
        fs::write(install_dir.join("keep.marker"), b"existing marker\0").unwrap();
        let before = snapshot_tree(&install_dir);
        let archive = sandbox.archive(
            installer.binary,
            &[TarEntry::file(
                installer.binary,
                fixture_binary(installer, "blocked"),
                0o755,
            )],
        );
        let digest = checksum(&archive, &sandbox);
        let result = run_installer(
            &sandbox,
            installer,
            &install_dir,
            &archive_args(&archive, &digest),
        );
        result.assert_bounded("legacy binary refusal");
        assert_eq!(result.status.code(), Some(73));
        assert!(result.stderr_text().contains("refusing"));
        assert_eq!(snapshot_tree(&install_dir), before, "legacy refusal mutated destination");
    }
}

#[test]
fn invalid_invocation_and_checksum_fail_without_destination_mutation() {
    for installer in INSTALLERS {
        let sandbox = Sandbox::new("arguments");
        let install_dir = sandbox.path().join("existing destination");
        fs::create_dir_all(&install_dir).unwrap();
        fs::write(install_dir.join("marker"), b"preserve me").unwrap();
        let before = snapshot_tree(&install_dir);

        for (args, expected_code, label) in [
            (vec![], 64, "missing archive"),
            (
                vec![OsString::from("--archive"), OsString::from("unused.tar.gz")],
                64,
                "missing checksum",
            ),
            (
                vec![OsString::from("--unknown-contract-flag")],
                64,
                "unknown flag",
            ),
            (
                vec![
                    OsString::from("--archive"),
                    sandbox.path().join("does-not-exist.tar.gz").into_os_string(),
                    OsString::from("--checksum"),
                    OsString::from("0".repeat(64)),
                ],
                66,
                "missing archive file",
            ),
        ] {
            let result = run_installer(&sandbox, installer, &install_dir, &args);
            result.assert_bounded(label);
            assert_eq!(result.status.code(), Some(expected_code), "{label}: {}", installer.script);
            assert_eq!(snapshot_tree(&install_dir), before, "{label} mutated destination");
        }
    }
}

#[test]
fn unsafe_archive_members_are_rejected_without_destination_mutation_or_escape() {
    for installer in INSTALLERS {
        for (case, unsafe_entry) in [
            (
                "traversal",
                TarEntry::file("../escaped-marker", b"must not escape staging".to_vec(), 0o600),
            ),
            (
                "absolute",
                TarEntry::file(
                    sandbox.path().join("absolute-escape-marker").to_string_lossy().into_owned(),
                    b"must not escape staging".to_vec(),
                    0o600,
                ),
            ),
            (
                "symlink",
                TarEntry::symlink("payload-link", "../../symlink-escape"),
            ),
            ("fifo", TarEntry::fifo("payload-pipe")),
        ] {
            let sandbox = Sandbox::new(&format!("unsafe-{case}"));
            let install_dir = sandbox.path().join("unsafe install/bin");
            fs::create_dir_all(&install_dir).unwrap();
            fs::write(install_dir.join("marker"), b"pre-existing destination").unwrap();
            let before = snapshot_tree(&install_dir);
            let mut entries = vec![
                TarEntry::file(
                    installer.binary,
                    fixture_binary(installer, "unsafe"),
                    0o755,
                ),
                unsafe_entry,
            ];
            if case == "symlink" {
                // If extraction follows the archive link, the later child
                // would write only beneath this disposable root; assert that
                // no such write occurs. The root is removed by Sandbox::drop.
                entries.push(TarEntry::file(
                    "payload-link/escape-marker",
                    b"symlink traversal must not write".to_vec(),
                    0o600,
                ));
            }
            let archive = sandbox.archive(&format!("{}-{case}", installer.binary), &entries);
            let digest = checksum(&archive, &sandbox);
            let result = run_installer(
                &sandbox,
                installer,
                &install_dir,
                &archive_args(&archive, &digest),
            );
            result.assert_bounded(&format!("{case} archive rejection"));
            assert!(
                !result.status.success(),
                "{} accepted unsafe {case} archive member; stdout={} stderr={}",
                installer.script,
                result.stdout_text(),
                result.stderr_text()
            );
            assert_eq!(snapshot_tree(&install_dir), before, "unsafe {case} mutated destination");
            assert_eq!(
                fs::read_dir(sandbox.temp()).unwrap().count(),
                0,
                "unsafe {case} left temporary staging content"
            );
            let escaped_path = match case {
                // mktemp creates the stage directly beneath this disposable
                // TMPDIR; one .. can reach only this test-owned root.
                "traversal" => sandbox.temp().join("escaped-marker"),
                // The archive's absolute name points to a unique path inside
                // this test-owned root, never an arbitrary host location.
                "absolute" => sandbox.path().join("absolute-escape-marker"),
                "symlink" => sandbox.path().join("symlink-escape/escape-marker"),
                _ => continue,
            };
            assert!(!escaped_path.exists(), "{case} member escaped private staging");
        }

        let sandbox = Sandbox::new("unsafe-duplicate");
        let install_dir = sandbox.path().join("duplicate install/bin");
        fs::create_dir_all(&install_dir).unwrap();
        fs::write(install_dir.join("marker"), b"duplicate destination marker").unwrap();
        let before = snapshot_tree(&install_dir);
        let archive = sandbox.archive(
            &format!("{}-duplicate", installer.binary),
            &[
                TarEntry::file(
                    installer.binary,
                    fixture_binary(installer, "first"),
                    0o755,
                ),
                TarEntry::file(
                    installer.binary,
                    fixture_binary(installer, "second"),
                    0o755,
                ),
            ],
        );
        let digest = checksum(&archive, &sandbox);
        let result = run_installer(
            &sandbox,
            installer,
            &install_dir,
            &archive_args(&archive, &digest),
        );
        result.assert_bounded("duplicate archive member rejection");
        assert!(
            !result.status.success(),
            "{} accepted duplicate executable archive entries",
            installer.script
        );
        assert_eq!(snapshot_tree(&install_dir), before, "duplicate member mutated destination");
    }
}
