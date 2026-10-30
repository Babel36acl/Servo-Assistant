<script setup lang="ts">
import { onMounted, ref } from "vue";
import { servoApi } from "../../api";
import type { EthercatStatus } from "../../types";
import { useServo } from "../../composables/useServo";

const { activePage, communicationTab, ethercatConnected, showError } = useServo();
const adapters = ref<Array<{ name: string; description: string }>>([]);
const adapter = ref("");
const confirmation = ref("");
const status = ref<EthercatStatus>({ connected: false, adapter: "", slaves: [], driverDropped: null });
const busy = ref(false);
const error = ref("");

async function refreshAdapters() {
  try {
    adapters.value = await servoApi.ethercatAdapters();
    if (!adapters.value.some(item => item.name === adapter.value)) adapter.value = adapters.value[0]?.name ?? "";
  } catch (reason) {
    error.value = String(reason);
  }
}

async function refreshStatus() {
  try {
    status.value = await servoApi.ethercatStatus(true);
    ethercatConnected.value = status.value.connected;
  } catch (reason) {
    error.value = String(reason);
  }
}

async function connectMaster() {
  if (busy.value || !adapter.value) return;
  busy.value = true;
  error.value = "";
  try {
    status.value = await servoApi.ethercatConnect(adapter.value, confirmation.value);
    ethercatConnected.value = status.value.connected;
  } catch (reason) {
    error.value = String(reason);
    showError(reason);
  } finally {
    busy.value = false;
  }
}

async function disconnectMaster() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await servoApi.ethercatDisconnect();
    status.value = { connected: false, adapter: "", slaves: [], driverDropped: null };
    ethercatConnected.value = false;
  } catch (reason) {
    error.value = String(reason);
    showError(reason);
  } finally {
    busy.value = false;
  }
}

onMounted(async () => {
  await refreshAdapters();
  await refreshStatus();
});

function openMaster() {
  communicationTab.value = "ethercat";
  activePage.value = "communication";
}
</script>

<template>
  <div class="ksu-list">
    <div class="ksu-row ksu-row--static">
      <div class="ksu-row-body">
        <span class="ksu-row-title">EtherCAT 主站</span>
        <span class="ksu-row-sub">接管所选网卡并初始化从站；仅 PRE-OP，不进入 OP、不使能伺服、不发送周期输出</span>
      </div>
      <div class="ksu-row-control">
        <span class="ksu-pill" :class="status.connected ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ status.connected ? `已连接 · ${status.slaves.length} 从站` : '未连接' }}</span>
      </div>
    </div>
  </div>

  <div class="ksu-form-grid">
    <label class="ksu-field">
      <span class="ksu-field-label">专用网卡</span>
      <div class="inline-field">
        <select class="ksu-select" aria-label="EtherCAT 专用网卡" v-model="adapter" :disabled="status.connected || busy">
          <option value="">选择专用网卡</option>
          <option v-for="item in adapters" :key="item.name" :value="item.name">{{ item.description }}</option>
        </select>
        <button class="ksu-btn ksu-btn--sm" :disabled="status.connected || busy" @click="refreshAdapters">刷新</button>
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

  <div class="button-row">
    <button v-if="!status.connected" class="ksu-btn ksu-btn--primary" :disabled="busy || !adapter" @click="connectMaster">连接 EtherCAT 主站</button>
    <button v-else class="ksu-btn ksu-btn--danger" :disabled="busy" @click="disconnectMaster">断开主站</button>
    <button class="ksu-btn ksu-btn--tonal" :disabled="busy" @click="refreshStatus">刷新状态</button>
    <button class="ksu-btn ksu-btn--ghost" @click="openMaster">打开 EtherCAT 主站</button>
  </div>

  <p v-if="status.connected" class="ksu-hint">
    驱动丢包：{{ status.driverDropped ?? '未知' }} · 主站连接后可在“通信工作台 → EtherCAT 主站”导入对象字典并读写 SDO。
  </p>
  <p v-else class="ksu-hint">
    Windows 需要安装 Npcap 并重启应用；连接前请停用其他主站，确认设备处于可配置状态。
  </p>
</template>
