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
tmp fixtures only (no mocks, no host writes). T04/T05 run the dynamic
installer checks FIRST so RED names the current installer gap (failed
identity validation destroys the preexisting binary instead of
restoring it); the workflow assertions follow, proving the missing
wiring as well. T04 fixture-sanity passes via the existing script,
proving RED is missing behavior, not fixture/syntax error.
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
        # Dynamic first: real installer/scripts must invoke actual signing
        # tooling or fail closed when identities are absent. A comment or
        # `echo signed` fixture is not evidence; require executable checks.
        examined: list[Path] = []
        if INSTALLER.is_file():
            examined.append(INSTALLER)
        for cand in (
            ROOT / ".github" / "workflows" / "release.yml",
            ROOT / "scripts" / "sign-release.sh",
            ROOT / "scripts" / "verify-release.sh",
        ):
            if cand.is_file() and cand not in examined:
                examined.append(cand)
        text = "\n".join(p.read_text(encoding="utf-8", errors="replace") for p in examined)
        # Strip comment-only lines so a "# codesign ..." comment cannot pass.
        code_lines = [ln for ln in text.splitlines() if not ln.lstrip().startswith("#")]
        code = "\n".join(code_lines)
        low_code = code.lower()
        self.assertIn("codesign", low_code, "missing real macOS signing command (codesign)")
        self.assertIn("notarytool", low_code, "missing real macOS notarization command (notarytool)")
        self.assertTrue(
            ("signtool" in low_code) or ("azuresigntool" in low_code),
            "missing real Windows signing command (signtool or AzureSignTool)",
        )
        # Executable fail-closed secret check: a real empty-secret guard
        # referencing a signing-identity variable (not a bare `set -u`,
        # not a comment, not `echo signed`).
        self.assertTrue(
            re.search(
                r"\[\s*-([nz])\s+[\"']?\$(?:\{)?\s*(?:APPLE|MACOS|NOTARY|AZURE|WINDOWS|SIGN)",
                code,
            )
            is not None
            or re.search(
                r":\s*\$\{\s*(?:APPLE|MACOS|NOTARY|AZURE|WINDOWS|SIGN)[A-Z_]*\s*:\?",
                code,
            )
            is not None,
            "missing executable fail-closed secret check for signing identity",
        )
        self.assertNotRegex(
            code, r"(?im)^\s*echo\s+['\"]?signed['\"]?\s*$",
            "pretend signature without real tooling",
        )
        body = read_workflow(self)  # RED while workflow absent
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
        # Dynamic FIRST: a nonempty platform-mismatched executable whose
        # --version/--help identity is intentionally non-oc2 must be
        # rejected, and rejection must leave a preexisting destination
        # binary byte-identical (current installer gap: rm -f destroys it).
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            dest = tmp / "dest"
            dest.mkdir()
            pre = dest / "oc2"
            pre.write_bytes(b"v1-good-binary-bytes")
            pre.chmod(pre.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
            pre_hash = sha256(pre)
            pre_len = len(pre.read_bytes())
            self.assertGreater(pre_len, 0, "preexisting fixture must be nonempty")
            foreign = tmp / "oc2"
            foreign.write_text(
                "#!/usr/bin/env sh\n"
                'if [ "$1" = "--version" ]; then echo "foreign-tool 9.9.9"; exit 0; fi\n'
                'if [ "$1" = "--help" ]; then echo "foreign-tool 9.9.9 usage help"; exit 0; fi\n'
                'echo "foreign-tool 9.9.9"; exit 0\n',
                encoding="utf-8",
            )
            foreign.chmod(foreign.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
            wrong = make_archive(tmp, "wrong-arch.tar.gz", foreign)
            r0 = install(wrong, dest)
            self.assertNotEqual(r0.returncode, 0, "platform-mismatched binary must be rejected")
            self.assertTrue(pre.is_file(), "rejection must preserve preexisting binary (not delete it)")
            self.assertEqual(sha256(pre), pre_hash, "preexisting binary must be byte-identical")
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
        body = read_workflow(self)  # RED while workflow absent
        low = body.lower()
        self.assertIn("arch", low, "workflow must gate architecture")
        self.assertIn("checksum", low, "workflow must gate checksum")

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
        # Dynamic FIRST: install v1, then attempt a checksum-valid archive
        # whose oc2 fails identity validation; the installer must exit
        # nonzero, restore v1 bytes/version, and leave disposable user
        # data unchanged (current gap: identity rm -f leaves no binary).
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            dest = tmp / "bin"
            dest.mkdir()
            data = tmp / "userdata.txt"
            data.write_text("must-survive", encoding="utf-8")
            v1 = make_archive(tmp, "v1.tar.gz", make_oc2_stub(tmp, "oc2 1.0.0"))
            self.assertEqual(install(v1, dest).returncode, 0, "v1 install failed")
            v1_bytes = (dest / "oc2").read_bytes()
            v1_hash = sha256(dest / "oc2")
            foreign_src = tmp / "foreign-src"
            foreign_src.mkdir()
            imposter = foreign_src / "oc2"
            imposter.write_text(
                "#!/usr/bin/env sh\n"
                'if [ "$1" = "--version" ]; then echo "foreign-tool 2.0.0"; exit 0; fi\n'
                'if [ "$1" = "--help" ]; then echo "foreign-tool 2.0.0 usage help"; exit 0; fi\n'
                'echo "foreign-tool 2.0.0"; exit 0\n',
                encoding="utf-8",
            )
            imposter.chmod(imposter.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
            bad_upgrade = make_archive(tmp, "bad-upgrade.tar.gz", imposter)
            r_bad = install(bad_upgrade, dest)
            self.assertNotEqual(r_bad.returncode, 0, "identity-failing upgrade must be rejected")
            self.assertTrue((dest / "oc2").is_file(), "failed upgrade must restore v1 binary")
            self.assertEqual(sha256(dest / "oc2"), v1_hash, "v1 bytes must be restored")
            self.assertEqual((dest / "oc2").read_bytes(), v1_bytes, "v1 bytes must match exactly")
            self.assertIn("oc2 1.0.0", run([str(dest / "oc2"), "--version"]).stdout)
            self.assertEqual(data.read_text(encoding="utf-8"), "must-survive")
            v2 = make_archive(tmp, "v2.tar.gz", make_oc2_stub(tmp, "oc2 2.0.0"))
            r = install(v2, dest)
            self.assertEqual(r.returncode, 0, f"upgrade failed: {r.stderr[-500:]}")
            self.assertEqual(data.read_text(encoding="utf-8"), "must-survive")
            self.assertIn("oc2 2.0.0", run([str(dest / "oc2"), "--version"]).stdout)
        body = read_workflow(self)  # RED while workflow absent
        low = body.lower()
        self.assertTrue(
            ("rollback" in low) or ("previous" in low and "artifact" in low),
            "workflow must define rollback to previous artifact",
        )
        self.assertTrue(
            ("schema" in low) or ("migrat" in low) or ("user data" in low or "user-data" in low),
            "workflow must guard against silent data/schema loss",
        )


if __name__ == "__main__":
    unittest.main()
