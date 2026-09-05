import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import vm from 'node:vm';
const require = createRequire(import.meta.url), ts = require('typescript'), vue = require('vue');
function fixture(invoke) {
  const script = readFileSync(new URL('../src/components/CommunicationWorkbench.vue', import.meta.url), 'utf8').split('<script setup lang="ts">')[1].split('</script>')[0];
  const module = { exports: {} }, timers = new Map(); let id = 0, unmount;
  const source = script + '\nexport { page, path, fileMode, query, direction, rows, visible, total, pageBusy, browseError, follow, toggleFollow, details, selected, decoded, modbusDetail, tick, start, stop, slaveFilter, functionFilter, addressFilter, outcomeFilter, triggerEnabled, preSeconds, postSeconds, triggerKeyword, locatePoint };';
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 } }).outputText, {
    module, exports: module.exports, defineEmits: () => () => {},
    require: name => name === 'vue' ? { ...vue, onMounted() {}, onUnmounted(fn) { unmount = fn; } } : name === '@tauri-apps/api/core' ? { invoke } : {},
    setTimeout(fn) { timers.set(++id, fn); return id; }, clearTimeout(id) { timers.delete(id); }, console,
  });
  return { app: module.exports, timers, unmount: () => unmount() };
}
function row(sequence, protocol = 'modbus-rtu') { return { sequence, timestampUs: 0, source: 'COM-test/1', protocol, direction: 'rx', transaction: 3, bytes: [1,3], detail: '' }; }
const page = (records, total = records.length) => ({ records, total, offset: 0, nextOffset: null, warning: null });

test('session search is sent to backend and does not discard indexed byte matches locally', async () => {
  let request;
  const { app } = fixture(async (cmd, args) => { assert.equal(cmd, 'recording_page'); request = args; return page([row(250)], 1); });
  app.path.value = 'session-A'; app.query.value = '01 03'; app.direction.value = 'rx'; await vue.nextTick();
  await app.page(0);
  assert.equal(request.filter.query, '01 03'); assert.equal(request.filter.direction, 'rx');
  assert.equal(app.visible.value.length, 1); assert.equal(app.total.value, 1);
});
test('changing sessions invalidates an in-flight page; late results cannot overwrite the new session', async () => {
  let finish;
  const { app } = fixture(async (_cmd, args) => args.path === 'session-A' ? new Promise(resolve => { finish = resolve; }) : page([row(2)]));
  app.path.value = 'session-A'; await vue.nextTick(); const pending = app.page(0);
  app.path.value = 'session-B'; await vue.nextTick(); await app.page(0);
  finish(page([row(1)])); await pending;
  assert.equal(app.rows.value[0].sequence, 2); assert.equal(app.pageBusy.value, false);
});
test('pausing follow keeps the viewed rows and ignores a pending tail response', async () => {
  let finish, calls = 0;
  const { app } = fixture(async (_cmd, args) => { assert.equal(args.tail, true); return ++calls === 1 ? page([row(100)], 100) : new Promise(resolve => { finish = resolve; }); });
  app.path.value = 'session'; await vue.nextTick(); await app.toggleFollow();
  assert.equal(app.follow.value, true);
  const pending = app.page(0, true); await app.toggleFollow(); finish(page([row(200)], 200)); await pending;
  assert.equal(app.follow.value, false); assert.equal(app.rows.value[0].sequence, 100);
});
test('transaction selection pauses follow and associates by source, protocol and transaction; stale details are ignored', async () => {
  let finish, request;
  const { app } = fixture(async (cmd, args) => { assert.equal(cmd, 'recording_modbus_transaction'); request = args; return new Promise(resolve => { finish = resolve; }); });
  app.path.value = 'session'; await vue.nextTick(); app.follow.value = true;
  const pending = app.details(row(1)); assert.equal(app.follow.value, false);
  assert.equal(request.source, 'COM-test/1'); assert.equal(request.protocol, 'modbus-rtu'); assert.equal(request.transaction, 3);
  app.path.value = 'other-session'; await vue.nextTick(); finish({ outcome: 'old response' }); await pending;
  assert.equal(app.modbusDetail.value, null); assert.equal(app.selected.value, null);
});
test('follow failures are visible and stop polling; disposal ignores late replies', async () => {
  const f = fixture(async () => { throw new Error('index unavailable'); });
  f.app.path.value = 'session'; await vue.nextTick(); await f.app.toggleFollow();
  assert.equal(f.app.follow.value, false); assert.match(f.app.browseError.value, /index unavailable/);
  let finish; const g = fixture(async () => new Promise(resolve => { finish = resolve; }));
  g.app.path.value = 'session'; await vue.nextTick(); const pending = g.app.page(0); g.unmount(); finish(page([row(1)])); await pending;
  assert.equal(g.app.rows.value.length, 0);
});

test('PCAP uses the global index and structured numeric filters validate before IPC', async () => {
  const calls=[];
  const { app }=fixture(async(cmd,args)=>{ calls.push({cmd,args}); return page([row(250,'ethernet')]); });
  app.path.value='captures'; app.fileMode.value=true; app.query.value='01 03'; await vue.nextTick();
  await app.page(0); assert.equal(calls[0].cmd,'recording_page'); assert.equal(calls[0].args.filter.query,'01 03');
  app.fileMode.value=false; app.slaveFilter.value='1'; app.functionFilter.value='0x03'; app.addressFilter.value='0x10'; app.outcomeFilter.value='failure'; await vue.nextTick();
  await app.page(0); assert.equal(calls[1].args.filter.address,16); assert.equal(calls[1].args.filter.function,3); assert.equal(calls[1].args.filter.outcome,'failure');
  app.slaveFilter.value='invalid'; await vue.nextTick(); await app.page(0); assert.equal(calls.length,2); assert.match(app.browseError.value,/筛选数值/);
});

test('start snapshots mapping and sends the configured trigger before following', async () => {
  const calls=[], status={active:true,path:'new-session',accepted:0,written:0};
  const {app}=fixture(async(cmd,args)=>{calls.push({cmd,args}); if(cmd==='recording_sessions')return ['new-session']; if(cmd==='recording_page')return page([]); if(cmd==='network_capture_status')return {active:false}; return status;});
  app.triggerEnabled.value=true; app.preSeconds.value=20; app.postSeconds.value=5; app.triggerKeyword.value='timeout'; await app.start();
  assert.equal(calls[0].cmd,'recording_mapping'); assert.equal(calls[1].cmd,'start_recording');
  assert.equal(calls[1].args.trigger.preSeconds,20); assert.equal(calls[1].args.trigger.postSeconds,5); assert.equal(calls[1].args.trigger.keyword,'timeout');
  assert.equal(app.path.value,'new-session'); assert.equal(app.follow.value,true);
});

test('history point locates its raw record independently of active text filters', async()=>{
  const calls=[];
  const {app}=fixture(async(cmd,args)=>{calls.push({cmd,args}); if(cmd==='recording_page')return page([row(250)]); return {outcome:'matched'};});
  app.path.value='session'; app.query.value='would hide row'; await vue.nextTick();
  await app.locatePoint(row(250));
  assert.equal(calls[0].args.filter.sequence,250); assert.equal(calls[0].args.filter.query,undefined);
  assert.equal(calls[1].cmd,'recording_modbus_transaction'); assert.equal(app.selected.value.sequence,250); assert.equal(app.follow.value,false);
});
