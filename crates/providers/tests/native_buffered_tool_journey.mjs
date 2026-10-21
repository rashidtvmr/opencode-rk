// Real native PTY -> daemon -> buffered Responses -> permission broker -> code edit.
// Explicit --live uses the user's requested local 9router model; default is deterministic.
// node crates/providers/tests/native_buffered_tool_journey.mjs BINARY LIB_DIR [--live]
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import http from 'node:http';
import path from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {spawn} from 'node:child_process';
import {setTimeout as delay} from 'node:timers/promises';
const binary=path.resolve(process.argv[2]),library=path.resolve(process.argv[3]),live=process.argv.includes('--live');
const root=path.resolve('target/buffered-tool-evidence');await fs.mkdir(root,{recursive:true});
const fixture=await fs.mkdtemp(path.join(root,live?'live-':'fixture-'));
const data=path.join(fixture,'data'),home=path.join(fixture,'home'),config=path.join(fixture,'config'),state=path.join(fixture,'state');
const model='oc/deepseek-v4.1-flash-free';
const original='export function sum(a, b) { return a - b; }\n';
const corrected='export function sum(a, b) { return a + b; }\n';
const workspace=path.join(fixture,'workspace');await fs.mkdir(workspace);
const reviewTarget=path.join(fixture,'approval-target');await fs.mkdir(reviewTarget);
const filename=path.join(reviewTarget,'sum.mjs');await fs.writeFile(filename,original);
let providerCalls=0;const captured=[];
const provider=http.createServer(async(request,response)=>{
  if(request.url==='/v1/models') {response.setHeader('Content-Type','application/json');response.end(JSON.stringify({data:[{id:model}]}));return;}
  let body='';for await(const bytes of request) body+=bytes;
  const payload=JSON.parse(body);assert.equal(payload.stream,false);assert.equal(payload.model,model);providerCalls++;captured.push(payload);
  const last=payload.input.at(-1);
  const edit=last.role==='user'&&last.content.includes('CODEMOD');
  const output=edit?[{type:'function_call',call_id:`call-${providerCalls}`,name:'write',arguments:JSON.stringify({path:filename,content:corrected})}]:[{type:'message',role:'assistant',content:[{type:'output_text',text:'NATIVE_READY'}]}];
  response.setHeader('Content-Type','application/json');response.end(JSON.stringify({status:'completed',error:null,output}));
});
await new Promise(resolve=>provider.listen(0,'127.0.0.1',resolve));
const reserve=http.createServer();await new Promise(resolve=>reserve.listen(0,'127.0.0.1',resolve));const daemonPort=reserve.address().port;await new Promise(resolve=>reserve.close(resolve));
for(const dir of [home,config,path.join(state,'opencode')]) await fs.mkdir(dir,{recursive:true});
await fs.writeFile(path.join(config,'custom-providers.json'),JSON.stringify({providers:{'9router':{name:'9router',base_url:live?'http://localhost:20128/v1':`http://127.0.0.1:${provider.address().port}/v1`,buffered_model_ids:[model]}}}));
await fs.writeFile(path.join(state,'opencode/model.json'),JSON.stringify({recent:[{providerID:'9router',modelID:model}],favorite:[],variant:{'keep-me':'high'}}));
const driver=fileURLToPath(new URL('../../cli/tests/native_interaction_driver.exp',import.meta.url));
let child,exited,clientNumber=0,startedAt=0;
function launch(){
    clientNumber++;
    startedAt=Date.now();
  child=spawn('/usr/bin/expect',[driver,binary,data,path.join(fixture,`terminal-${clientNumber}.txt`),library],{cwd:workspace,stdio:['pipe','ignore','inherit'],env:{PATH:'/usr/bin:/bin:/usr/sbin:/sbin',HOME:home,XDG_STATE_HOME:state,OPENCODE_CONFIG_DIR:config,OPENCODE_AUTH_CONTENT:'{}',OPENCODE_RK_DAEMON_ADDR:`127.0.0.1:${daemonPort}`,OPENCODE_RK_TURN_TOOLS:'write',OPENCODE_RK_TURN_MAX_STEPS:'4',TERM:'xterm-256color'}});
  exited=new Promise(resolve=>child.once('exit',(code,signal)=>resolve({code,signal})));
}
const send=text=>child.stdin.write(Buffer.from(text).toString('hex')+'\n');
async function events(){
  try {
    const files=(await fs.readdir(path.join(data,'logs'))).filter(name=>name.startsWith('client-')&&name.endsWith('.jsonl')&&Number(name.split('-').at(-1).slice(0,-6))>=startedAt).sort((a,b)=>Number(a.split('-').at(-1).slice(0,-6))-Number(b.split('-').at(-1).slice(0,-6)));
    return (await fs.readFile(path.join(data,'logs',files.at(-1)),'utf8')).split('\n').filter(Boolean).flatMap(line=>{try{return [JSON.parse(line)];}catch{return [];}});
  }catch{return [];}
}
const latest=rows=>rows.filter(row=>row.event==='ui.state').at(-1)?.fields;
async function waitFor(predicate,label,timeout=live?90000:10000){
  const end=Date.now()+timeout;
  while(Date.now()<end){const rows=await events();if(predicate(rows)) return rows;if(child.exitCode!==null)throw Error(`${label}: PTY exited`);await delay(30);}
  throw Error(`${label}: timed out; evidence ${fixture}`);
}
async function messages(){
  const descriptor=JSON.parse(await fs.readFile(path.join(data,'runtime/backend.json'),'utf8'));
  const headers={Authorization:`Bearer ${descriptor.auth_token||descriptor.authToken}`};
  const origin=descriptor.http_origin||descriptor.httpOrigin;
  const listing=await fetch(origin+'/api/sessions',{headers});assert(listing.ok);
  const sessions=(await listing.json()).sessions;assert.equal(sessions.length,1);
  const response=await fetch(origin+`/api/sessions/${sessions[0].id}/messages`,{headers});assert(response.ok);
  return (await response.json()).messages;
}
async function settleAfter(count){
  return waitFor(rows=>rows.filter(row=>row.event==='turn.start').length===count && latest(rows)?.busy===false && rows.filter(row=>row.event==='turn.frame'&&row.fields.type==='assistant_message').length>=count,'turn completed');
}
async function stopClient(){send('\x04');const result=await Promise.race([exited,delay(5000).then(()=>{throw Error('PTY exit timeout');})]);assert.equal(result.code,0);}
const editPrompt=`CODEMOD: Fix the subtraction bug in ${filename}. Current contents are ${JSON.stringify(original)}. Call the write tool with path=${JSON.stringify(filename)}, content=${JSON.stringify(corrected)} and append=false. Do not just describe a change. Use this exact provided code, and stop after the write result.`;
try {
  launch();await waitFor(rows=>latest(rows)?.model===`9router/${model}`,'saved model restored');
  // Denial must settle the request without changing the existing source file.
  send('\x1b[200~'+editPrompt+'\x1b[201~\r');
  await waitFor(rows=>latest(rows)?.permission===true,'first real write approval');
  send('\x1b');
  await settleAfter(1);
  assert.equal(await fs.readFile(filename,'utf8'),original,'denied write changed the source');
  const denied=await messages();
  assert(JSON.stringify(denied).includes('write'),'typed denied call absent');
  // A separate explicit turn authorizes the same codemod once.
  send('\x1b[200~'+editPrompt+'\x1b[201~\r');
  await waitFor(rows=>latest(rows)?.permission===true,'second real write approval');
  send('\x0e\r');
  await settleAfter(2);
  assert.equal(await fs.readFile(filename,'utf8'),corrected);
  const module=await import(pathToFileURL(filename).href);assert.equal(module.sum(5,2),7);
  send('Reply exactly NATIVE_READY without tools.\r');await settleAfter(3);
  const before=await messages();
  assert(before.filter(message=>message.role==='tool').length>=2,'durable tool display rows missing');
  await stopClient();launch();
  await waitFor(rows=>latest(rows)?.model===`9router/${model}`,'restart restored model');
  await delay(250);
  assert.deepEqual(await messages(),before,'restart mutated/replayed durable history');
  assert.equal(await fs.readFile(filename,'utf8'),corrected);
  assert.equal(JSON.parse(await fs.readFile(path.join(state,'opencode/model.json'),'utf8')).variant['keep-me'],'high');
  send('Reply exactly NATIVE_READY without tools.\r');await settleAfter(1);
  if(!live){
    const input=captured.at(-1).input;
    const calls=input.filter(item=>item.type==='function_call');
    const outputs=input.filter(item=>item.type==='function_call_output');
    assert.equal(calls.length,2,'restart lost typed tool calls');
    assert.equal(outputs.length,2,'restart lost typed tool outputs');
    assert.deepEqual(calls.map(call=>call.call_id),outputs.map(output=>output.call_id));
    assert(calls.every(call=>call.name==='write'&&JSON.parse(call.arguments).path===filename));
  }
  await stopClient();
  if(!live)assert.equal(providerCalls,5,'unexpected provider retry or replay');
  console.log(JSON.stringify({passed:true,live,model:`9router/${model}`,fixture,denial_side_effect_free:true,codemod_executed:true,code_result:7,second_turn:true,restart_history_equal:true,provider_requests:live?'live external':providerCalls}));
}finally{
  if(child&&child.exitCode===null&&child.signalCode===null){child.kill('SIGTERM');await exited;}
  try{const descriptor=JSON.parse(await fs.readFile(path.join(data,'runtime/backend.json'),'utf8'));if(Number.isSafeInteger(descriptor.pid)&&descriptor.pid>1)process.kill(descriptor.pid,'SIGINT');}catch{}
  await new Promise(resolve=>provider.close(resolve));
}
