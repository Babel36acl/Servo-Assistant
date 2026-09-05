<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import EthercatMaster from './EthercatMaster.vue';
type RecordItem = { sequence: number; timestampUs: number; source: string; protocol: string; direction: string; transaction: number; bytes: number[]; detail: string };
type Page = { records: RecordItem[]; nextOffset: number | null; warning: string | null };
type RecordingStatus = { active: boolean; path: string; accepted: number; written: number; dropped: number; unwritten: number; bytes: number; error: string | null };
type CaptureStatus = { active: boolean; path: string; packets: number; bytes: number; driverDropped: number | null; error: string | null };
const tab = ref('recording'), error = ref(''), notice = ref(''), busy = ref(false), limit = ref(1024);
const status = ref<RecordingStatus | null>(null), capture = ref<CaptureStatus | null>(null);
const sessions = ref<string[]>([]), path = ref(''), fileMode = ref(false), offset = ref(0), next = ref<number | null>(null);
const rows = ref<RecordItem[]>([]), query = ref(''), direction = ref(''), selected = ref<RecordItem | null>(null), decoded = ref<unknown>(null);
const mapping = ref('{"mailboxes":[],"pdo":[]}');
const adapters = ref<Array<{ name: string; description: string }>>([]), adapter = ref(''), onlyEthercat = ref(true);
const visible = computed(() => rows.value.filter(r => (!direction.value || r.direction === direction.value) && (!query.value || `${r.source} ${r.protocol} ${r.detail} ${r.transaction}`.toLowerCase().includes(query.value.toLowerCase()))));
let timer: ReturnType<typeof setTimeout> | undefined; let disposed = false;
async function action(fn: () => Promise<void>) { if (busy.value) return; busy.value = true; error.value = ''; notice.value = ''; try { await fn(); } catch (e) { error.value = String(e); } finally { busy.value = false; } }
async function refresh() { [status.value, capture.value, sessions.value] = await Promise.all([invoke<RecordingStatus>('recording_status'), invoke<CaptureStatus>('network_capture_status'), invoke<string[]>('recording_sessions')]); }
async function exportPcap() { await action(async () => { const result = await invoke<{ path: string; warning: string | null }>('export_recording_pcap', { path: path.value }); notice.value = '已导出可用以太网帧：' + result.path; if (result.warning) error.value = result.warning; }); }
async function start() { await action(async () => { status.value = await invoke<RecordingStatus>('start_recording', { limitMb: limit.value }); path.value = status.value.path; fileMode.value = false; await refresh(); }); }
async function stop() { await action(async () => { status.value = await invoke<RecordingStatus>('stop_recording'); await refresh(); }); }
async function page(at: number) { const p = await invoke<Page>(fileMode.value ? 'capture_file_page' : 'recording_page', { path: path.value, offset: at }); rows.value = p.records; offset.value = at; next.value = p.nextOffset; selected.value = null; decoded.value = null; if (p.warning) error.value = p.warning; }
async function details(r: RecordItem) { selected.value = r; decoded.value = null; if (r.protocol === 'ethernet') { try { const result = await invoke('decode_ethercat', { bytes: r.bytes, config: JSON.parse(mapping.value) }); if (selected.value === r) decoded.value = result; } catch (e) { error.value = String(e); } } }
function hex(bytes: number[]) { return bytes.map(b => b.toString(16).padStart(2, '0').toUpperCase()).join(' '); }
function time(us: number) { return us ? new Date(us / 1000).toLocaleString() + `.${String(us % 1000000).padStart(6, '0')}` : '时间未知'; }
async function importMap(e: Event) { const f = (e.target as HTMLInputElement).files?.[0]; if (!f) return; await action(async () => { if (f.size > 1024 * 1024) throw new Error('映射文件超过 1 MiB'); const text = await f.text(); await invoke('decode_ethercat', { bytes: [], config: JSON.parse(text) }); mapping.value = text; }); }
function mailboxes(value: { mailboxes: Array<{ station: number; offset: number }> }) { try { const config = JSON.parse(mapping.value); config.mailboxes = value.mailboxes; mapping.value = JSON.stringify(config, null, 2); notice.value = '已使用当前主站发现的邮箱地址，PDO 映射保持不变。'; } catch (e) { error.value = String(e); } }
async function tick() { if (disposed) return; try { if (!busy.value) await refresh(); } catch (e) { error.value = String(e); } if (!disposed) timer = setTimeout(tick, 1000); }
onMounted(() => { void tick(); }); onUnmounted(() => { disposed = true; clearTimeout(timer); });
</script>

<template>
  <section class="panel workbench">
    <div class="controls"><h2>通信工作台</h2><button :class="{ active: tab === 'recording' }" @click="tab = 'recording'">录制与解析</button><button :class="{ active: tab === 'master' }" @click="tab = 'master'">EtherCAT 主站</button></div>
    <p v-if="error" class="error" role="alert">{{ error }}</p><p v-if="notice" class="notice">{{ notice }}</p>
    <EthercatMaster v-show="tab === 'master'" @mapping="mailboxes" />
    <div v-show="tab === 'recording'" class="recording-grid">
      <h3>统一通信录制</h3>
      <p>保存应用实际收发的串口字节、EtherCAT 主站帧和模拟器事件。暂停曲线或切换页面不停止录制；未知协议原样保留。</p>
      <div class="controls"><label>容量上限 <input v-model.number="limit" type="number" min="16" max="16384" /> MiB</label><button :disabled="busy || status?.active" @click="start">开始录制</button><button :disabled="busy || !status?.path" @click="stop">停止并落盘</button><span>{{ status?.active ? '正在录制' : '未录制' }} · 已接收 {{ status?.accepted ?? 0 }} · 已写 {{ status?.written ?? 0 }} · 队列丢弃 {{ status?.dropped ?? 0 }} · 未写入 {{ status?.unwritten ?? 0 }}</span></div>
      <p class="path">{{ status?.path }}</p><p v-if="status?.error" class="error">{{ status.error }}</p>
      <details><summary>在线网卡捕获 · 独立写入 PCAPNG</summary>
        <p>需要 Npcap。网卡仅能捕获所在位置可见的流量；使用主站网卡或实际 TAP/镜像口。捕获方向未知时不推断收发。</p>
        <div class="controls"><button :disabled="busy" @click="action(async () => { adapters = await invoke('ethercat_adapters'); adapter = adapters[0]?.name ?? ''; })">刷新网卡</button><select v-model="adapter" :disabled="capture?.active"><option value="">选择捕获网卡</option><option v-for="a in adapters" :key="a.name" :value="a.name">{{ a.description }}</option></select><label><input v-model="onlyEthercat" type="checkbox" :disabled="capture?.active" />仅 EtherCAT（含 VLAN）</label><button :disabled="busy || !adapter || capture?.active" @click="action(async () => { capture = await invoke('start_network_capture', { adapter, ethercatOnly: onlyEthercat, limitMb: limit }); })">开始网卡捕获</button><button :disabled="busy || !capture?.path" @click="action(async () => { capture = await invoke('stop_network_capture'); })">停止网卡捕获</button></div>
        <p>{{ capture?.active ? '捕获中' : '未捕获' }} · {{ capture?.packets ?? 0 }} 帧 · 驱动丢包 {{ capture?.driverDropped ?? '未知' }}</p><p class="path">{{ capture?.path }}</p><p v-if="capture?.error" class="error">{{ capture.error }}</p>
      </details>
      <h3>离线浏览与解析</h3>
      <div class="controls"><select aria-label="历史录制会话" @change="path = ($event.target as HTMLSelectElement).value; fileMode = false"><option value="">历史录制会话</option><option v-for="s in sessions" :key="s" :value="s">{{ s }}</option></select><label><input v-model="fileMode" type="checkbox" />打开 PCAP / PCAPNG 文件</label></div>
      <div class="controls"><input v-model="path" class="path-input" :placeholder="fileMode ? '输入 PCAP / PCAPNG 文件完整路径' : '录制会话目录完整路径'" aria-label="录制或捕获路径" /><button :disabled="busy || !path" @click="action(() => page(0))">打开 / 刷新</button><button :disabled="busy || !path || fileMode || status?.active" @click="exportPcap">导出以太网 PCAPNG</button></div>
      <details><summary>邮箱 / PDO 解析映射（不猜测设备字段）</summary><label>导入 JSON 映射 <input type="file" accept=".json" @change="importMap" /></label><textarea v-model="mapping" rows="7" aria-label="解析映射 JSON" /><p>邮箱地址来自主站发现或工程配置；PDO 使用实际逻辑地址、位偏移和位宽。SDO 分段显示原始段，不假定丢失报文已恢复。</p></details>
      <div class="controls"><input v-model="query" placeholder="本页筛选：来源、协议、事件、事务" aria-label="报文筛选" /><select v-model="direction" aria-label="方向筛选"><option value="">所有方向</option><option>tx</option><option>rx</option><option>event</option><option>unknown</option></select><button :disabled="busy || offset === 0" @click="action(() => page(Math.max(0, offset - 100)))">上一页</button><button :disabled="busy || next === null" @click="action(() => page(next!))">下一页</button><span>第 {{ Math.floor(offset / 100) + 1 }} 页 · 本页 {{ rows.length }} 条</span></div>
      <div class="table-scroll"><table><thead><tr><th>序号 / 时间</th><th>来源 / 协议</th><th>方向 / 事务</th><th>字节</th><th>事件</th></tr></thead><tbody><tr v-for="r in visible" :key="r.sequence" :class="{ chosen: selected === r }" @click="details(r)"><td>{{ r.sequence }}<br />{{ time(r.timestampUs) }}</td><td>{{ r.source }}<br />{{ r.protocol }}</td><td>{{ r.direction }} / {{ r.transaction || '—' }}</td><td>{{ r.bytes.length }}</td><td>{{ r.detail }}</td></tr></tbody></table><p v-if="!visible.length">暂无报文。开始录制后执行通信，或打开已有录制与捕获文件。</p></div>
      <div v-if="selected" class="detail-grid"><div><h3>原始字节</h3><pre>{{ hex(selected.bytes) || '无字节，只有事件' }}</pre></div><div><h3>标准字段解析</h3><pre>{{ decoded ? JSON.stringify(decoded, null, 2) : '该记录保留原始内容；当前无以太网字段解析。' }}</pre></div></div>
      <p>回放浏览不会向设备发送报文。丢包、清空缓冲和未完整落盘的数据均不能视为完整总线记录。</p>
    </div>
  </section>
</template>
<style scoped>
label{display:inline-flex;align-items:center;gap:6px}input[type=checkbox]{width:16px;height:16px;min-height:16px;margin:0}input[type=file]{max-width:260px;height:auto}
.workbench{padding:18px;margin-bottom:14px;display:grid;gap:14px}.controls{display:flex;flex-wrap:wrap;gap:10px;align-items:center}.recording-grid{display:grid;gap:12px}h3{font-size:14px;margin:0}p{font-size:12px;color:var(--muted)}.error{color:var(--red)}.notice{color:var(--green)}.active{border-color:var(--cyan);color:var(--cyan)}input[type=number]{width:100px}.path{overflow-wrap:anywhere}.path-input{flex:1;min-width:300px}select{max-width:650px}details{padding:12px;border:1px solid var(--line)}summary{cursor:pointer;margin-bottom:8px}textarea{width:100%;background:#081217;color:var(--cyan);border:1px solid var(--line);margin-top:8px}.table-scroll{max-height:440px;overflow:auto}table{width:100%;border-collapse:collapse}td,th{text-align:left;padding:8px;border-bottom:1px solid var(--line);font-size:12px;max-width:400px;overflow-wrap:anywhere}tbody tr{cursor:pointer}tbody tr:hover,.chosen{background:#20343e}.detail-grid{display:grid;grid-template-columns:1fr 1fr;gap:12px;min-width:0}.detail-grid>div{min-width:0}pre{white-space:pre-wrap;overflow-wrap:anywhere;max-height:500px;overflow:auto;background:#081217;padding:12px;font-size:12px}
</style>
