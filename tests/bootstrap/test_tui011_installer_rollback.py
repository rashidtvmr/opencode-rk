"""Independent RED contract for non-destructive oc2 installer rollback."""
from __future__ import annotations

import hashlib
import pathlib
import platform
import shutil
import subprocess
import tarfile
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "install-oc2.sh"


def host_profile() -> tuple[str, str]:
    system = platform.system()
    machine = platform.machine()
    os_name = {"Darwin": "macos", "Linux": "linux"}.get(system)
    arch = {"x86_64": "x64", "amd64": "x64", "arm64": "arm64", "aarch64": "arm64"}.get(machine)
    if os_name is None or arch is None:
        raise unittest.SkipTest(f"unsupported host profile: {system}/{machine}")
    return f"{os_name}-{arch}", ".dylib" if os_name == "macos" else ".so"


class InstallerRollbackTests(unittest.TestCase):
    def make_archive(self, root: pathlib.Path, profile: str, suffix: str) -> pathlib.Path:
        archive = root / "release.tar.gz"
        binary = b"#!/bin/sh\nprintf '%s\\n' oc2 fixture\n"
        native = b"new native library bytes"
        with tarfile.open(archive, "w:gz", dereference=False) as tar:
            for name, payload, mode in (
                ("oc2", binary, 0o755),
                (f"native/lib/{profile}/libopentui{suffix}", native, 0o644),
            ):
                info = tarfile.TarInfo(name)
                info.size = len(payload)
                info.mode = mode
                info.type = tarfile.REGTYPE
                tar.addfile(info, __import__("io").BytesIO(payload))
        return archive

    def test_second_rename_failure_restores_existing_binary_and_native(self) -> None:
        profile, suffix = host_profile()
        with tempfile.TemporaryDirectory(prefix="tui011-rollback-") as tmp:
            root = pathlib.Path(tmp)
            home = root / "home"
            install = home / ".local" / "bin"
            native_dir = install.parent / "lib"
            install.mkdir(parents=True)
            native_dir.mkdir(parents=True)
            old_binary = b"OLD-BINARY-BYTES"
            old_native = b"OLD-NATIVE-BYTES"
            (install / "oc2").write_bytes(old_binary)
            (native_dir / f"libopentui{suffix}").write_bytes(old_native)
            archive = self.make_archive(root, profile, suffix)
            checksum = hashlib.sha256(archive.read_bytes()).hexdigest()

            wrapper_bin = root / "wrapper-bin"
            wrapper_bin.mkdir()
            real_mv = shutil.which("mv") or "/bin/mv"
            target = str(install / "oc2")
            (wrapper_bin / "mv").write_text(
                "#!/bin/sh\n"
                "last=\"\"\n"
                "for arg do last=\"$arg\"; done\n"
                f"[ \"$last\" = {target!r} ] && exit 77\n"
                f"exec {real_mv} \"$@\"\n",
                encoding="utf-8",
            )
            (wrapper_bin / "mv").chmod(0o755)
            env = {
                "HOME": str(home),
                "TMPDIR": str(root / "tmp"),
                "PATH": f"{wrapper_bin}:/usr/bin:/bin",
                "OC2_INSTALL_DIR": str(install),
            }
            pathlib.Path(env["TMPDIR"]).mkdir()
            result = subprocess.run(
                ["/bin/sh", str(SCRIPT), "--archive", str(archive), "--checksum", checksum],
                cwd=ROOT,
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                timeout=15,
                check=False,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertEqual((install / "oc2").read_bytes(), old_binary, result.stdout)
            self.assertEqual((native_dir / f"libopentui{suffix}").read_bytes(), old_native, result.stdout)


if __name__ == "__main__":
    unittest.main()
