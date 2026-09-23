"""Frozen RED tests for TUI-011 installer security defects.

Tests reproduce three defects in scripts/install-oc2.sh (base 764087b):

1. Pre-existing symlinked install dir / sibling lib dir is not rejected before
   writes; installer can write outside caller-selected root.
2. --uninstall leaves sibling libopentui residue; repaired uninstall must remove
   binary/library link objects without following their external targets.
3. Controlled failure after destination temp creation leaves .oc2.tmp.* /
   .libopentui.tmp.* residue and does not restore old binary/library bytes.

Python stdlib only. Disposable TemporaryDirectory roots. Isolated HOME/TMPDIR.
Constant argv, no eval, no real secrets.

Run:
    python3 tests/bootstrap/test_tui011_installer_security.py
"""
import hashlib
import io
import os
import pathlib
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


class Tui011InstallerSecurityTests(unittest.TestCase):
    def setUp(self):
        self.platform, self.library_suffix = _host_profile()
        self.assertTrue(SCRIPT.is_file())

    def _fixture(self):
        outer = tempfile.TemporaryDirectory(prefix="tui011 sec ")
        root = pathlib.Path(outer.name)
        fixture = root / "fixture root with spaces"
        fixture.mkdir()
        (fixture / "home").mkdir()
        (fixture / "tmp").mkdir()
        return outer, root, fixture

    def _oc2(self):
        return b"#!/bin/sh\nprintf '%s\\n' 'oc2 fixture 1.0'\n"

    def _native_name(self):
        return f"libopentui{self.library_suffix}"

    def _native_archive_name(self):
        return f"native/lib/{self.platform}/{self._native_name()}"

    def _valid_archive(self, fixture):
        archive = fixture / "valid security archive.tar.gz"
        _archive(
            archive,
            [
                ("oc2", self._oc2(), 0o755),
                (self._native_archive_name(), b"native security fixture\n", 0o644),
            ],
        )
        return archive

    def _env(self, fixture, path=None):
        return {
            "HOME": str(fixture / "home"),
            "TMPDIR": str(fixture / "tmp"),
            "PATH": path or "/usr/bin:/bin:/usr/sbin:/sbin",
            "LC_ALL": "C",
        }

    def _run_install(self, fixture, archive, install_dir, *, env=None):
        return subprocess.run(
            [
                "sh",
                str(SCRIPT),
                "--archive",
                str(archive),
                "--checksum",
                _archive_sha256(archive),
                "--install-dir",
                str(install_dir),
                "--version",
                "fixture",
            ],
            cwd=ROOT,
            env=env or self._env(fixture),
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )

    def _run_uninstall(self, fixture, install_dir):
        return subprocess.run(
            ["sh", str(SCRIPT), "--install-dir", str(install_dir), "--uninstall"],
            cwd=ROOT,
            env=self._env(fixture),
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )

    def test_rejects_symlinked_destination_directories_before_any_write(self):
        scenarios = ("bin", "lib")
        for symlinked in scenarios:
            with self.subTest(symlinked=symlinked):
                outer, _root, fixture = self._fixture()
                try:
                    archive = self._valid_archive(fixture)
                    selected = fixture / "selected prefix"
                    selected.mkdir()
                    outside = fixture / "outside destination"
                    outside.mkdir()
                    install_dir = selected / "bin"
                    native_dir = selected / "lib"

                    if symlinked == "bin":
                        outside_bin = outside / "bin"
                        outside_bin.mkdir()
                        install_dir.symlink_to(outside_bin, target_is_directory=True)
                        native_dir.mkdir()
                    else:
                        install_dir.mkdir()
                        outside_lib = outside / "lib"
                        outside_lib.mkdir()
                        native_dir.symlink_to(outside_lib, target_is_directory=True)

                    marker = outside / "preserve.txt"
                    marker.write_bytes(b"outside-before\n")
                    result = self._run_install(fixture, archive, install_dir)

                    self.assertNotEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(marker.read_bytes(), b"outside-before\n")
                    self.assertFalse((outside / "bin" / "oc2").exists())
                    self.assertFalse((outside / "lib" / self._native_name()).exists())
                    self.assertFalse((install_dir / "oc2").exists())
                    self.assertFalse((native_dir / self._native_name()).exists())
                finally:
                    outer.cleanup()

    def test_uninstall_removes_native_closure_without_following_file_symlinks(self):
        outer, _root, fixture = self._fixture()
        try:
            selected = fixture / "installed prefix"
            install_dir = selected / "bin"
            native_dir = selected / "lib"
            install_dir.mkdir(parents=True)
            native_dir.mkdir()
            binary = install_dir / "oc2"
            native = native_dir / self._native_name()
            unrelated = native_dir / "keep.txt"
            binary.write_bytes(b"installed-binary\n")
            native.write_bytes(b"installed-native\n")
            unrelated.write_bytes(b"unrelated\n")

            result = self._run_uninstall(fixture, install_dir)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(binary.exists())
            self.assertFalse(native.exists())
            self.assertEqual(unrelated.read_bytes(), b"unrelated\n")

            outside_binary = fixture / "outside-binary"
            outside_native = fixture / "outside-native"
            outside_binary.write_bytes(b"external-binary\n")
            outside_native.write_bytes(b"external-native\n")
            binary.symlink_to(outside_binary)
            native.symlink_to(outside_native)

            result = self._run_uninstall(fixture, install_dir)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(binary.exists())
            self.assertFalse(binary.is_symlink())
            self.assertFalse(native.exists())
            self.assertFalse(native.is_symlink())
            self.assertEqual(outside_binary.read_bytes(), b"external-binary\n")
            self.assertEqual(outside_native.read_bytes(), b"external-native\n")
            self.assertEqual(unrelated.read_bytes(), b"unrelated\n")
        finally:
            outer.cleanup()

    def test_setup_failure_cleans_destination_temps_and_preserves_old_files(self):
        outer, _root, fixture = self._fixture()
        try:
            archive = self._valid_archive(fixture)
            selected = fixture / "upgrade prefix"
            install_dir = selected / "bin"
            native_dir = selected / "lib"
            install_dir.mkdir(parents=True)
            native_dir.mkdir()
            binary = install_dir / "oc2"
            native = native_dir / self._native_name()
            binary.write_bytes(b"old-binary\n")
            native.write_bytes(b"old-native\n")

            wrapper_dir = fixture / "controlled path"
            wrapper_dir.mkdir()
            counter = fixture / "mktemp-count"
            real_mktemp = pathlib.Path("/usr/bin/mktemp")
            if not real_mktemp.is_file():
                real_mktemp = pathlib.Path("/bin/mktemp")
            if not real_mktemp.is_file():
                self.skipTest("system mktemp path unavailable")
            wrapper = wrapper_dir / "mktemp"
            wrapper.write_text(
                "#!/bin/sh\n"
                "count=0\n"
                "[ ! -f \"$TUI011_MKTEMP_COUNT\" ] || count=$(cat \"$TUI011_MKTEMP_COUNT\")\n"
                "count=$((count + 1))\n"
                "printf '%s\\n' \"$count\" >\"$TUI011_MKTEMP_COUNT\"\n"
                "[ \"$count\" -ne 3 ] || exit 1\n"
                f"exec {real_mktemp} \"$@\"\n",
                encoding="utf-8",
            )
            wrapper.chmod(0o755)
            env = self._env(fixture, f"{wrapper_dir}:/usr/bin:/bin:/usr/sbin:/sbin")
            env["TUI011_MKTEMP_COUNT"] = str(counter)

            result = self._run_install(fixture, archive, install_dir, env=env)
            self.assertNotEqual(result.returncode, 0, result.stderr)
            self.assertEqual(counter.read_text(encoding="utf-8").strip(), "3")
            self.assertEqual(binary.read_bytes(), b"old-binary\n")
            self.assertEqual(native.read_bytes(), b"old-native\n")
            self.assertEqual(list(install_dir.glob(".oc2.tmp.*")), [])
            self.assertEqual(list(native_dir.glob(".libopentui.tmp.*")), [])
            self.assertEqual(list(install_dir.glob(".oc2.backup.*")), [])
            self.assertEqual(list(native_dir.glob(".libopentui.backup.*")), [])
        finally:
            outer.cleanup()


if __name__ == "__main__":
    unittest.main()
