<script setup lang="ts">
import { computed } from "vue";
import { useServo } from "../../composables/useServo";

/** 设备状态卡：Profile 摘要 + 当前连接方式 + 主操作（连接 / 断开）。
 *  状态仍来自 useServo 单例，卡片只负责这一段排版。 */
const {
  bleDeviceAddress, bleDevices, baudRate, busy, connect, connected, connectionBlockedReason,
  connectionMode, disconnect, ethercatConnected, host, parity, port, portName, profile, protocol,
  slaveId, stopBits, summary,
} = useServo();

const modeLabels = { simulator: "配置模拟器", serial: "真实串口", ble: "蓝牙桥接", tcp: "Modbus TCP", ethercat: "EtherCAT" } as const;
const modeLabel = computed(() => modeLabels[connectionMode.value]);
const modbusMode = computed(() => connectionMode.value !== "ethercat");
</script>

<template>
  <section class="ksu-card">
    <div class="ksu-card-head">
      <div>
        <h2 class="ksu-card-title">设备状态</h2>
        <p class="ksu-card-sub">模拟器演练、Modbus 串口 / TCP，或 EtherCAT 主站</p>
      </div>
      <div class="ksu-card-actions">
        <span class="ksu-pill" :class="connected || (connectionMode === 'ethercat' && ethercatConnected) ? 'ksu-pill--ok' : 'ksu-pill--muted'">
          {{ connectionMode === 'ethercat' ? (ethercatConnected ? '主站已连接' : '主站未连接') : connected ? '已连接' : '未连接' }}
        </span>
      </div>
    </div>
    <div class="ksu-card-body">
      <div>
        <h2 class="ksu-device-name">{{ summary?.deviceName ?? (profile ? '设备 Profile 已加载' : '尚未导入设备 Profile') }}</h2>
        <p class="ksu-hint">
          <template v-if="summary">Profile {{ summary.profileVersion }} · {{ summary.parameterCount }} 参数 · {{ summary.statusCount }} 状态量</template>
          <template v-else-if="profile">Profile 已就绪，选择连接方式后连接。</template>
          <template v-else>导入设备 JSON Profile 后即可建立连接。</template>
        </p>
      </div>
      <div class="ksu-row-actions">
        <span class="ksu-pill" :class="connectionMode === 'simulator' ? 'ksu-pill--muted' : 'ksu-pill--ok'">{{ modeLabel }}</span>
        <span v-if="connectionMode === 'serial'" class="ksu-pill">{{ portName || '未选择串口' }}</span>
        <span v-else-if="connectionMode === 'ble'" class="ksu-pill">{{ bleDevices.find((item) => item.address === bleDeviceAddress)?.name || bleDeviceAddress || '未选择蓝牙桥' }}</span>
        <span v-else-if="connectionMode === 'tcp'" class="ksu-pill">{{ host }}:{{ port }}</span>
        <span v-else-if="connectionMode === 'ethercat'" class="ksu-pill">{{ ethercatConnected ? '已接管网卡' : '未接管网卡' }}</span>
        <template v-if="modbusMode">
          <span class="ksu-pill">站号 {{ slaveId }}</span>
          <template v-if="connectionMode !== 'tcp'">
            <span class="ksu-pill">{{ baudRate }} baud</span>
            <span class="ksu-pill">{{ protocol === 'rtu' ? 'RTU' : 'ASCII' }} · 8{{ { none: 'N', even: 'E', odd: 'O' }[parity] }}{{ stopBits }}</span>
          </template>
        </template>
      </div>
      <button v-if="modbusMode && !connected" class="ksu-btn ksu-btn--primary ksu-btn--block" :disabled="busy || !!connectionBlockedReason" :title="connectionBlockedReason" @click="connect">连接</button>
      <button v-else-if="connected" class="ksu-btn ksu-btn--danger ksu-btn--block" :disabled="busy" @click="disconnect">断开连接</button>
      <p v-if="modbusMode && !connected && connectionBlockedReason" class="ksu-hint">{{ connectionBlockedReason }}</p>
      <p v-else-if="connectionMode === 'ethercat'" class="ksu-hint">EtherCAT 主站通过下方“连接参数”中的网卡选择进行接管；参数工作区仍使用 Modbus Profile。</p>
    </div>
  </section>
</template>
