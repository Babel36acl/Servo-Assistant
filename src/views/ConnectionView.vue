<script setup lang="ts">
import { useServo } from "../composables/useServo";
const { activePage, baudRate, baudRates, busy, cancelDiscovery, connect, connected, connectionBlockedReason, connectionMode, disconnect, discoverConnection, discovery, discoveryActive, discoveryEnd, discoveryMessage, discoveryStart, discoveryTimeout, handleProfileFile, parity, portName, ports, profile, protocol, refreshPorts, serialModeLabel, slaveId, stationMax, stopBits, summary, timeoutMs } = useServo();
</script>

<template>
  <div class="ksu-page">
    <header class="ksu-page-head">
      <div>
        <h1 class="ksu-page-title">设备与连接</h1>
        <p class="ksu-page-sub">导入 Profile，选择模式并建立会话</p>
      </div>
      <div class="ksu-page-actions">
        <span class="ksu-pill" :class="connected ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ connected ? '运行中' : '未连接' }}</span>
      </div>
    </header>

    <div class="ksu-page-body">

        <!-- 状态大卡：设备名 + 连接状态 + 关键信息 + 主操作 -->
      <section class="ksu-card">
        <div class="ksu-card-head">
          <div>
            <h2 class="ksu-card-title">设备状态</h2>
              <p class="ksu-card-sub">模拟器演练或真实 Modbus 串口会话</p>
            </div>
            <div class="ksu-card-actions">
              <span class="ksu-pill" :class="connected ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ connected ? '已连接' : '未连接' }}</span>
            </div>
          </div>
          <div class="ksu-card-body">
            <div>
              <h2 class="ksu-page-title">{{ summary?.deviceName ?? (profile ? '设备 Profile 已加载' : '尚未导入设备 Profile') }}</h2>
              <p class="ksu-hint">
                <template v-if="summary">Profile {{ summary.profileVersion }} · {{ summary.parameterCount }} 参数 · {{ summary.statusCount }} 状态量</template>
                <template v-else-if="profile">Profile 已就绪，选择运行模式后连接。</template>
                <template v-else>导入设备 JSON Profile 后即可建立连接。</template>
              </p>
            </div>
            <div class="ksu-row-actions">
              <span class="ksu-pill" :class="connectionMode === 'serial' ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ connectionMode === 'serial' ? '真实串口' : '配置模拟器' }}</span>
              <span v-if="connectionMode === 'serial'" class="ksu-pill">{{ portName || '未选择串口' }}</span>
              <span class="ksu-pill">站号 {{ slaveId }}</span>
              <span class="ksu-pill">{{ baudRate }} baud</span>
              <span class="ksu-pill">{{ protocol === 'rtu' ? 'RTU' : 'ASCII' }} · 8{{ { none: 'N', even: 'E', odd: 'O' }[parity] }}{{ stopBits }}</span>
            </div>
            <button v-if="!connected" class="ksu-btn ksu-btn--primary ksu-btn--block" :disabled="busy || !!connectionBlockedReason" :title="connectionBlockedReason" @click="connect">连接</button>
            <button v-else class="ksu-btn ksu-btn--danger ksu-btn--block" :disabled="busy" @click="disconnect">断开连接</button>
            <p v-if="!connected && connectionBlockedReason" class="ksu-hint">{{ connectionBlockedReason }}</p>
          </div>
        </section>

      <div class="ksu-columns">
        <div class="ksu-col ksu-col--wide">

        <!-- 设备配置卡：Profile 导入 / 摘要 / 进入参数 -->
        <section class="ksu-card">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">设备配置</h2>
              <p class="ksu-card-sub">JSON Profile 定义参数、寄存器地址与允许的操作</p>
            </div>
            <div class="ksu-card-actions">
              <span class="ksu-pill" :class="profile ? 'ksu-pill--ok' : 'ksu-pill--warn'">{{ profile ? '已加载' : '未加载' }}</span>
              <button v-if="profile" class="ksu-btn ksu-btn--ghost ksu-btn--sm" @click="activePage = 'parameters'">参数工作区</button>
            </div>
          </div>
          <div class="ksu-card-body">
            <div v-if="summary" class="ksu-list">
              <div class="ksu-row ksu-row--static">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">{{ summary.deviceName }}</span>
                  <span class="ksu-row-sub">Profile {{ summary.profileVersion }}</span>
                </div>
              </div>
              <div class="ksu-row ksu-row--static">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">参数与状态量</span>
                  <span class="ksu-row-sub">{{ summary.parameterCount }} 参数 · {{ summary.statusCount }} 状态量</span>
                </div>
              </div>
            </div>
            <div v-else class="ksu-empty">
              <strong>从设备配置开始</strong>
              <p>导入设备 JSON Profile，加载参数定义、寄存器地址和允许的操作。</p>
            </div>
            <div class="button-row">
              <label class="ksu-btn ksu-btn--tonal" :class="{ disabled: busy || connected }">
                导入 JSON
                <input type="file" hidden accept="application/json,.json" :disabled="busy || connected" @change="handleProfileFile" />
              </label>
            </div>
            <p class="ksu-hint">{{ connected ? '连接中不可更换配置。' : '替换配置需要先断开设备。' }}</p>
          </div>
        </section>

        <!-- 自动查找卡 -->
        <section v-if="connectionMode === 'serial'" class="ksu-card">
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

            <div class="button-row">
              <button class="ksu-btn ksu-btn--tonal" :disabled="busy || connected || !!connectionBlockedReason" :title="connectionBlockedReason" @click="discoverConnection">自动查找</button>
              <button v-if="discoveryActive" class="ksu-btn" @click="cancelDiscovery">取消查找</button>
            </div>
          </div>
        </section>
        </div>
        <div class="ksu-col ksu-col--narrow">
            <section class="ksu-card">
              <div class="ksu-card-head">
                <div>
                  <h2 class="ksu-card-title">连接参数</h2>
                  <p class="ksu-card-sub">串口参数需与从站一致</p>
                </div>
              </div>
              <div class="ksu-card-body">
                <div class="ksu-field">
                  <span class="ksu-field-label">运行模式</span>
                  <div class="ksu-segmented" role="group" aria-label="运行模式">
                    <button
                      type="button" class="ksu-segmented-item" :class="{ active: connectionMode === 'simulator' }"
                      :disabled="connected || busy" @click="connectionMode = 'simulator'"
                    >配置模拟器</button>
                    <button
                      type="button" class="ksu-segmented-item" :class="{ active: connectionMode === 'serial' }"
                      :disabled="connected || busy" @click="connectionMode = 'serial'"
                    >真实串口</button>
                  </div>
                </div>

                <div v-if="connectionMode === 'serial'" class="ksu-field">
                  <span class="ksu-field-label">串口</span>
                  <div class="inline-field">
                    <select class="ksu-select" v-model="portName" :disabled="connected || busy">
                      <option value="" disabled>请选择串口</option>
                      <option v-for="port in ports" :key="port.name" :value="port.name">{{ port.name }}</option>
                    </select>
                    <button class="ksu-btn ksu-btn--sm" :disabled="connected || busy" title="刷新串口" aria-label="刷新串口" @click="refreshPorts">刷新</button>
                  </div>
                </div>

                <div class="ksu-form-grid">
                  <label class="ksu-field">
                    <span class="ksu-field-label">站号</span>
                    <input class="ksu-input" v-model.number="slaveId" type="number" min="1" :max="stationMax" :disabled="connected || busy" />
                  </label>
                  <label class="ksu-field">
                    <span class="ksu-field-label">波特率</span>
                    <select class="ksu-select" v-model.number="baudRate" :disabled="connected || busy">
                      <option v-for="baud in baudRates" :key="baud" :value="baud">{{ baud }}</option>
                    </select>
                  </label>
                  <label class="ksu-field">
                    <span class="ksu-field-label">协议</span>
                    <select class="ksu-select" v-model="protocol" :disabled="connected || busy">
                      <option value="rtu">Modbus RTU</option>
                      <option value="ascii">Modbus ASCII</option>
                    </select>
                  </label>
                  <label class="ksu-field">
                    <span class="ksu-field-label">停止位</span>
                    <select class="ksu-select" v-model.number="stopBits" :disabled="connected || busy">
                      <option :value="1">1</option>
                      <option :value="2">2</option>
                    </select>
                  </label>
                  <label class="ksu-field">
                    <span class="ksu-field-label">校验</span>
                    <select class="ksu-select" v-model="parity" :disabled="connected || busy">
                      <option value="none">N · 无校验</option>
                      <option value="even">E · 偶校验</option>
                      <option value="odd">O · 奇校验</option>
                    </select>
                  </label>
                  <label class="ksu-field">
                    <span class="ksu-field-label">超时 ms</span>
                    <input class="ksu-input" v-model.number="timeoutMs" type="number" min="100" max="10000" :disabled="connected || busy" />
                  </label>
                </div>

                <div v-if="!connected && connectionBlockedReason" class="ksu-banner ksu-banner--info" role="status">
                  <div class="ksu-banner-body">
                    <strong>连接条件未满足</strong>
                    <span>{{ connectionBlockedReason }}</span>
                  </div>
                  <div v-if="!profile" class="ksu-banner-actions">
                    <label class="ksu-btn ksu-btn--sm">
                      导入设备 Profile
                      <input type="file" hidden accept="application/json,.json" :disabled="busy" @change="handleProfileFile" />
                    </label>
                  </div>
                </div>
              </div>
            </section>
        </div>
      </div>
    </div>
    <div class="ksu-page-bottom" aria-hidden="true"></div>
  </div>
</template>
