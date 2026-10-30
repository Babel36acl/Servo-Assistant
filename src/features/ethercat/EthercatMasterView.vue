<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { CHART_COLORS } from '../../styles/chart-palette';
import ScopeChart from '../../components/ScopeChart.vue';
import type { EthercatDiscovery, SlaveDiagnostics } from '../../types';
type Adapter = { name: string; description: string };
type Slave = { position: number; station: number; name: string; vendor: number; product: number; revision: number; state: number; alCode: number; mailboxOut: number; mailboxIn: number };
type Status = { connected: boolean; adapter: string; slaves: Slave[]; driverDropped: number | null };
type ObjectDef = { id: string; name: string; index: number; subIndex: number; size: number; signed: boolean; writable: boolean; min: string; max: string; scale: number; unit: string };
type Profile = { schemaVersion: string; name: string; vendor: number; product: number; objects: ObjectDef[] };
type Value = { id: string; raw: string; value: number; timestampUs: number };
const emit = defineEmits<{ connection: [connected: boolean]; mapping: [value: { mailboxes: Array<{ station: number; offset: number }> }]; activity: [label: string] }>();
const adapters = ref<Adapter[]>([]), adapter = ref(''), confirmation = ref('');
const status = ref<Status>({ connected: false, adapter: '', slaves: [], driverDropped: null });
function reportConnection(): void {
  const slaves = status.value.slaves.length;
  emit('connection', status.value.connected);
  emit('activity', status.value.connected ? `EtherCAT 主站已连接 · ${slaves} 从站` : '');
}
watch(() => [status.value.connected, status.value.slaves.length], reportConnection, { immediate: true });
const profile = ref<Profile | null>(null), slave = ref(1), error = ref(''), notice = ref('');
const busy = ref(false), polling = ref(false), interval = ref(1000), selection = ref<string[]>([]);
const values = ref<Record<string, Value>>({}), drafts = ref<Record<string, string>>({}), history = ref<Record<string, number[]>>({});
const diagnostics = ref<SlaveDiagnostics | null>(null);
const confirmWrite = ref(''); let timer: ReturnType<typeof setTimeout> | undefined; let disposed = false;
async function action(fn: () => Promise<void>) { if (busy.value) return; busy.value = true; error.value = ''; try { await fn(); } catch (e) { error.value = String(e); } finally { busy.value = false; } }
async function refreshAdapters() { await action(async () => { adapters.value = await invoke<Adapter[]>('ethercat_adapters'); if (!adapters.value.some(a => a.name === adapter.value)) adapter.value = adapters.value[0]?.name ?? ''; }); }
async function connect() { await action(async () => { status.value = await invoke<Status>('ethercat_connect', { adapter: adapter.value, confirmation: confirmation.value }); values.value = {}; history.value = {}; diagnostics.value = null; notice.value = '主站已连接：仅请求 PRE-OP，未发送 PDO 输出。'; }); }
async function disconnect() { polling.value = false; await action(async () => { await invoke('ethercat_disconnect'); status.value = { connected: false, adapter: '', slaves: [], driverDropped: null }; values.value = {}; history.value = {}; diagnostics.value = null; }); }
async function importProfile(event: Event) { const file = (event.target as HTMLInputElement).files?.[0]; if (!file) return; await action(async () => { if (file.size > 1024 * 1024) throw new Error('Profile 超过 1 MiB'); const data: unknown = JSON.parse(await file.text()); profile.value = await invoke<Profile>('ethercat_profile', { profile: data }); values.value = {}; drafts.value = {}; selection.value = profile.value.objects.slice(0, 4).map(o => o.id); history.value = {}; }); }
async function discoverProfile() {
  await action(async () => {
    const result = await invoke<EthercatDiscovery>('ethercat_discover_profile', { slave: slave.value });
    profile.value = await invoke<Profile>('ethercat_profile', { profile: result.profile });
    values.value = {};
    drafts.value = {};
    history.value = {};
    diagnostics.value = null;
    selection.value = profile.value.objects.slice(0, 4).map(o => o.id);
    notice.value = `已从从站 ${slave.value} 生成 Profile：纳入 ${result.included}/${result.total} 个对象`
      + (result.skipped ? `，${result.skipped} 个超出 512 上限未纳入` : '')
      + (result.failed ? `，${result.failed} 个对象读取失败` : '');
  });
}
async function read(id: string) { const v = await invoke<Value>('ethercat_read', { slave: slave.value, id }); values.value[id] = v; if (drafts.value[id] === undefined) drafts.value[id] = v.raw; return v; }
async function write(o: ObjectDef) { await action(async () => {
  const before = values.value[o.id]; if (!before) throw new Error('先读取当前值');
  try { const v = await invoke<Value>('ethercat_write', { slave: slave.value, id: o.id, value: drafts.value[o.id], expected: before.raw, confirmation: confirmWrite.value }); values.value[o.id] = v; notice.value = `${o.name} 已写入并回读校验`; }
  catch (e) { delete values.value[o.id]; throw e; } finally { confirmWrite.value = ''; }
}); }
function selectSlave() { values.value = {}; drafts.value = {}; history.value = {}; diagnostics.value = null; }
function stateName(state: number) { const base = ({ 1: 'INIT', 2: 'PRE-OP', 3: 'BOOT', 4: 'SAFE-OP', 8: 'OP' } as Record<number, string>)[state & 15] ?? '未知'; return `${base}${state & 16 ? ' + ERROR' : ''}`; }
async function diagnose() { await action(async () => { diagnostics.value = await invoke<SlaveDiagnostics>('ethercat_diagnostics', { slave: slave.value }); }); }
function portNames(mask: number) { const ports = [0, 1, 2, 3].filter(index => (mask & (1 << index)) !== 0).map(index => 'ABCD'[index]); return ports.length ? ports.join(' / ') : '无'; }
function portLetter(value: number) { return value >= 0 && value < 4 ? 'ABCD'[value] : `P${value}`; }
function errorBytes(values: number[] | undefined, start: number, end: number) { return (values ?? []).slice(start, end).map(value => value.toString(16).toUpperCase().padStart(2, '0')).join(' ') || '—'; }
function useMailboxes() { emit('mapping', { mailboxes: status.value.slaves.flatMap(s => [s.mailboxOut, s.mailboxIn].filter(Boolean).map(offset => ({ station: s.station, offset }))) }); }
const chartSeries = computed(() => selection.value.map((id, i) => ({ id, name: profile.value!.objects.find(o => o.id === id)?.name ?? id, unit: profile.value!.objects.find(o => o.id === id)?.unit ?? '', color: CHART_COLORS[i % CHART_COLORS.length], values: history.value[id] ?? [] })));
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
  <div class="ksu-grid">
    <section class="ksu-card ksu-span-2">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">EtherCAT 主站</h2>
          <p class="ksu-card-sub">连接会接管所选网卡上的总线并初始化从站；当前仅支持 PRE-OP 下的 CoE/SDO 配置与轮询</p>
        </div>
        <div class="ksu-card-actions">
          <span class="ksu-pill" :class="status.connected ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ status.connected ? '已连接' : '未连接' }}</span>
        </div>
      </div>
      <div class="ksu-card-body">
        <div class="ksu-form-grid">
          <label class="ksu-field">
            <span class="ksu-field-label">专用网卡</span>
            <div class="inline-field">
              <select class="ksu-select" aria-label="EtherCAT 专用网卡" v-model="adapter" :disabled="status.connected || busy">
                <option value="">选择专用网卡</option>
                <option v-for="a in adapters" :key="a.name" :value="a.name">{{ a.description }}</option>
              </select>
              <button class="ksu-btn ksu-btn--sm" :disabled="busy" @click="refreshAdapters">刷新网卡</button>
            </div>
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">接管确认</span>
            <input class="ksu-input" v-model="confirmation" :disabled="status.connected || busy" placeholder="输入：接管 EtherCAT 总线" aria-label="主站接管确认" />
          </label>
        </div>

        <div v-if="error" class="ksu-banner ksu-banner--error" role="alert">
          <div class="ksu-banner-body"><strong>主站操作失败</strong><span>{{ error }}</span></div>
        </div>
        <div v-if="notice" class="ksu-banner ksu-banner--info" role="status">
          <div class="ksu-banner-body"><span>{{ notice }}</span></div>
        </div>

        <button v-if="!status.connected" class="ksu-btn ksu-btn--primary ksu-btn--block" :disabled="busy || !adapter" @click="connect">连接并发现从站</button>
        <button v-else class="ksu-btn ksu-btn--danger ksu-btn--block" :disabled="busy" @click="disconnect">断开主站</button>

        <p class="ksu-hint">主站：{{ status.connected ? '已连接' : '未连接' }} · 驱动丢包：{{ status.driverDropped ?? '未知' }} · 不提供 OP、伺服使能或周期运动输出。</p>
      </div>
    </section>

    <section class="ksu-card">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">EtherCAT Profile</h2>
          <p class="ksu-card-sub">导入对象字典后启用对象读写与监控；Profile 超过 1 MiB 会被拒绝</p>
        </div>
        <div class="ksu-card-actions">
          <span class="ksu-pill" :class="profile ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ profile?.name ?? '未导入' }}</span>
        </div>
      </div>
      <div class="ksu-card-body">
        <div class="button-row">
          <label class="ksu-btn ksu-btn--tonal" :class="{ disabled: busy }">
            导入 Profile JSON
            <input type="file" hidden accept=".json" :disabled="busy" @change="importProfile" />
          </label>
          <button class="ksu-btn ksu-btn--primary" :disabled="!status.connected || busy" @click="discoverProfile">从设备生成</button>
        </div>
        <p class="ksu-hint">{{ profile ? `${profile.objects.length} 个对象 · 厂商 ${profile.vendor} · 产品 ${profile.product}` : '尚未导入配置。连接主站后可从在线对象字典生成，或导入 JSON。' }}</p>
        <p class="ksu-hint">当前目标从站 {{ slave }}（在下方对象监控中选择）；生成会替换现有 Profile，只读取对象字典，不写设备。</p>
      </div>
    </section>

    <section v-if="status.slaves.length" class="ksu-card ksu-span-full">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">从站列表</h2>
          <p class="ksu-card-sub">{{ status.slaves.length }} 个从站 · 位置 / 站号 / 厂商信息 / AL 状态</p>
        </div>
        <div class="ksu-card-actions">
          <button class="ksu-btn ksu-btn--sm" :disabled="busy || !status.connected" @click="action(async () => { status = await invoke<Status>('ethercat_status', { refresh: true }); })">刷新从站</button>
          <button class="ksu-btn ksu-btn--sm" @click="useMailboxes">邮箱地址用于解析</button>
        </div>
      </div>
      <div class="ksu-list">
        <div v-for="s in status.slaves" :key="s.position" class="ksu-row ksu-row--static">
          <div class="ksu-row-icon" aria-hidden="true">{{ s.position }}</div>
          <div class="ksu-row-body">
            <span class="ksu-row-title">{{ s.name }}</span>
            <span class="ksu-row-sub">站号 0x{{ s.station.toString(16) }} · Vendor {{ s.vendor }} / Product {{ s.product }} / Revision {{ s.revision }}</span>
            <span class="ksu-row-sub">AL 状态码 0x{{ s.alCode.toString(16) }}</span>
          </div>
          <span class="ksu-pill" :class="s.state & 16 ? 'ksu-pill--bad' : 'ksu-pill--ok'">{{ stateName(s.state) }}</span>
        </div>
      </div>
    </section>

    <section v-if="status.connected" class="ksu-card ksu-span-full">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">从站诊断</h2>
          <p class="ksu-card-sub">只读 PRE-OP 诊断：AL 状态、拓扑端口、DC 与 ESC 错误计数原始寄存器；不进入 OP</p>
        </div>
        <div class="ksu-card-actions">
          <span class="ksu-pill ksu-pill--muted">从站 {{ slave }}</span>
          <button class="ksu-btn ksu-btn--sm" :disabled="busy" @click="diagnose">读取诊断</button>
        </div>
      </div>
      <div class="ksu-card-body">
        <div v-if="!diagnostics" class="ksu-empty">
          <strong>尚未读取</strong>
          <p>选择从站后读取诊断；错误计数寄存器可能因设备或固件不支持而不可读。</p>
        </div>
        <template v-else>
          <div class="ksu-grid">
            <div class="ksu-stat">
              <span class="ksu-stat-label">AL 状态</span>
              <span class="ksu-stat-value ksu-display">{{ stateName(diagnostics.state) }}</span>
              <span class="ksu-stat-sub">状态码 0x{{ diagnostics.alCode.toString(16).toUpperCase().padStart(4, '0') }}</span>
            </div>
            <div class="ksu-stat">
              <span class="ksu-stat-label">活动端口</span>
              <span class="ksu-stat-value ksu-display">{{ portNames(diagnostics.activePorts) }}</span>
              <span class="ksu-stat-sub">位图 0x{{ diagnostics.activePorts.toString(16).toUpperCase().padStart(2, '0') }}</span>
            </div>
            <div class="ksu-stat">
              <span class="ksu-stat-label">拓扑</span>
              <span class="ksu-stat-value ksu-display">{{ portLetter(diagnostics.parentPort) }} → {{ portLetter(diagnostics.entryPort) }}</span>
              <span class="ksu-stat-sub">上级端口 → 本从站入口端口</span>
            </div>
            <div class="ksu-stat">
              <span class="ksu-stat-label">分布式时钟</span>
              <span class="ksu-stat-value ksu-display">{{ diagnostics.dcActive ? '启用' : '未启用' }}</span>
              <span class="ksu-stat-sub">DCnext {{ diagnostics.dcNext }}</span>
            </div>
          </div>
          <div v-if="diagnostics.errorCountersValid" class="ksu-list">
            <div class="ksu-row ksu-row--static">
              <div class="ksu-row-body">
                <span class="ksu-row-title mono">0x0300–0x0307</span>
                <span class="ksu-row-sub">RX 错误计数（8 字节原始值）</span>
              </div>
              <span class="ksu-pill ksu-pill--muted mono">{{ errorBytes(diagnostics.errorCounters, 0, 8) }}</span>
            </div>
            <div class="ksu-row ksu-row--static">
              <div class="ksu-row-body">
                <span class="ksu-row-title mono">0x0308–0x030B</span>
                <span class="ksu-row-sub">转发 RX 错误计数</span>
              </div>
              <span class="ksu-pill ksu-pill--muted mono">{{ errorBytes(diagnostics.errorCounters, 8, 12) }}</span>
            </div>
            <div class="ksu-row ksu-row--static">
              <div class="ksu-row-body">
                <span class="ksu-row-title mono">0x030C–0x030F</span>
                <span class="ksu-row-sub">ECAT 处理单元 / PDI 错误 / 错误码</span>
              </div>
              <span class="ksu-pill ksu-pill--muted mono">{{ errorBytes(diagnostics.errorCounters, 12, 16) }}</span>
            </div>
            <div class="ksu-row ksu-row--static">
              <div class="ksu-row-body">
                <span class="ksu-row-title mono">0x0310–0x0311</span>
                <span class="ksu-row-sub">丢链计数</span>
              </div>
              <span class="ksu-pill ksu-pill--muted mono">{{ errorBytes(diagnostics.errorCounters, 16, 18) }}</span>
            </div>
          </div>
          <p v-else class="ksu-hint">ESC 错误计数寄存器读取失败（0x0300..0x0311）；AL、拓扑与 DC 信息仍有效。</p>
        </template>
      </div>
    </section>

    <section v-if="profile" class="ksu-card ksu-span-full">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">对象监控</h2>
          <p class="ksu-card-sub">非实时 SDO 轮询，最多 6 个对象；原始整数保留精度，工程值用浮点显示</p>
        </div>
        <div class="ksu-card-actions">
          <span class="ksu-pill" :class="polling ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ polling ? '监控中' : '已暂停' }}</span>
        </div>
      </div>
      <div class="ksu-card-body">
        <div class="ksu-form-grid">
          <label class="ksu-field">
            <span class="ksu-field-label">目标从站</span>
            <select class="ksu-select" v-model.number="slave" :disabled="busy" @change="selectSlave">
              <option v-for="s in status.slaves" :key="s.position" :value="s.position">{{ s.position }} · {{ s.name }}</option>
            </select>
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">读取完成后等待 ms</span>
            <input class="ksu-input" v-model.number="interval" type="number" min="100" max="60000" />
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">写入确认</span>
            <input class="ksu-input" v-model="confirmWrite" placeholder="写入 对象ID" :disabled="busy" />
          </label>
        </div>
        <div class="ksu-list">
          <div class="ksu-row ksu-row--static">
            <div class="ksu-row-body">
              <span class="ksu-row-title">监控所选对象</span>
              <span class="ksu-row-sub">最多 6 个，轮询期间会累积最近 300 点曲线</span>
            </div>
            <div class="ksu-row-control">
              <button type="button" class="ksu-switch" role="switch" :aria-checked="polling" :disabled="!status.connected" aria-label="监控所选对象" @click="polling = !polling">
                <span class="ksu-switch-knob"></span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </section>

    <section v-if="profile" class="ksu-card ksu-span-full">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">对象表</h2>
          <p class="ksu-card-sub">{{ profile.objects.length }} 个对象 · 写入后立即回读校验；勾选进入监控</p>
        </div>
        <div class="ksu-card-actions">
          <span class="ksu-pill ksu-pill--muted">{{ selection.length }}/6 监控</span>
        </div>
      </div>
      <div class="ksu-list">
        <div v-for="o in profile.objects" :key="o.id" class="ksu-row ksu-row--static">
          <div class="ksu-row-body">
            <span class="ksu-row-title">
              {{ o.name }}
              <span v-if="o.writable" class="ksu-pill ksu-pill--warn">可写</span>
            </span>
            <span class="ksu-row-sub mono">0x{{ o.index.toString(16) }}:{{ o.subIndex }} · {{ o.id }} · {{ o.min }} … {{ o.max }} {{ o.unit }}</span>
            <span class="ksu-row-sub">原始 <b class="mono">{{ values[o.id]?.raw ?? '未读取 / 已过期' }}</b> · 工程 {{ values[o.id]?.value ?? '—' }} {{ o.unit }}</span>
            <div class="ksu-row-actions">
              <input class="ksu-input" v-model="drafts[o.id]" :disabled="!o.writable || busy" :aria-label="`${o.name} 待写值`" />
              <button class="ksu-btn ksu-btn--sm" :disabled="busy || !status.connected" @click="action(async () => { await read(o.id); })">读取</button>
              <button v-if="o.writable" class="ksu-btn ksu-btn--sm ksu-btn--tonal" :disabled="busy || !status.connected || !values[o.id]" @click="write(o)">写入并回读</button>
            </div>
          </div>
          <div class="ksu-row-control">
            <input v-model="selection" type="checkbox" :value="o.id" :disabled="!selection.includes(o.id) && selection.length >= 6" :aria-label="`监控 ${o.name}`" />
          </div>
        </div>
      </div>
    </section>

    <section v-if="profile && selection.length" class="ksu-card ksu-span-full">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">对象曲线</h2>
          <p class="ksu-card-sub">所选 {{ selection.length }} 个对象 · 各通道独立缩放 · 最近 300 点</p>
        </div>
      </div>
      <div class="ksu-chart">
        <ScopeChart :series="chartSeries" />
      </div>
    </section>
  </div>
</template>
