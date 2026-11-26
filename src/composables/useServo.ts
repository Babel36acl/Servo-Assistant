/* 对外聚合入口：第一个调用方必须是 App.vue（父组件 setup 先执行），
   onMounted / onUnmounted 因此绑在应用壳上；各视图只调用 useServo() 拿同一份状态。
   领域实现按职责拆在 ./servo/ 下：
     core.ts          页面导航 / Profile / busy / 审计 / 销毁标记
     confirm.ts       确认弹窗状态机（短语校验 + 焦点还原）
     communication.ts 串口重试与链路统计
     monitoring.ts    状态轮询 / 曲线采样 / 稳定性测试
     parameters.ts    参数读取、写入回读、快照比较、应用与持久化
     connection.ts    Profile 导入 / 串口·BLE·TCP·模拟器会话 / 自动查找
     format.ts        hex / 风险文案 / 时间格式 等纯函数
   新增跨页状态加到对应领域，不要再往这里堆字段；对外字段名保持稳定。 */
import { onMounted, onUnmounted } from "vue";
import { servoApi } from "../api";
import type { ScopeSeries } from "../components/ScopeChart.vue";
import { createCommunication } from "./servo/communication";
import { createConfirmDialog } from "./servo/confirm";
import { createConnection } from "./servo/connection";
import { createServoCore } from "./servo/core";
import { formatTimestamp, hex, riskLabel } from "./servo/format";
import { createMonitoring } from "./servo/monitoring";
import { createParameters } from "./servo/parameters";

export type { ScopeSeries };

let singleton: ReturnType<typeof createServo> | null = null;

export function useServo() {
  if (!singleton) singleton = createServo();
  return singleton;
}

export type Servo = ReturnType<typeof createServo>;

function createServo() {
  const core = createServoCore();
  const dialog = createConfirmDialog();
  const communication = createCommunication(core);
  const monitoring = createMonitoring(core, communication);
  const parameters = createParameters(core, communication, monitoring, dialog);
  const connection = createConnection(core, communication, monitoring, parameters);

  onMounted(core.refreshAudit);
  onUnmounted(() => {
    core.dispose();
    monitoring.stopTimers();
    connection.stopTimers();
    if (connection.discoveryActive.value) void servoApi.cancelDiscovery();
  });

  const api = {
    ...core,
    ...dialog,
    ...communication,
    ...monitoring,
    ...parameters,
    ...connection,
    hex,
    riskLabel,
    formatTimestamp,
  };

  // 内部句柄（定时器 / 在飞请求 / 确认回调）保持只读 getter，便于诊断与测试观察。
  Object.defineProperties(api, {
    disposed: { get: () => core.lifecycle.disposed, enumerable: true },
    statusTimer: { get: () => monitoring.statusTimer, enumerable: true },
    pollInFlight: { get: () => monitoring.pollInFlight, enumerable: true },
    discoveryTimer: { get: () => connection.discoveryTimer, enumerable: true },
    confirmResolve: { get: () => dialog.confirmResolve, enumerable: true },
    confirmOpener: { get: () => dialog.confirmOpener, enumerable: true },
  });

  return api;
}
