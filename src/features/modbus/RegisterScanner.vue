<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { servoApi } from "../../api";
import type { ScanStatus } from "../../types";
import { useServo } from "../../composables/useServo";

const { allowUnsupportedBaud, baudRate, busy, connected, hex, parity, portName, protocol, showError, slaveId, stopBits, timeoutMs } = useServo();
const startAddress = ref(0);
const endAddress = ref(255);
const blockSize = ref(16);
const includeZero = ref(false);
const scanning = ref(false);
const cancelling = ref(false);
const status = ref<ScanStatus | null>(null);
const error = ref("");
let timer: number | undefined;
let disposed = false;

const span = computed(() => Number(endAddress.value) - Number(startAddress.value) + 1);
const shownValues = computed(() => (status.value?.values ?? []).slice(0, 300));
const progress = computed(() => status.value ? `${status.value.completed}/${status.value.total} 块` : "");
const canStart = computed(() => !scanning.value && !busy.value && !connected.value && !!portName.value && span.value > 0 && span.value <= 4096);

async function poll() {
  if (disposed || !scanning.value) return;
  try { status.value = await servoApi.registerScanStatus(); } catch { /* 最终命令返回错误 */ }
  if (scanning.value && !disposed) timer = window.setTimeout(poll, 200);
}

async function start() {
  const start = Number(startAddress.value), end = Number(endAddress.value), block = Number(blockSize.value);
  if (![start, end, block].every(Number.isInteger) || start < 0 || end > 65535 || start > end || block < 1 || block > 100 || end - start + 1 > 4096) {
    error.value = "地址须为 0..65535 且起始不大于结束，单次最多 4096 个地址，每块 1..100 个寄存器";
    return;
  }
  if (!portName.value) { error.value = "请先在连接参数中选择串口"; return; }
  error.value = "";
  status.value = null;
  scanning.value = true;
  cancelling.value = false;
  timer = window.setTimeout(poll, 200);
  try {
    status.value = await servoApi.scanRegisters({
      connection: {
        protocol: protocol.value,
        preset: "profile",
        mode: "serial",
        portName: portName.value,
        slaveId: slaveId.value,
        baudRate: baudRate.value,
        allowUnsupportedBaud: allowUnsupportedBaud.value,
        parity: parity.value,
        stopBits: stopBits.value,
        timeoutMs: timeoutMs.value,
      },
      startAddress: start,
      endAddress: end,
      blockSize: block,
      includeZero: includeZero.value,
    });
  } catch (reason) {
    error.value = String(reason);
    showError(reason);
  } finally {
    window.clearTimeout(timer);
    scanning.value = false;
    cancelling.value = false;
  }
}

async function cancel() {
  cancelling.value = true;
  try { await servoApi.cancelRegisterScan(); } catch (reason) { showError(reason); }
}

function exportCsv() {
  const values = status.value?.values ?? [];
  if (!values.length) return;
  const lines = ["address,hex,raw", ...values.map(value => `${value.address},${hex(value.address)},${value.raw}`)];
  const blob = new Blob(["\uFEFF" + lines.join("\r\n")], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `register-scan-${Date.now()}.csv`;
  anchor.click();
  URL.revokeObjectURL(url);
}

onUnmounted(() => {
  disposed = true;
  window.clearTimeout(timer);
  if (scanning.value) void servoApi.cancelRegisterScan().catch(() => {});
});
</script>

<template>
  <section class="ksu-card">
    <div class="ksu-card-head">
      <div>
        <h2 class="ksu-card-title">寄存器扫描</h2>
        <p class="ksu-card-sub">只读 FC03 扫描保持寄存器并列出非零值；不写入设备，扫描期间独占串口</p>
      </div>
      <div class="ksu-card-actions">
        <span class="ksu-pill" :class="scanning ? 'ksu-pill--warn' : status ? 'ksu-pill--ok' : 'ksu-pill--muted'">
          {{ scanning ? '扫描中' : status ? '已完成' : '未扫描' }}
        </span>
      </div>
    </div>
    <div class="ksu-card-body">
      <div class="ksu-form-grid">
        <label class="ksu-field">
          <span class="ksu-field-label">起始地址（十进制）</span>
          <input class="ksu-input" v-model.number="startAddress" type="number" min="0" max="65535" :disabled="scanning" />
        </label>
        <label class="ksu-field">
          <span class="ksu-field-label">结束地址（十进制）</span>
          <input class="ksu-input" v-model.number="endAddress" type="number" min="0" max="65535" :disabled="scanning" />
        </label>
        <label class="ksu-field">
          <span class="ksu-field-label">每块寄存器数</span>
          <input class="ksu-input" v-model.number="blockSize" type="number" min="1" max="100" :disabled="scanning" />
        </label>
      </div>
      <p class="ksu-hint">
        {{ hex(Number(startAddress)) }} – {{ hex(Number(endAddress)) }} · 共 {{ span }} 个地址 ·
        单次上限 4096 个 · 串口 {{ portName || '未选择' }} · 站号 {{ slaveId }}
      </p>
      <div class="ksu-list">
        <div class="ksu-row ksu-row--static">
          <div class="ksu-row-body">
            <span class="ksu-row-title">包含 0 值</span>
            <span class="ksu-row-sub">关闭时只显示有非零值的寄存器；打开后显示范围内全部地址</span>
          </div>
          <div class="ksu-row-control">
            <button type="button" class="ksu-switch" role="switch" :aria-checked="includeZero" :disabled="scanning" aria-label="包含 0 值" @click="includeZero = !includeZero">
              <span class="ksu-switch-knob"></span>
            </button>
          </div>
        </div>
      </div>
      <p v-if="connected" class="ksu-hint">设备已连接。扫描需要独占串口，请先在“设备与连接”中断开。</p>
      <p v-else-if="!portName" class="ksu-hint">请先在“连接参数”中选择串口。</p>
      <div class="button-row">
        <button class="ksu-btn ksu-btn--primary" :disabled="!canStart" @click="start">开始扫描</button>
        <button v-if="scanning" class="ksu-btn" :disabled="cancelling" @click="cancel">{{ cancelling ? '正在取消' : '取消扫描' }}</button>
        <button class="ksu-btn ksu-btn--tonal" :disabled="!status?.values.length" @click="exportCsv">导出 CSV</button>
      </div>
      <p v-if="error" class="ksu-banner ksu-banner--error" role="alert">{{ error }}</p>
      <p v-if="status" class="ksu-hint" role="status">
        进度 {{ progress }} · 非零/已显示 {{ status.values.length }} 项 · 失败块 {{ status.failedBlocks }} · 耗时 {{ status.elapsedMs }} ms
        <template v-if="status.cancelled"> · 已取消</template>
      </p>
      <div v-if="status?.values.length" class="ksu-list">
        <div v-for="value in shownValues" :key="value.address" class="ksu-row ksu-row--static">
          <div class="ksu-row-body">
            <span class="ksu-row-title mono">{{ hex(value.address) }}</span>
            <span class="ksu-row-sub">十进制 {{ value.raw }} · 0x{{ value.raw.toString(16).toUpperCase().padStart(4, '0') }}</span>
          </div>
        </div>
      </div>
      <p v-if="status && status.values.length > shownValues.length" class="ksu-hint">仅显示前 300 项，导出 CSV 可查看全部 {{ status.values.length }} 项。</p>
      <div v-else-if="status && !status.values.length" class="ksu-empty">
        <strong>范围内没有非零值</strong>
        <p>可尝试打开“包含 0 值”，或调整地址范围。</p>
      </div>
    </div>
  </section>
</template>
