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
from tools.delivery import engine, model
from tools.delivery.store import Store

FIXTURE=ROOT/'tests/delivery/fixtures/agent.py'
ENTRY=ROOT/'tools/continuous_delivery.py'


def git(root,*args):
    return subprocess.check_output(['git',*args],cwd=root,text=True,stderr=subprocess.STDOUT).strip()


class RepositoryFixture(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='delivery-test-')
        self.root=pathlib.Path(self.temp.name)
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

    def tearDown(self): self.temp.cleanup()

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
