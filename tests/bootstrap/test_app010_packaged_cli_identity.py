"""APP-010/G7 contract for the identity of the installed ``oc2`` entrypoint.

This deliberately exercises an installed copy rather than a Cargo target.  The
same test is therefore useful before and after the release-entrypoint fix, and
does not encode a source hash or inspect implementation details.
"""

import os
import pathlib
import shutil
import subprocess
import tempfile
import unittest


MAX_OUTPUT = 64 * 1024
TIMEOUT_SECONDS = 3
LEGACY_IDENTITIES = ("opencode-rk", "opencode2")


def _installed_binary() -> pathlib.Path:
    value = os.environ.get("OC2_TEST_BINARY")
    if not value:
        raise AssertionError("OC2_TEST_BINARY must name the installed oc2 binary")
    path = pathlib.Path(value)
    if not path.is_file():
        raise AssertionError(f"OC2_TEST_BINARY is not a file: {path}")
    return path


def _run_inspection(binary: pathlib.Path, root: pathlib.Path, *args: str) -> str:
    data = root / "data"
    project = root / "project"
    env = {
        "HOME": str(root / "home"),
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "LANG": "C",
        "LC_ALL": "C",
        "TMPDIR": str(root / "tmp"),
        "XDG_CONFIG_HOME": str(root / "config"),
        "XDG_DATA_HOME": str(root / "xdg-data"),
        "XDG_RUNTIME_DIR": str(root / "runtime"),
        "OPENCODE_RK_HOME": str(data),
        "OPENCODE_PROJECT_DIR": str(project),
        "OC2_OFFLINE": "1",
    }
    for directory in env.values():
        # Only directory-valued entries are created below; the fixed switches
        # and locale values are intentionally not interpreted as paths.
        if directory.startswith(str(root)):
            pathlib.Path(directory).mkdir(parents=True, exist_ok=True)

    completed = subprocess.run(
        [str(binary), *args],
        cwd=project,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=TIMEOUT_SECONDS,
        check=False,
    )
    output = completed.stdout[: MAX_OUTPUT + 1]
    if len(output) > MAX_OUTPUT:
        raise AssertionError(f"{args!r} exceeded the {MAX_OUTPUT}-byte output bound")
    text = output.decode("utf-8", errors="replace")
    if completed.returncode != 0:
        raise AssertionError(f"{args!r} exited {completed.returncode}: {text!r}")
    return text


def _forbidden_or_generated_paths(root: pathlib.Path) -> list[pathlib.Path]:
    found = []
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if path.name in {"backend.json", "sessions.db", "session.db"}:
            found.append(path)
        elif path.suffix.lower() in {".sqlite", ".sqlite3", ".db"}:
            found.append(path)
    return found


class PackagedCliIdentityTests(unittest.TestCase):
    def test_installed_version_is_oc2_identity_and_side_effect_free(self):
        source = _installed_binary()
        with tempfile.TemporaryDirectory(prefix="app010-version-") as temporary:
            root = pathlib.Path(temporary)
            binary = root / "install" / "bin" / "oc2"
            binary.parent.mkdir(parents=True)
            shutil.copy2(source, binary)
            output = _run_inspection(binary, root, "--version")

            self.assertIn("oc2", output)
            for legacy in LEGACY_IDENTITIES:
                self.assertNotIn(legacy, output)
            self.assertEqual(_forbidden_or_generated_paths(root), [])

    def test_installed_help_is_oc2_usage_and_side_effect_free(self):
        source = _installed_binary()
        with tempfile.TemporaryDirectory(prefix="app010-help-") as temporary:
            root = pathlib.Path(temporary)
            binary = root / "install" / "bin" / "oc2"
            binary.parent.mkdir(parents=True)
            shutil.copy2(source, binary)
            output = _run_inspection(binary, root, "--help")

            self.assertIn("Usage: oc2", output)
            for legacy in LEGACY_IDENTITIES:
                self.assertNotIn(legacy, output)
            self.assertEqual(_forbidden_or_generated_paths(root), [])


if __name__ == "__main__":
    unittest.main()
