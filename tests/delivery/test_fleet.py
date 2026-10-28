"""Full rolling fleet acceptance with real subprocesses and disposable Git repos."""
import json
import pathlib
import signal
import subprocess
import sys
import time
import unittest

from test_engine import RepositoryFixture, git, ENTRY
from tools.delivery import engine


class FleetTests(RepositoryFixture):
    def test_twenty_features_converge_without_source_branch_growth(self):
        self.store = engine.setup(self.repo, self.root/'fleet',
            dict(self.cfg, workers=20, max_wip=40, lease_seconds=30, stage_timeout=90))
        self.store.admit([self.spec('F%02d'%i) for i in range(20)], self.store.meta('config'))
        result = self.runflow(timeout=360)
        self.assertEqual(result.returncode, 0, result.stdout[-6000:]+result.stderr[-6000:])
        self.assertEqual(len(self.store.tasks()), 20)
        self.assertTrue(all(t['state']=='accepted' for t in self.store.tasks()))
        self.assertEqual(git(self.repo, 'rev-list', '--count', 'HEAD'), '21')
        self.assertEqual(len(git(self.repo, 'for-each-ref', 'refs/heads').splitlines()), 1)
        self.assertEqual(git(self.repo, 'status', '--porcelain'), '')
        self.assertEqual(len(list((self.repo/'features').glob('*.txt'))), 20)
        stages = {j['stage'] for j in self.store.jobs()}
        self.assertTrue({'implement','review','test','prepare','publish'} <= stages)

    def test_watch_admits_next_task_without_reprompting(self):
        manifest = self.root/'approved-tasks.json'
        a, b = self.spec('A'), self.spec('B', deps=['A'])
        manifest.write_text(json.dumps({'tasks':[a]}))
        log = self.root/'watch.log'
        with log.open('w') as out:
            proc = subprocess.Popen([sys.executable, str(ENTRY), 'run', '--state', str(self.store.root),
                '--publish', '--watch', '--tasks-file', str(manifest)], stdout=out, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic()+90
                while time.monotonic()<deadline:
                    rows = self.store.tasks()
                    if rows and rows[0]['state']=='accepted': break
                    if proc.poll() is not None: self.fail(log.read_text())
                    time.sleep(0.1)
                else: self.fail('first task was not accepted: '+log.read_text())
                pending = manifest.with_suffix('.new')
                pending.write_text(json.dumps({'tasks':[a,b]})); pending.replace(manifest)
                while time.monotonic()<deadline:
                    rows = self.store.tasks()
                    if len(rows)==2 and all(t['state']=='accepted' for t in rows): break
                    if proc.poll() is not None: self.fail(log.read_text())
                    time.sleep(0.1)
                else: self.fail('watched dependency was not accepted: '+log.read_text())
                self.assertEqual(git(self.repo,'rev-list','--count','HEAD'), '3')
            finally:
                if proc.poll() is None: proc.send_signal(signal.SIGINT)
                try: proc.wait(timeout=12)
                except subprocess.TimeoutExpired: proc.kill(); proc.wait(timeout=5)


if __name__=='__main__': unittest.main()
