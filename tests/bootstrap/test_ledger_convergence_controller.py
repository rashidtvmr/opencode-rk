"""RED contract for the disposable ledger-convergence controller.

The product module is deliberately optional while this suite is authored.  A
missing module therefore produces assertion failures, never collection errors.
All mutations happen below TemporaryDirectory roots.
"""

from __future__ import annotations

import contextlib
import hashlib
import importlib
import json
import math
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CONTROLLER_PATH = ROOT / "tools" / "ledger_convergence_controller.py"
MARKER = b"ledger-convergence-fixture/v1\n"
TXID = "redfixture001"

RETIRE_IDS = frozenset(
    """ACP-001 BASE-004 FIX-LOGROTATE FIX-LOOP-RULES FIX-NATIVE-DAEMON
FIX-PACKAGING FIX-SANDBOX FIX-SESSIONS-STUBS FIX-SQLITE-GATE FIX-TIMELINE
G6-CHAT-DATADIR HEAD-001 HEAD-002 LANE-APPSTART-VIEW LANE-AUTH-401
LANE-AUTODRIVE-CLAMP LANE-CHAT-ORIGIN LANE-CI-CAPS LANE-DESC-STALE
LANE-FILE-AUTHZ LANE-LOOP-CAP LANE-MAIN-ONCE2 LANE-ONBOARD-SETUP
LANE-PROV-FALLBACK LANE-RALPH-MAX2 LANE-SHELL-AUTHZ LANE-SRV-ROUTER
LANE-TIMELINE-LAND LANE-TOOL-PERM LANE-TRANSCRIPT-LAND LANE-TUI-HOST
LANE-TURN-SETTLE LANE-WEB-HONEST OPS-009 PROV-018 PROV-019 PROV-020
PROV-021 PROV-022 REL-003 RUN-001 SDK-001 SDK-002 SYNC-001 SYNC-002
TOOL-012 TOOL-018 TOOL-019 WEB-004 WEB-005 WEB-006
LANE-AGENT-FILES LANE-CI LANE-CI-EXT LANE-CI-FLAG LANE-COMMANDS-LIVE
LANE-CONTEXT-ACCOUNT LANE-CONTEXT-CMD LANE-DISPATCH-DENY LANE-GLOBS
LANE-GLOBS-LIVE LANE-LOOP LANE-LOOP-LIVE LANE-MCP-LIVE LANE-RULES
LANE-SANDBOX LANE-SUBAGENT-LIVE LANE-THEMES LANE-TUI-GRAPH
LANE-ULTRA-CODEGEN LANE-WEB-CANVAS LANE-WF-CREATE LANE-WF-TIMELINE
RC-01 RC-02 RC-03 WEB-EVENT-STREAM WEB-HINT""".split()
)
DEMOTION_IDS = (
    "AUD-017",
    "AUD-020",
    "INSTALLED-DEFAULT-CONTRACT-INTEGRATION",
)

CRASH_POINTS = (
    "before-temp-fsync",
    "after-temp-fsync-before-rename",
    "after-rename-before-directory-fsync",
    "after-directory-fsync-before-phase-a-commit",
    "after-phase-a-commit-before-push",
    "after-phase-a-push-before-phase-b-temp-fsync",
    "after-phase-b-temp-fsync-before-ledger-rename",
    "after-ledger-rename-before-directory-fsync",
    "after-ledger-fsync-before-phase-b-commit",
    "after-phase-b-commit-before-push",
    "after-phase-b-push-before-receipt",
    "after-receipt-temp-fsync",
    "after-receipt-rename",
    "after-receipt-directory-fsync-before-phase-c-commit",
    "after-phase-c-commit-before-push",
    "after-phase-c-push",
)


def _git_env(home: Path) -> dict[str, str]:
    return {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": str(home),
        "LANG": "C",
        "LC_ALL": "C",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_AUTHOR_NAME": "Ledger Fixture",
        "GIT_AUTHOR_EMAIL": "ledger-fixture@example.invalid",
        "GIT_COMMITTER_NAME": "Ledger Fixture",
        "GIT_COMMITTER_EMAIL": "ledger-fixture@example.invalid",
        "GIT_AUTHOR_DATE": "2000-01-01T00:00:00Z",
        "GIT_COMMITTER_DATE": "2000-01-01T00:00:00Z",
    }


def _run_git(root: Path, home: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=root,
        env=_git_env(home),
        text=True,
        capture_output=True,
        timeout=10,
        check=True,
    )


def _copy_fixture_source(root: Path) -> None:
    for rel in (
        "PLAN.md",
        "ralph.json",
        "tasks/completion/claims.json",
        "worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md",
        "worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md",
    ):
        target = root / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / rel, target)
    (root / ".ledger-convergence-disposable").write_bytes(MARKER)


@contextlib.contextmanager
def disposable_fixture() -> Path:
    with tempfile.TemporaryDirectory(prefix="ledger-convergence-red-") as name:
        root = Path(name)
        home = root / ".home"
        home.mkdir()
        _copy_fixture_source(root)
        _run_git(root, home, "init", "-q", "-b", "plan/ledger-convergence")
        _run_git(root, home, "config", "user.name", "Ledger Fixture")
        _run_git(root, home, "config", "user.email", "ledger-fixture@example.invalid")
        _run_git(root, home, "add", "-A")
        _run_git(root, home, "commit", "-qm", "fixture")
        yield root


def _claims_path(root: Path) -> Path:
    return root / "tasks/completion/claims.json"


def _protected_bytes(root: Path) -> dict[str, bytes]:
    paths = (
        "tasks/completion/claims.json",
        "PLAN.md",
        "ralph.json",
        "worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md",
        "worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md",
    )
    return {rel: (root / rel).read_bytes() for rel in paths}


def _read_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def _simulation_dir(root: Path, txid: str = TXID) -> Path:
    return root / ".git" / "ledger-convergence" / "simulations" / txid


def _json_sidecars(root: Path, txid: str = TXID) -> list[tuple[Path, object]]:
    directory = _simulation_dir(root, txid)
    if not directory.exists():
        return []
    found = []
    for path in sorted(directory.rglob("*.json")):
        try:
            found.append((path, _read_json(path)))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError):
            continue
    return found


class IoHooks:
    """Bounded named fault seam used by the controller contract."""

    def __init__(self, fail: str | None = None) -> None:
        self.fail = fail
        self.events: list[str] = []

    def _event(self, name: str, *args: object, **kwargs: object) -> None:
        del args, kwargs
        self.events.append(name)
        if self.fail == name:
            raise OSError(f"injected-{name}")

    def write(self, *args: object, **kwargs: object) -> None:
        self._event("write", *args, **kwargs)

    def flush(self, *args: object, **kwargs: object) -> None:
        self._event("flush", *args, **kwargs)

    def file_fsync(self, *args: object, **kwargs: object) -> None:
        self._event("file_fsync", *args, **kwargs)

    def fsync_file(self, *args: object, **kwargs: object) -> None:
        self._event("file_fsync", *args, **kwargs)

    def replace(self, *args: object, **kwargs: object) -> None:
        self._event("replace", *args, **kwargs)

    def directory_fsync(self, *args: object, **kwargs: object) -> None:
        self._event("directory_fsync", *args, **kwargs)

    def fsync_dir(self, *args: object, **kwargs: object) -> None:
        self._event("directory_fsync", *args, **kwargs)


class ControllerRedTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        try:
            cls.controller = importlib.import_module("tools.ledger_convergence_controller")
        except (ImportError, ModuleNotFoundError):
            cls.controller = None

    def _controller(self):
        self.assertIsNotNone(
            self.controller,
            "tools.ledger_convergence_controller is absent; RED must remain an assertion failure",
        )
        return self.controller

    def _api(self, name: str):
        controller = self._controller()
        self.assertTrue(callable(getattr(controller, name, None)), f"missing public API: {name}")
        return getattr(controller, name)

    def _run_cli(
        self,
        root: Path,
        *args: str,
        timeout: int = 15,
    ) -> subprocess.CompletedProcess[str]:
        self.assertTrue(CONTROLLER_PATH.is_file(), "controller CLI module is absent")
        env = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "PYTHONHASHSEED": "0",
            "PYTHONIOENCODING": "utf-8",
            "LANG": "C",
            "LC_ALL": "C",
            "GIT_TERMINAL_PROMPT": "0",
        }
        return subprocess.run(
            [sys.executable, str(CONTROLLER_PATH), *args],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
            timeout=timeout,
        )

    def _result(self, completed: subprocess.CompletedProcess[str]) -> dict[str, object]:
        self.assertTrue(completed.stdout.strip(), f"missing JSON output: {completed.stderr}")
        try:
            value = json.loads(completed.stdout)
        except json.JSONDecodeError as exc:
            self.fail(f"non-JSON controller output: {exc}: {completed.stdout!r}")
        self.assertIsInstance(value, dict)
        return value

    def _error_code(self, payload: dict[str, object]) -> str | None:
        direct = payload.get("error")
        if isinstance(direct, str):
            return direct
        errors = payload.get("errors")
        if isinstance(errors, list):
            for error in errors:
                if isinstance(error, str):
                    return error
                if isinstance(error, dict) and isinstance(error.get("code"), str):
                    return error["code"]
        return None

    def _simulate(self, root: Path, txid: str = TXID, *extra: str) -> subprocess.CompletedProcess[str]:
        return self._run_cli(root, "--root", str(root), "--mode", "simulate", "--txid", txid, *extra)

    def _assert_no_protected_mutation(self, root: Path, before: dict[str, bytes]) -> None:
        self.assertEqual(before, _protected_bytes(root))

    # T01-T03: independent canonical bytes and non-circular domains.
    def test_t01_jcs_utf8_order_arrays_duplicates_and_nonfinite(self) -> None:
        canonical_json = self._api("canonical_json")
        self.assertEqual(
            canonical_json({"b": 1, "a": [3, True, None, "é"]}),
            '{"a":[3,true,null,"é"],"b":1}'.encode("utf-8"),
        )
        self.assertEqual(canonical_json({"integer": 1.0, "zero": -0.0}), b'{"integer":1,"zero":0}')
        with self.assertRaises(Exception):
            canonical_json('{"duplicate":1,"duplicate":2}')
        for value in (math.nan, math.inf, -math.inf):
            with self.subTest(value=value), self.assertRaises(Exception):
                canonical_json({"value": value})

    def test_t02_hash_domains_match_fixed_vectors(self) -> None:
        row = {"completedNote": "ok", "session": "s", "status": "completed"}
        self.assertEqual(
            self._api("row_hash")(row),
            "92bca4c9c7cc53b53854ff687764fab4db0be9784a1a35bd4ba3a1d19923c339",
        )
        with tempfile.TemporaryDirectory(prefix="ledger-hash-vector-") as tmp:
            vector = Path(tmp) / "vector.bin"
            vector.write_bytes(b'{"x":1}\n')
            self.assertEqual(
                self._api("file_hash")(vector),
                "bb157861a164e35cdde9d726b0af9ce2765a8f530c35d9e45732b94ee65e9557",
            )
        document = {"claims": {"x": {"status": "completed"}}, "schemaVersion": 1}
        expected = (
            b'{\n  "claims": {\n    "x": {\n      "status": "completed"\n    }\n  },\n  "schemaVersion": 1\n}\n'
        )
        self.assertEqual(self._api("ledger_bytes")(document), expected)
        self.assertEqual(
            self._api("ledger_hash")(document),
            "ab97c8b7b1aa2188adb6b1c0cd66f2fd12340bff8ea0ed1fc05fb54464f86fd5",
        )

    def test_t03_manifest_and_receipt_hashes_exclude_their_own_member(self) -> None:
        manifest = {"format": "ledger-convergence-phase-a/v1", "manifestHash": "ignored", "phase": "A"}
        receipt = {"format": "ledger-convergence-receipt/v1", "phase": "C", "receiptHash": "ignored"}
        manifest_hash = self._api("manifest_hash")
        receipt_hash = self._api("receipt_hash")
        self.assertEqual(manifest_hash(manifest), manifest_hash({"format": manifest["format"], "phase": "A"}))
        self.assertEqual(receipt_hash(receipt), receipt_hash({"format": receipt["format"], "phase": "C"}))
        self.assertEqual(manifest_hash(manifest), manifest_hash(manifest))
        self.assertNotEqual(manifest_hash({**manifest, "phase": "B"}), manifest_hash(manifest))
        self.assertNotEqual(receipt_hash({**receipt, "phase": "B"}), receipt_hash(receipt))

    # T04-T06: phase records and exact candidate operation.
    def test_t04_phase_a_manifest_has_sorted_r78_and_no_dependent_fields(self) -> None:
        manifest_hash = self._api("manifest_hash")
        manifest = {
            "format": "ledger-convergence-phase-a/v1",
            "phase": "A",
            "transactionId": TXID,
            "source": {
                "branch": "refs/heads/plan/ledger-convergence",
                "baseCommit": "a" * 40,
                "ledgerPath": "tasks/completion/claims.json",
                "ledgerSha256": "b" * 64,
                "planRefs": [{"path": "PLAN.md", "commit": "a" * 40, "sha256": "c" * 64}],
                "mappingRefs": [
                    {"path": "worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md", "commit": "a" * 40, "sha256": "d" * 64},
                    {"path": "worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md", "commit": "a" * 40, "sha256": "e" * 64},
                ],
            },
            "operation": {"removeIds": sorted(RETIRE_IDS), "demote": []},
            "canonicalization": {"json": "RFC8785-JCS-UTF8", "ledger": "ledger-v1"},
        }
        self.assertEqual(manifest["operation"]["removeIds"], sorted(RETIRE_IDS))
        self.assertNotIn("manifestHash", manifest)
        self.assertNotIn("phaseACommit", manifest)
        self.assertNotIn("candidateLedgerHash", manifest)
        self.assertNotIn("remoteTip", manifest)
        self.assertNotIn("receiptHash", manifest)
        self.assertRegex(manifest_hash(manifest), r"^[0-9a-f]{64}$")

    def test_t05_build_candidate_changes_exactly_r78_and_three_demotions(self) -> None:
        with disposable_fixture() as root:
            source = _read_json(_claims_path(root))
            candidate = self._api("build_candidate")(source)
            self.assertIsInstance(candidate, dict)
            self.assertEqual(set(candidate), set(source))
            source_claims = source["claims"]
            candidate_claims = candidate["claims"]
            self.assertEqual(set(source_claims) - set(candidate_claims), RETIRE_IDS)
            self.assertEqual(set(candidate_claims) - set(source_claims), set())
            for task_id in DEMOTION_IDS:
                before = source_claims[task_id]
                after = candidate_claims[task_id]
                self.assertEqual(before["status"], "completed")
                self.assertEqual(after["status"], "blocked")
                self.assertEqual(after["blockedNote"], before["completedNote"])
                self.assertNotIn("completedNote", after)
            for task_id, row in source_claims.items():
                if task_id not in RETIRE_IDS and task_id not in DEMOTION_IDS:
                    self.assertEqual(candidate_claims[task_id], row)

    def test_t06_phase_hashes_are_append_only_and_never_parent_acceptance(self) -> None:
        with disposable_fixture() as root:
            before = _protected_bytes(root)
            result = self._simulate(root)
            payload = self._result(result)
            for key in (
                "mode",
                "status",
                "transactionId",
                "source",
                "candidate",
                "manifestHash",
                "receiptHash",
                "sideEffects",
                "errors",
            ):
                self.assertIn(key, payload)
            self.assertEqual(payload["mode"], "simulate")
            self.assertIn(payload["status"], ("simulated", "recovered", "blocked"))
            self.assertIsNone(payload.get("receiptHash"))
            self.assertNotIn("acceptance", json.dumps(payload).lower())
            self._assert_no_protected_mutation(root, before)

    # T07-T10: simulation-only boundary and input safety.
    def test_t07_simulation_is_deterministic_and_preserves_canonical_bytes(self) -> None:
        with disposable_fixture() as root:
            before = _protected_bytes(root)
            first = self._simulate(root)
            second = self._simulate(root)
            self.assertEqual(first.returncode, 0, first.stderr)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertEqual(first.stdout, second.stdout)
            self._assert_no_protected_mutation(root, before)
            self.assertEqual(
                _claims_path(root).read_bytes(),
                before["tasks/completion/claims.json"],
            )

    def test_t08_simulation_never_calls_save_ledger_or_raw_publication(self) -> None:
        controller = self._controller()
        completion_claims = importlib.import_module("tools.completion_claims")
        calls: list[object] = []
        original = completion_claims.save_ledger
        completion_claims.save_ledger = lambda *args, **kwargs: calls.append((args, kwargs))
        try:
            with disposable_fixture() as root:
                result = controller.simulate(root, txid=TXID)
                self.assertIn(result.get("status"), ("simulated", "recovered", "blocked"))
        finally:
            completion_claims.save_ledger = original
        self.assertEqual(calls, [])
        source = CONTROLLER_PATH.read_text(encoding="utf-8")
        self.assertNotIn("shell=True", source)

    def test_t09_apply_always_refuses_before_opening_canonical_ledger(self) -> None:
        with disposable_fixture() as root:
            before = _protected_bytes(root)
            token = root / "authority.token"
            token.write_text("plain-token\n", encoding="utf-8")
            result = self._run_cli(
                root,
                "--root",
                str(root),
                "--mode",
                "apply",
                "--authority-token-file",
                str(token),
            )
            payload = self._result(result)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(self._error_code(payload), "apply-disabled")
            self._assert_no_protected_mutation(root, before)

    def test_t10_unsafe_paths_marker_json_txid_bounds_and_secret_canary_fail_closed(self) -> None:
        cases = (
            ("absolute output", ("--txid", TXID, "--output", str(Path(tempfile.gettempdir()) / "outside.json"))),
            ("traversal output", ("--txid", TXID, "--output", "../outside.json")),
            ("empty txid", ("--txid", "")),
            ("uppercase txid", ("--txid", "BadTxid")),
            ("too long txid", ("--txid", "a" * 65)),
        )
        for label, args in cases:
            with self.subTest(label=label), disposable_fixture() as root:
                result = self._run_cli(root, "--root", str(root), "--mode", "simulate", *args)
                before = _protected_bytes(root)
                self.assertEqual(result.returncode, 1, result.stderr)
                self._result(result)
                self._assert_no_protected_mutation(root, before)

        with disposable_fixture() as root:
            marker = root / ".ledger-convergence-disposable"
            marker.write_bytes(b"wrong-marker\n")
            before = _protected_bytes(root)
            result = self._simulate(root)
            self.assertEqual(result.returncode, 1, result.stderr)
            self._result(result)
            self._assert_no_protected_mutation(root, before)

        with disposable_fixture() as root:
            _claims_path(root).write_bytes(b"{not-json\n")
            before = _protected_bytes(root)
            result = self._simulate(root)
            self.assertEqual(result.returncode, 1, result.stderr)
            self._result(result)
            self._assert_no_protected_mutation(root, before)

        with disposable_fixture() as root:
            before = _protected_bytes(root)
            (root / "secret-canary.txt").write_text("SECRET-CANARY-DO-NOT-LEAK", encoding="utf-8")
            result = self._simulate(root)
            payload = self._result(result)
            self.assertNotIn("SECRET-CANARY", result.stdout)
            self.assertNotIn("SECRET-CANARY", json.dumps(payload))
            self._assert_no_protected_mutation(root, before)

    # T11-T15: lock, initial/final/remote CAS, and idempotency.
    def test_t11_common_git_dir_lock_is_0600_nonblocking_and_fail_closed(self) -> None:
        with disposable_fixture() as root:
            lock = root / ".git" / "ledger-convergence" / "lock"
            lock.parent.mkdir(parents=True)
            lock.write_text(json.dumps({"txid": "other-tx", "pid": 1, "started": "fixed"}) + "\n", encoding="utf-8")
            lock.chmod(stat.S_IRUSR | stat.S_IWUSR)
            before = _protected_bytes(root)
            result = self._simulate(root)
            payload = self._result(result)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(self._error_code(payload), "lock-busy")
            self.assertEqual(stat.S_IMODE(lock.stat().st_mode), 0o600)
            self.assertEqual(lock.read_bytes(), b'{"txid": "other-tx", "pid": 1, "started": "fixed"}\n')
            self._assert_no_protected_mutation(root, before)

    def test_t12_initial_cas_rejects_detached_dirty_wrong_schema_status_membership_and_mapping(self) -> None:
        mutations = ("detached", "dirty", "schema", "status", "membership", "mapping")
        for mutation in mutations:
            with self.subTest(mutation=mutation), disposable_fixture() as root:
                home = root / ".home"
                if mutation == "detached":
                    _run_git(root, home, "checkout", "--detach", "HEAD")
                elif mutation == "dirty":
                    (root / "PLAN.md").write_bytes((root / "PLAN.md").read_bytes() + b"dirty\n")
                elif mutation == "schema":
                    document = _read_json(_claims_path(root))
                    document["schemaVersion"] = 999
                    _claims_path(root).write_text(json.dumps(document) + "\n", encoding="utf-8")
                elif mutation == "status":
                    document = _read_json(_claims_path(root))
                    document["claims"][sorted(RETIRE_IDS)[0]]["status"] = "blocked"
                    _claims_path(root).write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
                elif mutation == "membership":
                    document = _read_json(_claims_path(root))
                    del document["claims"][sorted(RETIRE_IDS)[0]]
                    _claims_path(root).write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
                else:
                    mapping = root / "worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md"
                    mapping.write_bytes(mapping.read_bytes() + b"mapping drift\n")
                before = _protected_bytes(root)
                result = self._simulate(root)
                self.assertEqual(result.returncode, 2, result.stderr)
                payload = self._result(result)
                self.assertIn(self._error_code(payload), {"cas-mismatch", "input-invalid", "recovery-blocked"})
                self._assert_no_protected_mutation(root, before)

    def test_t13_final_cas_injection_leaves_canonical_bytes_unchanged(self) -> None:
        hooks = IoHooks()
        with disposable_fixture() as root:
            before = _protected_bytes(root)
            controller = self._controller()
            result = controller.simulate(root, txid=TXID, io_hooks=hooks)
            self.assertEqual(result.get("status"), "blocked")
            self.assertIn("cas", json.dumps(result).lower())
            self._assert_no_protected_mutation(root, before)

    def test_t14_remote_advance_after_phase_a_is_a_hard_nonforce_failure(self) -> None:
        class RemoteAdvanceHooks(IoHooks):
            def before_final_cas(self, *args: object, **kwargs: object) -> None:
                self._event("remote_advance", *args, **kwargs)

        with disposable_fixture() as root:
            home = root / ".home"
            bare = root / "remote.git"
            subprocess.run(["git", "init", "-q", "--bare", str(bare)], cwd=root, env=_git_env(home), check=True, timeout=10)
            _run_git(root, home, "remote", "add", "origin", str(bare))
            _run_git(root, home, "push", "-q", "-u", "origin", "plan/ledger-convergence")
            before = _protected_bytes(root)
            result = self._controller().simulate(root, txid=TXID, io_hooks=RemoteAdvanceHooks())
            self.assertEqual(result.get("status"), "blocked")
            self.assertIn("remote", json.dumps(result).lower())
            self._assert_no_protected_mutation(root, before)

    def test_t15_same_txid_is_idempotent_and_different_owner_hash_never_overwrites(self) -> None:
        with disposable_fixture() as root:
            first = self._simulate(root)
            second = self._simulate(root)
            self.assertEqual(first.returncode, 0, first.stderr)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertEqual(first.stdout, second.stdout)
            other = self._simulate(root, "differenttxid")
            self.assertIn(other.returncode, (0, 2))
            sidecars = _simulation_dir(root)
            self.assertTrue(sidecars.is_dir())
            self.assertLessEqual(len(list(sidecars.iterdir())), 8)

    # T16-T18: durable publication, injected failures, crash matrix.
    def test_t16_all_durable_publication_hooks_are_observable(self) -> None:
        hooks = IoHooks()
        with disposable_fixture() as root:
            result = self._controller().simulate(root, txid=TXID, io_hooks=hooks)
            self.assertEqual(result.get("status"), "simulated")
        for event in ("write", "flush", "file_fsync", "replace", "directory_fsync"):
            self.assertIn(event, hooks.events)

    def test_t17_each_durable_failure_returns_stable_blocked_code_and_preserves_ledger(self) -> None:
        for event in ("write", "flush", "file_fsync", "replace", "directory_fsync"):
            with self.subTest(event=event), disposable_fixture() as root:
                before = _protected_bytes(root)
                hooks = IoHooks(fail=event)
                result = self._controller().simulate(root, txid=TXID, io_hooks=hooks)
                self.assertEqual(result.get("status"), "blocked")
                self.assertIn(self._error_code(result), {"atomic-publication-failed", "write-failed", "fsync-failed"})
                self._assert_no_protected_mutation(root, before)

    def test_t18_every_documented_crash_point_is_bounded_and_nonpublishing(self) -> None:
        for fault in CRASH_POINTS:
            with self.subTest(fault=fault), disposable_fixture() as root:
                before = _protected_bytes(root)
                result = self._simulate(root, TXID, "--fault", fault)
                self.assertEqual(result.returncode, 2, result.stderr)
                payload = self._result(result)
                self.assertIn(self._error_code(payload), {"crash-injected", "recovery-blocked", "atomic-publication-failed"})
                self._assert_no_protected_mutation(root, before)

    # T19-T21: recovery, sidecar bounds, guarded rollback.
    def test_t19_recovery_is_state_driven_and_rejects_corrupt_or_changed_state(self) -> None:
        with disposable_fixture() as root:
            before = _protected_bytes(root)
            crashed = self._simulate(root, TXID, "--fault", "after-phase-a-push-before-phase-b-temp-fsync")
            self.assertEqual(crashed.returncode, 2, crashed.stderr)
            recovered = self._simulate(root)
            self.assertIn(recovered.returncode, (0, 2))
            self._assert_no_protected_mutation(root, before)

        with disposable_fixture() as root:
            self._simulate(root, TXID, "--fault", "after-phase-b-temp-fsync-before-ledger-rename")
            journal = root / ".git" / "ledger-convergence" / "transactions" / TXID / "state.json"
            journal.parent.mkdir(parents=True, exist_ok=True)
            journal.write_text('{"corrupt":true}\n', encoding="utf-8")
            before = _protected_bytes(root)
            result = self._simulate(root)
            payload = self._result(result)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(self._error_code(payload), "recovery-blocked")
            self._assert_no_protected_mutation(root, before)

    def test_t20_journal_sidecars_and_backups_are_bounded_and_content_addressed(self) -> None:
        with disposable_fixture() as root:
            result = self._simulate(root)
            self.assertEqual(result.returncode, 0, result.stderr)
            journal = root / ".git" / "ledger-convergence" / "transactions" / TXID / "state.json"
            self.assertTrue(journal.is_file())
            transaction_dir = journal.parent
            files = [p for p in transaction_dir.rglob("*") if p.is_file()]
            self.assertLessEqual(len(files), 8)
            self.assertLessEqual(sum(p.stat().st_size for p in files), 16 * 1024 * 1024)
            backups = [p for p in files if "backup" in p.name]
            self.assertLessEqual(len(backups), 2)
            for backup in backups:
                self.assertNotEqual(backup.read_bytes(), b"")

    def test_t21_guarded_rollback_requires_expected_hash_tip_and_lock(self) -> None:
        with disposable_fixture() as root:
            before = _protected_bytes(root)
            result = self._simulate(root, TXID, "--fault", "after-ledger-rename-before-directory-fsync")
            self.assertEqual(result.returncode, 2, result.stderr)
            journal = root / ".git" / "ledger-convergence" / "transactions" / TXID / "state.json"
            if journal.exists():
                state = _read_json(journal)
                self.assertIsInstance(state, dict)
                state["expectedCandidateLedgerSha256"] = "0" * 64
                journal.write_text(json.dumps(state, sort_keys=True) + "\n", encoding="utf-8")
            (root / ".ledger-convergence-disposable").write_bytes(MARKER + b"changed")
            retry = self._simulate(root)
            payload = self._result(retry)
            self.assertEqual(retry.returncode, 2)
            self.assertIn(self._error_code(payload), {"rollback-guard-mismatch", "recovery-blocked", "cas-mismatch"})
            self._assert_no_protected_mutation(root, before)


if __name__ == "__main__":
    unittest.main()
