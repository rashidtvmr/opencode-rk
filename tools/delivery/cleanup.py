"""Retire owned execution workspaces; retain audit patches and failed candidates."""
from __future__ import annotations
import contextlib
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import time


def collect(store):
    base = store.root / 'workspaces'
    if not base.is_dir(): return 0
    tasks = store.tasks(); jobs = {j['id']: j for j in store.jobs()}
    # Never clean a candidate needed by an unfinished task or a live job.
    held = {t['workspace'] for t in tasks if t['workspace'] and t['state']!='accepted'}
    for j in jobs.values():
        if j['state'] in {'running','finished'}:
            held.add(str(base/j['id']))
            p=json.loads(j['payload'])
            if p.get('workspace'):held.add(p['workspace'])
    count=0
    for directory in base.iterdir():
        if directory.is_symlink() or not directory.is_dir() or str(directory) in held:continue
        if not re.fullmatch(r'[0-9a-f]{32}(?:-red|-post)?',directory.name):continue
        owner=directory/'.git/delivery-owner.json'
        if not owner.is_file():continue
        try:data=json.loads(owner.read_text())
        except (OSError,ValueError):continue
        job=jobs.get(data.get('job'))
        if not job or data.get('token')!=job['token'] or job['state']!='consumed':continue
        if job.get('pid'):
            try:os.kill(job['pid'],0);continue
            except ProcessLookupError:pass
            except PermissionError:continue
        # Failed/out-of-scope partial writes require explicit human disposition.
        result=json.loads(job['result'] or '{}')
        if result.get('kind') in {'failure','authority','runtime','cancelled','timeout','interrupted','conflict','suspended','output_limit'}:continue
        # Keep an exact patch for a superseded successful candidate before retiring it.
        if result.get('kind') in {'candidate','prepared'}:
            archive=store.root/'evidence'/job['id']/'candidate.patch'
            archive.parent.mkdir(parents=True,exist_ok=True)
            p=subprocess.run(['git','diff','--binary',result['base'],result['candidate']],cwd=directory,capture_output=True,timeout=30)
            if p.returncode:continue
            archive.write_bytes(p.stdout)
        # Guard against replacement/symlink races at the owned root boundary.
        if directory.resolve().parent!=base.resolve() or directory.is_symlink():continue
        shutil.rmtree(directory)
        with store.connection(write=True) as cx:
            store.event(cx,'workspace.retired',{'job':job['id'],'path':str(directory)},job['task'])
        count+=1
    return count
