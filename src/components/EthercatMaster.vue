<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import ScopeChart from './ScopeChart.vue';
type Adapter = { name: string; description: string };
type Slave = { position: number; station: number; name: string; vendor: number; product: number; revision: number; state: number; alCode: number; mailboxOut: number; mailboxIn: number };
type Status = { connected: boolean; adapter: string; slaves: Slave[]; driverDropped: number | null };
type ObjectDef = { id: string; name: string; index: number; subIndex: number; size: number; signed: boolean; writable: boolean; min: string; max: string; scale: number; unit: string };
type Profile = { schemaVersion: string; name: string; vendor: number; product: number; objects: ObjectDef[] };
type Value = { id: string; raw: string; value: number; timestampUs: number };
const emit = defineEmits<{ mapping: [value: { mailboxes: Array<{ station: number; offset: number }> }] }>();
const adapters = ref<Adapter[]>([]), adapter = ref(''), confirmation = ref('');
const status = ref<Status>({ connected: false, adapter: '', slaves: [], driverDropped: null });
const profile = ref<Profile | null>(null), slave = ref(1), error = ref(''), notice = ref('');
const busy = ref(false), polling = ref(false), interval = ref(1000), selection = ref<string[]>([]);
const values = ref<Record<string, Value>>({}), drafts = ref<Record<string, string>>({}), history = ref<Record<string, number[]>>({});
const confirmWrite = ref(''); let timer: ReturnType<typeof setTimeout> | undefined; let disposed = false;
async function action(fn: () => Promise<void>) { if (busy.value) return; busy.value = true; error.value = ''; try { await fn(); } catch (e) { error.value = String(e); } finally { busy.value = false; } }
async function refreshAdapters() { await action(async () => { adapters.value = await invoke<Adapter[]>('ethercat_adapters'); if (!adapters.value.some(a => a.name === adapter.value)) adapter.value = adapters.value[0]?.name ?? ''; }); }
async function connect() { await action(async () => { status.value = await invoke<Status>('ethercat_connect', { adapter: adapter.value, confirmation: confirmation.value }); values.value = {}; history.value = {}; notice.value = '主站已连接：仅请求 PRE-OP，未发送 PDO 输出。'; }); }
async function disconnect() { polling.value = false; await action(async () => { await invoke('ethercat_disconnect'); status.value = { connected: false, adapter: '', slaves: [], driverDropped: null }; values.value = {}; history.value = {}; }); }
async function importProfile(event: Event) { const file = (event.target as HTMLInputElement).files?.[0]; if (!file) return; await action(async () => { if (file.size > 1024 * 1024) throw new Error('Profile 超过 1 MiB'); const data: unknown = JSON.parse(await file.text()); profile.value = await invoke<Profile>('ethercat_profile', { profile: data }); values.value = {}; drafts.value = {}; selection.value = profile.value.objects.slice(0, 4).map(o => o.id); history.value = {}; }); }
async function read(id: string) { const v = await invoke<Value>('ethercat_read', { slave: slave.value, id }); values.value[id] = v; if (drafts.value[id] === undefined) drafts.value[id] = v.raw; return v; }
async function write(o: ObjectDef) { await action(async () => {
  const before = values.value[o.id]; if (!before) throw new Error('先读取当前值');
  try { const v = await invoke<Value>('ethercat_write', { slave: slave.value, id: o.id, value: drafts.value[o.id], expected: before.raw, confirmation: confirmWrite.value }); values.value[o.id] = v; notice.value = `${o.name} 已写入并回读校验`; }
  catch (e) { delete values.value[o.id]; throw e; } finally { confirmWrite.value = ''; }
}); }
function selectSlave() { values.value = {}; drafts.value = {}; history.value = {}; }
function stateName(state: number) { const base = ({ 1: 'INIT', 2: 'PRE-OP', 3: 'BOOT', 4: 'SAFE-OP', 8: 'OP' } as Record<number, string>)[state & 15] ?? '未知'; return `${base}${state & 16 ? ' + ERROR' : ''}`; }
function useMailboxes() { emit('mapping', { mailboxes: status.value.slaves.flatMap(s => [s.mailboxOut, s.mailboxIn].filter(Boolean).map(offset => ({ station: s.station, offset }))) }); }
async function tick() {
  if (disposed) return;
  if (polling.value && status.value.connected && !busy.value) {
    await action(async () => { status.value = await invoke<Status>('ethercat_status', { refresh: true }); for (const id of selection.value) { if (disposed || !polling.value) break; try { const v = await read(id); history.value[id] = [...(history.value[id] ?? []), v.value].slice(-300); } catch (e) { delete values.value[id]; throw e; } } });
  }
  if (!disposed) timer = setTimeout(tick, Math.min(60000, Math.max(100, interval.value || 1000)));
}
onMounted(async () => { try { status.value = await invoke<Status>('ethercat_status', { refresh: false }); } catch (e) { error.value = String(e); } void tick(); });
onUnmounted(() => { disposed = true; polling.value = false; clearTimeout(timer); });
</script>

<template>
  <section class="ec-master">
    <h2>EtherCAT 主站 · 配置与监控</h2>
    <p>连接会接管所选网卡上的 EtherCAT 总线并初始化从站。请先停用其他主站；当前仅支持 PRE-OP 下的 CoE/SDO 配置与轮询。</p>
    <div class="controls">
      <button :disabled="busy" @click="refreshAdapters">刷新网卡</button>
      <select v-model="adapter" :disabled="status.connected || busy"><option value="">选择专用网卡</option><option v-for="a in adapters" :key="a.name" :value="a.name">{{ a.description }}</option></select>
      <input v-model="confirmation" :disabled="status.connected || busy" placeholder="输入：接管 EtherCAT 总线" aria-label="主站接管确认" />
      <button :disabled="busy || status.connected || !adapter" @click="connect">连接并发现从站</button>
      <button :disabled="busy || !status.connected" @click="disconnect">断开主站</button>
    </div>
    <div class="controls">
      <label>EtherCAT Profile <input type="file" accept=".json" :disabled="busy" @change="importProfile" /></label>
      <span>{{ profile?.name ?? '未导入配置' }}</span>
      <button :disabled="busy || !status.connected" @click="action(async () => { status = await invoke<Status>('ethercat_status', { refresh: true }); })">刷新从站状态</button>
      <button :disabled="!status.slaves.length" @click="useMailboxes">将从站邮箱地址用于解析</button>
    </div>
    <p v-if="error" class="error" role="alert">{{ error }}</p><p v-if="notice">{{ notice }}</p>
    <p>主站：{{ status.connected ? '已连接' : '未连接' }} · 驱动丢包：{{ status.driverDropped ?? '未知' }}</p>
    <table v-if="status.slaves.length"><thead><tr><th>从站</th><th>名称</th><th>Vendor / Product / Revision</th><th>状态</th><th>AL 状态码</th></tr></thead><tbody>
      <tr v-for="s in status.slaves" :key="s.position"><td>{{ s.position }} / 0x{{ s.station.toString(16) }}</td><td>{{ s.name }}</td><td>{{ s.vendor }} / {{ s.product }} / {{ s.revision }}</td><td>{{ stateName(s.state) }}</td><td>0x{{ s.alCode.toString(16) }}</td></tr>
    </tbody></table>
    <div v-if="profile" class="controls">
      <label>目标从站 <select v-model.number="slave" :disabled="busy" @change="selectSlave"><option v-for="s in status.slaves" :key="s.position" :value="s.position">{{ s.position }} · {{ s.name }}</option></select></label>
      <label><input v-model="polling" type="checkbox" :disabled="!status.connected" />监控所选对象（最多 6 个）</label>
      <label>读取完成后等待 <input v-model.number="interval" type="number" min="100" max="60000" /> ms</label>
    </div>
    <table v-if="profile"><thead><tr><th>监控</th><th>对象</th><th>原始值 / 工程值</th><th>待写原始整数</th><th>操作</th></tr></thead><tbody>
      <tr v-for="o in profile.objects" :key="o.id"><td><input v-model="selection" type="checkbox" :value="o.id" :disabled="!selection.includes(o.id) && selection.length >= 6" /></td><td>{{ o.name }}<br /><code>0x{{ o.index.toString(16) }}:{{ o.subIndex }} · {{ o.id }}</code></td><td>{{ values[o.id]?.raw ?? '未读取 / 已过期' }}<br />{{ values[o.id]?.value ?? '—' }} {{ o.unit }}</td><td><input v-model="drafts[o.id]" :disabled="!o.writable || busy" :aria-label="`${o.name} 待写值`" /><small>{{ o.min }} … {{ o.max }}</small></td><td><button :disabled="busy || !status.connected" @click="action(async () => { await read(o.id); })">读取</button><button v-if="o.writable" :disabled="busy || !status.connected || !values[o.id]" @click="write(o)">写入并回读</button></td></tr>
    </tbody></table>
    <label v-if="profile">写入确认 <input v-model="confirmWrite" placeholder="写入 对象ID" :disabled="busy" /></label>
    <ScopeChart v-if="profile && selection.length" :series="selection.map((id, i) => ({ id, name: profile!.objects.find(o => o.id === id)?.name ?? id, unit: profile!.objects.find(o => o.id === id)?.unit ?? '', color: ['#56d6df','#ffbf69','#66e59f','#ff6b78','#a58cff','#f58bc2'][i], values: history[id] ?? [] }))" />
    <p>监控是非实时 SDO 轮询；原始整数保留精度，工程值使用浮点显示。不提供 OP、伺服使能或周期运动输出。</p>
  </section>
</template>
<style scoped>
label{display:inline-flex;align-items:center;gap:6px}input[type=checkbox]{width:16px;height:16px;min-height:16px;margin:0}input[type=file]{max-width:260px;height:auto}
.ec-master{display:grid;gap:12px}.controls{display:flex;gap:10px;flex-wrap:wrap;align-items:center}p,small{font-size:12px;color:var(--muted)}.error{color:var(--red)}table{width:100%;border-collapse:collapse}td,th{padding:8px;text-align:left;border-bottom:1px solid var(--line);font-size:12px}select{max-width:440px}input[type=number]{width:100px}td input{width:140px}td input[type=checkbox]{width:auto}small{display:block}
</style>
