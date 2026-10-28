"""Deterministic subprocess test double; NEVER presented as a live model run."""
import json
import os
import pathlib
import sys
import time

p=json.loads(os.environ['DELIVERY_JOB_SPEC'])
s=p['task'];stage=p['stage'];mode=s['goal']
if 'SLOW_FIXTURE' in mode: time.sleep(2)
if stage=='implement':
    filename=s['paths'][0]
    pathlib.Path(filename).parent.mkdir(parents=True,exist_ok=True)
    if 'OUT_OF_SCOPE_FIXTURE' in mode: filename='outside.txt'
    if 'WRONG_FIRST_FIXTURE' in mode and not p.get('report'):
        pathlib.Path(filename).write_text('wrong\n')
    else:
        pathlib.Path(filename).write_text(s['id']+'\n')
    print(json.dumps({'summary':'fixture implementation'}))
elif stage=='review':
    target=pathlib.Path(s['paths'][0])
    verdict='APPROVE' if target.is_file() and target.read_text()==s['id']+'\n' else 'NEEDS_FIX'
    print(json.dumps({'verdict':verdict,'findings':['Checked concrete fixture file content']}))
elif stage=='diagnose':
    print(json.dumps({'diagnosis':'Fixture content must equal task id','approach':'Read failure and set the exact task id'}))
else:
    sys.exit(4)
