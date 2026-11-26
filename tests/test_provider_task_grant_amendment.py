"""Hermetic tests for the operator grant-amendment command.

The tests exercise the real ``tools.delivery.model``/``store.Store`` schema and
the real ``tasks``/``jobs`` ownership rows.  No fabricated reservation API is
used and no new operational state schema is created: the amendment works only
through the real delivery Store.  All authority, target, runtime and custody
artifacts are made in the disposable fixture and the module's pinned paths are
swapped for those fixture paths for the duration of each test.  No repository
outside the fixture, shared evidence file, branch, or user state is touched.
"""

from __future__ import annotations

import contextlib
import hashlib
import io
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.delivery import model
from tools.delivery.store import Store
import tools.provider_task_grant_amendment as amendment


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_git(cwd: pathlib.Path, *args: str) -> str:
    result = subprocess.run(
        ("git", "-C", str(cwd), *args),
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    if result.returncode:
        raise AssertionError(f"git {' '.join(args)} failed: {result.stdout}")
    return result.stdout.strip()


class AmendmentTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory(prefix="provider-grant-amendment-")
        self.root = pathlib.Path(self.tmp.name).resolve()
        self.canonical = self.root / "canonical"
        self.worktree = self.root / "retained-worktree"
        self.canonical.mkdir()
        run_git(self.canonical, "init", "--quiet", "--template=")
        run_git(self.canonical, "config", "user.name", "provider-grant-test")
        run_git(self.canonical, "config", "user.email", "provider-grant-test@localhost.invalid")
        (self.canonical / "README.fixture").write_text("base\n", encoding="utf-8")
        run_git(self.canonical, "add", "README.fixture")
        run_git(self.canonical, "commit", "--quiet", "-m", "fixture base")
        self.base = run_git(self.canonical, "rev-parse", "HEAD")
        # A real linked worktree on a real branch, exactly like the retained
        # feature workspace: ``.git`` is a gitdir file, HEAD is a branch, and the
        # owner record lives at the worktree root.
        self.feature_ref = "refs/heads/recovery/feature-05"
        run_git(
            self.canonical,
            "worktree",
            "add",
            "-b",
            "recovery/feature-05",
            "--quiet",
            str(self.worktree),
            self.base,
        )
        self.worktree = self.worktree.resolve()

        self.custody_file = self.worktree / "retained-source.txt"
        self.custody_file.write_bytes(b"retained source bytes\n")
        self.token = "retained-owner-token"
        self.owner_file = self.worktree / ".delivery-owner.json"
        # The real provisioner record: no ``generation`` field, and a real
        # branch ref in ``feature_ref``.
        self.owner_file.write_text(
            json.dumps(
                {
                    "base": self.base,
                    "feature_ref": self.feature_ref,
                    "job": amendment.JOB,
                    "leased_at": 1795033531.975415,
                    "stage": "implement",
                    "task": amendment.TARGET,
                    "token": self.token,
                    "workspace": str(self.worktree),
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )

        self.state = self.root / "delivery-state"
        self.store = Store(self.state)
        self.config = {"fixture": True, "security_paths": [], "protected_paths": []}
        self.config_hash = model.digest(self.config)
        self.store.meta("repo", str(self.canonical))
        self.store.meta("config", self.config)
        self.store.meta("config_hash", self.config_hash)

        self.spec = model.task(
            {
                "id": amendment.TARGET,
                "goal": "Retain the browser login task while adding its missing dependency grants.",
                "source": "Operator-approved browser-login grant amendment fixture.",
                "paths": ["crates/providers/src/openai_browser_auth.rs"],
                "dependencies": [],
                "priority": 2,
                "reviews": 1,
                "checks": [
                    {
                        "id": "provider-task-grant-amendment",
                        "argv": ["python3", "-B", "-m", "unittest", "-v"],
                        "required_output": ["OK"],
                        "timeout": 180,
                    }
                ],
                "red_checks": [],
                "security_review": True,
            }
        )
        self.store.admit([self.spec], self.config)
        self.negative_reviews = [
            {
                "candidate": "candidate-with-negative-review",
                "reviewer": "independent-reviewer",
                "verdict": "REJECT",
                "findings": ["missing provider module grant"],
            }
        ]
        self.changed_paths = ["crates/providers/src/openai_browser_auth.rs"]
        self.accepted_sha = "retained-acceptance-evidence"
        with self.store.connection(write=True) as cx:
            cx.execute(
                """UPDATE tasks SET state='blocked', stage='implement', generation=1,
                   reviews=?, candidate=?, changed_paths=?, accepted_sha=?,
                   report=?, base=?, workspace=? WHERE id=?""",
                (
                    json.dumps(self.negative_reviews, sort_keys=True),
                    "candidate-with-negative-review",
                    json.dumps(self.changed_paths),
                    self.accepted_sha,
                    "retained negative evidence",
                    self.base,
                    str(self.worktree),
                    amendment.TARGET,
                ),
            )
            # The real owner job, consumed by the normal Store lifecycle.  Store
            # keeps the terminal pid/lease metadata for auditability.
            cx.execute(
                """INSERT INTO jobs
                   (id,task,stage,token,state,route,generation,payload,result,pid,
                    lease_until,created,updated)
                   VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)""",
                (
                    amendment.JOB,
                    amendment.TARGET,
                    "implement",
                    self.token,
                    "consumed",
                    "fixture",
                    1,
                    "{}",
                    "{}",
                    999999,
                    9999999999,
                    0,
                    0,
                ),
            )

        self._old_constants = {
            name: getattr(amendment, name)
            for name in (
                "AUTHORIZATION_PATH",
                "AUTH_SHA",
                "TARGET_SNAPSHOT_PATH",
                "TARGET_SHA",
                "TARGET_SPEC_SHA",
                "RUNTIME_ROOT",
                "RUNTIME_MANIFEST_PATH",
                "RUNTIME_SHA",
                "RUNTIME_MANIFEST_SHA",
                "CUSTODY_PATH",
                "CUSTODY_SHA",
                "BASE",
                "PRODUCTION_STATE",
                "PRODUCTION_REPO",
                "RESERVED_WORKSPACE",
            )
        }
        self._write_pinned_artifacts()

    def tearDown(self) -> None:
        for name, value in self._old_constants.items():
            setattr(amendment, name, value)
        self.owner_file.unlink(missing_ok=True)
        self.tmp.cleanup()

    def _write_pinned_artifacts(self) -> None:
        evidence = self.root / "evidence"
        evidence.mkdir(parents=True)
        authorization = evidence / "authorization.md"
        authorization.write_text(
            "Authorize amendment for the retained browser-login task.\n",
            encoding="utf-8",
        )

        runtime_root = evidence / "trusted-runtime"
        (runtime_root / "tools" / "delivery").mkdir(parents=True)
        run_git(runtime_root, "init", "--quiet", "--template=")
        runtime_file = runtime_root / "closure.txt"
        runtime_file.write_text("independently reviewed runtime closure\n", encoding="utf-8")
        runtime_oid = run_git(runtime_root, "hash-object", "--", str(runtime_file))
        source_root = pathlib.Path(__file__).resolve().parents[1]
        runtime_modules = {}
        for relative in ("tools/delivery/model.py", "tools/delivery/store.py"):
            target = runtime_root / relative
            target.write_bytes((source_root / relative).read_bytes())
            runtime_modules[relative] = {
                "oid": run_git(runtime_root, "hash-object", "--", str(target))
            }
        runtime_manifest = evidence / "trusted-runtime-manifest.json"
        runtime_candidate = "1" * 40
        runtime_manifest.write_text(
            json.dumps(
                {
                    "candidateSha": runtime_candidate,
                    "all_files_git_blob_verified": True,
                    "files": {
                        "closure.txt": {"oid": runtime_oid},
                        **runtime_modules,
                    },
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )

        # The target snapshot mirrors the real retained task row (id, generation,
        # spec hash, retained workspace and base) -- not an invented reservation.
        target_snapshot = evidence / "target.json"
        target_snapshot.write_text(
            json.dumps(
                {
                    "id": amendment.TARGET,
                    "generation": 1,
                    "spec_hash": model.digest(self.spec),
                    "spec": json.dumps(self.spec, sort_keys=True),
                    "config_hash": self.config_hash,
                    "workspace": str(self.worktree),
                    "base": self.base,
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )
        custody = evidence / "custody.json"
        custody.write_text(
            json.dumps(
                {
                    "task": amendment.TARGET,
                    "job": amendment.JOB,
                    "base": self.base,
                    "files": {"retained-source.txt": {"sha256": digest(self.custody_file)}},
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )

        amendment.AUTHORIZATION_PATH = str(authorization)
        amendment.AUTH_SHA = digest(authorization)
        amendment.TARGET_SNAPSHOT_PATH = str(target_snapshot)
        amendment.TARGET_SHA = digest(target_snapshot)
        amendment.TARGET_SPEC_SHA = model.digest(self.spec)
        amendment.RUNTIME_ROOT = str(runtime_root)
        amendment.RUNTIME_MANIFEST_PATH = str(runtime_manifest)
        amendment.RUNTIME_SHA = runtime_candidate
        amendment.RUNTIME_MANIFEST_SHA = digest(runtime_manifest)
        amendment.CUSTODY_PATH = str(custody)
        amendment.CUSTODY_SHA = digest(custody)
        amendment.BASE = self.base
        amendment.PRODUCTION_STATE = str(self.state)
        amendment.PRODUCTION_REPO = str(self.canonical)
        amendment.RESERVED_WORKSPACE = str(self.worktree)
        amendment._TRUSTED_STORE_CLASS = None

    def request(self, **changes: object) -> dict:
        row = self.store.tasks()[0]
        request = {
            "task_id": amendment.TARGET,
            "expected_spec_hash": row["spec_hash"],
            "expected_generation": 1,
            "expected_config_hash": self.config_hash,
            "additive_paths": sorted(amendment.ADDITIONS),
            "authority_evidence": str(pathlib.Path(amendment.AUTHORIZATION_PATH).resolve()),
            "authority_sha256": amendment.AUTH_SHA,
            "target_sha256": amendment.TARGET_SHA,
            "runtime_candidate": amendment.RUNTIME_SHA,
            "workspace_identity": {
                "registered": True,
                "physical_path": str(self.worktree),
                "ref": self.feature_ref,
                "head": run_git(self.worktree, "rev-parse", "HEAD"),
                "source_hashes": {"retained-source.txt": digest(self.custody_file)},
            },
        }
        request.update(changes)
        return request

    def apply(self, **changes: object) -> dict:
        # This is the same production callable used by main; no trusted test
        # context or alternate Store entry point exists.
        request = self.request(**changes)
        trusted_store = amendment._runtime_store()(self.state)
        return amendment.amend(trusted_store, request)

    def test_positive_command_preserves_worktree_and_negative_evidence(self) -> None:
        before = self.custody_file.read_bytes()
        request_path = self.root / "request.json"
        request_path.write_text(json.dumps(self.request(), sort_keys=True), encoding="utf-8")
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            amendment.main(["--request", str(request_path)])
        receipt = json.loads(output.getvalue())

        row = self.store.tasks()[0]
        paths = set(json.loads(row["spec"])["paths"])
        self.assertEqual(paths, set(self.spec["paths"]) | amendment.ADDITIONS)
        self.assertEqual(row["state"], "ready")
        self.assertEqual(row["generation"], 2)
        self.assertEqual(json.loads(row["reviews"]), self.negative_reviews)
        self.assertEqual(row["candidate"], "candidate-with-negative-review")
        self.assertEqual(json.loads(row["changed_paths"]), self.changed_paths)
        self.assertEqual(row["accepted_sha"], self.accepted_sha)
        self.assertEqual(self.custody_file.read_bytes(), before)
        self.assertEqual(receipt["owner_job"], amendment.JOB)
        self.assertEqual(set(receipt["added_paths"]), amendment.ADDITIONS)

        with self.store.connection() as cx:
            event = cx.execute(
                "SELECT body FROM events WHERE kind='task.operator_grant_amended' ORDER BY seq DESC"
            ).fetchone()
        self.assertIsNotNone(event)
        self.assertEqual(json.loads(event["body"])["preserved"]["candidate"], "candidate-with-negative-review")

    def test_amendment_preserves_non_path_spec_fields_and_worktree_identity(self) -> None:
        # Every original obligation must survive the amendment: the goal,
        # source, dependencies, priority, review count, trusted checks, red
        # checks and security-review flag stay identical, the original grant
        # remains allowed, and the registered worktree's HEAD, branch and
        # custody bytes are untouched by the amendment transaction.
        before_spec = json.loads(self.store.tasks()[0]["spec"])
        head_before = run_git(self.worktree, "rev-parse", "HEAD")
        ref_before = run_git(self.worktree, "symbolic-ref", "HEAD")
        custody_before = self.custody_file.read_bytes()

        self.apply()

        after_spec = json.loads(self.store.tasks()[0]["spec"])
        self.assertEqual(
            {key: value for key, value in after_spec.items() if key != "paths"},
            {key: value for key, value in before_spec.items() if key != "paths"},
        )
        self.assertEqual(set(after_spec["paths"]), set(before_spec["paths"]) | amendment.ADDITIONS)
        self.assertIn("crates/providers/src/openai_browser_auth.rs", after_spec["paths"])
        self.assertEqual(after_spec["checks"], before_spec["checks"])
        self.assertTrue(after_spec["security_review"])
        self.assertEqual(run_git(self.worktree, "rev-parse", "HEAD"), head_before)
        self.assertEqual(run_git(self.worktree, "symbolic-ref", "HEAD"), ref_before)
        self.assertEqual(self.custody_file.read_bytes(), custody_before)

    def test_amendment_uses_existing_store_schema_without_new_state(self) -> None:
        # The amendment must not invent an operational state schema.  It works
        # only through the real tasks/jobs/events rows; no new table appears and
        # the retained worktree is still recorded on the task row.
        with self.store.connection() as cx:
            before = {row[0] for row in cx.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        self.apply()
        with self.store.connection() as cx:
            after = {row[0] for row in cx.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        self.assertEqual(after, before)
        self.assertEqual(self.store.tasks()[0]["generation"], 2)
        self.assertEqual(self.store.tasks()[0]["workspace"], str(self.worktree))

    def test_consumed_owner_liveness_metadata_does_not_discard_retained_task(self) -> None:
        # The normal Store lifecycle does not clear pid/lease_until on a
        # consumed job.  A future lease and a PID therefore cannot by
        # themselves turn a terminal owner back into a live owner.
        self.apply()
        self.assertEqual(self.store.tasks()[0]["generation"], 2)

    def test_forged_authority_pins_and_scope_are_rejected(self) -> None:
        for key, value in (
            ("authority_evidence", str(self.root / "forged.md")),
            ("authority_sha256", "0" * 64),
            ("target_sha256", "0" * 64),
            ("runtime_candidate", "0" * 40),
            ("additive_paths", ["Cargo.toml"]),
        ):
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.apply(**{key: value})
        with self.assertRaises(ValueError):
            amendment.amend(self.store, self.request(), {"workspace": "caller-bypass"})  # type: ignore[arg-type]

    def test_wrong_registered_identity_or_custody_is_rejected(self) -> None:
        base = self.request()["workspace_identity"]
        for key, value in (
            ("physical_path", str(self.root / "foreign")),
            ("ref", "refs/heads/foreign"),
            ("head", "0" * 40),
            ("source_hashes", {}),
        ):
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.apply(workspace_identity=dict(base, **{key: value}))

    def test_owner_record_identity_is_fenced(self) -> None:
        # The owner record is the real provisioner artifact; a mismatched token,
        # task, base, workspace or feature_ref must fail closed.
        for key, value in (
            ("token", "other-token"),
            ("task", "OTHER-TASK"),
            ("base", "0" * 40),
            ("workspace", str(self.root / "elsewhere")),
            ("feature_ref", "refs/heads/other"),
        ):
            with self.subTest(key=key):
                record = json.loads(self.owner_file.read_text(encoding="utf-8"))
                record[key] = value
                self.owner_file.write_text(json.dumps(record, sort_keys=True), encoding="utf-8")
                with self.assertRaises(ValueError):
                    self.apply()
                record[key] = self._owner_default(key)
                self.owner_file.write_text(json.dumps(record, sort_keys=True), encoding="utf-8")
        self.apply()
        self.assertEqual(self.store.tasks()[0]["generation"], 2)

    def _owner_default(self, key: str) -> object:
        return {
            "token": self.token,
            "task": amendment.TARGET,
            "base": self.base,
            "workspace": str(self.worktree),
            "feature_ref": self.feature_ref,
        }[key]

    def test_finished_owner_and_existing_publication_are_fences(self) -> None:
        with self.store.connection(write=True) as cx:
            cx.execute(
                """INSERT INTO jobs
                   (id,task,stage,token,state,route,generation,payload,result,pid,
                    lease_until,created,updated)
                   VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)""",
                ("finished-owner", amendment.TARGET, "implement", "x", "finished", "r", 1, "{}", None, None, 0, 0, 0),
            )
        with self.assertRaises(ValueError):
            self.apply()

    def test_running_owner_is_a_fence(self) -> None:
        # The historical owner job is consumed, but a still-running job means a
        # live worker owns the task.  The amendment must never race that owner,
        # even though the retained (consumed) job is terminal.
        with self.store.connection(write=True) as cx:
            cx.execute(
                """INSERT INTO jobs
                   (id,task,stage,token,state,route,generation,payload,result,pid,
                    lease_until,created,updated)
                   VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)""",
                ("running-owner", amendment.TARGET, "implement", "y", "running", "r", 1, "{}", None, None, 0, 0, 0),
            )
        with self.assertRaises(ValueError):
            self.apply()
        self.assertEqual(self.store.tasks()[0]["state"], "blocked")

    def test_non_blocked_task_is_rejected(self) -> None:
        # Only the retained blocked task may be amended.  Any other lifecycle
        # state (for example an already-ready row) is not the authorised target.
        with self.store.connection(write=True) as cx:
            cx.execute("UPDATE tasks SET state='ready' WHERE id=?", (amendment.TARGET,))
        with self.assertRaises(ValueError):
            self.apply()
        self.assertEqual(self.store.tasks()[0]["state"], "ready")

    def test_stale_generation_collision_and_rollback_leave_original_untouched(self) -> None:
        before = self.store.tasks()[0]
        with self.assertRaises(ValueError):
            self.apply(expected_generation=8)
        after = self.store.tasks()[0]
        self.assertEqual(after["spec_hash"], before["spec_hash"])
        self.assertEqual(after["generation"], before["generation"])

        other = model.task(
            {
                "id": "other-retained-task",
                "goal": "Retained collision fixture task.",
                "source": "collision fixture",
                "paths": ["Cargo.lock"],
                "dependencies": [],
                "priority": 2,
                "reviews": 1,
                "checks": [
                    {
                        "id": "collision-check",
                        "argv": ["python3", "-m", "unittest"],
                        "required_output": ["OK"],
                        "timeout": 180,
                    }
                ],
            }
        )
        self.store.admit([other], self.config)
        with self.store.connection(write=True) as cx:
            cx.execute("UPDATE tasks SET state='blocked' WHERE id=?", (other["id"],))
        with self.assertRaises(ValueError):
            self.apply()
        self.assertEqual(self.store.tasks()[0]["spec_hash"], before["spec_hash"])

    def test_existing_global_publication_lock_is_not_stolen(self) -> None:
        lock = self.canonical / ".git" / "recovery-state" / "integration.lock"
        lock.parent.mkdir(mode=0o700)
        lock.write_text(
            json.dumps({"owner": "delivery:other-job", "failure": "other-task", "job": "other-job"}),
            encoding="utf-8",
        )
        with self.assertRaises(ValueError):
            self.apply()
        self.assertTrue(lock.exists())

    def test_amendment_fence_is_owner_bound_and_released(self) -> None:
        with amendment._publication_fence(self.canonical, amendment.JOB, amendment.TARGET):
            lock = self.canonical / ".git" / "recovery-state" / "integration.lock"
            payload = json.loads(lock.read_text(encoding="utf-8"))
            self.assertEqual(payload["kind"], "provider-grant-amendment")
            self.assertEqual(payload["job"], amendment.JOB)
            self.assertEqual(payload["task"], amendment.TARGET)
            self.assertIsInstance(payload["pid"], int)
            self.assertRegex(payload["nonce"], r"^[0-9a-f]{32}$")
        self.assertFalse(lock.exists())

    def test_fence_release_refuses_a_foreign_owner_record(self) -> None:
        # The job-identifying record is what makes a crashed fence recoverable;
        # a different owner (different nonce) must never be able to unlink a
        # live fence, and the real owner must still release it.
        with amendment._publication_fence(self.canonical, amendment.JOB, amendment.TARGET) as record:
            lock = self.canonical / ".git" / "recovery-state" / "integration.lock"
            foreign = dict(record, nonce="f" * 32, pid=-1)
            amendment._release_fence(lock, foreign)
            self.assertTrue(lock.exists())
            self.assertEqual(json.loads(lock.read_text(encoding="utf-8")), record)
        self.assertFalse(lock.exists())


if __name__ == "__main__":
    unittest.main()
