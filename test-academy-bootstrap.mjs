import assert from 'node:assert/strict';
import test from 'node:test';
import {readFileSync,existsSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import vm from 'node:vm';

const root=fileURLToPath(new URL('.',import.meta.url));
const film=existsSync(join(root,'apps/filmcraft-web/web/index.html'));
const web=join(root,film?'apps/filmcraft-web/web':'apps/soundcraft-web');
let html;
if(film)html=readFileSync(join(web,'index.html'),'utf8');
else {
 const result=spawnSync(process.platform==='win32'?'python':'python3',['-c',"import aca_build_web as p;print(p.browser_html((p.ROOT/'apps/soundcraft-web/index.html').read_text()))"],{cwd:root,encoding:'utf8'});
 assert.equal(result.status,0,result.stderr);html=result.stdout;
}
const modules=[...html.matchAll(/<script type="module">([\s\S]*?)<\/script>/g)];
assert.ok(modules.length);
let bootstrap=modules.at(-1)[1];
bootstrap=bootstrap.replace('await import("./filmcraft_web.js" + v)','await Promise.resolve(bindingsFixture)').replace("import init, * as bindings from './soundcraft_web.js';","const bindings=bindingsFixture,init=bindings.default;");
const bridge=readFileSync(join(web,'academy-bridge.js'),'utf8');

test('actual bootstrap keeps uninitialized WASM exports private during deferred init',async()=>{
 const timers=[],sent=[],calls=[],listeners={};let initialized=false,resolveInit,exportCalls=0;
 const deferred=new Promise(resolve=>{resolveInit=()=>{initialized=true;resolve();};});
 const bindings={default:()=>deferred,start:async()=>{},academyReady:()=>{exportCalls++;assert.ok(initialized,'uninitialized native export called');return true;},academyTheme:value=>calls.push(value)};
 const window={open:()=>null,filmcraftLoad:{}};
 const parent={postMessage:message=>sent.push(message)};
 const context={window,parent,bindingsFixture:bindings,location:{origin:'https://academy.test',href:'https://academy.test/studios/native/app/',search:''},document:{documentElement:{dataset:{},style:{}},getElementById:()=>({style:{}})},performance:{now:()=>1},URL,URLSearchParams,WebAssembly,localStorage:{getItem:()=>null},setTimeout:fn=>timers.push(fn),addEventListener:(name,fn)=>{listeners[name]=fn;},removeEventListener:()=>{}};
 vm.runInNewContext(bridge,context);
 const pending=vm.runInNewContext('(async()=>{'+bootstrap+'})()',context);
 await Promise.resolve();await Promise.resolve();
 assert.equal(window.wasmBindings,undefined);
 listeners.message({origin:'https://academy.test',source:parent,data:{type:'aca:theme',version:1,theme:'light'}});
 timers.shift()();
 assert.equal(exportCalls,0);
 resolveInit();await pending;await Promise.resolve();await Promise.resolve();
 assert.equal(window.wasmBindings,bindings);
 timers.shift()();
 assert.ok(exportCalls>0);assert.deepEqual(calls,[true]);assert.equal(sent[0].type,'aca:ready');
});
