import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

ROOT=pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT))
from tools.delivery import engine, model, jobs
from tools.delivery.store import Store

FIXTURE=ROOT/'tests/delivery/fixtures/agent.py'
ENTRY=ROOT/'tools/continuous_delivery.py'


def git(root,*args):
    return subprocess.check_output(['git',*args],cwd=root,text=True,stderr=subprocess.STDOUT).strip()


class RepositoryFixture(unittest.TestCase):
    def setUp(self):
        self.root=pathlib.Path(os.environ.get('R8_TEST_ROOT','/Users/mymac/Projects/.oc2-features-20261103/artifacts/parity-20261112/bootstrap-repair/r8/resume-39/engine-fixture')) / __import__('uuid').uuid4().hex
        self.root.mkdir(parents=True, exist_ok=False)
        self.temp=None
        self.repo=self.root/'repo';self.repo.mkdir()
        git(self.repo,'init','-q','-b','main-v2')
        git(self.repo,'config','user.name','Fixture')
        git(self.repo,'config','user.email','fixture@localhost.invalid')
        (self.repo/'README.md').write_text('initial\n')
        (self.repo/'check.py').write_text('''import pathlib,sys
files=list(pathlib.Path('features').glob('*.txt'))
for p in files:
    assert p.read_text().strip()==p.stem, 'behavior mismatch: '+str(p)
print('CHECKED',len(files))
''')
        git(self.repo,'add','.');git(self.repo,'commit','-qm','initial')
        checks=[{'id':'suite','argv':[sys.executable,'check.py'],'required_output':['CHECKED']}]
        self.cfg={'branch':'main-v2','workers':4,'lease_seconds':10,'stage_timeout':30,'attempts':3,
                  'full_checks':checks,'post_merge_checks':checks,
                  'runtime':{'kind':'command','argv':[sys.executable,str(FIXTURE)]},
                  'routes':[{'agent':'fixture-a','max_parallel':40},{'agent':'fixture-b','max_parallel':40}]}
        self.store=engine.setup(self.repo,self.root/'state',self.cfg)

    def tearDown(self): return

    def spec(self,name,extra='',deps=None):
        return {'id':name,'goal':'Create and verify feature '+name+' '+extra,'source':'owned deterministic test fixture',
                'paths':['features/'+name+'.txt'],'dependencies':deps or [],
                'checks':[{'id':'feature','argv':[sys.executable,'-c',
                    "import pathlib; assert pathlib.Path('features/%s.txt').read_text()=='%s\\n'; print('1 assertion passed')"%(name,name)],
                    'required_output':['1 assertion passed']}]}

    def runflow(self,timeout=80,publish=True):
        p=subprocess.run([sys.executable,str(ENTRY),'run','--state',str(self.store.root),
                          *(['--publish'] if publish else []),'--max-seconds',str(timeout)],
                         capture_output=True,text=True,timeout=timeout+15)
        self.last_run = p
        return p



class EngineTests(RepositoryFixture):
    def test_no_code_failure_is_not_candidate_success_protocol(self):
        self.store.admit([self.spec("E")], self.store.meta("config"))
        job = self.store.claim("E", "implement", "fixture-a", {}, 60)
        self.assertIsNotNone(job)
        import tools.delivery.jobs as delivery_jobs
        original = delivery_jobs.runtime.agent
        delivery_jobs.runtime.agent = lambda *args, **kwargs: {"summary": "no-op"}
        try:
            result = delivery_jobs.stage(self.store, job)
        except Exception as exc:
            self.assertEqual(getattr(exc, "kind", None), "candidate_empty")
            self.assertIn("no code", str(exc))
        else:
            self.fail("no-code implementation unexpectedly produced candidate: %r" % (result,))
        finally:
            delivery_jobs.runtime.agent = original

    def test_candidate_empty_worker_result_retries_as_implementation_failure(self):
        self.store.admit([self.spec("E")], self.store.meta("config"))
        job = self.store.claim("E", "implement", "fixture-a", {}, 60)
        self.assertTrue(self.store.finish(job["id"], job["token"], {"kind": "candidate_empty", "error": "implementation produced no code or tests"}))
        engine.consume(self.store)
        row = self.store.tasks()[0]
        self.assertIn(row["state"], {"ready", "blocked"})
        self.assertEqual(row["stage"], "implement")

    def test_launched_job_idle_reconcile_resume_pipeline(self):
        """Real supervisor -> idle config reconcile -> real resumed publication."""
        import hashlib, uuid
        persistent_root = pathlib.Path(os.environ.get(
            "R8_IDLE_ROOT",
            "/Users/mymac/Projects/.oc2-features-20261103/artifacts/parity-20261112/bootstrap-repair/r8/resume-31/idle-fixture"))
        persistent_root = persistent_root / uuid.uuid4().hex
        repo = persistent_root / "repo"
        repo.mkdir(parents=True)
        git(repo, "init", "-q", "-b", "main-v2")
        git(repo, "config", "user.name", "IdleFixture")
        git(repo, "config", "user.email", "idle@localhost.invalid")
        (repo / "check.py").write_text("import pathlib\nfor p in pathlib.Path('features').glob('*.txt'): assert p.read_text().strip()==p.stem\nprint('CHECKED')\n")
        git(repo, "add", "."); git(repo, "commit", "-qm", "base")
        base = git(repo, "rev-parse", "HEAD")
        git(repo, "branch", "recovery/feature-01", base)
        workspaces = persistent_root / "workspaces"; evidence = persistent_root / "evidence"; build = persistent_root / "build"
        for p in (workspaces, evidence, build): p.mkdir(parents=True)
        checks = [{"id": "suite", "argv": [sys.executable, "check.py"], "required_output": ["CHECKED"]}]
        cfg = {"branch": "main-v2", "workers": 2, "lease_seconds": 60, "stage_timeout": 60, "attempts": 3,
               "full_checks": checks, "post_merge_checks": checks,
               "runtime": {"kind": "command", "argv": [sys.executable, str(ROOT / "tests/delivery/fixtures/agent.py")]},
               "routes": [{"agent": "fixture-a", "max_parallel": 2}], "require_managed_workspaces": True,
               "workspaces": {"root": str(workspaces), "allowed_refs": ["refs/heads/main-v2", "refs/heads/recovery/feature-01"],
                              "author_ref": "refs/heads/recovery/feature-01", "feature_refs": ["refs/heads/recovery/feature-01"],
                              "evidence_root": str(evidence), "build_root": str(build)}}
        state = persistent_root / "state"
        store = engine.setup(repo, state, cfg)
        task = {"id": "A", "goal": "Create and verify feature A", "source": "persistent idle lifecycle",
                "paths": ["features/A.txt"], "dependencies": [],
                "checks": [{"id": "feature", "argv": [sys.executable, "-c", "import pathlib; assert pathlib.Path('features/A.txt').read_text()=='A\\n'; print('ok')"], "required_output": ["ok"]}]}
        store.admit([task], store.meta("config"))
        phase_a = engine.supervise(store, publish=False, max_seconds=100)
        self.assertEqual(phase_a, 2)
        row_a = store.tasks()[0]
        self.assertEqual(row_a["stage"], "publish")
        self.assertTrue(row_a["candidate"])
        authored = row_a["candidate"]
        with store.connection() as cx:
            stale = dict(cx.execute("SELECT id,token,generation FROM jobs ORDER BY created DESC LIMIT 1").fetchone())
        slot = pathlib.Path(row_a["workspace"])
        (slot / "untracked-unique.txt").write_text("unique-untracked\n")
        exclude = pathlib.Path(git(slot, "rev-parse", "--git-common-dir")) / "info" / "exclude"
        if not exclude.is_absolute(): exclude = (slot / exclude).resolve()
        exclude.parent.mkdir(parents=True, exist_ok=True); exclude.write_text("ignored-unique.txt\n")
        (slot / "ignored-unique.txt").write_text("unique-ignored\n")
        cfg2 = dict(cfg); cfg2["stage_timeout"] = 61
        store2 = engine.setup(repo, state, cfg2)
        row_b = store2.tasks()[0]
        self.assertEqual(row_b["candidate"], authored)
        self.assertEqual(row_b["base"], row_a["base"])
        self.assertEqual(row_b["workspace"], row_a["workspace"])
        self.assertEqual(json.loads(row_b["reviews"]), [])
        self.assertGreater(row_b["generation"], row_a["generation"])
        self.assertFalse(store2.valid(stale["id"], stale["token"]))
        self.assertEqual((slot / "untracked-unique.txt").read_text(), "unique-untracked\n")
        self.assertEqual((slot / "ignored-unique.txt").read_text(), "unique-ignored\n")
        phase_c = engine.supervise(store2, publish=True, max_seconds=120)
        self.assertEqual(phase_c, 0)
        final = git(repo, "rev-parse", "HEAD")
        row_c = store2.tasks()[0]
        self.assertEqual(row_c["state"], "accepted")
        self.assertEqual(store2.meta("last_verified_sha"), final)
        self.assertTrue(subprocess.run(["git", "merge-base", "--is-ancestor", authored, final], cwd=repo).returncode == 0)
        self.assertEqual((slot / "untracked-unique.txt").read_text(), "unique-untracked\n")
        self.assertEqual((slot / "ignored-unique.txt").read_text(), "unique-ignored\n")
        with store2.connection() as cx:
            hist = [r[0] for r in cx.execute("SELECT state FROM publication_history WHERE task='A' ORDER BY updated")]
        self.assertEqual(hist[-2:], ["landed", "verified"])
    def test_consume_malformed_candidate_result_blocks_without_crashing(self):
        self.store.admit([self.spec("M")], self.store.meta("config"))
        job = self.store.claim("M", "implement", "fixture-a", {}, 60)
        self.assertIsNotNone(job)
        self.assertTrue(self.store.finish(job["id"], job["token"], {"kind": "candidate", "candidate": "deadbeef"}))
        engine.consume(self.store)
        row = self.store.tasks()[0]
        self.assertEqual(row["state"], "blocked")
        self.assertIn("malformed candidate result", row["report"])

    def assert_managed_provenance(self, store, rows, refs, final):
        """A1/A5 ownership and prepare provenance, not authored ancestry."""
        repo = self.repo
        with store.connection() as cx:
            jobs = [dict(r) for r in cx.execute("SELECT * FROM jobs ORDER BY created")]
        for task in rows:
            impl = [j for j in jobs if j["task"] == task["id"] and j["stage"] == "implement" and j.get("result") and json.loads(j["result"]).get("kind") == "candidate"]
            self.assertTrue(impl, task["id"])
            authored = json.loads(impl[-1]["result"])["candidate"]
            reservation = store.workspace_reservation(task["id"])
            self.assertIsNotNone(reservation)
            ref = reservation["feature_ref"]
            tip = git(self.repo, "rev-parse", ref)
            self.assertEqual(json.loads(impl[0]["payload"])["task"]["id"], task["id"])
            self.assertIn(str(pathlib.Path(reservation["path"]).resolve()), git(self.repo, "worktree", "list", "--porcelain"))
            # The registered author checkpoint remains the ref/worktree tip;
            # accepted/prepared commits may be newer after canonical rebase.
            author_tip = git(self.repo, "rev-parse", ref)
            self.assertEqual(author_tip, authored)
            prep = [j for j in jobs if j["task"] == task["id"] and j["stage"] == "prepare" and j.get("result") and 'prepared' in j["result"]]
            # Canonical may advance between prepares, so a task can prepare more
            # than once: each prepare rebases the previous candidate onto the
            # newer base. The provenance claim is therefore a *chain* from the
            # authored commit to the landed one, not "every payload equals the
            # authored commit".
            landed = authored
            for j in prep:
                payload, result = json.loads(j["payload"]), json.loads(j["result"])
                self.assertEqual(payload.get("task", {}).get("id"), task["id"])
                self.assertEqual(payload.get("candidate"), landed)
                self.assertEqual(git(self.repo, "rev-parse", result["candidate"] + "^"), result["base"])
                landed = result["candidate"]
            self.assertTrue(subprocess.run(["git", "merge-base", "--is-ancestor", landed, final], cwd=self.repo).returncode == 0)
            self.assertEqual(landed, task["accepted_sha"])
    def managed_cfg(self, ref="refs/heads/recovery/feature-01", root=None):
        root = root or (self.root / "managed-workspaces")
        evidence = self.root / "managed-evidence"
        build = self.root / "managed-build"
        root.mkdir(parents=True, exist_ok=True)
        branch = ref.removeprefix("refs/heads/")
        if subprocess.run(["git", "show-ref", "--verify", "--quiet", ref], cwd=str(self.repo)).returncode != 0:
            subprocess.check_call(["git", "branch", branch, "HEAD"], cwd=str(self.repo))
        evidence.mkdir(parents=True, exist_ok=True)
        build.mkdir(parents=True, exist_ok=True)
        cfg = dict(self.cfg)
        cfg["require_managed_workspaces"] = True
        cfg["workspaces"] = {"root": str(root),
            "allowed_refs": ["refs/heads/main-v2", ref],
            "author_ref": ref, "feature_refs": [ref],
            "evidence_root": str(evidence), "build_root": str(build)}
        return cfg

    def test_managed_workspace_full_pipeline_has_landed_history_and_fresh_postcheck(self):
        cfg = self.managed_cfg()
        store = engine.setup(self.repo, self.root / "managed-state", cfg)
        before = git(self.repo, "rev-parse", "HEAD")
        store.admit([self.spec("A")], store.meta("config"))
        p = subprocess.run([sys.executable, str(ENTRY), "run", "--state", str(store.root), "--publish", "--max-seconds", "120"],
                           cwd=str(self.repo), capture_output=True, text=True, timeout=150,
                           env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"})
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        row = store.tasks()[0]
        self.assertEqual(row["state"], "accepted")
        final = git(self.repo, "rev-parse", "HEAD")
        self.assertNotEqual(before, final)
        self.assertEqual((self.repo / "features/A.txt").read_text(), "A\n")
        self.assertEqual(store.meta("last_verified_sha"), final)
        with store.connection() as cx:
            events = [dict(r) for r in cx.execute("SELECT kind,body FROM events WHERE kind LIKE 'publication.%' ORDER BY seq")]
            history = [dict(r) for r in cx.execute("SELECT state FROM publication_history ORDER BY updated")]
        states = [json.loads(e["body"]).get("state") for e in events]
        states.extend(h["state"] for h in history)
        self.assertIn("landed", states)
        self.assertIn("verified", states)
        self.assertTrue(list((self.root / "managed-evidence").glob("*/agent.log")))
        self.assertTrue(any((self.root / "managed-build").iterdir()))

    def test_managed_repair_after_negative_review_preserves_candidate_and_invalidates_approval(self):
        cfg = self.managed_cfg()
        store = engine.setup(self.repo, self.root / "repair-state", cfg)
        store.admit([self.spec("A")], store.meta("config"))
        job = store.claim("A", "implement", "fixture-a", {}, 60)
        self.assertIsNotNone(job)
        result = __import__("tools.delivery.jobs", fromlist=["stage"]).stage(store, job)
        self.assertEqual(result["kind"], "candidate")
        self.assertTrue(store.finish(job["id"], job["token"], result))
        engine.consume(store)
        task = store.tasks()[0]
        self.assertEqual(task["stage"], "review")
        candidate = task["candidate"]
        review = store.claim("A", "review", "fixture-a", {"candidate": candidate, "base": task["base"], "workspace": task["workspace"], "author": task["author"], "task": json.loads(task["spec"]), "reviews": [], "publish_enabled": True}, 60)
        self.assertIsNotNone(review)
        bad = {"kind": "review", "candidate": candidate, "reviewer": review["id"], "verdict": "REJECT", "findings": "deliberate fixture rejection", "security": False}
        store.finish(review["id"], review["token"], bad)
        engine.consume(store)
        task = store.tasks()[0]
        self.assertEqual(task["candidate"], candidate)
        self.assertEqual(json.loads(task["reviews"]), [])

    def test_three_managed_tasks_use_distinct_retained_slots_and_complete_pipeline(self):
        refs = [f"refs/heads/recovery/feature-{i:02d}" for i in range(1, 4)]
        for ref in refs:
            subprocess.check_call(["git", "branch", ref.removeprefix("refs/heads/"), "HEAD"], cwd=str(self.repo))
        cfg = self.managed_cfg(refs[0])
        cfg["workspaces"]["allowed_refs"] = ["refs/heads/main-v2", *refs]
        cfg["workspaces"]["feature_refs"] = refs
        store = engine.setup(self.repo, self.root / "three-managed-state", cfg)
        specs = [self.spec(f"T{i}", "SLOW_FIXTURE") for i in range(1, 4)]
        store.admit(specs, store.meta("config"))
        p = subprocess.run([sys.executable, str(ENTRY), "run", "--state", str(store.root), "--publish", "--max-seconds", "180"],
                           cwd=str(self.repo), capture_output=True, text=True, timeout=210,
                           env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"})
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        rows = store.tasks()
        self.assertEqual({t["state"] for t in rows}, {"accepted"})
        workspaces = [pathlib.Path(t["workspace"]).resolve() for t in rows]
        self.assertEqual(len(set(workspaces)), 3)
        final = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.repo, text=True).strip()
        reservations = {t["id"]: store.workspace_reservation(t["id"]) for t in rows}
        for task in rows:
            ref = reservations[task["id"]]["feature_ref"]
            tip = subprocess.check_output(["git", "rev-parse", ref], cwd=self.repo, text=True).strip()
            self.assertTrue(subprocess.run(["git", "merge-base", "--is-ancestor", task["accepted_sha"], final], cwd=self.repo).returncode == 0)
            for path in json.loads(task["changed_paths"]):
                self.assertEqual(subprocess.check_output(["git", "rev-parse", f"{tip}:{path}"], cwd=self.repo, text=True).strip(),
                                 subprocess.check_output(["git", "rev-parse", f"{task['candidate']}:{path}"], cwd=self.repo, text=True).strip())
            parent = subprocess.check_output(["git", "rev-parse", f"{task['candidate']}^"], cwd=self.repo, text=True).strip()
            self.assertEqual(parent, task["base"])
            self.assertIn(str(pathlib.Path(reservations[task["id"]]["path"]).resolve()), subprocess.check_output(["git", "worktree", "list", "--porcelain"], cwd=self.repo, text=True))
        self.assertEqual(store.meta("last_verified_sha"), final)
        self.assert_managed_provenance(store, rows, refs, final)

    def test_same_slot_negative_review_repair_after_canonical_advance_completes(self):
        refs = ["refs/heads/recovery/feature-01", "refs/heads/recovery/feature-02"]
        for ref in refs:
            subprocess.check_call(["git", "branch", ref.removeprefix("refs/heads/"), "HEAD"], cwd=str(self.repo))
        cfg = self.managed_cfg(refs[0])
        cfg["workspaces"]["allowed_refs"] = ["refs/heads/main-v2", *refs]
        cfg["workspaces"]["feature_refs"] = refs
        store = engine.setup(self.repo, self.root / "same-slot-repair-state", cfg)
        store.admit([self.spec("T1"), self.spec("T2")], store.meta("config"))
        # Run T2 author first so its owned checkpoint is retained while T1
        # advances canonical; then reject T2 and resume the same task/ref.
        t2 = store.claim("T2", "implement", "fixture-a", {}, 60)
        authored = jobs.stage(store, t2); store.finish(t2["id"], t2["token"], authored); engine.consume(store)
        authored_sha = authored["candidate"]
        t1 = store.claim("T1", "implement", "fixture-a", {}, 60); r1 = jobs.stage(store, t1); store.finish(t1["id"], t1["token"], r1); engine.consume(store)
        for stage in ("review", "test", "prepare", "publish"):
            task = next(x for x in store.tasks() if x["id"] == "T1")
            payload = {"candidate": task["candidate"], "base": task["base"], "workspace": task["workspace"], "author": task["author"], "publish_enabled": True, "reviews": json.loads(task["reviews"] or "[]")}
            j = store.claim("T1", stage, "fixture-a" if stage == "review" else None, payload, 60); result = jobs.stage(store, j); store.finish(j["id"], j["token"], result)
            if stage != "publish": engine.consume(store)
        engine.consume(store)
        task = next(x for x in store.tasks() if x["id"] == "T2")
        reject = store.claim("T2", "review", "fixture-a", {"candidate": task["candidate"], "base": task["base"], "workspace": task["workspace"], "author": task["author"], "publish_enabled": True, "reviews": []}, 60)
        store.finish(reject["id"], reject["token"], {"kind": "review", "candidate": task["candidate"], "reviewer": reject["id"], "verdict": "REJECT", "findings": "deliberate retained-slot rejection"}); engine.consume(store)
        task = next(x for x in store.tasks() if x["id"] == "T2")
        with store.connection(write=True) as cx:
            cx.execute("UPDATE tasks SET state='ready',stage='implement',eligible_at=0 WHERE id='T2'")
        retry = store.claim("T2", "implement", "fixture-a", {}, 60); rerun = jobs.stage(store, retry)
        self.assertEqual(rerun["kind"], "candidate")
        self.assertEqual(rerun["workspace"], task["workspace"])
        self.assertNotEqual(rerun["candidate"], authored_sha)
        store.finish(retry["id"], retry["token"], rerun); engine.consume(store)
        # Resume the real supervisor from this durable Store; it owns every
        # child process, lease renewal and stage transition through fresh
        # review/test/prepare/publication/postcheck.
        run = subprocess.run([sys.executable, str(ENTRY), "run", "--state", str(store.root), "--publish", "--max-seconds", "120"],
                             cwd=str(self.repo), capture_output=True, text=True, timeout=150,
                             env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"})
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        self.assertEqual(next(x for x in store.tasks() if x["id"] == "T2")["state"], "accepted")

    def test_single_task_full_real_git_pipeline(self):
        before=git(self.repo,'rev-parse','HEAD')
        self.store.admit([self.spec('A')],self.store.meta('config'))
        p=self.runflow()
        self.assertEqual(p.returncode,0)
        row=self.store.tasks()[0]
        self.assertEqual(row['state'],'accepted')
        self.assertNotEqual(before,git(self.repo,'rev-parse','HEAD'))
        self.assertEqual((self.repo/'features/A.txt').read_text(),'A\n')
        self.assertEqual(git(self.repo,'status','--porcelain'),'')
        self.assertEqual(row['accepted_sha'],git(self.repo,'rev-parse','HEAD'))

    def test_parallel_and_dependency_with_rebase(self):
        self.store.admit([self.spec('A'),self.spec('B'),self.spec('C',deps=['A','B'])],self.store.meta('config'))
        p=self.runflow()
        self.assertEqual(p.returncode,0)
        self.assertTrue(all(t['state']=='accepted' for t in self.store.tasks()))
        self.assertEqual(len(list((self.repo/'features').glob('*.txt'))),3)
        events=[]
        with self.store.connection() as cx:
            events=[dict(x) for x in cx.execute('SELECT * FROM events ORDER BY seq')]
        accepted={e['task']:e['seq'] for e in events if e['kind']=='task.accepted'}
        child=min(e['seq'] for e in events if e['task']=='C' and e['kind']=='job.claimed')
        self.assertGreater(child,max(accepted['A'],accepted['B']))

    def test_negative_review_drives_optimizer(self):
        self.store.admit([self.spec('A','WRONG_FIRST_FIXTURE')],self.store.meta('config'))
        p=self.runflow()
        self.assertEqual(p.returncode,0)
        impls=[j for j in self.store.jobs() if j['stage']=='implement']
        self.assertEqual(len(impls),2)
        self.assertEqual((self.repo/'features/A.txt').read_text(),'A\n')

    def test_out_of_scope_is_blocked(self):
        before=git(self.repo,'rev-parse','HEAD')
        self.store.admit([self.spec('A','OUT_OF_SCOPE_FIXTURE')],self.store.meta('config'))
        p=self.runflow()
        self.assertEqual(p.returncode,2)
        self.assertEqual(self.store.tasks()[0]['state'],'blocked')
        self.assertEqual(before,git(self.repo,'rev-parse','HEAD'))
        self.assertFalse((self.repo/'outside.txt').exists())

    def test_dirty_source_is_preserved(self):
        before=git(self.repo,'rev-parse','HEAD')
        (self.repo/'README.md').write_text('USER UNSAVED WORK\n')
        self.store.admit([self.spec('A')],self.store.meta('config'))
        p=self.runflow(timeout=8)
        self.assertEqual(p.returncode,3)
        self.assertEqual((self.repo/'README.md').read_text(),'USER UNSAVED WORK\n')
        self.assertEqual(before,git(self.repo,'rev-parse','HEAD'))

    def test_all_twenty_can_claim_implementation(self):
        cfg=dict(self.cfg,workers=20)
        store=engine.setup(self.repo,self.root/'many',cfg)
        store.admit([self.spec('F%02d'%i) for i in range(20)],store.meta('config'))
        jobs=[]
        for row in engine.eligible_tasks(store,False):
            job=store.claim(row['id'],'implement','fixture-a',{},10)
            if job: jobs.append(job)
        self.assertEqual(len(jobs),20)
        self.assertEqual({j['stage'] for j in jobs},{'implement'})


    def test_all_twenty_start_real_implementation_processes(self):
        cfg=dict(self.cfg,workers=20,lease_seconds=60,stage_timeout=60)
        store=engine.setup(self.repo,self.root/'parallel-processes',cfg)
        store.admit([self.spec('P%02d'%i,'SLOW_FIXTURE') for i in range(20)],store.meta('config'))
        children={};owned={}
        try:
            count=engine.dispatch(store,False,children,owned)
            self.assertEqual(count,20)
            self.assertEqual(len(children),20)
            self.assertTrue(all(p.poll() is None for p in children.values()))
            self.assertEqual({j['stage'] for j in store.jobs()}, {'implement'})
            self.assertEqual(len({p.pid for p in children.values()}),20)
        finally:
            with store.connection(write=True) as cx:
                cx.execute("UPDATE jobs SET lease_until=0 WHERE state='running'")
            for proc in children.values():
                try:proc.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    os.killpg(proc.pid,15);proc.wait(timeout=5)

if __name__=='__main__': unittest.main()
