#!/usr/bin/env python3
"""Bounded offline integration-test owner harness for native cross-page contracts.

This harness deliberately never builds, starts a product PTY, calls a real
provider, reads the user's database, or claims an installed acceptance run.
It runs the existing fixture self-checks and verifies the source-level route /
focus contracts that an installed capture must exercise. Missing artifacts are
an infrastructure failure, never a skipped/pass result.
"""
from __future__ import annotations
import hashlib, json, pathlib, subprocess, sys, time

REPO = pathlib.Path('/Users/mymac/Projects/opencode-rk-recovery-20261005')
OUT = pathlib.Path('/Users/mymac/Projects/opencode-rk/19-integration-tests')
BASE = '78aedc42eddf0800716cf64c9a98f653f4a8758b'
PIN = '95daf90670b7c039c436c85537da5fbfe2205b41'

SELF_CHECKS = [
 ('utf8-draft-route', 'tests/e2e/native_utf8_input.py'),
 ('escape-draft-route', 'tests/e2e/native_escape_input.py'),
 ('second-client-identity', 'tests/e2e/native_second_client.py'),
 ('bracketed-paste-draft', 'tests/e2e/native_bracketed_paste.py'),
 ('tool-ack-history', 'tests/e2e/native_stream_tool.py'),
]
SOURCE_CONTRACTS = {
 'crates/cli/src/native_view.rs': [
   'open_commands_page', 'GET /api/models (picker via /models or Ctrl-P)',
   'ack_permission_reply', 'backend-ACKed session bind',
 ],
 'crates/cli/src/native_view_dialogs.rs': [
   'Permission', 'ApproveOnce', 'UnknownPermissionChoice',
   'backend acknowledgement',
 ],
 'crates/cli/src/native_input_decoder.rs': [
   'InputEvent::Escape', 'ESCAPE_GRACE', 'Paste',
 ],
 'crates/cli/src/native_view_pages.rs': [
   '"/sessions"', '"/models"', 'Page::Commands',
 ],
}

def sha(path):
 h=hashlib.sha256(); h.update(path.read_bytes()); return h.hexdigest()

def run(cmd):
 started=time.monotonic()
 p=subprocess.run(cmd, cwd=REPO, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                  timeout=30, check=False)
 return {'command':' '.join(cmd), 'exit_code':p.returncode, 'duration_ms':round((time.monotonic()-started)*1000),
         'output':p.stdout[-8192:]}

def main():
 OUT.mkdir(parents=True, exist_ok=True)
 results=[]; failed=False
 # Contract checks are intentionally fail-closed and bounded.
 for rel, needles in SOURCE_CONTRACTS.items():
  path=REPO/rel; text=path.read_text()
  missing=[needle for needle in needles if needle not in text]
  item={'kind':'source-contract','path':rel,'sha256':sha(path),'required':needles,'missing':missing,'pass':not missing}
  results.append(item); failed |= bool(missing)
 for name, rel in SELF_CHECKS:
  item=run([sys.executable, str(REPO/rel), '--self-check'])
  item.update({'kind':'offline-fixture-self-check','name':name,'pass':item['exit_code']==0})
  results.append(item); failed |= item['exit_code'] != 0
 evidence={
  'schema':'native-crosspage-integration-v1', 'status':'FAIL' if failed else 'PASS',
  'base_sha':BASE, 'upstream_pin':PIN, 'repo':str(REPO),
  'scope':['real input route transitions','Escape/draft UTF-8 retention','permission/API-key focus ownership','backend-selection identity and ACK gates'],
  'execution':'offline fixture self-checks and source contract checks only',
  'forbidden':['product PTY','Cargo/Bun/network/build','real providers','secrets','user database'],
  'results':results,
 }
 (OUT/'native-crosspage-integration.json').write_text(json.dumps(evidence,ensure_ascii=False,indent=2)+'\n')
 # Human-readable capture regression output is bounded and redaction-safe.
 lines=[f"status: {evidence['status']}",f"base_sha: {BASE}",f"upstream_pin: {PIN}","mode: offline; no product PTY/build/network"]
 for x in results:
  label=x.get('name',x.get('path')); lines.append(f"{'PASS' if x['pass'] else 'FAIL'} {label}")
  if x.get('missing'): lines.append('  missing: '+', '.join(x['missing']))
 (OUT/'native-crosspage-capture.txt').write_text('\n'.join(lines)+'\n')
 print(OUT/'native-crosspage-integration.json')
 return 1 if failed else 0
if __name__=='__main__': raise SystemExit(main())
