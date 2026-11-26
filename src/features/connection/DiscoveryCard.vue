<script setup lang="ts">
import { useServo } from "../../composables/useServo";

/** 自动查找卡：站号区间 + 波特率多选 + 帧格式组合探测，只回填参数不自动连接。 */
const {
  allowUnsupportedBaud, baudRate, baudRates, busy, cancelDiscovery, connected, connectionBlockedReason,
  discoverConnection, discovery, discoveryActive, discoveryBaudRates, discoveryEnd, discoveryMessage,
  discoveryStart, discoveryTimeout, selectAllDiscoveryBaudRates, serialModeLabel, toggleDiscoveryBaud,
  connectionMode, stationMax,
} = useServo();
</script>

<template>
  <section v-if="connectionMode === 'serial' || connectionMode === 'ble'" class="ksu-card">
    <div class="ksu-card-head">
      <div>
        <h2 class="ksu-card-title">自动查找通讯配置</h2>
        <p class="ksu-card-sub">优先当前配置，再查找 RTU / ASCII 的 8E1、8N1、8O1，每个配置连续两次只读响应后确认</p>
      </div>
      <div class="ksu-card-actions">
        <span class="ksu-pill" :class="discoveryActive ? 'ksu-pill--warn' : 'ksu-pill--muted'">{{ discoveryActive ? '查找中' : '待查找' }}</span>
      </div>
    </div>
    <div class="ksu-card-body">
      <div class="ksu-form-grid">
        <label class="ksu-field">
          <span class="ksu-field-label">起始站号</span>
          <input class="ksu-input" v-model.number="discoveryStart" type="number" min="1" :max="stationMax" :disabled="busy || connected" />
        </label>
        <label class="ksu-field">
          <span class="ksu-field-label">结束站号</span>
          <input class="ksu-input" v-model.number="discoveryEnd" type="number" min="1" :max="stationMax" :disabled="busy || connected" />
        </label>
        <label class="ksu-field">
          <span class="ksu-field-label">探测超时 ms</span>
          <input class="ksu-input" v-model.number="discoveryTimeout" type="number" min="100" max="2000" :disabled="busy || connected" />
        </label>
      </div>

      <div class="ksu-field">
        <span class="ksu-field-label">搜索波特率（当前波特率必选，可多选）</span>
        <div class="ksu-actions">
          <button
            v-for="rate in baudRates" :key="rate" type="button"
            class="ksu-btn ksu-btn--sm"
            :class="rate === baudRate ? 'ksu-btn--primary' : discoveryBaudRates.includes(rate) ? 'ksu-btn--tonal' : 'ksu-btn--ghost'"
            :disabled="busy || connected" :aria-pressed="rate === baudRate || discoveryBaudRates.includes(rate)"
            @click="toggleDiscoveryBaud(rate)"
            >{{ rate }}</button>
            <button class="ksu-btn ksu-btn--ghost ksu-btn--sm" :disabled="busy || connected" @click="selectAllDiscoveryBaudRates">全选</button>
          </div>
          <p class="ksu-hint">
            先在当前站号上轮询所选波特率与 RTU / ASCII 的 8E1、8N1、8O1，再扫其余站号；
            {{ allowUnsupportedBaud ? '已允许 Profile 外波特率。' : '超出 Profile 的波特率需先打开“扩展波特率”。' }}
          </p>
        </div>

        <div v-if="discoveryActive && discovery" class="ksu-grid">
          <div class="ksu-stat">
            <span class="ksu-stat-label">查找进度</span>
            <span class="ksu-stat-value ksu-display">{{ discovery.completed }}<span class="ksu-stat-unit">/{{ discovery.total }} 组</span></span>
            <span class="ksu-stat-sub">正在扫描站号范围</span>
          </div>
          <div class="ksu-stat">
            <span class="ksu-stat-label">站号</span>
            <span class="ksu-stat-value ksu-display">{{ discovery.slaveId }}</span>
            <span class="ksu-stat-sub">从站地址</span>
          </div>
          <div class="ksu-stat">
            <span class="ksu-stat-label">波特率</span>
            <span class="ksu-stat-value ksu-display">{{ discovery.baudRate }}<span class="ksu-stat-unit">baud</span></span>
            <span class="ksu-stat-sub">当前探测配置</span>
          </div>
          <div class="ksu-stat">
            <span class="ksu-stat-label">帧格式</span>
            <span class="ksu-stat-value ksu-display">{{ discovery.serialMode ? serialModeLabel(discovery.serialMode) : '—' }}</span>
            <span class="ksu-stat-sub">串口模式</span>
          </div>
        </div>

        <p v-if="discoveryMessage" class="ksu-hint" role="status">{{ discoveryMessage }}</p>

        <div class="ksu-actions">
          <button class="ksu-btn ksu-btn--tonal" :disabled="busy || connected || !!connectionBlockedReason" :title="connectionBlockedReason" @click="discoverConnection">自动查找</button>
          <button v-if="discoveryActive" class="ksu-btn" @click="cancelDiscovery">取消查找</button>
        </div>
      </div>
    </section>
</template>
