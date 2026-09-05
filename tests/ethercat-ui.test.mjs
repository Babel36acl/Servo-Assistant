import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import vm from 'node:vm';
const require = createRequire(import.meta.url), ts = require('typescript'), vue = require('vue');
function fixture(invoke) {
  const script = readFileSync(new URL('../src/components/EthercatMaster.vue', import.meta.url), 'utf8').split('<script setup lang="ts">')[1].split('</script>')[0];
  const source = script + '\nexport { action, write, read, tick, selectSlave, status, slave, profile, values, drafts, confirmWrite, error, busy, polling, selection, history };';
  const module = { exports: {} };
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 } }).outputText, {
    module, exports: module.exports, defineEmits: () => () => {},
    require: id => id === 'vue' ? { ...vue, onMounted() {}, onUnmounted() {} } : id === '@tauri-apps/api/core' ? { invoke } : {},
    setTimeout() {}, clearTimeout() {}, console,
  });
  return module.exports;
}
test('EtherCAT uncertain write invalidates current value and clears confirmation without retry', async () => {
  let calls = 0;
  const app = fixture(async cmd => { assert.equal(cmd, 'ethercat_write'); calls++; throw new Error('timeout after write'); });
  app.values.value.gain = { id: 'gain', raw: '42', value: 42 };
  app.drafts.value.gain = '43'; app.confirmWrite.value = '写入 gain';
  await app.write({ id: 'gain', name: 'Gain' });
  assert.equal(calls, 1); assert.equal(app.values.value.gain, undefined);
  assert.equal(app.confirmWrite.value, ''); assert.match(app.error.value, /timeout/);
  assert.equal(app.drafts.value.gain, '43'); assert.equal(app.busy.value, false);
});
test('monitoring preserves unsent edits and removes failed readings', async () => {
  let fail = false;
  const app = fixture(async cmd => {
    if (cmd === 'ethercat_status') return { connected: true, slaves: [] };
    if (fail) throw new Error('SDO abort');
    return { id: 'gain', raw: '42', value: 42 };
  });
  app.status.value.connected = true; app.polling.value = true; app.selection.value = ['gain']; app.drafts.value.gain = '99';
  await app.tick(); assert.equal(app.values.value.gain.raw, '42'); assert.equal(app.drafts.value.gain, '99');
  fail = true; await app.tick(); assert.equal(app.values.value.gain, undefined); assert.match(app.error.value, /SDO abort/);
  app.selectSlave(); assert.equal(Object.keys(app.drafts.value).length, 0);
});
test('concurrent user operation is ignored while an SDO transaction is in flight', async () => {
  const app = fixture(async () => {}); let release; let calls = 0;
  const first = app.action(() => new Promise(resolve => { release = resolve; }));
  await app.action(async () => { calls++; }); assert.equal(calls, 0); release(); await first;
});
