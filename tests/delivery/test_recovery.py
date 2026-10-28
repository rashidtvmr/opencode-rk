import json
import os
import pathlib
import subprocess
import sys
import time
import unittest

import test_engine as fixtures
from test_engine import git, ENTRY
from tools.delivery import engine, jobs


class RecoveryTests(fixtures.RepositoryFixture):
    def prepared(self):
        self.store.admit([self.spec('A')],self.store.meta('config'))
        base=git(self.repo,'rev-parse','HEAD')
        candidate_dir=self.root/'prepared'
        subprocess.check_call(['git','clone','-q',str(self.repo),str(candidate_dir)])
        git(candidate_dir,'config','user.name','Fixture');git(candidate_dir,'config','user.email','fixture@localhost.invalid')
        (candidate_dir/'features').mkdir();(candidate_dir/'features/A.txt').write_text('A\n')
        git(candidate_dir,'add','.');git(candidate_dir,'commit','-qm','candidate')
        sha=git(candidate_dir,'rev-parse','HEAD')
        review={'candidate':sha,'reviewer':'independent-fixture','verdict':'APPROVE','security':False}
        with self.store.connection(write=True) as cx:
            cx.execute("UPDATE tasks SET state='ready',stage='publish',base=?,candidate=?,workspace=?,author='fixture-author',reviews=?",
                       (base,sha,str(candidate_dir),json.dumps([review])))
        p={'base':base,'candidate':sha,'workspace':str(candidate_dir),'author':'fixture-author','reviews':[review],
           'publish_enabled':True,'config_hash':self.store.meta('config_hash')}
        job=self.store.claim('A','publish',None,p,20)
        return base,sha,candidate_dir,job

    def test_crash_after_merge_rechecks_without_duplicate_merge(self):
        base,sha,directory,job=self.prepared()
        self.store.publish_intent(job['id'],'A',base,sha,{'trusted_fixture':'simulated interruption after validated publication'})
        git(self.repo,'fetch','-q',str(directory),sha);git(self.repo,'merge','--ff-only',sha)
        self.store.publication(job['id'],'landed')
        self.store.finish(job['id'],job['token'],{'kind':'interrupted','error':'simulated crash'})
        engine.consume(self.store)
        self.assertEqual(self.store.tasks()[0]['stage'],'postcheck')
        p=self.runflow(timeout=30)
        self.assertEqual(p.returncode,0)
        self.assertEqual(git(self.repo,'rev-list','--count','HEAD'),'2')
        self.assertEqual(self.store.tasks()[0]['accepted_sha'],sha)
        self.assertEqual(self.store.publication(job['id'])['state'],'verified')

    def test_crash_before_merge_retries_only_unlanded_intent(self):
        base,sha,directory,job=self.prepared()
        self.store.publish_intent(job['id'],'A',base,sha,{'trusted_fixture':'intent only'})
        self.store.finish(job['id'],job['token'],{'kind':'interrupted','error':'simulated crash'})
        engine.consume(self.store)
        self.assertEqual(self.store.tasks()[0]['stage'],'publish')
        p=self.runflow(timeout=30)
        self.assertEqual(p.returncode,0)
        self.assertEqual(git(self.repo,'rev-list','--count','HEAD'),'2')

    def test_ambiguous_publication_requires_reconciliation(self):
        base,sha,directory,job=self.prepared()
        self.store.publish_intent(job['id'],'A',base,sha,{})
        (self.repo/'other.txt').write_text('external change\n');git(self.repo,'add','.');git(self.repo,'commit','-qm','external')
        actual=git(self.repo,'rev-parse','HEAD')
        self.store.finish(job['id'],job['token'],{'kind':'interrupted','error':'simulated crash'})
        engine.consume(self.store)
        self.assertEqual(self.store.tasks()[0]['state'],'blocked')
        self.assertEqual(git(self.repo,'rev-parse','HEAD'),actual)

    def test_full_suite_failure_never_lands(self):
        base,sha,directory,job=self.prepared()
        c=self.store.meta('config')
        c['full_checks']=[{'id':'intentional-failure','argv':[sys.executable,'-c',"raise AssertionError('full suite fixture failed')"],
                          'required_output':[],'timeout':30}]
        self.store.meta('config',c)
        from tools.delivery.runtime import Failure
        with self.assertRaises(Failure):jobs.stage(self.store,job)
        self.assertEqual(git(self.repo,'rev-parse','HEAD'),base)
        self.assertIsNone(self.store.publication(job['id']))

    def test_post_merge_failure_is_not_accepted(self):
        base,sha,directory,job=self.prepared()
        c=self.store.meta('config')
        c['post_merge_checks']=[{'id':'post-failure','argv':[sys.executable,'-c',"raise AssertionError('post-merge failure fixture')"],
                                'required_output':[],'timeout':30}]
        self.store.meta('config',c)
        result=jobs.stage(self.store,job)
        self.assertEqual(result['kind'],'regression')
        self.assertEqual(self.store.publication(job['id'])['state'],'postcheck_failed')
        self.assertTrue(self.store.meta('stop_new_publications'))
        self.assertIsNone(self.store.tasks()[0]['accepted_sha'])

    def test_source_guard_blocks_without_reimplementation_or_merge(self):
        base,sha,directory,job=self.prepared()
        c=self.store.meta('config')
        c['guard_checks']=[{'id':'guard','argv':[sys.executable,'-c',"raise SystemExit('guard failure')"],
                           'required_output':[],'timeout':30}]
        self.store.meta('config',c)
        from tools.delivery.runtime import Failure
        with self.assertRaises(Failure) as cm:jobs.stage(self.store,job)
        self.assertEqual(cm.exception.kind,'wait')
        self.assertEqual(git(self.repo,'rev-parse','HEAD'),base)


    def test_postcheck_uses_fresh_published_checkout(self):
        base,sha,directory,job=self.prepared()
        cfg=self.store.meta('config')
        cfg['full_checks']=[{'id':'build-side-effect','argv':[sys.executable,'-c',
            "import pathlib;pathlib.Path('premerge-only.tmp').write_text('fixture');print('PRECHECK PASS')"],
            'required_output':['PRECHECK PASS'],'timeout':30}]
        cfg['post_merge_checks']=[{'id':'clean-checkout','argv':[sys.executable,'-c',
            "import pathlib;assert not pathlib.Path('premerge-only.tmp').exists();print('FRESH POSTCHECK')"],
            'required_output':['FRESH POSTCHECK'],'timeout':30}]
        self.store.meta('config',cfg)
        result=jobs.stage(self.store,job)
        self.assertEqual(result['kind'],'accepted')

    def test_inflight_publish_cannot_be_cancelled_as_if_unlanded(self):
        base,sha,directory,job=self.prepared()
        with self.assertRaises(ValueError):self.store.cancel('A')
        self.assertTrue(self.store.valid(job['id'],job['token']))

    def test_previously_landed_commit_rewind_is_not_replayed(self):
        base,sha,directory,job=self.prepared()
        self.store.publish_intent(job['id'],'A',base,sha,{})
        self.store.publication(job['id'],'landed')
        # Canonical happens to be at old SHA, but durable history proves this
        # was published previously. That is ambiguous, not a safe first retry.
        result=jobs.reconcile_publication(self.store,job,lambda:True)
        self.assertEqual(result['kind'],'blocked')

    def test_cancel_retry_rejects_old_finished_generation(self):
        self.store.admit([self.spec('A')],self.store.meta('config'))
        old=self.store.claim('A','implement','fixture-a',{},30)
        self.store.finish(old['id'],old['token'],{'kind':'candidate','candidate':'a'*40,'base':'b'*40,
            'workspace':'/does-not-exist','author':old['id'],'changed_paths':['features/A.txt']})
        self.store.cancel('A'); self.store.retry('A','explicit retry after cancellation')
        engine.consume(self.store)
        row=self.store.tasks()[0]
        self.assertEqual(row['stage'],'implement');self.assertEqual(row['state'],'pending')
        self.assertIsNone(row['candidate'])

    def test_source_policy_guard_detects_unreviewed_controller_change(self):
        (self.repo/'AGENTS.md').write_text('unreviewed policy change\n')
        git(self.repo,'add','AGENTS.md');git(self.repo,'commit','-qm','policy change')
        self.assertTrue(any('protected policy' in x for x in jobs.source_gate(self.store)))

    def test_verified_dependency_history_cannot_disappear_silently(self):
        base=git(self.repo,'rev-parse','HEAD')
        (self.repo/'approved.txt').write_text('verified\n');git(self.repo,'add','.');git(self.repo,'commit','-qm','verified')
        verified=git(self.repo,'rev-parse','HEAD');self.store.meta('last_verified_sha',verified)
        # Only this disposable test repository is reset, never a user repository.
        git(self.repo,'reset','--hard',base)
        self.assertTrue(any('lost a verified commit' in x for x in jobs.source_gate(self.store)))

if __name__=='__main__':unittest.main()
