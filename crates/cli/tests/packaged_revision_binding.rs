#![forbid(unsafe_code)]

//! Executable RED contract for package -> manifest -> installer -> runtime binding.
//! POSIX coverage uses the checked-in installer seam. Windows coverage is a
//! source contract because this test file cannot execute PowerShell on macOS/Linux.

#[cfg(unix)]
mod unix_package_tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::atomic::{AtomicU64, Ordering};

    const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
    const OTHER_REVISION: &str = "89abcdef0123456789abcdef0123456789abcdef";
    const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
    const SYSTEM_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    #[derive(Clone)]
    struct Member {
        name: String,
        payload: Vec<u8>,
        mode: u32,
    }

    impl Member {
        fn new(name: impl Into<String>, payload: impl Into<Vec<u8>>, mode: u32) -> Self {
            Self {
                name: name.into(),
                payload: payload.into(),
                mode,
            }
        }
    }

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "oc2-packaged-receipt-{label}-{}-{id}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("tmp")).expect("create disposable test root");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn install_dir(&self) -> PathBuf {
            self.0.join("install/bin")
        }

        fn native_dir(&self) -> PathBuf {
            self.0.join("install/lib")
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct Fixture {
        root: TestRoot,
        archive: PathBuf,
        manifest: PathBuf,
        checksum: String,
    }

    impl Fixture {
        fn new(label: &str, entries: &[Member]) -> Self {
            Self::new_with_manifest_entries(label, entries, entries)
        }

        fn new_with_manifest_entries(
            label: &str,
            archive_entries: &[Member],
            manifest_entries: &[Member],
        ) -> Self {
            let root = TestRoot::new(label);
            let archive = make_archive(root.path(), label, archive_entries);
            let checksum = sha256_file(&archive);
            let manifest = root.path().join(format!("{label}.manifest.json"));
            fs::write(
                &manifest,
                manifest_bytes(manifest_entries, &checksum, REVISION, platform()),
            )
            .expect("write deterministic manifest");
            seed_targets(&root);
            Self {
                root,
                archive,
                manifest,
                checksum,
            }
        }

        fn new_without_manifest(label: &str, entries: &[Member]) -> Self {
            let root = TestRoot::new(label);
            let archive = make_archive(root.path(), label, entries);
            let checksum = sha256_file(&archive);
            seed_targets(&root);
            let manifest = root.path().join("missing.manifest.json");
            Self {
                root,
                archive,
                manifest,
                checksum,
            }
        }

        fn run(&self, manifest: Option<&Path>) -> Output {
            let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/install-oc2.sh");
            let mut command = Command::new("/bin/sh");
            command
                .arg(script)
                .arg("--archive")
                .arg(&self.archive)
                .arg("--checksum")
                .arg(&self.checksum)
                .arg("--install-dir")
                .arg(self.root.install_dir());
            if let Some(manifest) = manifest {
                command.arg("--manifest").arg(manifest);
            }
            command
                .current_dir(self.root.path())
                .env_clear()
                .env("PATH", SYSTEM_PATH)
                .env("HOME", self.root.path().join("home"))
                .env("TMPDIR", self.root.path().join("tmp"))
                .env("LC_ALL", "C")
                .output()
                .expect("run install-oc2.sh")
        }

        fn assert_unchanged(&self, output: &Output) {
            let target = self.root.install_dir().join("oc2");
            let native = self.root.native_dir().join(native_name());
            assert_eq!(
                fs::read(&target).expect("read binary sentinel"),
                b"old-binary\n",
                "installer changed binary target\n{}",
                output_text(output)
            );
            assert_eq!(
                fs::read(&native).expect("read native sentinel"),
                b"old-native\n",
                "installer changed native target\n{}",
                output_text(output)
            );
        }

        fn assert_installed_revision(&self) {
            let target = self.root.install_dir().join("oc2");
            let output = Command::new(&target)
                .arg("--version")
                .env_clear()
                .env("PATH", SYSTEM_PATH)
                .env("HOME", self.root.path().join("home"))
                .output()
                .expect("invoke installed fixture binary");
            assert!(
                output.status.success(),
                "installed binary failed: {}",
                output_text(&output)
            );
            assert_eq!(String::from_utf8_lossy(&output.stdout), expected_receipt());
            assert!(
                output.stderr.is_empty(),
                "installed binary wrote stderr: {}",
                output_text(&output)
            );
        }
    }

    #[test]
    fn packaged_revision_t01_missing_manifest_is_usage_error_before_write() {
        let native = platform_native_member();
        let fixture = Fixture::new_without_manifest(
            "t01",
            &[
                Member::new(native, b"native fixture\n".to_vec(), 0o644),
                Member::new("oc2", runtime_payload(REVISION), 0o755),
                Member::new("sbom.json", sbom_payload(), 0o644),
            ],
        );

        let output = fixture.run(None);

        assert_eq!(output.status.code(), Some(64), "{}", output_text(&output));
        fixture.assert_unchanged(&output);
    }

    #[test]
    fn packaged_revision_t02_canonical_good_manifest_installs_and_observes_runtime_receipt() {
        let fixture = Fixture::new("t02", &good_members());

        let output = fixture.run(Some(&fixture.manifest));

        assert_eq!(output.status.code(), Some(0), "{}", output_text(&output));
        assert!(fixture.root.install_dir().join("oc2").is_file());
        assert!(fixture.root.native_dir().join(native_name()).is_file());
        fixture.assert_installed_revision();
    }

    #[test]
    fn packaged_revision_t03_manifest_archive_member_and_runtime_revision_mismatches_fail_closed() {
        let cases = [
            ("archive-digest", 65),
            ("member-digest", 65),
            ("runtime-revision", 74),
        ];

        for (label, expected_code) in cases {
            let root = TestRoot::new(&format!("t03-{label}"));
            let entries = good_members();
            let archive = make_archive(root.path(), label, &entries);
            let checksum = sha256_file(&archive);
            let manifest = root.path().join("manifest.json");
            let mut spec = manifest_spec(&entries, &checksum, REVISION);
            match label {
                "archive-digest" => spec.archive_sha256 = "f".repeat(64),
                "member-digest" => {
                    let hash = spec
                        .members
                        .iter_mut()
                        .find(|(name, _)| name == "oc2")
                        .expect("oc2 member");
                    hash.1 = "e".repeat(64);
                }
                "runtime-revision" => spec.revision = OTHER_REVISION.to_owned(),
                _ => unreachable!(),
            }
            fs::write(&manifest, spec.to_bytes()).expect("write mismatch manifest");
            seed_targets(&root);
            let fixture = Fixture {
                archive,
                manifest,
                checksum,
                root,
            };

            let output = fixture.run(Some(&fixture.manifest));

            assert_eq!(
                output.status.code(),
                Some(expected_code),
                "{label}: {}",
                output_text(&output)
            );
            fixture.assert_unchanged(&output);
        }
    }

    #[test]
    fn packaged_revision_t04_extra_traversal_and_duplicate_members_fail_before_write() {
        let native = platform_native_member();
        let sbom = sbom_payload();
        let cases = [
            (
                "extra",
                vec![
                    Member::new(native.clone(), b"native fixture\n".to_vec(), 0o644),
                    Member::new("oc2", runtime_payload(REVISION), 0o755),
                    Member::new("sbom.json", sbom.clone(), 0o644),
                    Member::new("extra.txt", b"extra\n".to_vec(), 0o644),
                ],
            ),
            (
                "duplicate",
                vec![
                    Member::new(native.clone(), b"native fixture\n".to_vec(), 0o644),
                    Member::new("oc2", runtime_payload(REVISION), 0o755),
                    Member::new("oc2", runtime_payload(REVISION), 0o755),
                    Member::new("sbom.json", sbom.clone(), 0o644),
                ],
            ),
            (
                "traversal",
                vec![
                    Member::new(native, b"native fixture\n".to_vec(), 0o644),
                    Member::new("oc2", runtime_payload(REVISION), 0o755),
                    Member::new("sbom.json", sbom, 0o644),
                    Member::new("../escape", b"escape\n".to_vec(), 0o644),
                ],
            ),
        ];

        for (label, entries) in cases {
            let fixture = Fixture::new(&format!("t04-{label}"), &entries);
            let output = fixture.run(Some(&fixture.manifest));

            assert_eq!(
                output.status.code(),
                Some(65),
                "{label}: {}",
                output_text(&output)
            );
            fixture.assert_unchanged(&output);
        }
    }

    #[test]
    fn packaged_revision_t05_staged_and_installed_version_use_exact_revision_grammar() {
        let fixture = Fixture::new("t05", &good_members());

        let output = fixture.run(Some(&fixture.manifest));

        assert_eq!(output.status.code(), Some(0), "{}", output_text(&output));
        fixture.assert_installed_revision();
    }

    fn good_members() -> Vec<Member> {
        vec![
            Member::new(
                platform_native_member(),
                b"native fixture\n".to_vec(),
                0o644,
            ),
            Member::new("oc2", runtime_payload(REVISION), 0o755),
            Member::new("sbom.json", sbom_payload(), 0o644),
        ]
    }

    fn runtime_payload(revision: &str) -> Vec<u8> {
        let receipt = format!("oc2 {PACKAGE_VERSION} revision={revision}");
        format!(
            "#!/bin/sh\ncase \"${{1-}}\" in\n  --version) printf '%s\\n' '{receipt}' ;;\n  --help) printf '%s\\n' 'oc2 fixture help' ;;\n  *) exit 64 ;;\nesac\n"
        )
        .into_bytes()
    }

    fn expected_receipt() -> String {
        format!("oc2 {PACKAGE_VERSION} revision={REVISION}\n")
    }

    fn sbom_payload() -> Vec<u8> {
        br#"{"spdxVersion":"SPDX-2.3","name":"oc2-package-receipt-test"}"#.to_vec()
    }

    fn platform() -> &'static str {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            "macos-arm64"
        }
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            "macos-x64"
        }
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            "linux-arm64"
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            "linux-x64"
        }
        #[cfg(not(any(
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "linux", target_arch = "aarch64"),
            all(target_os = "linux", target_arch = "x86_64")
        )))]
        {
            panic!("packaged receipt test has no fixture for this Unix target")
        }
    }

    fn platform_native_member() -> String {
        format!("native/lib/{}/{}", platform(), native_name())
    }

    fn native_name() -> &'static str {
        #[cfg(target_os = "macos")]
        {
            "libopentui.dylib"
        }
        #[cfg(target_os = "linux")]
        {
            "libopentui.so"
        }
    }

    fn make_archive(root: &Path, stem: &str, entries: &[Member]) -> PathBuf {
        let tar_path = root.join(format!("{stem}.tar"));
        let archive = root.join(format!("{stem}.tar.gz"));
        let mut tar = Vec::new();
        for entry in entries {
            let header = tar_header(entry);
            tar.extend_from_slice(&header);
            tar.extend_from_slice(&entry.payload);
            let padding = (512 - (entry.payload.len() % 512)) % 512;
            tar.extend(std::iter::repeat_n(0_u8, padding));
        }
        tar.extend(std::iter::repeat_n(0_u8, 1024));
        fs::write(&tar_path, tar).expect("write deterministic tar fixture");

        let output = Command::new("gzip")
            .args(["-n", "-9", "-c"])
            .arg(&tar_path)
            .env_clear()
            .env("PATH", SYSTEM_PATH)
            .env("LC_ALL", "C")
            .output()
            .expect("gzip fixture");
        assert!(
            output.status.success(),
            "gzip failed: {}",
            output_text(&output)
        );
        fs::write(&archive, output.stdout).expect("write deterministic archive");
        let _ = fs::remove_file(tar_path);
        archive
    }

    fn tar_header(entry: &Member) -> [u8; 512] {
        assert!(entry.name.len() <= 100, "fixture name too long");
        let mut header = [0_u8; 512];
        header[..entry.name.len()].copy_from_slice(entry.name.as_bytes());
        put_octal(&mut header[100..108], entry.mode as u64, 7);
        put_octal(&mut header[108..116], 0, 7);
        put_octal(&mut header[116..124], 0, 7);
        put_octal(&mut header[124..136], entry.payload.len() as u64, 11);
        put_octal(&mut header[136..148], 0, 11);
        header[148..156].fill(b' ');
        header[156] = b'0';
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        let checksum: u32 = header.iter().map(|byte| *byte as u32).sum();
        let checksum = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(checksum.as_bytes());
        header
    }

    fn put_octal(field: &mut [u8], value: u64, digits: usize) {
        let value = format!("{value:0digits$o}");
        assert_eq!(value.len(), digits);
        field[..digits].copy_from_slice(value.as_bytes());
        field[digits] = 0;
    }

    struct ManifestSpec {
        archive_sha256: String,
        members: Vec<(String, String)>,
        platform: String,
        revision: String,
        sbom_sha256: String,
    }

    impl ManifestSpec {
        fn to_bytes(&self) -> Vec<u8> {
            let members = self
                .members
                .iter()
                .map(|(name, hash)| format!("\"{name}\":\"{hash}\""))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"archive_sha256\":\"{}\",\"members\":{{{}}},\"platform\":\"{}\",\"revision\":\"{}\",\"sbom\":{{\"path\":\"sbom.json\",\"sha256\":\"{}\"}},\"schema\":\"oc2-release-receipt/v1\"}}",
                self.archive_sha256,
                members,
                self.platform,
                self.revision,
                self.sbom_sha256
            )
            .into_bytes()
        }
    }

    fn manifest_spec(entries: &[Member], archive_sha256: &str, revision: &str) -> ManifestSpec {
        let mut members: Vec<(String, String)> = entries
            .iter()
            .map(|entry| (entry.name.clone(), sha256_bytes(&entry.payload)))
            .collect();
        members.sort_by(|left, right| left.0.cmp(&right.0));
        let sbom_sha256 = members
            .iter()
            .find(|(name, _)| name == "sbom.json")
            .map(|(_, hash)| hash.clone())
            .expect("sbom member");
        ManifestSpec {
            archive_sha256: archive_sha256.to_owned(),
            members,
            platform: platform().to_owned(),
            revision: revision.to_owned(),
            sbom_sha256,
        }
    }

    fn manifest_bytes(
        entries: &[Member],
        archive_sha256: &str,
        revision: &str,
        platform: &str,
    ) -> Vec<u8> {
        let mut spec = manifest_spec(entries, archive_sha256, revision);
        spec.platform = platform.to_owned();
        spec.to_bytes()
    }

    fn seed_targets(root: &TestRoot) {
        let install = root.install_dir();
        let native = root.native_dir();
        fs::create_dir_all(&install).expect("create install sentinel directory");
        fs::create_dir_all(&native).expect("create native sentinel directory");
        fs::write(install.join("oc2"), b"old-binary\n").expect("seed binary sentinel");
        fs::write(native.join(native_name()), b"old-native\n").expect("seed native sentinel");
    }

    fn sha256_file(path: &Path) -> String {
        for (program, args) in [("shasum", vec!["-a", "256"]), ("sha256sum", vec![])] {
            let output = Command::new(program)
                .args(args)
                .arg(path)
                .env_clear()
                .env("PATH", SYSTEM_PATH)
                .output();
            if let Ok(output) = output {
                if output.status.success() {
                    let digest = String::from_utf8_lossy(&output.stdout)
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_owned();
                    assert_eq!(
                        digest.len(),
                        64,
                        "bad sha256 output: {}",
                        output_text(&output)
                    );
                    return digest;
                }
            }
        }
        panic!("no sha256 utility available")
    }

    fn sha256_bytes(bytes: &[u8]) -> String {
        let path = std::env::temp_dir().join(format!(
            "oc2-package-receipt-hash-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).expect("write hash input");
        let digest = sha256_file(&path);
        let _ = fs::remove_file(path);
        digest
    }

    fn output_text(output: &Output) -> String {
        format!(
            "status={:?}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }
}

#[cfg(windows)]
#[test]
fn windows_installer_contract_declares_manifest_receipt_and_native_members() {
    let source = include_str!("../../../scripts/install-oc2.ps1");
    for required in [
        "$Manifest",
        "ZipArchive",
        "sbom.json",
        "opentui.dll",
        "libopentui.dll.a",
        "revision=",
    ] {
        assert!(
            source.contains(required),
            "PowerShell installer is missing {required:?}"
        );
    }
}
