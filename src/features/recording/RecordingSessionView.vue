<script setup lang="ts">

import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import RecordingLibrary from './RecordingLibrary.vue';
import RecordingHistory from './RecordingHistory.vue';
const emit = defineEmits<{ activity: [label: string] }>();
type RecordItem = { sequence: number; timestampUs: number; source: string; protocol: string; direction: string; transaction: number; bytes: number[]; detail: string };
type Page = { records: RecordItem[]; nextOffset: number | null; warning: string | null; total?: number; offset?: number };
type ModbusFrame = { hex: string; checksum: string; slave: number | null; function: number | null; address: number | null; count: number | null; values: number[]; exception: string | null; error: string | null };
type ModbusTransaction = { source: string; protocol: string; transaction: number; outcome: string; durationUs: number | null; request: ModbusFrame; response: ModbusFrame; events: string[]; warning: string | null };
type RecordingStatus = { active: boolean; path: string; accepted: number; written: number; dropped: number; unwritten: number; bytes: number; error: string | null; armed?: boolean; discarded?: number; triggeredAtUs?: number | null };
type CaptureStatus = { active: boolean; path: string; packets: number; bytes: number; driverDropped: number | null; error: string | null };
const error = ref(''), notice = ref(''), busy = ref(false), limit = ref(1024);
const status = ref<RecordingStatus | null>(null), capture = ref<CaptureStatus | null>(null);
watch(() => [status.value?.active, status.value?.error, capture.value?.active, capture.value?.error], () => emit('activity', [status.value?.error ? '录制异常：' + status.value.error : status.value?.active ? '通信录制中' : '', capture.value?.error ? '捕获异常：' + capture.value.error : capture.value?.active ? '网卡捕获中' : ''].filter(Boolean).join(' · ')), { immediate: true });
const sessions = ref<string[]>([]), path = ref(''), fileMode = ref(false), offset = ref(0), next = ref<number | null>(null);
const rows = ref<RecordItem[]>([]), query = ref(''), direction = ref(''), selected = ref<RecordItem | null>(null), decoded = ref<unknown>(null);
const recordTable = ref<HTMLElement | null>(null);
const follow = ref(false), pageBusy = ref(false), browseError = ref(''), pageWarning = ref(''), total = ref<number | null>(null);
const modbusDetail = ref<ModbusTransaction | null>(null), detailBusy = ref(false);
const triggerEnabled = ref(false), preSeconds = ref(30), postSeconds = ref(10), triggerKeyword = ref('');
const fromTime = ref(''), toTime = ref(''), slaveFilter = ref(''), functionFilter = ref(''), addressFilter = ref(''), outcomeFilter = ref('');
const sourceFilter = ref(''), protocolFilter = ref('');
const fromUs = computed(() => fromTime.value ? new Date(fromTime.value).getTime()*1000 : null);
const toUs = computed(() => toTime.value ? new Date(toTime.value).getTime()*1000 : null);
function numericFilter(value: string, max: number) { if(!value.trim()) return null; const n = Number(value); if(!Number.isInteger(n) || n<0 || n>max) throw new Error(`筛选数值须为 0～${max}，可使用 0x 十六进制`); return n; }
function filters() { if((fromUs.value !== null && !Number.isFinite(fromUs.value)) || (toUs.value !== null && !Number.isFinite(toUs.value))) throw new Error('时间无效'); return { query:query.value,direction:direction.value,source:sourceFilter.value,protocol:protocolFilter.value,fromUs:fromUs.value,toUs:toUs.value,slave:numericFilter(slaveFilter.value,247),function:numericFilter(functionFilter.value,127),address:numericFilter(addressFilter.value,65535),outcome:outcomeFilter.value }; }
function clearFilters() { query.value=''; direction.value=''; sourceFilter.value=''; protocolFilter.value=''; fromTime.value=''; toTime.value=''; slaveFilter.value=''; functionFilter.value=''; addressFilter.value=''; outcomeFilter.value=''; }
let pageVersion = 0, detailVersion = 0;
let filterTimer: ReturnType<typeof setTimeout> | undefined;
const mapping = ref('{"mailboxes":[],"pdo":[]}');
const adapters = ref<Array<{ name: string; description: string }>>([]), adapter = ref(''), onlyEthercat = ref(true);
const visible = computed(() => rows.value);
let timer: ReturnType<typeof setTimeout> | undefined; let disposed = false;
async function action(fn: () => Promise<void>) { if (busy.value) return; busy.value = true; error.value = ''; notice.value = ''; try { await fn(); } catch (e) { error.value = String(e); } finally { busy.value = false; } }
async function refresh() { [status.value, capture.value, sessions.value] = await Promise.all([invoke<RecordingStatus>('recording_status'), invoke<CaptureStatus>('network_capture_status'), invoke<string[]>('recording_sessions')]); }
async function exportPcap() { await action(async () => { const result = await invoke<{ path: string; warning: string | null }>('export_recording_pcap', { path: path.value }); notice.value = '已导出可用以太网帧：' + result.path; if (result.warning) error.value = result.warning; }); }
async function start() { await action(async () => { await invoke('recording_mapping', { config:JSON.parse(mapping.value) }); status.value = await invoke<RecordingStatus>('start_recording', { limitMb: limit.value, trigger:triggerEnabled.value ? { preSeconds:preSeconds.value,postSeconds:postSeconds.value,keyword:triggerKeyword.value } : null }); path.value = status.value.path; fileMode.value = false; await refresh(); follow.value = true; await page(0, true); }); }
async function stop() { await action(async () => { status.value = await invoke<RecordingStatus>('stop_recording'); await refresh(); if (follow.value) await page(0, true); }); }
function clearDetail() { detailVersion++; selected.value = null; decoded.value = null; modbusDetail.value = null; detailBusy.value = false; }
function invalidatePage() { pageVersion++; pageBusy.value = false; rows.value = []; offset.value = 0; next.value = null; total.value = null; pageWarning.value = ''; browseError.value = ''; clearDetail(); }
watch([path, fileMode], () => { follow.value = false; invalidatePage(); });
watch([query, direction,sourceFilter,protocolFilter,fromTime,toTime,slaveFilter,functionFilter,addressFilter,outcomeFilter], () => {
  invalidatePage(); clearTimeout(filterTimer);
  if (path.value) filterTimer = setTimeout(() => { void page(0, follow.value); }, 250);
});
async function page(at: number, tail = false) {
  if (!path.value || disposed) return;
  const version = ++pageVersion;
  const requestPath = path.value, requestMode = fileMode.value;
  pageBusy.value = true; browseError.value = '';
  if (!tail) follow.value = false;
  try {
    const p = await invoke<Page>('recording_page', {
      path: requestPath, offset: at, filter:filters(), tail:!requestMode && tail,
    });
    if (version !== pageVersion || disposed) return;
    rows.value = p.records; offset.value = p.offset ?? at; next.value = p.nextOffset;
    total.value = p.total ?? null; pageWarning.value = p.warning ?? ''; clearDetail();
    if (tail) { await nextTick(); if (version === pageVersion && follow.value && recordTable.value) recordTable.value.scrollTop = recordTable.value.scrollHeight; }
  } catch (e) { if (version === pageVersion && !disposed) { browseError.value = String(e); follow.value = false; } }
  finally { if (version === pageVersion) pageBusy.value = false; }
}
function pauseFollow() { if (!follow.value) return; follow.value = false; pageVersion++; pageBusy.value = false; }
async function toggleFollow() {
  if (follow.value) { pauseFollow(); return; }
  follow.value = true; await page(0, true);
}
async function details(r: RecordItem) {
  follow.value = false; pageVersion++; pageBusy.value = false; clearDetail();
  selected.value = r;
  const version = ++detailVersion; detailBusy.value = true;
  try {
    if (r.protocol === 'ethernet') {
      const context = !fileMode.value ? await invoke<{ mapping?: unknown }>('recording_context',{ path:path.value,sequence:r.sequence }) : {};
      const result = await invoke('decode_ethercat', { bytes: r.bytes, config:context.mapping ?? JSON.parse(mapping.value) });
      if (version === detailVersion && !disposed) decoded.value = result;
    } else if (!fileMode.value && r.transaction && ['modbus-rtu', 'modbus-ascii'].includes(r.protocol)) {
      const result = await invoke<ModbusTransaction>('recording_modbus_transaction', { path: path.value, source: r.source, protocol: r.protocol, transaction: r.transaction });
      if (version === detailVersion && !disposed) modbusDetail.value = result;
    } else if (r.protocol === 'status-sample' || r.protocol === 'recording-context') {
      decoded.value = JSON.parse(r.detail);
    }
  } catch (e) { if (version === detailVersion && !disposed) browseError.value = String(e); }
  finally { if (version === detailVersion) detailBusy.value = false; }
}
function hex(bytes: number[]) { return bytes.map(b => b.toString(16).padStart(2, '0').toUpperCase()).join(' '); }
function time(us: number) { return us ? new Date(us / 1000).toLocaleString() + `.${String(us % 1000000).padStart(6, '0')}` : '时间未知'; }
async function importMap(e: Event) { const f = (e.target as HTMLInputElement).files?.[0]; if (!f) return; await action(async () => { if (f.size > 1024 * 1024) throw new Error('映射文件超过 1 MiB'); const text = await f.text(); await invoke('decode_ethercat', { bytes: [], config: JSON.parse(text) }); mapping.value = text; }); }
async function saveMapping() { await action(async () => { await invoke('recording_mapping',{ config:JSON.parse(mapping.value) }); notice.value='已保存当前映射；后续录制记录使用此次配置。'; }); }
async function openSession(value: string, mode: boolean) { path.value=value; fileMode.value=mode; clearFilters(); await nextTick(); await page(0); }
async function locatePoint(point: {sequence:number;source:string;protocol:string;transaction:number}) {
  pauseFollow(); clearDetail(); const version = ++pageVersion; pageBusy.value=true; browseError.value='';
  try { const p = await invoke<Page>('recording_page',{ path:path.value,offset:0,filter:{ sequence:point.sequence },tail:false });
    if(version !== pageVersion || disposed) return;
    rows.value=p.records; offset.value=0; next.value=null; total.value=p.total ?? null; pageWarning.value=p.warning ?? '';
    if(p.records[0]) await details(p.records[0]);
  } catch(e) { if(version===pageVersion) browseError.value=String(e); } finally { if(version===pageVersion) pageBusy.value=false; }
}
function applyMailboxes(value: { mailboxes: Array<{ station: number; offset: number }> }) { try { const config = JSON.parse(mapping.value); config.mailboxes = value.mailboxes; mapping.value = JSON.stringify(config, null, 2); notice.value = '已使用当前主站发现的邮箱地址，PDO 映射保持不变。'; } catch (e) { error.value = String(e); } }
defineExpose({ applyMailboxes });
async function tick() { if (disposed) return; try { if (!busy.value) { await refresh(); if (follow.value && !pageBusy.value && !fileMode.value && path.value) await page(0, true); } } catch (e) { error.value = String(e); } if (!disposed) timer = setTimeout(tick, 1000); }
onMounted(() => { void tick(); }); onUnmounted(() => { disposed = true; pageVersion++; detailVersion++; clearTimeout(timer); clearTimeout(filterTimer); });
</script>
<template>
  <div class="ksu-grid">
    <!-- 单根节点：CommunicationView 用 v-show 控制录制 / EtherCAT 两页签 -->
    <section class="ksu-card ksu-span-2">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">统一通信录制</h2>
          <p class="ksu-card-sub">保存应用实际收发的串口字节、EtherCAT 主站帧和模拟器事件；暂停曲线或切换页面不停止录制，未知协议原样保留</p>
        </div>
        <div class="ksu-card-actions">
          <span class="ksu-pill" :class="status?.active ? 'ksu-pill--bad' : 'ksu-pill--muted'">{{ status?.active ? '正在录制' : '未录制' }}</span>
          <button v-if="!status?.active" class="ksu-btn ksu-btn--primary ksu-btn--sm" :disabled="busy" @click="start">开始录制</button>
          <button v-else class="ksu-btn ksu-btn--danger ksu-btn--sm" :disabled="busy || !status?.path" @click="stop">停止并落盘</button>
        </div>
      </div>
      <div class="ksu-card-body">
        <div v-if="error" class="ksu-banner ksu-banner--error" role="alert">
          <div class="ksu-banner-body"><strong>录制操作失败</strong><span>{{ error }}</span></div>
        </div>
        <div v-if="notice" class="ksu-banner ksu-banner--info" role="status">
          <div class="ksu-banner-body"><span>{{ notice }}</span></div>
        </div>

        <div class="ksu-form-grid">
          <label class="ksu-field">
            <span class="ksu-field-label">容量上限（MiB）</span>
            <input class="ksu-input" v-model.number="limit" type="number" min="16" max="16384" />
          </label>
        </div>
        <div class="ksu-grid">
          <div class="ksu-stat">
            <span class="ksu-stat-label">已接收</span>
            <span class="ksu-stat-value ksu-display">{{ status?.accepted ?? 0 }}</span>
            <span class="ksu-stat-sub">条消息</span>
          </div>
          <div class="ksu-stat">
            <span class="ksu-stat-label">已写</span>
            <span class="ksu-stat-value ksu-display">{{ status?.written ?? 0 }}</span>
            <span class="ksu-stat-sub">已落盘</span>
          </div>
          <div class="ksu-stat">
            <span class="ksu-stat-label">队列丢弃</span>
            <span class="ksu-stat-value ksu-display">{{ status?.dropped ?? 0 }}</span>
            <span class="ksu-stat-sub">缓冲溢出</span>
          </div>
          <div class="ksu-stat">
            <span class="ksu-stat-label">未写入</span>
            <span class="ksu-stat-value ksu-display">{{ status?.unwritten ?? 0 }}</span>
            <span class="ksu-stat-sub">待落盘</span>
          </div>
        </div>
        <p v-if="status?.path" class="ksu-hint">{{ status.path }}</p>
        <div v-if="status?.error" class="ksu-banner ksu-banner--error" role="alert">
          <div class="ksu-banner-body"><span>{{ status.error }}</span></div>
        </div>
        <div v-if="status?.armed" class="ksu-banner ksu-banner--info" role="status">
          <div class="ksu-banner-body"><span>正在等待故障触发 · 故障前缓冲主动淘汰 {{ status.discarded ?? 0 }} 条</span></div>
        </div>
        <div v-if="status?.triggeredAtUs" class="ksu-banner ksu-banner--info" role="status">
          <div class="ksu-banner-body"><span>已于 {{ time(status.triggeredAtUs) }} 触发{{ status.active ? '，正在保存故障后数据' : '，已停止' }} · 主动淘汰 {{ status.discarded ?? 0 }} 条</span></div>
        </div>

      </div>
    </section>

    <section class="ksu-card">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">故障触发录制</h2>
          <p class="ksu-card-sub">默认由超时、校验错误、设备异常等失败事务触发一次，故障后自动停止</p>
        </div>
        <div class="ksu-card-actions">
          <span class="ksu-pill" :class="triggerEnabled ? 'ksu-pill--warn' : 'ksu-pill--muted'">{{ triggerEnabled ? '已启用' : '未启用' }}</span>
        </div>
      </div>
      <div class="ksu-card-body">
        <div class="ksu-list">
          <div class="ksu-row ksu-row--static">
            <div class="ksu-row-body">
              <span class="ksu-row-title">等待故障后保存</span>
              <span class="ksu-row-sub">未触发便停止时，缓冲报文不保存</span>
            </div>
            <div class="ksu-row-control">
              <button type="button" class="ksu-switch" role="switch" :aria-checked="triggerEnabled" :disabled="!!status?.active" aria-label="故障触发录制" @click="triggerEnabled = !triggerEnabled">
                <span class="ksu-switch-knob"></span>
              </button>
            </div>
          </div>
        </div>
        <div class="ksu-form-grid">
          <label class="ksu-field">
            <span class="ksu-field-label">故障前（秒）</span>
            <input class="ksu-input" v-model.number="preSeconds" type="number" min="0" max="300" :disabled="!!status?.active" />
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">故障后（秒）</span>
            <input class="ksu-input" v-model.number="postSeconds" type="number" min="1" max="300" :disabled="!!status?.active" />
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">事件关键词</span>
            <input class="ksu-input" v-model="triggerKeyword" maxlength="256" :disabled="!!status?.active" placeholder="可选事件关键词；留空使用事务失败触发" aria-label="故障触发关键词" />
          </label>
        </div>
        <p class="ksu-hint">故障前缓冲最多 16 MiB / 300 秒，超过限制的旧记录会被主动淘汰；配置变更始终保存。</p>
      </div>
    </section>

    <section class="ksu-card">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">在线网卡捕获</h2>
          <p class="ksu-card-sub">独立写入 PCAPNG，需要 Npcap；网卡仅能捕获所在位置可见的流量，使用主站网卡或实际 TAP / 镜像口</p>
        </div>
        <div class="ksu-card-actions">
          <span class="ksu-pill" :class="capture?.active ? 'ksu-pill--bad' : 'ksu-pill--muted'">{{ capture?.active ? '捕获中' : '未捕获' }}</span>
          <button v-if="!capture?.active" class="ksu-btn ksu-btn--primary ksu-btn--sm" :disabled="busy || !adapter" @click="action(async () => { capture = await invoke('start_network_capture', { adapter, ethercatOnly: onlyEthercat, limitMb: limit }); })">开始捕获</button>
          <button v-else class="ksu-btn ksu-btn--danger ksu-btn--sm" :disabled="busy || !capture?.path" @click="action(async () => { capture = await invoke('stop_network_capture'); })">停止捕获</button>
        </div>
      </div>
      <div class="ksu-card-body">
        <div class="ksu-form-grid">
          <label class="ksu-field">
            <span class="ksu-field-label">捕获网卡</span>
            <div class="inline-field">
              <select class="ksu-select" v-model="adapter" :disabled="capture?.active">
                <option value="">选择捕获网卡</option>
                <option v-for="a in adapters" :key="a.name" :value="a.name">{{ a.description }}</option>
              </select>
              <button class="ksu-btn ksu-btn--sm" :disabled="busy" @click="action(async () => { adapters = await invoke('ethercat_adapters'); adapter = adapters[0]?.name ?? ''; })">刷新网卡</button>
            </div>
          </label>
        </div>
        <div class="ksu-list">
          <div class="ksu-row ksu-row--static">
            <div class="ksu-row-body">
              <span class="ksu-row-title">仅 EtherCAT（含 VLAN）</span>
              <span class="ksu-row-sub">捕获方向未知时不推断收发</span>
            </div>
            <div class="ksu-row-control">
              <button type="button" class="ksu-switch" role="switch" :aria-checked="onlyEthercat" :disabled="capture?.active" aria-label="仅捕获 EtherCAT" @click="onlyEthercat = !onlyEthercat">
                <span class="ksu-switch-knob"></span>
              </button>
            </div>
          </div>
        </div>
        <div class="ksu-grid">
          <div class="ksu-stat">
            <span class="ksu-stat-label">已捕获帧</span>
            <span class="ksu-stat-value ksu-display">{{ capture?.packets ?? 0 }}</span>
            <span class="ksu-stat-sub">PCAPNG 写入</span>
          </div>
          <div class="ksu-stat">
            <span class="ksu-stat-label">驱动丢包</span>
            <span class="ksu-stat-value ksu-display">{{ capture?.driverDropped ?? '—' }}</span>
            <span class="ksu-stat-sub">网卡层丢弃</span>
          </div>
        </div>
        <p v-if="capture?.path" class="ksu-hint">{{ capture.path }}</p>
        <div v-if="capture?.error" class="ksu-banner ksu-banner--error" role="alert">
          <div class="ksu-banner-body"><span>{{ capture.error }}</span></div>
        </div>
      </div>
    </section>

    <section class="ksu-card ksu-span-2">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">录制会话与来源</h2>
          <p class="ksu-card-sub">打开历史会话、目录或 PCAP 分卷；回放浏览不会向设备发送报文</p>
        </div>
        <div class="ksu-card-actions">
          <button class="ksu-btn ksu-btn--sm" :disabled="busy || !path || fileMode" :aria-pressed="follow" @click="toggleFollow">{{ follow ? '暂停跟随' : '跟随最新报文' }}</button>
        </div>
      </div>
      <div class="ksu-card-body">
        <RecordingLibrary :path="path" :file-mode="fileMode" :sessions="sessions" @open="openSession" />
        <div class="ksu-list">
          <div class="ksu-row ksu-row--static">
            <div class="ksu-row-body">
              <span class="ksu-row-title">PCAP / PCAPNG 文件或分卷目录</span>
              <span class="ksu-row-sub">开启后按文件模式检索全部分卷</span>
            </div>
            <div class="ksu-row-control">
              <button type="button" class="ksu-switch" role="switch" :aria-checked="fileMode" aria-label="PCAP 文件模式" @click="fileMode = !fileMode">
                <span class="ksu-switch-knob"></span>
              </button>
            </div>
          </div>
        </div>
        <div class="inline-field">
          <input class="ksu-input" v-model="path" :placeholder="fileMode ? '输入 PCAP / PCAPNG 文件完整路径' : '录制会话目录完整路径'" aria-label="录制或捕获路径" />
          <button class="ksu-btn ksu-btn--sm" :disabled="busy || pageBusy || !path" @click="page(0)">{{ pageBusy ? '检索中…' : '打开 / 刷新' }}</button>
        </div>
        <div class="ksu-list">
          <div class="ksu-row ksu-row--static">
            <div class="ksu-row-body">
              <span class="ksu-row-title">导出以太网 PCAPNG</span>
              <span class="ksu-row-sub">仅导出可用以太网帧；录制中不可导出</span>
            </div>
            <div class="ksu-row-control">
              <button class="ksu-btn ksu-btn--sm" :disabled="busy || !path || fileMode || status?.active" @click="exportPcap">导出</button>
            </div>
          </div>
        </div>
      </div>
    </section>

    <section class="ksu-card ksu-span-2">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">筛选与检索</h2>
          <p class="ksu-card-sub">全部筛选作用于整个会话；站号、地址和结果关联完整事务</p>
        </div>
        <div class="ksu-card-actions">
          <button class="ksu-btn ksu-btn--sm" @click="clearFilters">清除筛选</button>
        </div>
      </div>
      <div class="ksu-card-body">
        <div class="ksu-form-grid">
          <label class="ksu-field">
            <span class="ksu-field-label">报文筛选</span>
            <input class="ksu-input" v-model="query" :placeholder="fileMode ? '全部 PCAP 分卷搜索：来源、协议、事件、字节' : '全会话搜索：来源、协议、事务、事件、十六进制字节'" aria-label="报文筛选" />
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">方向</span>
            <select class="ksu-select" v-model="direction" aria-label="方向筛选">
              <option value="">所有方向</option>
              <option>tx</option>
              <option>rx</option>
              <option>event</option>
              <option>unknown</option>
            </select>
          </label>
        </div>
        <div class="button-row">
          <button class="ksu-btn ksu-btn--sm" :disabled="busy || pageBusy || offset === 0" @click="page(Math.max(0, offset - 100))">上一页</button>
          <button class="ksu-btn ksu-btn--sm" :disabled="busy || pageBusy || next === null" @click="page(next!)">下一页</button>
          <span class="ksu-hint">{{ total === null ? '本页 ' + rows.length + ' 条' : '共 ' + total + ' 条匹配 · ' + (rows.length ? offset + 1 : 0) + '–' + (offset + rows.length) }}{{ follow ? ' · 自动跟随中' : '' }}</span>
        </div>

        <details class="ksu-field" open>
          <summary class="ksu-field-label">时间与协议精确筛选</summary>
          <div class="ksu-form-grid">
            <label class="ksu-field">
              <span class="ksu-field-label">开始</span>
              <input class="ksu-input" v-model="fromTime" type="datetime-local" step="0.001" />
            </label>
            <label class="ksu-field">
              <span class="ksu-field-label">结束</span>
              <input class="ksu-input" v-model="toTime" type="datetime-local" step="0.001" />
            </label>
            <label class="ksu-field">
              <span class="ksu-field-label">来源精确匹配</span>
              <input class="ksu-input" v-model="sourceFilter" placeholder="来源精确匹配（可从报文复制）" aria-label="来源筛选" />
            </label>
            <label class="ksu-field">
              <span class="ksu-field-label">协议</span>
              <input class="ksu-input" v-model="protocolFilter" placeholder="协议，如 modbus-rtu / ethernet" aria-label="协议筛选" />
            </label>
          </div>
          <div v-if="!fileMode" class="ksu-form-grid">
            <label class="ksu-field">
              <span class="ksu-field-label">站号</span>
              <input class="ksu-input" v-model="slaveFilter" placeholder="站号" aria-label="站号筛选" />
            </label>
            <label class="ksu-field">
              <span class="ksu-field-label">功能码</span>
              <input class="ksu-input" v-model="functionFilter" placeholder="功能码，如 3 / 0x06" aria-label="功能码筛选" />
            </label>
            <label class="ksu-field">
              <span class="ksu-field-label">寄存器地址</span>
              <input class="ksu-input" v-model="addressFilter" placeholder="包含寄存器地址，如 0x100" aria-label="地址筛选" />
            </label>
            <label class="ksu-field">
              <span class="ksu-field-label">事务结果</span>
              <select class="ksu-select" v-model="outcomeFilter" aria-label="事务结果筛选">
                <option value="">所有事务结果</option>
                <option value="success">成功</option>
                <option value="failure">失败 / 设备异常</option>
                <option value="incomplete">缺少事务结束</option>
              </select>
            </label>
          </div>
        </details>
        <p class="ksu-hint">{{ fileMode ? '搜索与时间筛选覆盖所选文件或目录中的全部 PCAP 分卷。首次建立缓存可能较慢；源文件变化后重建，请优先打开已停止的捕获。' : '全部筛选作用于整个会话；站号、地址和结果关联完整事务。跟随显示最新 100 条已落盘匹配记录，查看详情时录制继续。' }}</p>
      </div>
    </section>

    <section class="ksu-card">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">邮箱 / PDO 解析映射</h2>
          <p class="ksu-card-sub">历史录制优先使用报文当时保存的映射；旧会话或外部 PCAP 没有映射时使用此处手动配置</p>
        </div>
        <div class="ksu-card-actions">
          <label class="ksu-btn ksu-btn--sm">
            导入 JSON
            <input type="file" hidden accept=".json" @change="importMap" />
          </label>
        </div>
      </div>
      <div class="ksu-card-body">
        <button class="ksu-btn ksu-btn--tonal" :disabled="busy" @click="saveMapping">保存映射到录制上下文</button>
        <label class="ksu-field">
          <span class="ksu-field-label">解析映射 JSON</span>
          <textarea class="ksu-textarea" v-model="mapping" rows="7" aria-label="解析映射 JSON" />
        </label>
        <p class="ksu-hint">PDO 字段必须来自实际工程定义。</p>
      </div>
    </section>

    <RecordingHistory :path="path" :file-mode="fileMode" :from-us="fromUs" :to-us="toUs" @select="locatePoint" />

    <section class="ksu-card ksu-span-full">
      <div class="ksu-card-head">
        <div>
          <h2 class="ksu-card-title">报文列表</h2>
          <p class="ksu-card-sub">丢包、清空缓冲和未完整落盘的数据均不能视为完整总线记录 · {{ total === null ? '本页 ' + visible.length + ' 条' : '共 ' + total + ' 条匹配' }}</p>
        </div>
        <div class="ksu-card-actions">
          <span v-if="follow" class="ksu-pill ksu-pill--ok">自动跟随</span>
          <span v-if="pageBusy" class="ksu-pill ksu-pill--warn">检索中</span>
        </div>
      </div>
      <div v-if="browseError" class="ksu-banner ksu-banner--error" role="alert">
        <div class="ksu-banner-body"><span>{{ browseError }}</span></div>
      </div>
      <div v-if="pageWarning" class="ksu-banner ksu-banner--info" role="status">
        <div class="ksu-banner-body"><span>{{ pageWarning }}</span></div>
      </div>
      <div ref="recordTable" class="ksu-list" @wheel.passive="pauseFollow" @focusin="pauseFollow">
        <button
          v-for="r in visible" :key="r.sequence" type="button" class="ksu-row"
          :aria-current="selected === r ? 'true' : undefined" @click="details(r)"
        >
          <div class="ksu-row-body">
            <span class="ksu-row-title mono">#{{ r.sequence }} · {{ r.source }}</span>
            <span class="ksu-row-sub">{{ time(r.timestampUs) }} · {{ r.protocol }} · {{ r.direction }} / 事务 {{ r.transaction || '—' }}</span>
            <span class="ksu-row-sub">{{ r.bytes.length }} 字节 · {{ r.detail }}</span>
          </div>
          <span class="ksu-row-chevron" aria-hidden="true">›</span>
        </button>
        <div v-if="!visible.length" class="ksu-empty">{{ pageBusy ? '正在建立索引或检索报文…' : '没有匹配的报文。打开会话、清除筛选或开启跟随查看新记录。' }}</div>
      </div>
    </section>



    <section v-if="detailBusy" class="ksu-card ksu-span-full">
      <div class="ksu-card-body"><p class="ksu-hint" role="status">正在关联完整事务…</p></div>
    </section>

    <section v-if="modbusDetail" class="ksu-card ksu-span-full">
      <div class="ksu-card-head">
        <div>
          <h3 class="ksu-card-title">Modbus 事务 #{{ modbusDetail.transaction }} · {{ modbusDetail.outcome }}</h3>
          <p class="ksu-card-sub">{{ modbusDetail.source }} · {{ modbusDetail.protocol }} · 应用观测耗时 {{ modbusDetail.durationUs === null ? '未知' : modbusDetail.durationUs + ' μs' }}（不是线缆传输时间）</p>
        </div>
      </div>
      <div class="ksu-card-body">
        <div v-if="modbusDetail.warning" class="ksu-banner ksu-banner--info"><div class="ksu-banner-body"><span>{{ modbusDetail.warning }}</span></div></div>
        <div class="ksu-form-grid">
          <div v-for="(frame, i) in [modbusDetail.request, modbusDetail.response]" :key="i" class="ksu-field">
            <span class="ksu-field-label">{{ i ? '响应' : '请求' }} · {{ frame.checksum }}</span>
            <p class="ksu-hint">站号 {{ frame.slave ?? '—' }} · 功能码 {{ frame.function === null ? '—' : '0x' + frame.function.toString(16).toUpperCase() }} · 起始地址 {{ frame.address === null ? '—' : '0x' + frame.address.toString(16).toUpperCase() }} · 数量 {{ frame.count ?? '—' }}</p>
            <p v-if="frame.error" class="ksu-hint" role="alert">{{ frame.error }}</p>
            <p v-if="frame.exception" class="ksu-hint" role="alert">{{ frame.exception }}</p>
            <pre class="mono">{{ frame.hex || '未记录字节' }}</pre>
            <p v-if="frame.values.length" class="ksu-hint">原始寄存器值（未按设备 Profile 缩放）</p>
            <pre v-if="frame.values.length" class="mono">{{ frame.values.join(', ') }}</pre>
          </div>
        </div>
        <details class="ksu-field">
          <summary class="ksu-field-label">事务事件（包含部分收发、清空与结束结果）</summary>
          <pre class="mono">{{ modbusDetail.events.join('\n') || '无事件' }}</pre>
        </details>
      </div>
    </section>

    <section v-if="selected && !modbusDetail && !detailBusy" class="ksu-card ksu-span-full">
      <div class="ksu-card-head">
        <div>
          <h3 class="ksu-card-title">记录 #{{ selected.sequence }} 原始内容</h3>
          <p class="ksu-card-sub">{{ selected.source }} · {{ selected.protocol }} · {{ time(selected.timestampUs) }}</p>
        </div>
      </div>
      <div class="ksu-card-body">
        <div class="ksu-form-grid">
          <div class="ksu-field">
            <span class="ksu-field-label">原始字节</span>
            <pre class="mono">{{ hex(selected.bytes) || '无字节，只有事件' }}</pre>
          </div>
          <div class="ksu-field">
            <span class="ksu-field-label">标准字段解析</span>
            <pre class="mono">{{ decoded ? JSON.stringify(decoded, null, 2) : '该记录保留原始内容；没有可用的结构化协议解析。' }}</pre>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>
