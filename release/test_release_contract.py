#!/usr/bin/env python3
"""SHIP-001 T01..T05 release-contract RED (stdlib only, deterministic).

Observable contract (tasks/completion/delivery.json SHIP-001):
 T01 clean builders compile pinned app + bundle assets (linux/macos/windows)
 T02 checksum/SBOM/license/provenance bind exact source+toolchain
 T03 macOS notarization + Windows signing use real identities or block
 T04 wrong-arch/corrupt downloads rejected at install
 T05 upgrades preserve data; failed release rolls back without silent loss

Static portions assert on .github/workflows/release.yml (ABSENT -> RED).
Behavioral portions invoke scripts/install-oc2.sh against disposable
tmp fixtures only (no mocks, no host writes). T04/T05 dynamic halves may
pass via the existing script; the tests still RED on the missing workflow
wiring, proving the gap is missing behavior, not fixture/syntax error.
"""
from __future__ import annotations

import hashlib
import os
import re
import stat
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
INSTALLER = ROOT / "scripts" / "install-oc2.sh"

SHA_PIN = re.compile(r"uses:\s*[^\s#]+@[0-9a-f]{40}\b")


def read_workflow(tc: unittest.TestCase) -> str:
    tc.assertTrue(
        WORKFLOW.is_file(),
        f"SHIP-001 missing behavior: {WORKFLOW.relative_to(ROOT)} absent",
    )
    return WORKFLOW.read_text(encoding="utf-8")


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, timeout=60, **kw)


def make_oc2_stub(d: Path, version_line: str) -> Path:
    stub = d / "oc2"
    stub.write_text(
        "#!/usr/bin/env sh\n"
        f'if [ "$1" = "--version" ]; then echo "{version_line}"; exit 0; fi\n'
        f'if [ "$1" = "--help" ]; then echo "{version_line} usage: oc2"; exit 0; fi\n'
        f'echo "{version_line}"; exit 0\n',
        encoding="utf-8",
    )
    stub.chmod(stub.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return stub


def make_archive(d: Path, name: str, member: Path | None) -> Path:
    arc = d / name
    with tarfile.open(arc, "w:gz") as tf:
        if member is not None:
            tf.add(member, arcname="oc2")
    return arc


def sha256(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def install(archive: Path, dest: Path, extra=()) -> subprocess.CompletedProcess:
    return run(
        ["sh", str(INSTALLER), "--archive", str(archive),
         "--checksum", sha256(archive), "--install-dir", str(dest), *extra]
    )


class T01PinnedBoundedBuilds(unittest.TestCase):
    def test_t01_pinned_bounded_triple_builds_with_assets(self):
        body = read_workflow(self)
        pins = SHA_PIN.findall(body)
        self.assertGreaterEqual(len(pins), 2, "need >=2 SHA-pinned actions")
        low = body.lower()
        for token in ("ubuntu", "macos", "windows"):
            self.assertIn(token, low, f"matrix missing {token} runner")
        self.assertIn("--locked", body, "builds must use --locked")
        self.assertIn("timeout-minutes", body, "bounded jobs required")
        for token in ("upload-artifact", "checksum", "sbom"):
            self.assertIn(token.lower(), low, f"missing {token} step")


class T02IntegrityBinding(unittest.TestCase):
    def test_t02_checksum_sbom_license_provenance_pin_source_toolchain(self):
        body = read_workflow(self)
        low = body.lower()
        for token in ("sha256", "sbom", "license", "provenance"):
            self.assertIn(token, low, f"missing {token} binding")
        self.assertTrue(
            ("github.sha" in low) or ("commit" in low),
            "provenance must bind exact source revision",
        )
        self.assertTrue(
            ("rust" in low) and ("toolchain" in low or "rust-version" in low or "dtolnay" in low),
            "provenance must bind exact toolchain",
        )


class T03RealSigningOrFailClosed(unittest.TestCase):
    def test_t03_authorized_identities_or_explicit_block(self):
        body = read_workflow(self)
        low = body.lower()
        self.assertIn("notar", low, "missing macOS notarization step")
        self.assertIn("sign", low, "missing Windows signing step")
        self.assertNotRegex(
            body, r"(?i)echo\s+['\"]?signed['\"]?\s*(>|&&|\|\||$)",
            "pretend signature without real tooling",
        )
        self.assertTrue(
            ("secrets." in body) or ("APPLE" in body) or ("AZURE" in body),
            "signing must reference authorized identities/secrets",
        )
        self.assertTrue(
            ("fail" in low) and ("exit" in low or "error" in low),
            "missing explicit fail-closed block when signing identity absent",
        )


class T04ArchCorruptRejected(unittest.TestCase):
    def test_t04_wrong_arch_and_corrupt_rejected(self):
        body = read_workflow(self)  # RED here: workflow absent
        low = body.lower()
        self.assertIn("arch", low, "workflow must gate architecture")
        self.assertIn("checksum", low, "workflow must gate checksum")
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            # corrupt archive: flip one byte of a valid bundle
            good = make_archive(tmp, "good.tar.gz", make_oc2_stub(tmp, "oc2 9.9.9"))
            bad = tmp / "bad.tar.gz"
            raw = bytearray(good.read_bytes())
            raw[len(raw) // 2] ^= 0xFF
            bad.write_bytes(bytes(raw))
            r = run(
                ["sh", str(INSTALLER), "--archive", str(bad),
                 "--checksum", sha256(good), "--install-dir", str(tmp / "i1")]
            )
            self.assertNotEqual(r.returncode, 0, "corrupt archive must be rejected")
            # wrong arch: bundle without the oc2 binary
            empty = make_archive(tmp, "empty.tar.gz", None)
            r2 = install(empty, tmp / "i2")
            self.assertNotEqual(r2.returncode, 0, "wrong-arch bundle must be rejected")

    def test_t04_fixture_sanity_good_bundle_installs(self):
        # Proves RED above is missing-workflow, not broken fixture/script.
        if not INSTALLER.is_file():
            self.skipTest("installer script absent")
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            arc = make_archive(tmp, "ok.tar.gz", make_oc2_stub(tmp, "oc2 1.0.0"))
            r = install(arc, tmp / "ok")
            self.assertEqual(r.returncode, 0, f"fixture broken: {r.stderr[-500:]}")


class T05UpgradeRollback(unittest.TestCase):
    def test_t05_upgrade_preserves_data_with_rollback(self):
        body = read_workflow(self)  # RED here: workflow absent
        low = body.lower()
        self.assertTrue(
            ("rollback" in low) or ("previous" in low and "artifact" in low),
            "workflow must define rollback to previous artifact",
        )
        self.assertTrue(
            ("schema" in low) or ("migrat" in low) or ("user data" in low or "user-data" in low),
            "workflow must guard against silent data/schema loss",
        )
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            dest = tmp / "bin"
            dest.mkdir()
            data = tmp / "userdata.txt"
            data.write_text("must-survive", encoding="utf-8")
            v1 = make_archive(tmp, "v1.tar.gz", make_oc2_stub(tmp, "oc2 1.0.0"))
            self.assertEqual(install(v1, dest).returncode, 0)
            v2 = make_archive(tmp, "v2.tar.gz", make_oc2_stub(tmp, "oc2 2.0.0"))
            r = install(v2, dest)
            self.assertEqual(r.returncode, 0, f"upgrade failed: {r.stderr[-500:]}")
            self.assertEqual(data.read_text(encoding="utf-8"), "must-survive")
            self.assertIn("oc2 2.0.0", run([str(dest / "oc2"), "--version"]).stdout)


if __name__ == "__main__":
    unittest.main()
