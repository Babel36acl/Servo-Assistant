/* 应用状态单例：原 App.vue <script setup> 整体搬迁。
   首个调用方必须是 App.vue（父组件 setup 先执行），onMounted / watch
   因此绑在应用壳上；各视图只调用 useServo() 拿同一份状态。
   新增跨页状态往这里加，不要在视图里另起 ref。 */
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import type { ScopeSeries } from "../components/ScopeChart.vue";
import { addressGroups } from "../communication";
import { servoApi } from "../api";
import { CHART_COLORS } from "../styles/chart-palette";
import type {
  AuditEntry,
  SerialProtocol,
  SerialMode,
  CommunicationStats,
  DiscoveryStatus,
  ConnectionMode,
  ParameterDefinition,
  ParameterValue,
  Parity,
  ProfileSummary,
  SerialPortInfo,
  ServoProfile,
  SnapshotDiff,
  StatusValue,
} from "../types";
import { pages } from '../shell/activities';

let singleton: ReturnType<typeof createServo> | null = null;

export function useServo() {
  if (!singleton) singleton = createServo();
  return singleton;
}

export type Servo = ReturnType<typeof createServo>;

function createServo() {

const backgroundActivity = ref('');
const activePage = ref<(typeof pages)[number]['id']>('connection');
const currentPage = computed(() => pages.find(page => page.id === activePage.value)!);
watch(activePage, () => {
  window.scrollTo({ top: 0 });
  document.getElementById("main")?.focus({ preventScroll: true });
});

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
const protocol = ref<SerialProtocol>("rtu");
const stationMax = 247;
const COMMON_BAUD_RATES = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
const allowUnsupportedBaud = ref(false);
const discoveryBaudRates = ref<number[]>([19200]);
const baudRates = computed(() => {
  const base = profile.value?.transport?.allowedBaudRates ?? [19200];
  const rates = allowUnsupportedBaud.value ? [...base, ...COMMON_BAUD_RATES] : [...base];
  if (!rates.includes(baudRate.value)) rates.push(baudRate.value);
  return [...new Set(rates)].sort((a, b) => a - b);
});
function toggleDiscoveryBaud(rate: number) {
  if (rate === baudRate.value) return;
  const selected = discoveryBaudRates.value;
  discoveryBaudRates.value = selected.includes(rate)
    ? selected.filter(item => item !== rate)
    : [...selected, rate];
}
function selectAllDiscoveryBaudRates() {
  discoveryBaudRates.value = [...baudRates.value];
}
function serialModeLabel(mode: SerialMode) {
  return `${mode.protocol.toUpperCase()} · 8${{ none: "N", even: "E", odd: "O" }[mode.parity]}${mode.stopBits}`;
}
const timeoutMs = ref(800);
const connectionBlockedReason = computed(() => {
  if (!profile.value) return "请先导入设备 Profile（JSON），连接和查找需要其中的寄存器地址。";
  if (connectionMode.value === "serial" && !portName.value) return "请选择串口，再连接或自动查找。";
  return "";
});
const values = ref<Record<string, ParameterValue>>({});
const drafts = ref<Record<string, number>>({});
const statuses = ref<StatusValue[]>([]);
const statusUpdatedAt = ref<number | null>(null);
const audit = ref<AuditEntry[]>([]);
const comparison = ref<SnapshotDiff[]>([]);
const snapshotExportPath = ref("");
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
const pollingEnabled = ref(true);
const communicationError = ref("");
const retries = ref(2);
const maxRegisters = ref(16);
const communicationStats = ref<CommunicationStats | null>(null);
const readActive = ref(false);
const cancelRequested = ref(false);
const readProgress = ref("");
const failedGroups = ref<ParameterDefinition[][]>([]);
const staleIds = ref(new Set<string>());
const testActive = ref(false);
const testCycles = ref(100);
const testProgress = ref("");
const testResults = ref<Array<{ label: string; count: number; total: number; first: number; recovered: number; failed: number; elapsed: number }>>([]);
let pollInFlight: Promise<void> | null = null;
let disposed = false;
const discoveryActive = ref(false);
const discovery = ref<DiscoveryStatus | null>(null);
const discoveryMessage = ref("");
const discoveryStart = ref(1);
const discoveryEnd = ref(247);
const discoveryTimeout = ref(200);
let discoveryTimer: number | undefined;

watch(baudRate, (rate) => {
  if (!discoveryBaudRates.value.includes(rate)) discoveryBaudRates.value = [...discoveryBaudRates.value, rate];
});
watch(allowUnsupportedBaud, (enabled) => {
  if (enabled) return;
  const allowed = profile.value?.transport?.allowedBaudRates ?? [19200];
  if (!allowed.includes(baudRate.value)) baudRate.value = profile.value?.transport?.defaultBaudRate ?? allowed[0];
  discoveryBaudRates.value = discoveryBaudRates.value.filter(rate => allowed.includes(rate));
  if (!discoveryBaudRates.value.length) discoveryBaudRates.value = [baudRate.value];
});

async function pollDiscovery() {
  if (!discoveryActive.value || disposed) return;
  try { discovery.value = await servoApi.discoveryStatus(); } catch { /* Final command returns errors. */ }
  if (discoveryActive.value && !disposed) discoveryTimer = window.setTimeout(pollDiscovery, 250);
}

async function discoverConnection() {
  if (busy.value || connected.value || !profile.value || !portName.value) return;
  if (![discoveryStart.value, discoveryEnd.value, discoveryTimeout.value].every(Number.isInteger) || discoveryStart.value < 1 || discoveryEnd.value > stationMax || discoveryStart.value > discoveryEnd.value || discoveryTimeout.value < 100 || discoveryTimeout.value > 2000) {
    showError(`站号范围须为 1..${stationMax}，探测超时须为 100..2000 ms`); return;
  }
  busy.value = true;
  discoveryActive.value = true;
  discovery.value = null;
  discoveryMessage.value = "正在查找";
  errorMessage.value = "";
  const baudSelection = [...new Set([baudRate.value, ...discoveryBaudRates.value])]
    .filter(rate => baudRates.value.includes(rate));
  if (!baudSelection.length) {
    discoveryActive.value = false;
    busy.value = false;
    showError("请至少选择一个搜索波特率");
    return;
  }
  const resultPromise = servoApi.discover({ connection: { protocol: protocol.value, preset: "profile", mode: 'serial', portName: portName.value, slaveId: slaveId.value, baudRate: baudRate.value, allowUnsupportedBaud: allowUnsupportedBaud.value, parity: parity.value, stopBits: stopBits.value, timeoutMs: discoveryTimeout.value }, startSlave: discoveryStart.value, endSlave: discoveryEnd.value, baudRates: baudSelection });
  discoveryTimer = window.setTimeout(pollDiscovery, 250);
  try {
    const result = await resultPromise;
    discovery.value = result;
    if (result.found && result.slaveId !== null && result.baudRate !== null) {
      slaveId.value = result.slaveId;
      baudRate.value = result.baudRate;
      if (result.serialMode) { protocol.value = result.serialMode.protocol; parity.value = result.serialMode.parity; stopBits.value = result.serialMode.stopBits; }
      discoveryMessage.value = `已找到：站号 ${result.slaveId}，${result.baudRate} baud，${serialModeLabel({ protocol: protocol.value, parity: parity.value, stopBits: stopBits.value })}。参数已填入，可点击连接。`;
    } else discoveryMessage.value = result.cancelled ? "查找已取消，串口已释放" : "未找到。请检查串口、Profile、校验位、停止位，或增大探测超时后重试。";
  } catch (error) { discoveryMessage.value = "查找失败"; showError(error); }
  finally { window.clearTimeout(discoveryTimer); discoveryActive.value = false; busy.value = false; await refreshAudit(); }
}

async function cancelDiscovery() {
  try { await servoApi.cancelDiscovery(); discoveryMessage.value = "正在等待当前读取结束"; }
  catch (error) { showError(error); }
}
const probeRange = computed(() => addressGroups(profile.value?.statuses ?? [], 100).sort((a, b) => b.length - a.length)[0] ?? []);

async function updateDiagnostics() {
  try { communicationStats.value = await servoApi.communicationStats(); } catch { /* Keep the primary operation's result. */ }
}

async function applyCommunicationSettings() {
  await servoApi.configureCommunication({ retries: retries.value, maxRegisters: maxRegisters.value });
}

async function saveCommunicationSettings() {
  if (busy.value) return;
  busy.value = true;
  try { await applyCommunicationSettings(); notice.value = "通讯设置已应用（当前应用会话）"; }
  catch (error) { showError(error); }
  finally { busy.value = false; }
}

async function pauseForOperation() {
  window.clearTimeout(statusTimer);
  await pollInFlight;
}

async function runStabilityTest() {
  if (busy.value || !connected.value || !probeRange.value.length) return;
  if (!Number.isInteger(testCycles.value) || testCycles.value < 1 || testCycles.value > 1000) { showError("测试轮数须为 1..1000"); return; }
  busy.value = true;
  testActive.value = true;
  cancelRequested.value = false;
  testResults.value = [1, probeRange.value.length].map((count, index) => ({ label: index ? "长帧" : "短帧", count, total: 0, first: 0, recovered: 0, failed: 0, elapsed: 0 }));
  const address = probeRange.value[0].address;
  try {
    await pauseForOperation();
    await applyCommunicationSettings();
    for (let cycle = 0; cycle < testCycles.value && !cancelRequested.value && !disposed; cycle++) {
      for (const row of testResults.value) {
        if (cancelRequested.value || disposed) break;
        const result = await servoApi.probeRead(address, row.count);
        row.total++;
        row.elapsed += result.elapsedMs;
        if (!result.success) { row.failed++; communicationError.value = result.error ?? "读取失败"; }
        else { if (result.attempts === 1) row.first++; else row.recovered++; communicationError.value = ""; }
        testProgress.value = `第 ${cycle + 1}/${testCycles.value} 轮 · ${row.label} · ${hex(address)}/${row.count}`;
        await updateDiagnostics();
      }
    }
    testProgress.value = `${cancelRequested.value ? "已取消" : "已完成"} · ${connectionMode.value === "simulator" ? "模拟器结果，不代表实机链路" : "实机只读测试"}`;
  } catch (error) { testProgress.value = "测试中断"; showError(error); }
  finally { testActive.value = false; busy.value = false; await refreshAudit(); scheduleStatusPoll(); }
}

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
const scopePalette = CHART_COLORS;
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

type ConfirmRequest = { title: string; body: string; danger?: boolean; phrase?: string };
const confirmReq = ref<ConfirmRequest | null>(null);
const confirmInput = ref("");
const confirmError = ref("");
const confirmField = ref<HTMLInputElement | null>(null);
let confirmResolve: ((value: string | null) => void) | null = null;
let confirmOpener: HTMLElement | null = null;
function requestConfirm(req: ConfirmRequest): Promise<string | null> {
  confirmReq.value = req;
  confirmInput.value = "";
  confirmError.value = "";
  confirmOpener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  return new Promise((resolve) => {
    confirmResolve = resolve;
    void nextTick(() => {
      const fallback = document.querySelector(".modal-actions button:last-child");
      const target = (confirmField.value ?? fallback) as HTMLElement | null;
      target?.focus();
    });
  });
}
function closeConfirm(action: "submit" | null) {
  const req = confirmReq.value;
  if (!req || !confirmResolve) return;
  if (action === null) {
    confirmReq.value = null;
    const done = confirmResolve;
    confirmResolve = null;
    done(null);
    confirmOpener?.focus();
    return;
  }
  if (req.phrase && confirmInput.value !== req.phrase) {
    confirmError.value = `请输入“${req.phrase}”再继续`;
    confirmField.value?.focus();
    return;
  }
  confirmReq.value = null;
  const done = confirmResolve;
  confirmResolve = null;
  done(req.phrase ?? "ok");
  confirmOpener?.focus();
}
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
      allowUnsupportedBaud.value = false;
      discoveryBaudRates.value = [...new Set(profile.value.transport.allowedBaudRates)];
      discoveryStart.value = 1;
      discoveryEnd.value = 247;
      protocol.value = profile.value.transport.kind === "modbus-ascii" ? "ascii" : "rtu";
      parity.value = profile.value.transport.parity;
      stopBits.value = profile.value.transport.stopBits;
      timeoutMs.value = profile.value.transport.timeoutMs;
    }
    values.value = {};
    staleIds.value = new Set();
    failedGroups.value = [];
    readProgress.value = "";
    testResults.value = [];
    testProgress.value = "";
    communicationError.value = "";
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
  if (busy.value) return;
  if (connectionBlockedReason.value) { showError(connectionBlockedReason.value); return; }
  busy.value = true;
  errorMessage.value = "";
  try {
    await applyCommunicationSettings();
    await servoApi.connect({
      mode: connectionMode.value,
      portName: connectionMode.value === "serial" ? portName.value || null : null,
      slaveId: slaveId.value,
      baudRate: baudRate.value,
      allowUnsupportedBaud: allowUnsupportedBaud.value,
      protocol: protocol.value,
      preset: "profile",
      parity: parity.value,
      stopBits: stopBits.value,
      timeoutMs: timeoutMs.value,
    });
    connected.value = true;
    communicationStats.value = null;
    statusUpdatedAt.value = null;
    notice.value = connectionMode.value === "simulator" ? "配置模拟器已连接" : `${portName.value} 已连接`;
    staleIds.value = new Set(profile.value?.parameters.map(item => item.parameterId));
    await readParameterGroups(false);
    scheduleStatusPoll(0);
  } catch (error) {
    showError(error);
  } finally {
    busy.value = false;
    await refreshAudit();
  }
}

async function disconnect() {
  if (busy.value) return;
  window.clearTimeout(statusTimer);
  busy.value = true;
  try {
    await pauseForOperation();
    await servoApi.disconnect();
    connected.value = false;
    staleIds.value = new Set(profile.value?.parameters.map(item => item.parameterId));
    communicationError.value = "";
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

async function readAll(failedOnly = false) {
  if (!connected.value || busy.value) return;
  busy.value = true;
  try { await readParameterGroups(failedOnly); }
  finally { busy.value = false; scheduleStatusPoll(); }
}

async function readParameterGroups(failedOnly: boolean) {
  readActive.value = true;
  cancelRequested.value = false;
  errorMessage.value = "";
  notice.value = "正在读取参数";
  try {
    await pauseForOperation();
    await applyCommunicationSettings();
    const pending = failedOnly ? [...failedGroups.value] : addressGroups(profile.value?.parameters ?? [], maxRegisters.value);
    failedGroups.value = [...pending];
    for (const group of pending) for (const item of group) staleIds.value.add(item.parameterId);
    for (let index = 0; index < pending.length && !cancelRequested.value && !disposed; index++) {
      const group = pending[index];
      readProgress.value = `${index + 1}/${pending.length} 地址组 · ${hex(group[0].address)}/${group.length}`;
      try {
        const result = await servoApi.readParameters(group.map(item => item.parameterId));
        for (const item of result) {
          // A refresh must not discard the user's uncommitted edits.
          const edited = values.value[item.parameterId] && drafts.value[item.parameterId] !== values.value[item.parameterId].value;
          values.value[item.parameterId] = item;
          if (!edited) drafts.value[item.parameterId] = item.value;
          staleIds.value.delete(item.parameterId);
        }
        failedGroups.value = failedGroups.value.filter(item => item[0].parameterId !== group[0].parameterId);
      } catch (error) { showError(error); }
    }
    notice.value = `${cancelRequested.value ? "读取已取消" : "读取结束"}：${pending.length - failedGroups.value.length}/${pending.length} 地址组成功，${failedGroups.value.length} 组失败或未完成`;
  } catch (error) {
    notice.value = "参数读取未完成";
    showError(error);
  } finally { readActive.value = false; await updateDiagnostics(); await refreshAudit(); }
}

async function exportSnapshot() {
  if (!connected.value) return;
  busy.value = true;
  errorMessage.value = "";
  notice.value = "请选择快照保存位置，确认后读取并导出";
  try {
    const result = await servoApi.exportSnapshot();
    if (!result) {
      notice.value = "已取消导出，未生成快照文件";
      return;
    }
    snapshotExportPath.value = result.path;
    notice.value = `已导出 ${result.parameterCount} 个参数，文件已保存至：${result.path}`;
  } catch (error) {
    notice.value = "快照文件导出未完成";
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
  const phrase = await requestConfirm({
    title: "选择性批量写入",
    body: `将按当前快照选择性写入并逐项回读，任何写前值变化都会让整批操作停止，共 ${selected.length} 项。`,
    danger: true,
    phrase: expectedPhrase,
  });
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

async function writeOne(parameter: ParameterDefinition) {
  const current = values.value[parameter.parameterId];
  const value = Number(drafts.value[parameter.parameterId]);
  if (!current || !Number.isFinite(value)) {
    showError("请先读取参数并填写有效数值。");
    return;
  }
  const highRisk = parameter.risk === "high" || parameter.risk === "critical";
  const phrase = await requestConfirm({
    title: `写入 ${parameter.parameterId}`,
    body: `${parameter.parameterId}：${current.value} → ${value} ${parameter.unit}。写入后立即回读校验，不会自动应用或持久化。`,
    danger: highRisk,
    phrase: highRisk ? `写入 ${parameter.parameterId}` : undefined,
  });
  if (phrase === null) return;
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
  const phrase = await requestConfirm({
    title: "应用参数",
    body: "该操作会让已写入参数在设备上生效。",
    phrase: "应用参数",
  });
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
  const phrase = await requestConfirm({
    title: "持久化参数",
    body: "即将执行 Profile 定义的持久化操作，会覆盖旧配置。",
    danger: true,
    phrase: "持久化参数",
  });
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
  if (!connected.value || !pollingEnabled.value || disposed) return;
  statusTimer = window.setTimeout(async () => {
    if (busy.value) { scheduleStatusPoll(); return; }
    const poll = async () => {
    try {
      statuses.value = await servoApi.readStatuses();
      statusUpdatedAt.value = Date.now();
      communicationError.value = "";
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
      communicationError.value = String(error);
    } finally {
      await updateDiagnostics();
    }
    };
    pollInFlight = poll();
    await pollInFlight;
    pollInFlight = null;
    const interval = Number(sampleInterval.value);
    scheduleStatusPoll(communicationError.value ? 3000 : Number.isFinite(interval) && interval >= 50 && interval <= 60000 ? interval : 250);
  }, delay);
}

watch(pollingEnabled, () => { if (!pollInFlight) scheduleStatusPoll(0); else window.clearTimeout(statusTimer); });
function normalizeSampleInterval() {
  if (!Number.isFinite(sampleInterval.value) || sampleInterval.value < 50 || sampleInterval.value > 60000) sampleInterval.value = 250;
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
onUnmounted(() => { disposed = true; cancelRequested.value = true; window.clearTimeout(statusTimer); window.clearTimeout(discoveryTimer); if (discoveryActive.value) void servoApi.cancelDiscovery(); });

  return { backgroundActivity, activePage, currentPage, profile, summary, connected, connectionMode, ports, portName, slaveId, baudRate, parity, stopBits, protocol, stationMax, baudRates, allowUnsupportedBaud, discoveryBaudRates, toggleDiscoveryBaud, selectAllDiscoveryBaudRates, timeoutMs, connectionBlockedReason, values, drafts, statuses, statusUpdatedAt, audit, comparison, snapshotExportPath, selectedBatch, scopeRunning, sampleInterval, scopeChannels, scopeData, query, selectedGroup, busy, notice, errorMessage, statusTimer, pollingEnabled, communicationError, retries, maxRegisters, communicationStats, readActive, cancelRequested, readProgress, failedGroups, staleIds, testActive, testCycles, testProgress, testResults, pollInFlight, disposed, discoveryActive, discovery, discoveryMessage, discoveryStart, discoveryEnd, discoveryTimeout, discoveryTimer, probeRange, groups, comparisonById, changedDifferences, headlineStatuses, scopePalette, scopeSeries, scopeOptions, canApply, canPersist, filteredParameters, confirmReq, confirmInput, confirmError, confirmField, confirmResolve, confirmOpener, serialModeLabel, pollDiscovery, discoverConnection, cancelDiscovery, updateDiagnostics, applyCommunicationSettings, saveCommunicationSettings, pauseForOperation, runStabilityTest, requestConfirm, closeConfirm, showError, loadProfileJson, handleProfileFile, refreshPorts, connect, disconnect, readAll, readParameterGroups, exportSnapshot, handleSnapshotFile, writeSelectedBatch, clearComparison, writeOne, applyChanges, persistChanges, scheduleStatusPoll, normalizeSampleInterval, refreshAudit, isDirty, riskLabel, hex, formatTimestamp };
}
