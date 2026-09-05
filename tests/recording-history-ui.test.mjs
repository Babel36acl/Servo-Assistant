import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import vm from 'node:vm';
const require=createRequire(import.meta.url), ts=require('typescript'), vue=require('vue');
function fixture(invoke) {
  const props=vue.reactive({path:'session-A',fileMode:false,fromUs:null,toUs:null}), events=[];
  const script=readFileSync(new URL('../src/components/RecordingHistory.vue',import.meta.url),'utf8').split('<script setup lang="ts">')[1].split('</script>')[0];
  const module={exports:{}};
  vm.runInNewContext(ts.transpileModule(script+'\nexport { inspect, load, history, overview, busy, address, select };',{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2020}}).outputText,{
    module,exports:module.exports,defineProps:()=>props,defineEmits:()=>((...args)=>events.push(args)),
    require:name=>name==='vue'?{...vue,onUnmounted(){}}:{invoke},setTimeout,clearTimeout,
  });
  return {app:module.exports,props,events};
}
const overview={firstUs:10,lastUs:20,count:6,channels:[{source:'port',protocol:'modbus-rtu',slave:1,firstAddress:16,lastAddress:16}],warning:null};
test('history overview ignores replies after the selected session changes',async()=>{
  let resolve;
  const f=fixture(()=>new Promise(r=>{resolve=r;}));
  const pending=f.app.inspect(); f.props.path='session-B'; await vue.nextTick(); resolve(overview); await pending;
  assert.equal(f.app.overview.value,null); assert.equal(f.app.busy.value,false);
});
test('history selection emits the recorded identity and changing address rejects a pending response',async()=>{
  let resolve, pendingMode=false;
  const point={sequence:42,timestampUs:10,source:'port',protocol:'modbus-rtu',transaction:7,raw:65535,value:-0.1,name:'speed',unit:'rpm'};
  const f=fixture(async cmd=>cmd==='recording_overview'?overview:pendingMode?new Promise(r=>{resolve=r;}):{points:[point],total:1,offset:0,warning:null});
  await f.app.inspect(); await vue.nextTick(); await f.app.load(0); f.app.select(0);
  assert.equal(f.events[0][0],'select'); assert.equal(f.events[0][1].sequence,42); assert.equal(f.events[0][1].transaction,7);
  pendingMode=true; const pending=f.app.load(0); f.app.address.value='17'; await vue.nextTick(); resolve({points:[point],total:1,offset:0}); await pending;
  assert.equal(f.app.history.value,null); assert.equal(f.app.busy.value,false);
});
