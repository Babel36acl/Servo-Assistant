/* 监控领域：状态轮询、曲线采样、通讯统计刷新节奏、稳定性只读测试。
   轮询是“单飞”的：同一时刻只有一个 readStatuses 在飞，长操作前先 await 它，
   避免参数写入和状态轮询抢同一条串口。 */
import { computed, ref, watch } from "vue";
import { servoApi } from "../../api";
import { addressGroups } from "../../communication";
import { CHART_COLORS } from "../../styles/chart-palette";
import type { ScopeSeries } from "../../components/ScopeChart.vue";
import type { StatusValue } from "../../types";
import type { ServoCore } from "./core";
import type { createCommunication } from "./communication";
import { hex } from "./format";

export type TestResultRow = {
  label: string;
  count: number;
  total: number;
  first: number;
  recovered: number;
  failed: number;
  elapsed: number;
};

export function createMonitoring(core: ServoCore, communication: ReturnType<typeof createCommunication>) {
  const statuses = ref<StatusValue[]>([]);
  const statusUpdatedAt = ref<number | null>(null);
  const pollingEnabled = ref(true);
  const communicationError = ref("");
  const sampleInterval = ref(250);
  let statusTimer: number | undefined;
  let pollInFlight: Promise<void> | null = null;

  const scopeRunning = ref(true);
  const scopeChannels = ref<string[]>([]);
  const scopeData = ref<Record<string, number[]>>({});
  const scopePalette = CHART_COLORS;
  const scopeSeries = computed<ScopeSeries[]>(() =>
    scopeChannels.value.map((id, index) => {
      const definition = core.profile.value?.statuses.find((item) => item.id === id);
      return {
        id,
        name: definition?.name ?? id,
        unit: definition?.unit ?? "",
        color: scopePalette[index % scopePalette.length],
        values: scopeData.value[id] ?? [],
      };
    }),
  );
  const scopeOptions = computed(() => core.profile.value?.statuses ?? []);

  /** 首页大数字：优先展示速度 / 转矩 / 电流 / 报警 / 电压 / 温度，最多 6 个。 */
  const headlineStatuses = computed(() => {
    const preferred = ["speed", "torque", "motor_current", "alarm", "bus_voltage", "temperature"];
    return [...statuses.value]
      .sort((left, right) => {
        const leftIndex = preferred.indexOf(left.id);
        const rightIndex = preferred.indexOf(right.id);
        return (leftIndex < 0 ? preferred.length : leftIndex) - (rightIndex < 0 ? preferred.length : rightIndex);
      })
      .slice(0, 6);
  });

  const testActive = ref(false);
  const testCycles = ref(100);
  const testProgress = ref("");
  const testResults = ref<TestResultRow[]>([]);
  /** 稳定性测试用 Profile 里最长的一段连续状态区（不跨空洞，避免读到不支持的寄存器）。 */
  const probeRange = computed(
    () => addressGroups(core.profile.value?.statuses ?? [], 100).sort((a, b) => b.length - a.length)[0] ?? [],
  );

  function scheduleStatusPoll(delay = 1500) {
    window.clearTimeout(statusTimer);
    if (!core.connected.value || !pollingEnabled.value || core.lifecycle.disposed) return;
    statusTimer = window.setTimeout(async () => {
      if (core.busy.value) {
        scheduleStatusPoll();
        return;
      }
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
          await communication.updateDiagnostics();
        }
      };
      pollInFlight = poll();
      await pollInFlight;
      pollInFlight = null;
      const interval = Number(sampleInterval.value);
      scheduleStatusPoll(
        communicationError.value ? 3000 : Number.isFinite(interval) && interval >= 50 && interval <= 60000 ? interval : 250,
      );
    }, delay);
  }

  watch(pollingEnabled, () => {
    if (!pollInFlight) scheduleStatusPoll(0);
    else window.clearTimeout(statusTimer);
  });

  function normalizeSampleInterval() {
    if (!Number.isFinite(sampleInterval.value) || sampleInterval.value < 50 || sampleInterval.value > 60000) {
      sampleInterval.value = 250;
    }
  }

  /** 长操作前停轮询并等当前报文收尾（串口是独占资源）。 */
  async function pauseForOperation() {
    window.clearTimeout(statusTimer);
    await pollInFlight;
  }

  async function runStabilityTest() {
    if (core.busy.value || !core.connected.value || !probeRange.value.length) return;
    if (!Number.isInteger(testCycles.value) || testCycles.value < 1 || testCycles.value > 1000) {
      core.showError("测试轮数须为 1..1000");
      return;
    }
    core.busy.value = true;
    testActive.value = true;
    core.cancelRequested.value = false;
    testResults.value = [1, probeRange.value.length].map((count, index) => ({
      label: index ? "长帧" : "短帧",
      count,
      total: 0,
      first: 0,
      recovered: 0,
      failed: 0,
      elapsed: 0,
    }));
    const address = probeRange.value[0].address;
    try {
      await pauseForOperation();
      await communication.applyCommunicationSettings();
      for (let cycle = 0; cycle < testCycles.value && !core.cancelRequested.value && !core.lifecycle.disposed; cycle++) {
        for (const row of testResults.value) {
          if (core.cancelRequested.value || core.lifecycle.disposed) break;
          const result = await servoApi.probeRead(address, row.count);
          row.total++;
          row.elapsed += result.elapsedMs;
          if (!result.success) {
            row.failed++;
            communicationError.value = result.error ?? "读取失败";
          } else {
            if (result.attempts === 1) row.first++;
            else row.recovered++;
            communicationError.value = "";
          }
          testProgress.value = `第 ${cycle + 1}/${testCycles.value} 轮 · ${row.label} · ${hex(address)}/${row.count}`;
          await communication.updateDiagnostics();
        }
      }
      testProgress.value = `${core.cancelRequested.value ? "已取消" : "已完成"} · ${
        core.connectionMode.value === "simulator" ? "模拟器结果，不代表实机链路" : "实机只读测试"
      }`;
    } catch (error) {
      testProgress.value = "测试中断";
      core.showError(error);
    } finally {
      testActive.value = false;
      core.busy.value = false;
      await core.refreshAudit();
      scheduleStatusPoll();
    }
  }

  /** 断开 / 重新导入 Profile 时清空曲线采样。 */
  function resetScope() {
    scopeData.value = {};
  }

  function resetTests() {
    testResults.value = [];
    testProgress.value = "";
  }

  function stopTimers() {
    window.clearTimeout(statusTimer);
  }

  return {
    statuses,
    statusUpdatedAt,
    pollingEnabled,
    communicationError,
    sampleInterval,
    scopeRunning,
    scopeChannels,
    scopeData,
    scopePalette,
    scopeSeries,
    scopeOptions,
    headlineStatuses,
    testActive,
    testCycles,
    testProgress,
    testResults,
    probeRange,
    scheduleStatusPoll,
    normalizeSampleInterval,
    pauseForOperation,
    runStabilityTest,
    resetScope,
    resetTests,
    stopTimers,
    get statusTimer() {
      return statusTimer;
    },
    get pollInFlight() {
      return pollInFlight;
    },
  };
}
