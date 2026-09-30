import hashlib
import io
import pathlib
import stat
import subprocess
import tarfile
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "install-oc2.sh"
MAX_ARCHIVE_MEMBERS = 4
MAX_ARCHIVE_PAYLOAD = 1024 * 1024


def _host_profile():
    system = subprocess.check_output(["uname", "-s"], text=True).strip()
    machine = subprocess.check_output(["uname", "-m"], text=True).strip()
    os_name = {"Linux": "linux", "Darwin": "macos"}.get(system)
    arch = {"x86_64": "x64", "amd64": "x64", "arm64": "arm64", "aarch64": "arm64"}.get(machine)
    if os_name is None or arch is None:
        raise unittest.SkipTest(f"install-oc2.sh does not support {system}/{machine}")
    suffix = ".so" if os_name == "linux" else ".dylib"
    return f"{os_name}-{arch}", suffix


def _archive(path, entries):
    with tarfile.open(path, "w:gz") as output:
        for name, payload, mode in entries:
            info = tarfile.TarInfo(name)
            info.size = len(payload)
            info.mode = mode
            info.uid = info.gid = 0
            info.uname = info.gname = "root"
            info.mtime = 0
            output.addfile(info, io.BytesIO(payload))


def _archive_sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Tui011InstallerNativeClosureTests(unittest.TestCase):
    def setUp(self):
        self.platform, self.library_suffix = _host_profile()
        self.assertTrue(SCRIPT.is_file())

    def _fixture(self):
        outer = tempfile.TemporaryDirectory(prefix="tui011 installer ")
        root = pathlib.Path(outer.name)
        fixture = root / "fixture root with spaces"
        fixture.mkdir()
        (fixture / "home").mkdir()
        (fixture / "tmp").mkdir()
        return outer, root, fixture

    def _run(self, fixture, archive, install_dir, checksum=None):
        env = {
            "HOME": str(fixture / "home"),
            "TMPDIR": str(fixture / "tmp"),
            "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            "LC_ALL": "C",
        }
        command = [
            "sh",
            str(SCRIPT),
            "--archive",
            str(archive),
            "--checksum",
            checksum or _archive_sha256(archive),
            "--install-dir",
            str(install_dir),
            "--version",
            "fixture",
        ]
        return subprocess.run(
            command,
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )

    def _oc2(self, text="oc2 fixture 1.0"):
        return f"#!/bin/sh\nprintf '%s\\n' '{text}'\n".encode()

    def _native_name(self):
        return f"libopentui{self.library_suffix}"

    def _native_archive_name(self):
        return f"native/lib/{self.platform}/{self._native_name()}"

    def _assert_bounded(self, archive):
        with tarfile.open(archive, "r:gz") as source:
            members = source.getmembers()
        self.assertLessEqual(len(members), MAX_ARCHIVE_MEMBERS)
        self.assertLessEqual(sum(max(member.size, 0) for member in members), MAX_ARCHIVE_PAYLOAD)

    def _assert_no_outer_writes(self, outer, root):
        self.assertEqual(sorted(path.name for path in root.iterdir()), ["fixture root with spaces"])
        self.assertTrue(root.is_dir())
        self.assertTrue(outer.name)

    def test_native_bundle_and_missing_library_are_atomic(self):
        failures = []

        outer, root, fixture = self._fixture()
        try:
            install_dir = fixture / "installed path with spaces" / "bin"
            archive = fixture / "valid native archive.tar.gz"
            _archive(
                archive,
                [
                    ("oc2", self._oc2(), 0o755),
                    (self._native_archive_name(), b"native closure fixture\n", 0o644),
                ],
            )
            self._assert_bounded(archive)
            result = self._run(fixture, archive, install_dir)
            binary = install_dir / "oc2"
            native = install_dir.parent / "lib" / self._native_name()
            if result.returncode != 0:
                failures.append(f"valid bundle returned {result.returncode}: {result.stderr}")
            if not binary.is_file():
                failures.append("valid bundle did not install oc2")
            elif not binary.stat().st_mode & stat.S_IXUSR:
                failures.append("valid bundle installed a non-executable oc2")
            if not native.is_file():
                failures.append(f"valid bundle did not install {native}")
            self._assert_no_outer_writes(outer, root)
        finally:
            outer.cleanup()

        outer, root, fixture = self._fixture()
        try:
            install_dir = fixture / "missing native install" / "bin"
            archive = fixture / "missing native archive.tar.gz"
            _archive(archive, [("oc2", self._oc2(), 0o755)])
            self._assert_bounded(archive)
            result = self._run(fixture, archive, install_dir)
            binary = install_dir / "oc2"
            if result.returncode == 0:
                failures.append("archive missing native library was accepted")
            if binary.exists():
                failures.append("missing native library left a partial installed binary")
            self._assert_no_outer_writes(outer, root)
        finally:
            outer.cleanup()

        self.assertEqual(failures, [], "\n".join(failures))

    def test_checksum_gate_remains_fail_closed(self):
        outer, root, fixture = self._fixture()
        try:
            install_dir = fixture / "checksum install" / "bin"
            archive = fixture / "checksum archive.tar.gz"
            _archive(archive, [("oc2", self._oc2(), 0o755), (self._native_archive_name(), b"native\n", 0o644)])
            result = self._run(fixture, archive, install_dir, checksum="0" * 64)
            self.assertEqual(result.returncode, 65, result.stderr)
            self.assertFalse((install_dir / "oc2").exists())
            self._assert_no_outer_writes(outer, root)
        finally:
            outer.cleanup()

    def test_identity_gate_remains_fail_closed(self):
        outer, root, fixture = self._fixture()
        try:
            install_dir = fixture / "identity install" / "bin"
            archive = fixture / "identity archive.tar.gz"
            _archive(
                archive,
                [
                    ("oc2", self._oc2("opencode-rk fixture 1.0"), 0o755),
                    (self._native_archive_name(), b"native\n", 0o644),
                ],
            )
            result = self._run(fixture, archive, install_dir)
            self.assertEqual(result.returncode, 74, result.stderr)
            self.assertFalse((install_dir / "oc2").exists())
            self.assertFalse((install_dir.parent / "lib" / self._native_name()).exists())
            self._assert_no_outer_writes(outer, root)
        finally:
            outer.cleanup()

    def test_malformed_archive_leaves_no_partial_binary(self):
        outer, root, fixture = self._fixture()
        try:
            install_dir = fixture / "malformed install" / "bin"
            archive = fixture / "malformed archive.tar.gz"
            archive.write_bytes(b"not a gzip archive")
            result = self._run(fixture, archive, install_dir)
            self.assertNotEqual(result.returncode, 0, result.stderr)
            self.assertFalse((install_dir / "oc2").exists())
            self._assert_no_outer_writes(outer, root)
        finally:
            outer.cleanup()

    def test_traversal_member_leaves_no_partial_binary(self):
        outer, root, fixture = self._fixture()
        try:
            install_dir = fixture / "traversal install" / "bin"
            archive = fixture / "traversal archive.tar.gz"
            _archive(
                archive,
                [
                    ("oc2", self._oc2(), 0o755),
                    (f"native/lib/{self.platform}/../../../../escape/{self._native_name()}", b"escape\n", 0o644),
                ],
            )
            self._assert_bounded(archive)
            result = self._run(fixture, archive, install_dir)
            self.assertNotEqual(result.returncode, 0, result.stderr)
            self.assertFalse((install_dir / "oc2").exists())
            self._assert_no_outer_writes(outer, root)
        finally:
            outer.cleanup()


if __name__ == "__main__":
    unittest.main()
