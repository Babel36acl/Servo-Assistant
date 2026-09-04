<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { servoApi } from "./api";
import ScopeChart, { type ScopeSeries } from "./components/ScopeChart.vue";
import type {
  AuditEntry,
  ConnectionMode,
  ParameterDefinition,
  ParameterValue,
  Parity,
  ProfileSummary,
  SerialPortInfo,
  ServoProfile,
  SnapshotDiff,
  StatusValue,
} from "./types";

const profile = ref<ServoProfile | null>(null);
const summary = ref<ProfileSummary | null>(null);
const connected = ref(false);
const connectionMode = ref<ConnectionMode>("simulator");
const ports = ref<SerialPortInfo[]>([]);
const portName = ref("");
const slaveId = ref(1);
const baudRate = ref(19200);
const parity = ref<Parity>("even");
const stopBits = ref(1);
const timeoutMs = ref(800);
const values = ref<Record<string, ParameterValue>>({});
const drafts = ref<Record<string, number>>({});
const statuses = ref<StatusValue[]>([]);
const audit = ref<AuditEntry[]>([]);
const comparison = ref<SnapshotDiff[]>([]);
const selectedBatch = ref<string[]>([]);
const scopeRunning = ref(true);
const sampleInterval = ref(250);
const scopeChannels = ref<string[]>([]);
const scopeData = ref<Record<string, number[]>>({});
const query = ref("");
const selectedGroup = ref("全部");
const busy = ref(false);
const notice = ref("请导入设备 JSON Profile");
const errorMessage = ref("");
let statusTimer: number | undefined;

const groups = computed(() => [
  "全部",
  ...new Set(profile.value?.parameters.map((item) => item.group) ?? []),
]);

const comparisonById = computed(() => new Map(comparison.value.map((item) => [item.parameterId, item])));
const changedDifferences = computed(() => comparison.value.filter((item) => item.changed));
const headlineStatuses = computed(() => {
  const preferred = ["speed", "torque", "motor_current", "alarm", "bus_voltage", "temperature"];
  return [...statuses.value].sort((left, right) => {
    const leftIndex = preferred.indexOf(left.id);
    const rightIndex = preferred.indexOf(right.id);
    return (leftIndex < 0 ? preferred.length : leftIndex) - (rightIndex < 0 ? preferred.length : rightIndex);
  }).slice(0, 6);
});
const scopePalette = ["#56d6df", "#ffbf69", "#66e59f", "#ff6b78", "#a58cff", "#f58bc2"];
const scopeSeries = computed<ScopeSeries[]>(() => scopeChannels.value.map((id, index) => {
  const definition = profile.value?.statuses.find((item) => item.id === id);
  return {
    id,
    name: definition?.name ?? id,
    unit: definition?.unit ?? "",
    color: scopePalette[index % scopePalette.length],
    values: scopeData.value[id] ?? [],
  };
}));
const scopeOptions = computed(() => profile.value?.statuses ?? []);
const canApply = computed(() => Boolean(profile.value?.operations?.apply));
const canPersist = computed(() => Boolean(profile.value?.operations?.persist));

const filteredParameters = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  return (profile.value?.parameters ?? []).filter((parameter) => {
    const inGroup = selectedGroup.value === "全部" || parameter.group === selectedGroup.value;
    const searchable = `${parameter.parameterId} ${parameter.name} ${parameter.semanticId}`.toLowerCase();
    return inGroup && (!keyword || searchable.includes(keyword));
  });
});

function showError(error: unknown) {
  errorMessage.value = error instanceof Error ? error.message : String(error);
}

async function loadProfileJson(json: string) {
  if (connected.value) {
    showError("导入新配置前必须先断开设备。");
    return;
  }
  busy.value = true;
  errorMessage.value = "";
  try {
    summary.value = await servoApi.importProfile(json);
    profile.value = await servoApi.getProfile();
    if (profile.value) {
      slaveId.value = profile.value.transport.defaultSlaveId;
      baudRate.value = profile.value.transport.defaultBaudRate;
      parity.value = profile.value.transport.parity;
      stopBits.value = profile.value.transport.stopBits;
      timeoutMs.value = profile.value.transport.timeoutMs;
    }
    values.value = {};
    drafts.value = {};
    comparison.value = [];
    selectedBatch.value = [];
    scopeData.value = {};
    scopeChannels.value = profile.value?.statuses.slice(0, 4).map((item) => item.id) ?? [];
    notice.value = `配置已校验：${summary.value.deviceName} · ${summary.value.parameterCount} 个参数`;
    await refreshAudit();
  } catch (error) {
    showError(error);
  } finally {
    busy.value = false;
  }
}

async function handleProfileFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  await loadProfileJson(await file.text());
  input.value = "";
}

async function refreshPorts() {
  errorMessage.value = "";
  try {
    ports.value = await servoApi.listPorts();
    if (!portName.value && ports.value.length) portName.value = ports.value[0].name;
  } catch (error) {
    showError(error);
  }
}

async function connect() {
  busy.value = true;
  errorMessage.value = "";
  try {
    await servoApi.connect({
      mode: connectionMode.value,
      portName: connectionMode.value === "serial" ? portName.value || null : null,
      slaveId: slaveId.value,
      baudRate: baudRate.value,
      parity: parity.value,
      stopBits: stopBits.value,
      timeoutMs: timeoutMs.value,
    });
    connected.value = true;
    notice.value = connectionMode.value === "simulator" ? "配置模拟器已连接" : `${portName.value} 已连接`;
    await readAll();
    scheduleStatusPoll(0);
  } catch (error) {
    showError(error);
  } finally {
    busy.value = false;
    await refreshAudit();
  }
}

async function disconnect() {
  window.clearTimeout(statusTimer);
  busy.value = true;
  try {
    await servoApi.disconnect();
    connected.value = false;
    statuses.value = [];
    scopeData.value = {};
    notice.value = "设备已断开";
  } catch (error) {
    showError(error);
  } finally {
    busy.value = false;
    await refreshAudit();
  }
}

async function readAll() {
  if (!connected.value) return;
  errorMessage.value = "";
  try {
    const result = await servoApi.readParameters();
    const nextValues: Record<string, ParameterValue> = {};
    const nextDrafts: Record<string, number> = {};
    for (const item of result) {
      nextValues[item.parameterId] = item;
      nextDrafts[item.parameterId] = item.value;
    }
    values.value = nextValues;
    drafts.value = nextDrafts;
    notice.value = `已读取并解码 ${result.length} 个参数`;
  } catch (error) {
    showError(error);
  }
}

async function exportSnapshot() {
  if (!connected.value) return;
  busy.value = true;
  errorMessage.value = "";
  try {
    const snapshot = await servoApi.captureSnapshot("手动导出");
    const timestamp = new Date(snapshot.createdAtMs).toISOString().replace(/[:.]/g, "-");
    downloadJson(`${snapshot.deviceId}_${timestamp}.servo-snapshot.json`, snapshot);
    notice.value = `已导出 ${snapshot.values.length} 个参数，并保存到本地审计库`;
  } catch (error) {
    showError(error);
  } finally {
    busy.value = false;
    await refreshAudit();
  }
}

async function handleSnapshotFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  busy.value = true;
  errorMessage.value = "";
  try {
    comparison.value = await servoApi.compareSnapshot(await file.text());
    const selected = comparison.value.filter((item) => item.changed && item.writable);
    selectedBatch.value = selected.map((item) => item.parameterId);
    drafts.value = {
      ...drafts.value,
      ...Object.fromEntries(selected.map((item) => [item.parameterId, item.targetValue])),
    };
    notice.value = `快照比较完成：${changedDifferences.value.length} 项差异，${selected.length} 项可选择写入`;
  } catch (error) {
    showError(error);
  } finally {
    input.value = "";
    busy.value = false;
    await refreshAudit();
  }
}

async function writeSelectedBatch() {
  const selected = comparison.value.filter((item) => item.changed && item.writable && selectedBatch.value.includes(item.parameterId));
  if (!selected.length) return;
  const expectedPhrase = `批量写入 ${selected.length} 项`;
  const phrase = window.prompt(`将按当前快照选择性写入并逐项回读，任何写前值变化都会让整批操作停止。\n请输入“${expectedPhrase}”：`);
  if (phrase === null) return;
  busy.value = true;
  errorMessage.value = "";
  try {
    const result = await servoApi.batchWrite({
      items: selected.map((item) => ({ parameterId: item.parameterId, value: item.targetValue, expectedRaw: item.currentRaw })),
      confirmationPhrase: phrase,
    });
    for (const item of result.completed) {
      values.value[item.parameterId] = { parameterId: item.parameterId, raw: item.readBackRaw, value: item.value };
      drafts.value[item.parameterId] = item.value;
    }
    selectedBatch.value = selectedBatch.value.filter((id) => !result.completed.some((item) => item.parameterId === id));
    if (result.error) {
      showError(`批量写入在 ${result.failedParameterId} 停止：${result.error}；此前 ${result.completed.length} 项已写入并回读。`);
    } else {
      notice.value = `${result.completed.length} 项已写入并逐项回读一致`;
      comparison.value = [];
      selectedBatch.value = [];
    }
  } catch (error) {
    showError(error);
  } finally {
    busy.value = false;
    await refreshAudit();
  }
}

function clearComparison() {
  comparison.value = [];
  selectedBatch.value = [];
  for (const [parameterId, current] of Object.entries(values.value)) drafts.value[parameterId] = current.value;
}

function downloadJson(filename: string, value: unknown) {
  const url = URL.createObjectURL(new Blob([JSON.stringify(value, null, 2)], { type: "application/json" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

async function writeOne(parameter: ParameterDefinition) {
  const current = values.value[parameter.parameterId];
  const value = Number(drafts.value[parameter.parameterId]);
  if (!current || !Number.isFinite(value)) {
    showError("请先读取参数并填写有效数值。");
    return;
  }
  if (!window.confirm(`${parameter.parameterId}：${current.value} → ${value} ${parameter.unit}\n\n写入后将立即回读校验，当前不会自动执行应用或持久化操作。`)) {
    return;
  }
  let phrase: string | null = null;
  if (parameter.risk === "high" || parameter.risk === "critical") {
    phrase = window.prompt(`这是${riskLabel(parameter.risk)}参数。请输入“写入 ${parameter.parameterId}”继续：`);
    if (phrase === null) return;
  }
  busy.value = true;
  errorMessage.value = "";
  try {
    const result = await servoApi.writeParameter({
      parameterId: parameter.parameterId,
      value,
      expectedRaw: current.raw,
      confirmed: true,
      confirmationPhrase: phrase,
    });
    values.value = {
      ...values.value,
      [parameter.parameterId]: {
        parameterId: parameter.parameterId,
        raw: result.readBackRaw,
        value: result.value,
      },
    };
    drafts.value = { ...drafts.value, [parameter.parameterId]: result.value };
    notice.value = `${parameter.parameterId} 写入成功，回读原始值 ${result.readBackRaw} 一致`;
  } catch (error) {
    showError(error);
  } finally {
    busy.value = false;
    await refreshAudit();
  }
}

async function applyChanges() {
  const phrase = window.prompt("该操作会让已写入参数生效。请输入“应用参数”：");
  if (phrase === null) return;
  busy.value = true;
  try {
    const result = await servoApi.apply(phrase);
    notice.value = `参数应用成功，设备状态 ${hex(result.observedStatus)}`;
  } catch (error) {
    showError(error);
  } finally {
    busy.value = false;
    await refreshAudit();
  }
}

async function persistChanges() {
  const phrase = window.prompt("即将执行 Profile 定义的持久化操作。请输入“持久化参数”：");
  if (phrase === null) return;
  busy.value = true;
  try {
    const result = await servoApi.persist(phrase);
    notice.value = `参数持久化成功，设备状态 ${hex(result.observedStatus)}`;
  } catch (error) {
    showError(error);
  } finally {
    busy.value = false;
    await refreshAudit();
  }
}

function scheduleStatusPoll(delay = 1500) {
  window.clearTimeout(statusTimer);
  if (!connected.value) return;
  statusTimer = window.setTimeout(async () => {
    try {
      statuses.value = await servoApi.readStatuses();
      if (scopeRunning.value) {
        const next = { ...scopeData.value };
        for (const id of scopeChannels.value) {
          const status = statuses.value.find((item) => item.id === id);
          if (!status) continue;
          next[id] = [...(next[id] ?? []), status.value].slice(-300);
        }
        scopeData.value = next;
      }
    } catch (error) {
      showError(error);
    } finally {
      scheduleStatusPoll(errorMessage.value ? 3000 : sampleInterval.value);
    }
  }, delay);
}

async function refreshAudit() {
  try {
    audit.value = await servoApi.getAuditLog();
  } catch {
    // Audit refresh must never hide the result of the primary operation.
  }
}

function isDirty(parameter: ParameterDefinition) {
  const current = values.value[parameter.parameterId];
  return current && drafts.value[parameter.parameterId] !== current.value;
}

function riskLabel(risk: ParameterDefinition["risk"]) {
  return { low: "低风险", medium: "中风险", high: "高风险", critical: "关键安全" }[risk];
}

function hex(value: number) {
  return `0x${value.toString(16).toUpperCase().padStart(4, "0")}`;
}

function formatTimestamp(timestampMs: number) {
  return new Date(timestampMs).toLocaleString("zh-CN", { hour12: false });
}

onMounted(refreshAudit);
onUnmounted(() => window.clearTimeout(statusTimer));
</script>

<template>
  <div class="app-shell">
    <header class="topbar">
      <div>
        <p class="eyebrow">SERVO PARAMETER COMMISSIONING</p>
        <h1>伺服参数调试器</h1>
      </div>
      <div class="connection-pill" :class="{ online: connected }">
        <span class="status-dot"></span>
        {{ connected ? (connectionMode === "simulator" ? "模拟器在线" : "设备在线") : "未连接" }}
      </div>
    </header>

    <div v-if="errorMessage" class="banner error">
      <strong>操作失败</strong><span>{{ errorMessage }}</span>
      <button @click="errorMessage = ''">关闭</button>
    </div>
    <div class="banner info"><strong>当前状态</strong><span>{{ notice }}</span></div>

    <main class="workspace">
      <aside class="sidebar">
        <section class="panel profile-panel">
          <div class="section-title"><span>01</span><h2>设备配置</h2></div>
          <div v-if="summary" class="profile-card">
            <strong>{{ summary.deviceName }}</strong>
            <span>Profile {{ summary.profileVersion }}</span>
            <small>{{ summary.parameterCount }} 参数 · {{ summary.statusCount }} 状态量</small>
          </div>
          <p v-else>未内置任何厂商或型号数据。请导入经授权的设备 Profile。</p>
          <div class="button-row">
            <label class="file-button" :class="{ disabled: busy || connected }">
              导入 JSON
              <input type="file" accept="application/json,.json" :disabled="busy || connected" @change="handleProfileFile" />
            </label>
          </div>
        </section>

        <section class="panel connection-panel">
          <div class="section-title"><span>02</span><h2>连接设置</h2></div>
          <label>运行模式
            <select v-model="connectionMode" :disabled="connected">
              <option value="simulator">配置模拟器（安全演练）</option>
              <option value="serial">真实 Modbus RTU</option>
            </select>
          </label>
          <template v-if="connectionMode === 'serial'">
            <label>串口
              <div class="inline-field">
                <select v-model="portName" :disabled="connected">
                  <option value="" disabled>请选择串口</option>
                  <option v-for="port in ports" :key="port.name" :value="port.name">{{ port.name }}</option>
                </select>
                <button class="icon-button" :disabled="connected" title="刷新串口" @click="refreshPorts">↻</button>
              </div>
            </label>
          </template>
          <div class="field-grid">
            <label>站号<input v-model.number="slaveId" type="number" min="1" max="247" :disabled="connected" /></label>
            <label>波特率
              <select v-model.number="baudRate" :disabled="connected">
                <option v-for="baud in profile?.transport.allowedBaudRates ?? [19200]" :key="baud" :value="baud">{{ baud }}</option>
              </select>
            </label>
            <label>校验
              <select v-model="parity" :disabled="connected">
                <option value="none">无</option><option value="even">偶</option><option value="odd">奇</option>
              </select>
            </label>
            <label>超时 ms<input v-model.number="timeoutMs" type="number" min="100" max="10000" :disabled="connected" /></label>
          </div>
          <button v-if="!connected" class="primary full" :disabled="busy || !profile" @click="connect">连接</button>
          <button v-else class="danger-outline full" :disabled="busy" @click="disconnect">断开连接</button>
        </section>

        <section class="panel safety-panel">
          <div class="section-title"><span>03</span><h2>参数事务</h2></div>
          <p>应用和持久化仅在 Profile 明确定义对应命令时可用，并且必须分别确认。</p>
          <button class="secondary full" :disabled="busy || !connected || !canApply" @click="applyChanges">应用参数</button>
          <button class="warning full" :disabled="busy || !connected || !canPersist" @click="persistChanges">持久化参数</button>
        </section>

        <section class="panel snapshot-panel">
          <div class="section-title"><span>04</span><h2>参数快照</h2></div>
          <p>快照保存设备原始值和工程值，可用于备份、差异比较和选择性恢复。</p>
          <button class="secondary full" :disabled="busy || !connected" @click="exportSnapshot">读取并导出快照</button>
          <label class="file-button full" :class="{ disabled: busy || !connected }">
            导入快照并比较
            <input type="file" accept="application/json,.json" :disabled="busy || !connected" @change="handleSnapshotFile" />
          </label>
        </section>
      </aside>

      <section class="content-column">
        <section class="status-strip">
          <article v-for="status in headlineStatuses" :key="status.id">
            <span>{{ status.name }}</span>
            <strong>{{ status.value }} <small>{{ status.unit }}</small></strong>
            <code>{{ hex(status.address ?? 0) }}</code>
          </article>
          <article v-if="!statuses.length" class="empty-status">
            <span>实时状态</span><strong>—</strong><small>连接后自动刷新</small>
          </article>
        </section>

        <section class="panel scope-panel">
          <div class="scope-toolbar">
            <div>
              <p class="eyebrow">LIVE SCOPE</p>
              <h2>实时曲线</h2>
            </div>
            <div class="scope-actions">
              <label>采样间隔
                <select v-model.number="sampleInterval" :disabled="!connected">
                  <option :value="200">200 ms</option>
                  <option :value="500">500 ms</option>
                  <option :value="1000">1 s</option>
                </select>
              </label>
              <button class="secondary" :disabled="!connected" @click="scopeRunning = !scopeRunning">{{ scopeRunning ? '暂停' : '继续' }}</button>
              <button class="secondary" @click="scopeData = {}">清空</button>
            </div>
          </div>
          <div class="channel-picker">
            <label v-for="channel in scopeOptions" :key="channel.id">
              <input v-model="scopeChannels" type="checkbox" :value="channel.id" :disabled="scopeChannels.length >= 6 && !scopeChannels.includes(channel.id)" />
              {{ channel.name }}
            </label>
          </div>
          <ScopeChart :series="scopeSeries" />
        </section>

        <section class="panel parameter-panel">
          <div class="parameter-toolbar">
            <div>
              <p class="eyebrow">PARAMETER WORKSPACE</p>
              <h2>参数读取与安全写入</h2>
            </div>
            <div class="toolbar-actions">
              <input v-model="query" class="search" placeholder="搜索参数 ID / 名称…" />
              <button class="secondary" :disabled="busy || !connected" @click="readAll">读取全部</button>
            </div>
          </div>

          <nav class="group-tabs">
            <button v-for="group in groups" :key="group" :class="{ active: selectedGroup === group }" @click="selectedGroup = group">{{ group }}</button>
          </nav>

          <div v-if="comparison.length" class="comparison-bar">
            <span>快照差异 <strong>{{ changedDifferences.length }}</strong> 项；已选 <strong>{{ selectedBatch.length }}</strong> 项</span>
            <div>
              <button class="secondary" @click="clearComparison">取消比较</button>
              <button class="warning" :disabled="busy || !selectedBatch.length" @click="writeSelectedBatch">选择性批量写入</button>
            </div>
          </div>

          <div class="table-wrap">
            <table>
              <thead><tr><th v-if="comparison.length">选择</th><th>参数</th><th>名称</th><th>当前值</th><th>目标值</th><th>范围</th><th>风险</th><th></th></tr></thead>
              <tbody>
                <tr v-for="parameter in filteredParameters" :key="parameter.parameterId" :class="{ dirty: isDirty(parameter) }">
                  <td v-if="comparison.length" class="select-cell">
                    <input v-if="comparisonById.get(parameter.parameterId)?.changed && comparisonById.get(parameter.parameterId)?.writable" v-model="selectedBatch" type="checkbox" :value="parameter.parameterId" />
                    <span v-else-if="comparisonById.get(parameter.parameterId)?.changed" title="只读参数不能批量写入">只读</span>
                    <span v-else>—</span>
                  </td>
                  <td class="parameter-id"><strong>{{ parameter.parameterId }}</strong><code>{{ hex(parameter.address) }}</code></td>
                  <td class="parameter-name"><span>{{ parameter.name }}</span><small v-if="parameter.description">{{ parameter.description }}</small></td>
                  <td class="current-value">
                    <template v-if="values[parameter.parameterId]">{{ values[parameter.parameterId].value }} {{ parameter.unit }}</template>
                    <span v-else>未读取</span>
                  </td>
                  <td>
                    <select v-if="parameter.enumValues.length" v-model.number="drafts[parameter.parameterId]" :disabled="!connected">
                      <option v-for="choice in parameter.enumValues" :key="choice.value" :value="choice.value">{{ choice.label }}</option>
                    </select>
                    <div v-else class="number-field">
                      <input v-model.number="drafts[parameter.parameterId]" type="number" :min="parameter.min" :max="parameter.max" :step="1 / 10 ** parameter.decimals" :disabled="!connected" />
                      <span>{{ parameter.unit }}</span>
                    </div>
                  </td>
                  <td><span class="range">{{ parameter.min }} … {{ parameter.max }}</span></td>
                  <td><span class="risk" :class="parameter.risk">{{ riskLabel(parameter.risk) }}</span></td>
                  <td><button class="write-button" :disabled="busy || !connected || !isDirty(parameter)" @click="writeOne(parameter)">写入并回读</button></td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>

        <section class="panel audit-panel">
          <div class="section-title"><span>LOG</span><h2>操作证据</h2></div>
          <div class="audit-list">
            <div v-for="entry in [...audit].reverse().slice(0, 20)" :key="`${entry.timestampMs}-${entry.action}`" :class="`audit-${entry.status}`">
              <time>{{ formatTimestamp(entry.timestampMs) }}</time>
              <code>{{ entry.action }}</code>
              <span>{{ entry.detail }}</span>
            </div>
            <p v-if="!audit.length">尚无操作记录。</p>
          </div>
        </section>
      </section>
    </main>
  </div>
</template>
