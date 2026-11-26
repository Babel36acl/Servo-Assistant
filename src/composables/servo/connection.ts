/* 连接领域：Profile 导入、串口 / BLE / TCP / 模拟器会话、站号与帧格式、
   自动查找（站点 + 波特率 + 校验位组合）。串口是独占资源，所有长操作都先让轮询收尾。 */
import { computed, ref, watch } from "vue";
import { servoApi } from "../../api";
import type {
  BleDeviceInfo,
  ModbusConnectionMode,
  Parity,
  SerialPortInfo,
  SerialProtocol,
  DiscoveryStatus,
  SerialMode,
} from "../../types";
import type { ServoCore } from "./core";
import type { createCommunication } from "./communication";
import type { createMonitoring } from "./monitoring";
import type { createParameters } from "./parameters";
import { serialModeLabel } from "./format";

const COMMON_BAUD_RATES = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];

export function createConnection(
  core: ServoCore,
  communication: ReturnType<typeof createCommunication>,
  monitoring: ReturnType<typeof createMonitoring>,
  parameters: ReturnType<typeof createParameters>,
) {
  const ports = ref<SerialPortInfo[]>([]);
  const portName = ref("");
  const bleDevices = ref<BleDeviceInfo[]>([]);
  const bleDeviceAddress = ref("");
  const bleScanning = ref(false);
  const host = ref("127.0.0.1");
  const port = ref(502);
  const ethercatConnected = ref(false);
  const slaveId = ref(1);
  const baudRate = ref(19200);
  const parity = ref<Parity>("even");
  const stopBits = ref(1);
  const protocol = ref<SerialProtocol>("rtu");
  const stationMax = 247;
  const allowUnsupportedBaud = ref(false);
  const discoveryBaudRates = ref<number[]>([19200]);
  const timeoutMs = ref(800);

  const baudRates = computed(() => {
    const base = core.profile.value?.transport?.allowedBaudRates ?? [19200];
    const rates = allowUnsupportedBaud.value ? [...base, ...COMMON_BAUD_RATES] : [...base];
    if (!rates.includes(baudRate.value)) rates.push(baudRate.value);
    return [...new Set(rates)].sort((a, b) => a - b);
  });

  const connectionBlockedReason = computed(() => {
    if (core.connectionMode.value === "ethercat") return "";
    if (!core.profile.value) return "请先导入设备 Profile（JSON），连接和查找需要其中的寄存器地址。";
    if (core.connectionMode.value === "serial" && !portName.value) return "请选择串口，再连接或自动查找。";
    if (core.connectionMode.value === "ble" && !bleDeviceAddress.value) return "请扫描并选择 ServoBridge 蓝牙桥设备。";
    if (
      core.connectionMode.value === "tcp" &&
      (!host.value.trim() || !Number.isInteger(port.value) || port.value < 1 || port.value > 65535)
    ) {
      return "请填写 Modbus TCP 主机与端口（1..65535）。";
    }
    return "";
  });

  async function refreshBleDevices() {
    if (bleScanning.value) return;
    bleScanning.value = true;
    try {
      bleDevices.value = await servoApi.bleScan("ServoBridge", 5000);
      if (bleDevices.value.length && !bleDevices.value.some((item) => item.address === bleDeviceAddress.value)) {
        bleDeviceAddress.value = bleDevices.value[0].address;
      }
    } catch (error) {
      core.showError(error);
    } finally {
      bleScanning.value = false;
    }
  }

  function toggleDiscoveryBaud(rate: number) {
    if (rate === baudRate.value) return;
    const selected = discoveryBaudRates.value;
    discoveryBaudRates.value = selected.includes(rate)
      ? selected.filter((item) => item !== rate)
      : [...selected, rate];
  }

  function selectAllDiscoveryBaudRates() {
    discoveryBaudRates.value = [...baudRates.value];
  }

  watch(baudRate, (rate) => {
    if (!discoveryBaudRates.value.includes(rate)) discoveryBaudRates.value = [...discoveryBaudRates.value, rate];
  });

  watch(allowUnsupportedBaud, (enabled) => {
    if (enabled) return;
    const allowed = core.profile.value?.transport?.allowedBaudRates ?? [19200];
    if (!allowed.includes(baudRate.value)) baudRate.value = core.profile.value?.transport?.defaultBaudRate ?? allowed[0];
    discoveryBaudRates.value = discoveryBaudRates.value.filter((rate) => allowed.includes(rate));
    if (!discoveryBaudRates.value.length) discoveryBaudRates.value = [baudRate.value];
  });

  async function refreshPorts() {
    core.errorMessage.value = "";
    try {
      ports.value = await servoApi.listPorts();
      if (!portName.value && ports.value.length) portName.value = ports.value[0].name;
    } catch (error) {
      core.showError(error);
    }
  }

  /* ---- 自动查找：先当前站号，再扫其余站号；结果只回填，不自动连接 ---- */
  const discoveryActive = ref(false);
  const discovery = ref<DiscoveryStatus | null>(null);
  const discoveryMessage = ref("");
  const discoveryStart = ref(1);
  const discoveryEnd = ref(247);
  const discoveryTimeout = ref(200);
  let discoveryTimer: number | undefined;

  async function pollDiscovery() {
    if (!discoveryActive.value || core.lifecycle.disposed) return;
    try {
      discovery.value = await servoApi.discoveryStatus();
    } catch {
      /* Final command returns errors. */
    }
    if (discoveryActive.value && !core.lifecycle.disposed) discoveryTimer = window.setTimeout(pollDiscovery, 250);
  }

  async function discoverConnection() {
    if (core.busy.value || core.connected.value || !core.profile.value) return;
    if (core.connectionMode.value === "ble" ? !bleDeviceAddress.value : !portName.value) return;
    if (
      ![discoveryStart.value, discoveryEnd.value, discoveryTimeout.value].every(Number.isInteger) ||
      discoveryStart.value < 1 ||
      discoveryEnd.value > stationMax ||
      discoveryStart.value > discoveryEnd.value ||
      discoveryTimeout.value < 100 ||
      discoveryTimeout.value > 2000
    ) {
      core.showError(`站号范围须为 1..${stationMax}，探测超时须为 100..2000 ms`);
      return;
    }
    core.busy.value = true;
    discoveryActive.value = true;
    discovery.value = null;
    discoveryMessage.value = "正在查找";
    core.errorMessage.value = "";
    const baudSelection = [...new Set([baudRate.value, ...discoveryBaudRates.value])].filter((rate) =>
      baudRates.value.includes(rate),
    );
    if (!baudSelection.length) {
      discoveryActive.value = false;
      core.busy.value = false;
      core.showError("请至少选择一个搜索波特率");
      return;
    }
    const mode = core.connectionMode.value === "ble" ? "ble" : "serial";
    const resultPromise = servoApi.discover({
      connection: {
        protocol: protocol.value,
        preset: "profile",
        mode,
        portName: core.connectionMode.value === "serial" ? portName.value : null,
        bleDevice: core.connectionMode.value === "ble" ? bleDeviceAddress.value : null,
        slaveId: slaveId.value,
        baudRate: baudRate.value,
        allowUnsupportedBaud: allowUnsupportedBaud.value,
        parity: parity.value,
        stopBits: stopBits.value,
        timeoutMs: discoveryTimeout.value,
      },
      startSlave: discoveryStart.value,
      endSlave: discoveryEnd.value,
      baudRates: baudSelection,
    });
    discoveryTimer = window.setTimeout(pollDiscovery, 250);
    try {
      const result = await resultPromise;
      discovery.value = result;
      if (result.found && result.slaveId !== null && result.baudRate !== null) {
        slaveId.value = result.slaveId;
        baudRate.value = result.baudRate;
        if (result.serialMode) {
          protocol.value = result.serialMode.protocol;
          parity.value = result.serialMode.parity;
          stopBits.value = result.serialMode.stopBits;
        }
        discoveryMessage.value = `已找到：站号 ${result.slaveId}，${result.baudRate} baud，${serialModeLabel({
          protocol: protocol.value,
          parity: parity.value,
          stopBits: stopBits.value,
        } as SerialMode)}。参数已填入，可点击连接。`;
      } else {
        discoveryMessage.value = result.cancelled
          ? "查找已取消，串口已释放"
          : "未找到。请检查串口、Profile、校验位、停止位，或增大探测超时后重试。";
      }
    } catch (error) {
      discoveryMessage.value = "查找失败";
      core.showError(error);
    } finally {
      window.clearTimeout(discoveryTimer);
      discoveryActive.value = false;
      core.busy.value = false;
      await core.refreshAudit();
    }
  }

  async function cancelDiscovery() {
    try {
      await servoApi.cancelDiscovery();
      discoveryMessage.value = "正在等待当前读取结束";
    } catch (error) {
      core.showError(error);
    }
  }

  /* ---- Profile 导入：重新填充传输参数并清空所有设备相关状态 ---- */
  async function loadProfileJson(json: string) {
    if (core.connected.value) {
      core.showError("导入新配置前必须先断开设备。");
      return;
    }
    core.busy.value = true;
    core.errorMessage.value = "";
    try {
      core.summary.value = await servoApi.importProfile(json);
      core.profile.value = await servoApi.getProfile();
      if (core.profile.value) {
        slaveId.value = core.profile.value.transport.defaultSlaveId;
        baudRate.value = core.profile.value.transport.defaultBaudRate;
        allowUnsupportedBaud.value = false;
        discoveryBaudRates.value = [...new Set(core.profile.value.transport.allowedBaudRates)];
        discoveryStart.value = 1;
        discoveryEnd.value = 247;
        protocol.value = core.profile.value.transport.kind === "modbus-ascii" ? "ascii" : "rtu";
        parity.value = core.profile.value.transport.parity;
        stopBits.value = core.profile.value.transport.stopBits;
        timeoutMs.value = core.profile.value.transport.timeoutMs;
      }
      parameters.resetParameterState();
      monitoring.resetTests();
      monitoring.resetScope();
      monitoring.communicationError.value = "";
      monitoring.scopeChannels.value = core.profile.value?.statuses.slice(0, 4).map((item) => item.id) ?? [];
      if (core.summary.value) {
        core.notice.value = `配置已校验：${core.summary.value.deviceName} · ${core.summary.value.parameterCount} 个参数`;
      }
      await core.refreshAudit();
    } catch (error) {
      core.showError(error);
    } finally {
      core.busy.value = false;
    }
  }

  async function handleProfileFile(event: Event) {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    await loadProfileJson(await file.text());
    input.value = "";
  }

  /* ---- 会话 ---- */
  async function connect() {
    if (core.busy.value) return;
    if (core.connectionMode.value === "ethercat") {
      core.showError("EtherCAT 主站请在连接参数卡片中连接");
      return;
    }
    if (connectionBlockedReason.value) {
      core.showError(connectionBlockedReason.value);
      return;
    }
    const mode = core.connectionMode.value as ModbusConnectionMode;
    core.busy.value = true;
    core.errorMessage.value = "";
    try {
      await communication.applyCommunicationSettings();
      await servoApi.connect({
        mode,
        portName: mode === "serial" ? portName.value || null : null,
        bleDevice: mode === "ble" ? bleDeviceAddress.value : null,
        host: mode === "tcp" ? host.value.trim() : null,
        port: mode === "tcp" ? port.value : null,
        slaveId: slaveId.value,
        baudRate: baudRate.value,
        allowUnsupportedBaud: allowUnsupportedBaud.value,
        protocol: protocol.value,
        preset: "profile",
        parity: parity.value,
        stopBits: stopBits.value,
        timeoutMs: timeoutMs.value,
      });
      core.connected.value = true;
      communication.communicationStats.value = null;
      monitoring.statusUpdatedAt.value = null;
      core.notice.value =
        mode === "simulator"
          ? "配置模拟器已连接"
          : mode === "tcp"
            ? `${host.value}:${port.value} 已连接`
            : mode === "ble"
              ? `蓝牙桥 ${bleDeviceAddress.value} 已连接`
              : `${portName.value} 已连接`;
      parameters.staleIds.value = new Set(core.profile.value?.parameters.map((item) => item.parameterId));
      await parameters.readParameterGroups(false);
      monitoring.scheduleStatusPoll(0);
    } catch (error) {
      core.showError(error);
    } finally {
      core.busy.value = false;
      await core.refreshAudit();
    }
  }

  async function disconnect() {
    if (core.busy.value) return;
    monitoring.stopTimers();
    core.busy.value = true;
    try {
      await monitoring.pauseForOperation();
      await servoApi.disconnect();
      core.connected.value = false;
      parameters.staleIds.value = new Set(core.profile.value?.parameters.map((item) => item.parameterId));
      monitoring.communicationError.value = "";
      monitoring.statuses.value = [];
      monitoring.resetScope();
      core.notice.value = "设备已断开";
    } catch (error) {
      core.showError(error);
    } finally {
      core.busy.value = false;
      await core.refreshAudit();
    }
  }

  function stopTimers() {
    window.clearTimeout(discoveryTimer);
  }

  return {
    ports,
    portName,
    bleDevices,
    bleDeviceAddress,
    bleScanning,
    host,
    port,
    ethercatConnected,
    slaveId,
    baudRate,
    parity,
    stopBits,
    protocol,
    stationMax,
    allowUnsupportedBaud,
    discoveryBaudRates,
    timeoutMs,
    baudRates,
    connectionBlockedReason,
    discoveryActive,
    discovery,
    discoveryMessage,
    discoveryStart,
    discoveryEnd,
    discoveryTimeout,
    refreshBleDevices,
    toggleDiscoveryBaud,
    selectAllDiscoveryBaudRates,
    refreshPorts,
    pollDiscovery,
    discoverConnection,
    cancelDiscovery,
    loadProfileJson,
    handleProfileFile,
    connect,
    disconnect,
    serialModeLabel,
    stopTimers,
    get discoveryTimer() {
      return discoveryTimer;
    },
  };
}
