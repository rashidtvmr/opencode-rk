"""Adversarial unit contracts for derived native-readiness probes.

These tests use only the six current, frozen native E2E sources as inputs.  They
do not launch the CLI, allocate a PTY, inspect runtime Recovery Mode state, or
change any of the source fixtures.
"""

from __future__ import annotations

import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock

from tools import recovery_readiness_extension as extension


ROOT = pathlib.Path(__file__).resolve().parents[2]
NATIVE_PATHS = (
    "tests/e2e/native_provider_setup.py",
    "tests/e2e/native_stream_tool.py",
    "tests/e2e/native_utf8_input.py",
    "tests/e2e/native_escape_input.py",
    "tests/e2e/native_resize.py",
    "tests/e2e/native_bracketed_paste.py",
)
STARTUP_LINES = {
    "tests/e2e/native_provider_setup.py": 259,
    "tests/e2e/native_stream_tool.py": 301,
    "tests/e2e/native_utf8_input.py": 105,
    "tests/e2e/native_escape_input.py": 250,
    "tests/e2e/native_resize.py": 184,
    "tests/e2e/native_bracketed_paste.py": 502,
}


def _source(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def _must_reject(call) -> None:
    """Assert input rejection without treating the implementation stub as RED."""
    try:
        call()
    except NotImplementedError:
        raise
    except (ValueError, RuntimeError, AssertionError):
        return
    raise AssertionError("unsafe or mutated input was accepted")


def _child_fixture_source(assertion: str) -> str:
    """Small fixture with the exact provider setup startup call at line 259."""
    # The subprocess prelude initializes master/buf/screen and starts a bounded
    # pipe producer. This source keeps the transformed call followed by the
    # original downstream assertions and marker write, all on its pinned line.
    prefix = "\n" * 258
    return prefix + (
        "read_until(master, time.monotonic() + 1.0, buf, b'OpenCode', 0, screen); "
        "assert b'\\x1b[?2026l' in buf; "
        f"{assertion}; "
        "open(marker_path, 'w', encoding='utf-8').write('downstream ran')\n"
    )


def _run_derived_child(source: str):
    """Execute only a derived fixture in a disposable, secret-free child env."""
    with tempfile.TemporaryDirectory(prefix="readiness-child-") as temp:
        root = pathlib.Path(temp)
        marker = root / "downstream.txt"
        module = root / "derived_fixture.py"
        module.write_text(
            "import os, select, time\n"
            "class _Screen:\n"
            "    def __init__(self): self.data = bytearray()\n"
            "    def feed(self, chunk): self.data.extend(chunk)\n"
            "    def contains(self, text): return text.casefold().encode() in bytes(self.data).lower()\n"
            f"marker_path = {str(marker)!r}\n"
            "read_fd, write_fd = os.pipe()\n"
            "def _produce():\n"
            "    os.write(write_fd, b'\\x1b[?2026hOpenCode\\r\\nAsk anything')\n"
            "    time.sleep(0.08)\n"
            "    os.write(write_fd, b'\\x1b[?2026l')\n"
            "    os.close(write_fd)\n"
            "import threading\n"
            "threading.Thread(target=_produce, daemon=True).start()\n"
            "master = read_fd\n"
            "buf = bytearray()\n"
            "screen = _Screen()\n"
            + source,
            encoding="utf-8",
        )
        child_env = {"HOME": str(root), "PATH": "/usr/bin:/bin", "PYTHONIOENCODING": "utf-8"}
        started = time.monotonic()
        result = subprocess.run(
            [sys.executable, "-I", str(module)],
            cwd=root,
            env=child_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=2.0,
            check=False,
        )
        marker_text = marker.read_text(encoding="utf-8") if marker.exists() else None
        return result, marker_text, time.monotonic() - started


class _Screen:
    """Minimal screen observation object accepted by readiness_wait."""

    def __init__(self) -> None:
        self.data = bytearray()

    def feed(self, chunk: bytes) -> None:
        self.data.extend(chunk)

    def contains(self, marker: str) -> bool:
        return marker.casefold().encode("utf-8") in bytes(self.data).lower()


class ReadinessDerivationTests(unittest.TestCase):
    def test_derives_each_current_frozen_native_startup_site(self):
        for path in NATIVE_PATHS:
            with self.subTest(path=path):
                original = _source(path)
                derived, report = extension.derive_source(original, path)

                self.assertIsInstance(derived, str)
                self.assertNotEqual(derived, original)
                self.assertIsInstance(report, dict)
                self.assertEqual(report.get("original_sha256"), hashlib.sha256(original.encode()).hexdigest())
                self.assertEqual(report.get("derived_sha256"), hashlib.sha256(derived.encode()).hexdigest())
                self.assertTrue(report.get("downstream_ast_sha256"))
                self.assertTrue(report.get("transformations"))
                # The allowlisted source invocation is the expected pinned site;
                # transformed helper code can legitimately shift output lines.
                site = STARTUP_LINES[path]
                self.assertIn("read_until(", original.splitlines()[site - 1])
                self.assertIn("b\"OpenCode\"", original.splitlines()[site - 1])
                self.assertIn("readiness_wait", derived)

                verified = extension.verify_derivation(original, derived, path)
                self.assertIsInstance(verified, dict)
                self.assertEqual(verified.get("original_sha256"), report["original_sha256"])
                self.assertEqual(verified.get("derived_sha256"), report["derived_sha256"])

    def test_rejects_paths_outside_the_explicit_six_source_grant(self):
        source = _source(NATIVE_PATHS[0])
        for path in (
            "tests/e2e/native_interactive_pty.py",
            "tests/e2e/native_second_client.py",
            "tools/recovery_state.py",
            "tests/e2e/../bootstrap/test_readiness_extension.py",
            "/tmp/native_provider_setup.py",
        ):
            with self.subTest(path=path):
                _must_reject(lambda: extension.derive_source(source, path))

    def test_requires_exactly_one_canonical_startup_wait(self):
        path = NATIVE_PATHS[0]
        source = _source(path)
        startup_call = 'read_until(master, time.monotonic()+20, buf, b"OpenCode", 0, screen)'
        self.assertEqual(source.count(startup_call), 1)
        _must_reject(lambda: extension.derive_source(
            source.replace(startup_call, startup_call.replace("OpenCode", "No marker"), 1), path
        ))
        duplicate = source.replace(
            startup_call,
            startup_call + "\n        " + startup_call,
            1,
        )
        _must_reject(lambda: extension.derive_source(duplicate, path))

    def test_verifier_rejects_downstream_semantic_assertion_mutation(self):
        path = NATIVE_PATHS[0]
        original = _source(path)
        derived, _ = extension.derive_source(original, path)
        before = 'if not screen.has_model_status(): raise AssertionError("selected model is not in current status projection")'
        after = 'if screen.has_model_status(): raise AssertionError("selected model is not in current status projection")'
        self.assertIn(before, derived)
        mutated = derived.replace(before, after, 1)
        _must_reject(lambda: extension.verify_derivation(original, mutated, path))

    def test_verifier_rejects_unapproved_import_change(self):
        path = NATIVE_PATHS[0]
        original = _source(path)
        derived, _ = extension.derive_source(original, path)
        self.assertIn("readiness_wait", derived)
        mutated = derived.replace("readiness_wait", "readiness_wait_untrusted", 1)
        _must_reject(lambda: extension.verify_derivation(original, mutated, path))

    def test_verifier_rejects_startup_fixture_argument_change(self):
        path = NATIVE_PATHS[0]
        original = _source(path)
        derived, _ = extension.derive_source(original, path)
        mutated = derived.replace('b"OpenCode"', 'b"OpenCodx"', 1)
        self.assertNotEqual(mutated, derived)
        _must_reject(lambda: extension.verify_derivation(original, mutated, path))

    def test_verifier_rejects_a_different_source_path_binding(self):
        path = NATIVE_PATHS[0]
        original = _source(path)
        derived, _ = extension.derive_source(original, path)
        _must_reject(lambda: extension.verify_derivation(original, derived, NATIVE_PATHS[1]))

    def test_independent_frozen_source_pin_rejects_changed_downstream_assertion(self):
        path = NATIVE_PATHS[0]
        baseline_source = _source(path)
        _derived, baseline = extension.derive_source(baseline_source, path)
        verifier = _api("verify_frozen_source")
        self.assertIsInstance(baseline.get("assertion_inventory_sha256"), str)
        changed = baseline_source.replace(
            'if not screen.has_model_status(): raise AssertionError("selected model is not in current status projection")',
            'if screen.has_model_status(): raise AssertionError("selected model is not in current status projection")',
            1,
        )
        self.assertNotEqual(changed, baseline_source)
        _must_reject(lambda: verifier(changed, path, baseline))

    def test_independent_frozen_source_pin_rejects_changed_import_and_loader(self):
        path = NATIVE_PATHS[0]
        baseline_source = _source(path)
        _derived, baseline = extension.derive_source(baseline_source, path)
        verifier = _api("verify_frozen_source")
        for changed in (
            baseline_source.replace("import argparse, hashlib", "import argparse, hashlib, untrusted", 1),
            baseline_source.replace("def read_until(", "def read_until_untrusted(", 1),
            baseline_source + "\n__import__('runpy').run_module('tools.phase1_judge')\n",
        ):
            with self.subTest(prefix=changed[:80]):
                _must_reject(lambda: verifier(changed, path, baseline))

    def test_derived_child_reaches_unchanged_downstream_assertion_on_completed_frame(self):
        original = _child_fixture_source(assertion="assert screen.contains('Ask anything')")
        derived, _ = extension.derive_source(original, NATIVE_PATHS[0])
        extension.verify_derivation(original, derived, NATIVE_PATHS[0])
        self.assertIn("assert screen.contains('Ask anything')", derived)
        result, marker, elapsed = _run_derived_child(derived)
        self.assertEqual(result.returncode, 0, result.stderr.decode("utf-8", "replace"))
        self.assertEqual(marker, "downstream ran", "downstream assertion did not execute")
        self.assertGreaterEqual(elapsed, 0.06, "helper returned on partial/legacy content")

    def test_derived_child_preserves_a_failing_downstream_assertion(self):
        original = _child_fixture_source(assertion="assert screen.contains('intentionally absent')")
        derived, _ = extension.derive_source(original, NATIVE_PATHS[0])
        extension.verify_derivation(original, derived, NATIVE_PATHS[0])
        self.assertIn("assert screen.contains('intentionally absent')", derived)
        result, marker, _ = _run_derived_child(derived)
        self.assertNotEqual(result.returncode, 0, "derived execution masked a false downstream assertion")
        self.assertIsNone(marker, "execution continued past the false assertion")


class ReadinessIdentityTests(unittest.TestCase):
    def setUp(self):
        self.identity = {
            "canonicalSha": "a" * 40,
            "branch": "main-v2",
            "clean": True,
            "protectedBlobs": {"tools/phase1_judge.py": "b" * 40},
            "policySha256": "c" * 64,
            "guardSourceSha256": "d" * 64,
            "refHookSha256": "e" * 64,
            "guardGreen": True,
        }

    def test_accepts_complete_identical_canonical_identity(self):
        self.assertIsNone(extension.verify_identity(self.identity, dict(self.identity)))

    def test_rejects_missing_required_identity_fields(self):
        incomplete = dict(self.identity)
        del incomplete["canonicalSha"]
        _must_reject(lambda: extension.verify_identity(self.identity, incomplete))

    def test_rejects_drift_in_each_canonical_or_frozen_guard_value(self):
        for key, changed in (
            ("canonicalSha", "f" * 40),
            ("branch", "recovery/platform"),
            ("clean", False),
            ("protectedBlobs", {"tools/phase1_judge.py": "f" * 40}),
            ("policySha256", "f" * 64),
            ("guardSourceSha256", "f" * 64),
            ("refHookSha256", "f" * 64),
            ("guardGreen", False),
        ):
            with self.subTest(field=key):
                actual = dict(self.identity)
                actual[key] = changed
                _must_reject(lambda: extension.verify_identity(self.identity, actual))

    def test_rejects_non_boolean_guard_and_clean_flags(self):
        for key in ("clean", "guardGreen"):
            with self.subTest(field=key):
                actual = dict(self.identity)
                actual[key] = "true"
                _must_reject(lambda: extension.verify_identity(self.identity, actual))


class OriginalJudgeReceiptTests(unittest.TestCase):
    def setUp(self):
        self.sha = "a" * 40
        self.receipt = {
            "schemaVersion": 2,
            "sourceSha": self.sha,
            "canonicalSha": self.sha,
            "branch": "main-v2",
            "cleanWorktree": True,
            "artifactMode": True,
            "status": "FAIL",
            "checks": [{"name": "native_provider_setup", "exit": 1}],
            "gateResults": {"P1-PROVIDER": {"status": "FAIL"}},
        }

    def test_retains_original_installed_judge_failure_without_promoting_it(self):
        validated = self._validate(self.receipt, self.sha)
        self.assertIsInstance(validated, dict)
        self.assertEqual(validated.get("status"), "FAIL")
        self.assertEqual(validated.get("sourceSha"), self.sha)
        self.assertFalse(validated.get("accepted", False))

    def test_rejects_judge_receipt_bound_to_different_sha(self):
        with self.assertRaises((ValueError, RuntimeError, AssertionError)):
            self._validate(self.receipt, "b" * 40)

    def test_rejects_receipt_missing_canonical_identity(self):
        receipt = dict(self.receipt)
        del receipt["canonicalSha"]
        with self.assertRaises((ValueError, RuntimeError, AssertionError)):
            self._validate(receipt, self.sha)

    def test_rejects_receipt_with_incomplete_check_or_unknown_status(self):
        for change in (
            {"status": "READINESS_EXTENSION_PASS"},
            {"checks": [{"exit": 1}]},
            {"checks": [{"name": "native_provider_setup", "exit": "1"}]},
        ):
            with self.subTest(change=change):
                receipt = dict(self.receipt)
                receipt.update(change)
                with self.assertRaises((ValueError, RuntimeError, AssertionError)):
                    self._validate(receipt, self.sha)

    def test_rejects_overall_pass_with_failed_check_or_failed_gate(self):
        contradictory = dict(self.receipt, status="PASS")
        for receipt in (
            dict(contradictory),
            dict(contradictory, checks=[{"name": "native_provider_setup", "exit": 0}]),
        ):
            with self.subTest(checks=receipt["checks"]):
                with self.assertRaises((ValueError, RuntimeError, AssertionError)):
                    self._validate(receipt, self.sha)

        failed_check = dict(contradictory, checks=[{"name": "native_provider_setup", "exit": 1}])
        failed_check["gateResults"] = {"P1-PROVIDER": {"status": "PASS"}}
        with self.assertRaises((ValueError, RuntimeError, AssertionError)):
            self._validate(failed_check, self.sha)

    def test_rejects_overall_fail_when_every_check_and_gate_passes(self):
        receipt = dict(self.receipt, status="FAIL", checks=[{"name": "native_provider_setup", "exit": 0}])
        receipt["gateResults"] = {"P1-PROVIDER": {"status": "PASS"}}
        with self.assertRaises((ValueError, RuntimeError, AssertionError)):
            self._validate(receipt, self.sha)

    def test_accepts_consistent_overall_pass_with_all_checks_and_gates_passing(self):
        receipt = dict(self.receipt, status="PASS", checks=[{"name": "native_provider_setup", "exit": 0}])
        receipt["gateResults"] = {"P1-PROVIDER": {"status": "PASS"}}
        validated = self._validate(receipt, self.sha)
        self.assertEqual(validated.get("status"), "PASS")
        self.assertEqual(validated.get("sourceSha"), self.sha)

    def test_accepts_consistent_fail_with_nonzero_check_even_if_gates_pass(self):
        receipt = dict(self.receipt, status="FAIL", checks=[{"name": "native_provider_setup", "exit": 2}])
        receipt["gateResults"] = {"P1-PROVIDER": {"status": "PASS"}}
        validated = self._validate(receipt, self.sha)
        self.assertEqual(validated.get("status"), "FAIL")

    def test_rejects_malformed_or_unknown_gate_statuses_and_duplicate_check_names(self):
        for gate_results, checks in (
            ({"P1-PROVIDER": {"status": "MAYBE"}}, [{"name": "native_provider_setup", "exit": 1}]),
            ({"P1-PROVIDER": {}}, [{"name": "native_provider_setup", "exit": 1}]),
            ({"P1-PROVIDER": "FAIL"}, [{"name": "native_provider_setup", "exit": 1}]),
            ({"P1-PROVIDER": {"status": "FAIL"}}, [
                {"name": "native_provider_setup", "exit": 1},
                {"name": "native_provider_setup", "exit": 0},
            ]),
        ):
            with self.subTest(gates=gate_results, checks=checks):
                receipt = dict(self.receipt, gateResults=gate_results, checks=checks)
                with self.assertRaises((ValueError, RuntimeError, AssertionError)):
                    self._validate(receipt, self.sha)

    def _validate(self, receipt, source_sha):
        validator = _api("validate_original_judge")
        return validator(receipt, source_sha)


class ArtifactBindingTests(unittest.TestCase):
    def setUp(self):
        self.expected = {
            "canonicalSha": "a" * 40,
            "binary_sha256": "b" * 64,
            "native_library_sha256": "c" * 64,
        }
        self.build = {
            "source_sha": self.expected["canonicalSha"],
            "binary_sha256": self.expected["binary_sha256"],
            "native_library_sha256": self.expected["native_library_sha256"],
        }

    def test_accepts_exact_source_and_artifact_pin(self):
        result = _api("verify_artifacts")(
            self.build,
            self.expected["binary_sha256"],
            self.expected["native_library_sha256"],
            self.expected,
        )
        self.assertIsInstance(result, dict)
        self.assertEqual(result.get("source_sha"), self.expected["canonicalSha"])
        self.assertEqual(result.get("binary_sha256"), self.expected["binary_sha256"])
        self.assertEqual(result.get("native_library_sha256"), self.expected["native_library_sha256"])

    def test_rejects_self_consistent_but_unpinned_build_claim(self):
        forged = dict(self.build)
        forged["binary_sha256"] = "d" * 64
        _must_reject(lambda: _api("verify_artifacts")(
            forged, forged["binary_sha256"], self.expected["native_library_sha256"], self.expected
        ))

    def test_rejects_build_for_another_sha_even_when_artifact_hashes_match(self):
        wrong_source = dict(self.build, source_sha="e" * 40)
        _must_reject(lambda: _api("verify_artifacts")(
            wrong_source, self.expected["binary_sha256"], self.expected["native_library_sha256"], self.expected
        ))

    def test_rejects_artifact_digest_that_disagrees_with_the_pin(self):
        _must_reject(lambda: _api("verify_artifacts")(
            self.build, "f" * 64, self.expected["native_library_sha256"], self.expected
        ))


class ReadinessWaitTests(unittest.TestCase):
    def test_partial_startup_marker_does_not_pass_until_current_composer_is_visible(self):
        read_fd, write_fd = os.pipe()
        screen = _Screen()
        buf = bytearray()
        finished = threading.Event()

        def producer():
            try:
                try:
                    os.write(write_fd, b"\x1b[?2026hOpen")
                except BrokenPipeError:
                    return
                time.sleep(0.04)
                try:
                    os.write(write_fd, b"Code\r\nAsk anything")
                except BrokenPipeError:
                    return
                time.sleep(0.08)
                try:
                    os.write(write_fd, b"\x1b[?2026l")
                except BrokenPipeError:
                    return
                finished.set()
            finally:
                os.close(write_fd)

        writer = threading.Thread(target=producer, daemon=True)
        writer.start()
        began = time.monotonic()
        try:
            extension.readiness_wait(
                read_fd,
                began + 1.0,
                buf,
                b"OpenCode",
                0,
                screen,
            )
            elapsed = time.monotonic() - began
            self.assertGreaterEqual(elapsed, 0.08)
            self.assertIn(b"OpenCode", buf)
            self.assertIn(b"\x1b[?2026l", buf)
            self.assertTrue(screen.contains("Ask anything"))
            writer.join(timeout=1)
            self.assertFalse(writer.is_alive())
            self.assertTrue(finished.is_set())
        finally:
            __import__("os").close(read_fd)
            writer.join(timeout=1)
            self.assertFalse(writer.is_alive())

    def test_predicate_is_required_in_addition_to_marker_and_screen_text(self):
        import os

        read_fd, write_fd = os.pipe()
        screen = _Screen()
        buf = bytearray()
        allowed = threading.Event()

        def producer():
            try:
                os.write(write_fd, b"\x1b[?2026hOpenCode\r\nAsk anything\x1b[?2026l")
                time.sleep(0.08)
                allowed.set()
                os.write(write_fd, b"\x1b[?2026hOpenCode\r\nAsk anything\x1b[?2026l")
            except BrokenPipeError:
                pass
            finally:
                os.close(write_fd)

        writer = threading.Thread(target=producer, daemon=True)
        writer.start()
        began = time.monotonic()
        try:
            extension.readiness_wait(
                read_fd,
                began + 1.0,
                buf,
                b"OpenCode",
                0,
                screen,
                predicate=allowed.is_set,
            )
            self.assertGreaterEqual(time.monotonic() - began, 0.06)
            self.assertTrue(allowed.is_set())
            self.assertTrue(screen.contains("Ask anything"))
            self.assertGreaterEqual(buf.count(b"\x1b[?2026l"), 2)
        finally:
            os.close(read_fd)
            writer.join(timeout=1)
            self.assertFalse(writer.is_alive())

    def test_legacy_branding_and_composer_without_completed_frame_is_not_ready(self):
        import os

        read_fd, write_fd = os.pipe()
        screen = _Screen()

        def producer():
            try:
                os.write(write_fd, b"OpenCode\r\nAsk anything")
            finally:
                os.close(write_fd)

        writer = threading.Thread(target=producer, daemon=True)
        writer.start()
        try:
            with self.assertRaises((AssertionError, RuntimeError, TimeoutError)):
                extension.readiness_wait(
                    read_fd,
                    time.monotonic() + 0.25,
                    bytearray(),
                    b"OpenCode",
                    0,
                    screen,
                )
        finally:
            os.close(read_fd)
            writer.join(timeout=1)
            self.assertFalse(writer.is_alive())

    def test_completed_current_composer_frame_does_not_require_legacy_branding(self):
        read_fd, write_fd = os.pipe()
        screen = _Screen()
        buf = bytearray()

        def producer():
            try:
                os.write(write_fd, b"\x1b[?2026hAsk anything\x1b[?2026l")
            finally:
                os.close(write_fd)

        writer = threading.Thread(target=producer, daemon=True)
        writer.start()
        try:
            extension.readiness_wait(
                read_fd, time.monotonic() + 0.5, buf, b"OpenCode", 0, screen
            )
            self.assertTrue(screen.contains("Ask anything"))
            self.assertNotIn(b"OpenCode", buf)
        finally:
            os.close(read_fd)
            writer.join(timeout=1)
            self.assertFalse(writer.is_alive())

    def test_rejects_a_legacy_branding_needle_as_the_readiness_contract(self):
        read_fd, write_fd = os.pipe()
        writer = threading.Thread(
            target=lambda: (os.close(write_fd)),
            daemon=True,
        )
        writer.start()
        try:
            with self.assertRaises((ValueError, RuntimeError, AssertionError)):
                extension.readiness_wait(
                    read_fd, time.monotonic() + 0.5, bytearray(), b"Ask anything", 0, _Screen()
                )
        finally:
            os.close(read_fd)
            writer.join(timeout=1)
            self.assertFalse(writer.is_alive())


class ProbeCleanupTests(unittest.TestCase):
    def test_normal_exit_does_not_signal_process_group_after_leader_is_reaped(self):
        execute_probe = _api("execute_probe")
        real_popen = subprocess.Popen
        real_killpg = os.killpg
        launched = []
        invalid_signals = []

        def capture_popen(*args, **kwargs):
            child = real_popen(*args, **kwargs)
            launched.append(child)
            return child

        def observe_killpg(pgid, sig):
            if sig != 0 and launched and pgid == launched[0].pid and launched[0].returncode is not None:
                invalid_signals.append((pgid, sig, launched[0].returncode))
                raise AssertionError("refusing process-group signal after the owned leader was reaped")
            return real_killpg(pgid, sig)

        with tempfile.TemporaryDirectory(prefix="readiness-exit-order-") as temp:
            root = pathlib.Path(temp)
            with mock.patch.object(extension.subprocess, "Popen", side_effect=capture_popen), \
                 mock.patch.object(extension.os, "killpg", side_effect=observe_killpg):
                result = execute_probe(
                    [sys.executable, "-c", "print('bounded normal exit')"],
                    root,
                    {"PATH": "/usr/bin:/bin"},
                    root / "probe.log",
                    timeout=2.0,
                )

        self.assertEqual(result, 0)
        self.assertEqual(len(launched), 1)
        self.assertIsNotNone(launched[0].returncode)
        self.assertEqual(invalid_signals, [])

    def test_timeout_kills_descendant_that_inherits_probe_output_pipe(self):
        execute_probe = _api("execute_probe")
        with tempfile.TemporaryDirectory(prefix="readiness-cleanup-") as temp:
            root = pathlib.Path(temp)
            pid_file = root / "descendant.pid"
            code = (
                "import pathlib, subprocess, sys; "
                "child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); "
                f"pathlib.Path({str(pid_file)!r}).write_text(str(child.pid))"
            )
            log = root / "probe.log"
            began = time.monotonic()
            with self.assertRaises(TimeoutError):
                execute_probe([sys.executable, "-c", code], root, {"PATH": "/usr/bin:/bin"}, log, timeout=0.3)
            self.assertLess(time.monotonic() - began, 4.0)
            self.assertTrue(pid_file.is_file(), "probe did not create its owned descendant")
            pid = int(pid_file.read_text(encoding="utf-8"))
            deadline = time.monotonic() + 2.0
            while time.monotonic() < deadline:
                try:
                    os.kill(pid, 0)
                except ProcessLookupError:
                    break
                time.sleep(0.02)
            else:
                self.fail("timed-out probe left its owned descendant alive")


def _api(name: str):
    function = getattr(extension, name, None)
    if not callable(function):
        raise AssertionError(f"required readiness extension API is missing: {name}")
    return function


if __name__ == "__main__":
    unittest.main()
