#!/usr/bin/env python3
"""Bounded loopback provider/daemon harness for the real-browser G6 journey."""
import argparse, hashlib, http.server, json, os, pathlib, subprocess, tempfile, threading, time

ROOT = pathlib.Path('/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp')
MARKER = 'G6 browser fixture marker\n'; CALL_ID = 'g6_write_1'; MAX_BODY = 512 * 1024

def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for b in iter(lambda: f.read(65536), b''): h.update(b)
    return h.hexdigest()

class State:
    def __init__(self, project): self.project, self.requests, self.failed = str(project), [], None
    def response(self, raw):
        if len(raw) > MAX_BODY: raise ValueError('provider request exceeded bound')
        head, _, body = raw.partition(b'\r\n\r\n')
        if b'authorization: bearer fixture-key' not in head.lower(): raise ValueError('wrong provider key')
        value = json.loads(body); self.requests.append(value)
        if len(self.requests) > 8: raise ValueError('provider request count exceeded bound')
        items = value.get('input', []); pair = [x for x in items if x.get('type') in ('function_call', 'function_call_output') and x.get('call_id') == CALL_ID]
        if len(self.requests) == 1:
            if pair: raise ValueError('first request unexpectedly contains tool pair')
            args = {'path': os.path.join(self.project, 'g6-browser-marker.txt'), 'content': MARKER, 'append': False}
            event = {'type':'response.output_item.done','item':{'type':'function_call','id':'g6-item-1','call_id':CALL_ID,'name':'write','arguments':json.dumps(args,separators=(',',':'))}}
            return self.sse(event, '')
        calls = [x for x in pair if x.get('type') == 'function_call']; outputs = [x for x in pair if x.get('type') == 'function_call_output']
        if len(calls) != 1 or len(outputs) != 1 or calls[0].get('name') != 'write' or outputs[0].get('output') != 'write success': raise ValueError('typed call/output pair missing or duplicated')
        text = {2:'G6 first turn settled', 3:'G6 second turn settled', 4:'G6 resumed turn settled'}.get(len(self.requests))
        if not text: raise ValueError('unexpected extra provider request')
        return self.sse(None, text)
    @staticmethod
    def sse(event, text):
        out = ''
        if event: out += 'event: response.output_item.done\ndata: '+json.dumps(event)+'\n\n'
        if text: out += 'event: response.output_text.delta\ndata: '+json.dumps({'type':'response.output_text.delta','delta':text})+'\n\n'
        return (out+'event: response.completed\ndata: {"type":"response.completed","response":{"id":"g6-fixture","status":"completed"}}\n\n').encode()

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        n = int(self.headers.get('Content-Length', '-1'))
        if n < 0 or n > MAX_BODY: self.send_error(413); return
        try:
            raw = ('POST '+self.path+' HTTP/1.1\r\n'+str(self.headers)+'\r\n').encode() + self.rfile.read(n)
            body = self.server.state.response(raw); self.send_response(200); self.send_header('Content-Type','text/event-stream'); self.send_header('Content-Length',str(len(body))); self.end_headers(); self.wfile.write(body)
        except Exception as e: self.server.state.failed = str(e); self.send_error(500, str(e))
    def log_message(self, *_): pass

def stop(child):
    if child and child.poll() is None:
        child.terminate()
        try: child.wait(2)
        except subprocess.TimeoutExpired: child.kill(); child.wait(2)

def main():
    p = argparse.ArgumentParser(); p.add_argument('--binary', required=True); p.add_argument('--artifact-dir', required=True); p.add_argument('--build-json'); a = p.parse_args()
    binary = pathlib.Path(a.binary).resolve(); artifact = pathlib.Path(a.artifact_dir).resolve(); artifact.mkdir(parents=True, exist_ok=True)
    build = json.loads(pathlib.Path(a.build_json or artifact/'build.json').read_text()); source = build.get('source_sha') or build.get('sourceSha') or build.get('git_sha'); expected = (build.get('binary_sha256') or build.get('binarySha256') or '').lower(); actual = sha256(binary)
    if not source or not expected or actual != expected: raise SystemExit('build.json identity/hash mismatch')
    ROOT.mkdir(parents=True, exist_ok=True); root = pathlib.Path(tempfile.mkdtemp(prefix='g6-web-', dir=ROOT)); home, data, project = [root/x for x in ('home','data','project')]
    for x in (home,data,project): x.mkdir()
    (home/'.codex').mkdir(); auth = home/'.codex/auth.json'; auth.write_text('{"access_token":"fixture-key"}\n'); os.chmod(auth, 0o600)
    state = State(project); server = http.server.ThreadingHTTPServer(('127.0.0.1',0), Handler); server.state = state; threading.Thread(target=server.serve_forever, daemon=True).start(); base = 'http://127.0.0.1:%d/v1' % server.server_address[1]
    env = {'HOME':str(home),'XDG_CONFIG_HOME':str(home/'.config'),'XDG_DATA_HOME':str(data),'OPENCODE_RK_HOME':str(data),'PATH':'/usr/bin:/bin','LANG':'C','OPENAI_API_KEY':'fixture-key','OPENAI_BASE_URL':base,'OPENCODE_RK_TURN_TOOLS':'write','OPENCODE_RK_TURN_MAX_STEPS':'4'}
    def launch(): return subprocess.Popen([str(binary),'serve','--listen','127.0.0.1:0'], cwd=project, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    child = launch(); control = artifact/'control'; control.mkdir(exist_ok=True); descriptor = data/'runtime/backend.json'; end = time.monotonic()+15
    while time.monotonic() < end and not descriptor.exists(): time.sleep(.02)
    if not descriptor.exists(): stop(child); raise SystemExit('daemon descriptor readiness timeout')
    d = json.loads(descriptor.read_text()); meta = {'origin':d['http_origin'],'launch_fragment':d['http_origin']+'#oc2-token='+d['auth_token'],'pid':child.pid,'project':str(project),'marker':str(project/'g6-browser-marker.txt'),'source_sha':str(source),'binary_sha256':actual,'provider':base,'control_dir':str(control),'request_evidence':str(artifact/'provider-requests.json'),'auth_json_mode':'0600'}; (artifact/'fixture-metadata.json').write_text(json.dumps(meta,indent=2)+'\n')
    try:
        started = time.monotonic()
        while time.monotonic()-started < 600 and not (control/'stop.request').exists():
            if (control/'restart.request').exists(): (control/'restart.request').unlink(); stop(child); child=launch(); meta['pid']=child.pid; (artifact/'fixture-metadata.json').write_text(json.dumps(meta,indent=2)+'\n')
            if state.failed: raise RuntimeError(state.failed)
            time.sleep(.05)
    finally:
        (artifact/'provider-requests.json').write_text(json.dumps(state.requests,indent=2)[:MAX_BODY]+'\n'); stop(child); server.shutdown(); server.server_close()

if __name__ == '__main__': main()
