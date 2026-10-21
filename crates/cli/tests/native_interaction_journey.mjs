// Installed native regression: real PTY, daemon, provider, saved model and logs.
// Run: node crates/cli/tests/native_interaction_journey.mjs BINARY NATIVE_LIB_DIR
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import http from 'node:http';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawn} from 'node:child_process';
import {setTimeout as delay} from 'node:timers/promises';
import {terminalCells} from './native_terminal_cells.mjs';

const binary = path.resolve(process.argv[2]);
const library = path.resolve(process.argv[3]);
const root = path.resolve('target/native-interaction-evidence');
await fs.mkdir(root, {recursive:true});
const fixture = await fs.mkdtemp(path.join(root, 'journey-'));
const data = path.join(fixture, 'data');
const home = path.join(fixture, 'home');
const config = path.join(fixture, 'config');
const state = path.join(fixture, 'state');
const model = 'oc/deepseek-v4.1-flash-free';
const prompt = 'fixture-secret-prompt-canary café 世界';
const calls = [];
let reply = 'reject';
const provider = http.createServer(async (request, response) => {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  if (request.url === '/v1/models') {
    // Discovery must complete while no key is pressed.
    await delay(150);
    response.setHeader('Content-Type', 'application/json');
    response.end(JSON.stringify({data:[{id:model}, {id:'other/model'}]}));
    return;
  }
  assert.equal(request.url, '/v1/responses');
  const payload = JSON.parse(Buffer.concat(chunks));
  calls.push(payload);
  if (reply === 'reject') {
    response.writeHead(429, {'Content-Type':'application/json'});
    response.end(JSON.stringify({error:{message:'fixture-provider-secret-canary'}}));
    return;
  }
  response.writeHead(200, {'Content-Type':'text/event-stream'});
  response.end('event: response.output_text.delta\ndata: '+JSON.stringify({type:'response.output_text.delta',delta:'FIXTURE_REPLY'})+'\n\nevent: response.completed\ndata: '+JSON.stringify({type:'response.completed',response:{status:'completed',output:[]}})+'\n\n');
});
await new Promise(resolve => provider.listen(0, '127.0.0.1', resolve));
const port = provider.address().port;
const reservation = http.createServer();
await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
const daemonPort = reservation.address().port;
await new Promise(resolve => reservation.close(resolve));
for (const directory of [home, config, path.join(state,'opencode')]) await fs.mkdir(directory,{recursive:true});
await fs.writeFile(path.join(config,'custom-providers.json'), JSON.stringify({providers:{'9router':{name:'9router',base_url:`http://127.0.0.1:${port}/v1`}}}));
await fs.writeFile(path.join(state,'opencode/model.json'),JSON.stringify({recent:[{providerID:'9router',modelID:model}],favorite:[],variant:{'preserve-me':'high'}}));
const transcript = path.join(fixture,'terminal.txt');
const driver = fileURLToPath(new URL('./native_interaction_driver.exp',import.meta.url));
const child = spawn('/usr/bin/expect',[driver,binary,data,transcript,library],{
  cwd:fixture,stdio:['pipe','ignore','pipe'],env:{PATH:'/usr/bin:/bin:/usr/sbin:/sbin',HOME:home,XDG_STATE_HOME:state,OPENCODE_CONFIG_DIR:config,OPENCODE_AUTH_CONTENT:'{}',OPENCODE_RK_DAEMON_ADDR:`127.0.0.1:${daemonPort}`,TERM:'xterm-256color',DYLD_LIBRARY_PATH:library},
});
let driverError = '';
child.stderr.on('data',bytes => { driverError += bytes.toString(); });
const exited = new Promise(resolve=>child.once('exit',(code,signal)=>resolve({code,signal})));
const send = bytes => child.stdin.write(Buffer.from(bytes).toString('hex')+'\n');
async function events() {
  try {
    const files = await fs.readdir(path.join(data,'logs'));
    const name = files.find(name=>name.startsWith('client-') && name.endsWith('.jsonl'));
    if (!name) return [];
    const text = await fs.readFile(path.join(data,'logs',name),'utf8');
    return text.split('\n').filter(Boolean).flatMap(line=>{try{return [JSON.parse(line)];}catch{return [];}});
  } catch { return []; }
}
async function waitFor(predicate, label, timeout=10000) {
  const end = Date.now()+timeout;
  while(Date.now()<end) {
    const rows=await events();
    if(predicate(rows)) return rows;
    if(child.exitCode!==null) throw Error(`${label}: PTY exited ${child.exitCode}: ${driverError}`);
    await delay(25);
  }
  throw Error(`${label}: timed out; evidence ${fixture}; driver ${driverError}`);
}
const lastState = rows => rows.filter(row=>row.event==='ui.state').at(-1)?.fields;
try {
  let rows = await waitFor(rows=>lastState(rows)?.model===`9router/${model}`, 'saved model restored without input');
  assert(rows.filter(row=>row.event==='ui.state').every(row=>row.fields.dialog==='None'), 'startup opened a modal');
  const initial = await fs.readFile(transcript,'utf8');
  assert(initial.includes('\x1b[?1006h'), 'native mouse capture missing');
  // Mouse drags, empty Enter and protocol replies must not submit or type.
  send('\x1b[<0;70;20M\x1b[<32;72;22M\x1b[<0;72;22m\r');
  await delay(150);
  assert.equal(calls.length,0);
  assert.equal(lastState(await events()).composer_bytes,0);
  send('\x1b[200~'+prompt+'\x1b[201~\r');
  rows = await waitFor(rows=>rows.some(row=>row.event==='turn.provider_failure'), 'provider refusal');
  await waitFor(rows=>lastState(rows)?.busy===false && lastState(rows)?.composer_bytes===Buffer.byteLength(prompt), 'refused draft preserved');
  assert.equal(calls.length,1);
  assert.equal(calls[0].model,model);
  // Palette and model search use their own fields, retaining the unsent draft.
  send('\x10model');
  await waitFor(rows=>lastState(rows)?.palette===true, 'command palette search');
  send('\r');
  await waitFor(rows=>lastState(rows)?.dialog==='Model', 'model dialog opened');
  send('deepseek-v4.1');
  await delay(100);
  assert(terminalCells(await fs.readFile(transcript,'utf8')).some(row=>row.includes('Search models:')), 'model search field missing');
  send('\x1b');
  await waitFor(rows=>lastState(rows)?.dialog==='None' && lastState(rows)?.composer_bytes===Buffer.byteLength(prompt), 'popup cancel preserved draft');
  reply='success';
  send('\r');
  await waitFor(rows=>rows.some(row=>row.event==='turn.frame' && row.fields.type==='assistant_message'), 'successful retry response');
  await waitFor(rows=>lastState(rows)?.busy===false && lastState(rows)?.composer_bytes===0, 'backend ACK consumed draft');
  assert.equal(calls.length,2, 'unexpected replay');
  const preferences=JSON.parse(await fs.readFile(path.join(state,'opencode/model.json'),'utf8'));
  assert.equal(preferences.variant['preserve-me'],'high');
  const logs=JSON.stringify(await events());
  assert(!logs.includes(prompt));
  assert(!logs.includes('fixture-provider-secret-canary'));
  send('\x04');
  const exit = await Promise.race([exited,delay(5000).then(()=>{throw Error('PTY failed to exit');})]);
  assert.equal(exit.code,0);
  const terminal=await fs.readFile(transcript,'utf8');
  assert(terminal.includes('\x1b[?1006l'), 'mouse capture not restored');
  console.log(JSON.stringify({passed:true,fixture,provider_requests:calls.length,startup_modal:false,search:true,mouse_capture_restored:true,draft_ack:true,metadata_only_logs:true}));
} finally {
  if(child.exitCode===null && child.signalCode===null) { child.kill('SIGTERM'); await exited; }
  try {
    const descriptor=JSON.parse(await fs.readFile(path.join(data,'runtime/backend.json'),'utf8'));
    if(Number.isSafeInteger(descriptor.pid) && descriptor.pid>1) process.kill(descriptor.pid,'SIGINT');
  } catch {}
  await new Promise(resolve=>provider.close(resolve));
}
