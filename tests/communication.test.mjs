import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import vm from 'node:vm';
const require = createRequire(import.meta.url);
const ts = require('typescript');
const vue = require('vue');

// Execute the actual setup logic with a controlled IPC boundary and timer queue.
// This checks lifecycle behavior, not native WebView rendering or a physical serial link.
function fixture(overrides = {}) {
  const timers = new Map();
  let timerId = 0;
  const api = {
    configureCommunication: async () => {},
    communicationStats: async () => ({}),
    getAuditLog: async () => [],
    readParameters: async ids => ids.map(parameterId => ({ parameterId, value: 42, raw: 42 })),
    readStatuses: async () => [],
    probeRead: async () => ({ success: true, attempts: 1, elapsedMs: 1, error: null }),
    ...overrides,
  };
  function evaluate(source) {
    const module = { exports: {} };
    const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 } }).outputText;
    vm.runInNewContext(compiled, {
      module, exports: module.exports,
      require: id => {
        if (id === 'vue') return { ...vue, onMounted() {}, onUnmounted() {} };
        if (id === './api') return { servoApi: api };
        if (id === './communication') return evaluate(readFileSync(new URL('../src/communication.ts', import.meta.url), 'utf8'));
        return {};
      },
      window: { clearTimeout: id => timers.delete(id), setTimeout: (fn, delay) => { timers.set(++timerId, { fn, delay }); return timerId; } },
      console,
    });
    return module.exports;
  }
  const script = readFileSync(new URL('../src/App.vue', import.meta.url), 'utf8').split('<script setup lang="ts">')[1].split('</script>')[0];
  const app = evaluate(script + '\nexport { readAll, runStabilityTest, scheduleStatusPoll, pollingEnabled, connected, profile, maxRegisters, values, drafts, staleIds, failedGroups, busy, cancelRequested, readProgress, errorMessage, communicationError, testResults, testCycles };');
  app.connected.value = true;
  app.profile.value = { parameters: [{ parameterId: 'P1', address: 1 }, { parameterId: 'P2', address: 3 }], statuses: [{ id: 'speed', address: 4096 }, { id: 'position', address: 4097 }] };
  return { app, timers, api };
}

test('partial reads retain successful values and edits; retry only unfinished groups', async () => {
  const calls = [];
  let fail = true;
  const { app } = fixture({ readParameters: async ids => {
    calls.push([...ids]);
    if (ids[0] === 'P2' && fail) throw new Error('CRC');
    return ids.map(parameterId => ({ parameterId, value: 42, raw: 42 }));
  } });
  app.values.value = { P1: { value: 1, raw: 1 }, P2: { value: 2, raw: 2 } };
  app.drafts.value = { P1: 9, P2: 2 };
  await app.readAll();
  assert.equal(app.values.value.P1.value, 42);
  assert.equal(app.drafts.value.P1, 9);
  assert.equal(app.values.value.P2.value, 2);
  assert.equal(app.staleIds.value.has('P2'), true);
  assert.equal(app.failedGroups.value.length, 1);
  fail = false;
  await app.readAll(true);
  assert.deepEqual(calls, [['P1'], ['P2'], ['P2']]);
  assert.equal(app.failedGroups.value.length, 0);
  assert.equal(app.staleIds.value.size, 0);
});

test('duplicate read is ignored and cancellation retains pending groups', async () => {
  let release;
  let calls = 0;
  const { app } = fixture({ readParameters: () => { calls++; return new Promise(resolve => { release = resolve; }); } });
  const running = app.readAll();
  while (!release) await Promise.resolve();
  await app.readAll();
  app.cancelRequested.value = true;
  release([{ parameterId: 'P1', value: 42, raw: 42 }]);
  await running;
  assert.equal(calls, 1);
  assert.equal(app.busy.value, false);
  assert.equal(app.failedGroups.value.length, 1);
});

test('poll recovery restores interval without clearing unrelated operation errors', async () => {
  let fail = true;
  const { app, timers } = fixture({ readStatuses: async () => { if (fail) throw new Error('CRC'); return []; } });
  app.errorMessage.value = 'write failed';
  app.scheduleStatusPoll(0);
  let timer = [...timers.values()][0]; timers.clear(); await timer.fn();
  assert.equal([...timers.values()][0].delay, 3000);
  fail = false;
  timer = [...timers.values()][0]; timers.clear(); await timer.fn();
  assert.equal([...timers.values()][0].delay, 250);
  assert.equal(app.communicationError.value, '');
  assert.equal(app.errorMessage.value, 'write failed');
  app.pollingEnabled.value = false;
  await vue.nextTick();
  assert.equal(timers.size, 0);
});

test('stability cancellation ends after the in-flight read and preserves metrics', async () => {
  let release;
  const { app } = fixture({ probeRead: () => new Promise(resolve => { release = resolve; }) });
  const running = app.runStabilityTest();
  while (!release) await Promise.resolve();
  app.cancelRequested.value = true;
  release({ success: true, attempts: 2, elapsedMs: 10, error: null });
  await running;
  assert.equal(app.testResults.value[0].recovered, 1);
  assert.equal(app.testResults.value[1].total, 0);
  assert.equal(app.busy.value, false);
});
