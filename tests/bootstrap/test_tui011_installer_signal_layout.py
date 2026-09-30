"""Independent installer signal/producer-layout contract tests."""
from __future__ import annotations

import hashlib
import io
import os
import pathlib
import platform
import shlex
import shutil
import subprocess
import tarfile
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = pathlib.Path(os.environ.get("TUI011_INSTALLER_SCRIPT", str(ROOT / "scripts" / "install-oc2.sh")))


def profile() -> tuple[str, str]:
    os_name = {"Darwin": "macos", "Linux": "linux"}.get(platform.system())
    arch = {"x86_64": "x64", "amd64": "x64", "arm64": "arm64", "aarch64": "arm64"}.get(platform.machine())
    if not os_name or not arch:
        raise unittest.SkipTest("unsupported host profile")
    return f"{os_name}-{arch}", ".dylib" if os_name == "macos" else ".so"


class InstallerSignalAndLayoutTests(unittest.TestCase):
    def archive(self, root: pathlib.Path, nested: bool) -> pathlib.Path:
        prof, suffix = profile()
        archive = root / ("nested.tar.gz" if nested else "two-members.tar.gz")
        binary = b"#!/bin/sh\nprintf '%s\\n' oc2 fixture\n"
        native_name = f"native/lib/{prof}/libopentui{suffix}"
        members = [("oc2", binary, 0o755), (native_name, b"new-native", 0o644)]
        if nested:
            members = [("native/", b"", 0), ("native/lib/", b"", 0), (f"native/lib/{prof}/", b"", 0)] + members
        with tarfile.open(archive, "w:gz") as tar:
            for name, payload, mode in members:
                info = tarfile.TarInfo(name)
                info.type = tarfile.DIRTYPE if name.endswith("/") else tarfile.REGTYPE
                info.size = len(payload)
                info.mode = mode
                tar.addfile(info, io.BytesIO(payload))
        return archive

    def run_installer(self, script: pathlib.Path, archive: pathlib.Path, install: pathlib.Path, root: pathlib.Path, wrapper: pathlib.Path) -> subprocess.CompletedProcess[str]:
        checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
        env = {"HOME": str(root / "home"), "TMPDIR": str(root / "tmp"), "PATH": f"{wrapper}:/usr/bin:/bin", "OC2_INSTALL_DIR": str(install)}
        pathlib.Path(env["HOME"]).mkdir(parents=True, exist_ok=True); pathlib.Path(env["TMPDIR"]).mkdir(parents=True, exist_ok=True)
        return subprocess.run(["/bin/sh", str(script), "--archive", str(archive), "--checksum", checksum], cwd=ROOT, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=15, check=False)

    def test_term_signal_after_native_swap_restores_both_old_files(self) -> None:
        prof, suffix = profile()
        with tempfile.TemporaryDirectory(prefix="tui011-signal-") as tmp:
            root = pathlib.Path(tmp); install = root / "home/.local/bin"; native_dir = install.parent / "lib"; install.mkdir(parents=True); native_dir.mkdir()
            old_binary, old_native = b"OLD-BINARY", b"OLD-NATIVE"
            (install / "oc2").write_bytes(old_binary); (native_dir / f"libopentui{suffix}").write_bytes(old_native)
            archive = self.archive(root, False); wrapper = root / "bin"; wrapper.mkdir(); real_mv = shutil.which("mv") or "/bin/mv"
            native_target = str(native_dir / f"libopentui{suffix}")
            installer_native_target = str(install / "../lib" / f"libopentui{suffix}")
            marker = root / "signal.marker"
            evidence = root / "injected-native.evidence"
            trace = root / "mv.trace"
            trace.touch()
            # The shim delegates every move to the real utility.  It injects
            # exactly once, at the native replacement (not at either staging
            # move and never while the installer is restoring files).
            q_trace = shlex.quote(str(trace))
            q_marker = shlex.quote(str(marker))
            q_evidence = shlex.quote(str(evidence))
            q_target = shlex.quote(installer_native_target)
            (wrapper / "mv").write_text(
                "#!/bin/sh\n"
                "source=\"\"; target=\"\"\n"
                "for arg do previous=\"$last\"; last=\"$arg\"; done\n"
                "source=\"$previous\"; target=\"$last\"\n"
                f"{real_mv} \"$@\"\n"
                "rc=$?\n"
                # Keep the diagnostic useful but bounded even if an installer
                # unexpectedly loops over moves.
                f"if [ \"$(wc -l < {q_trace} 2>/dev/null || printf 0)\" -lt 32 ]; then printf '%s|%s|%s\\n' \"$source\" \"$target\" \"$rc\" >> {q_trace}; fi\n"
                "[ $rc -eq 0 ] || exit $rc\n"
                f"if [ ! -e {q_marker} ] && [ \"$target\" = {q_target} ]; then\n"
                "  case \"${source##*/}\" in\n"
                "    .libopentui.tmp.*)\n"
                f"      {shlex.quote(shutil.which('cp') or '/bin/cp')} \"$target\" {q_evidence} || exit $?\n"
                f"      printf '%s|%s\\n' \"$source\" \"$target\" > {q_marker}\n"
                "      kill -TERM \"$PPID\"\n"
                "      sleep 0.1\n"
                "      ;;\n"
                "  esac\n"
                "fi\n"
                "exit 0\n",
                encoding="utf-8",
            ); (wrapper / "mv").chmod(0o755)
            result = self.run_installer(SCRIPT, archive, install, root, wrapper)
            diagnostic = result.stdout + "\nTRACE:\n" + (trace.read_text() if trace.exists() else "<missing>")
            self.assertEqual(result.returncode, 143, diagnostic)
            self.assertTrue(marker.is_file(), result.stdout + "\nTRACE:\n" + trace.read_text())
            replaced_source, replaced_target = marker.read_text().strip().split("|", 1)
            self.assertEqual(replaced_target, installer_native_target)
            self.assertTrue(pathlib.Path(replaced_source).name.startswith(".libopentui.tmp."), diagnostic)
            self.assertTrue(evidence.is_file(), diagnostic)
            self.assertEqual(evidence.read_bytes(), b"new-native", diagnostic)
            self.assertIn(f"{replaced_source}|{replaced_target}|0", trace.read_text(), diagnostic)
            self.assertEqual((install / "oc2").read_bytes(), old_binary, result.stdout)
            self.assertEqual((native_dir / f"libopentui{suffix}").read_bytes(), old_native, diagnostic)

    def test_historical_nested_directory_entries_are_accepted(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tui011-layout-") as tmp:
            root = pathlib.Path(tmp); install = root / "home/.local/bin"; wrapper = root / "bin"; wrapper.mkdir(parents=True)
            (wrapper / "mv").symlink_to(shutil.which("mv") or "/bin/mv")
            result = self.run_installer(SCRIPT, self.archive(root, True), install, root, wrapper)
            self.assertEqual(result.returncode, 0, result.stdout)
            self.assertTrue((install / "oc2").is_file(), result.stdout)
            _prof, suffix = profile()
            self.assertTrue((install.parent / "lib" / f"libopentui{suffix}").is_file(), result.stdout)


if __name__ == "__main__":
    unittest.main()
