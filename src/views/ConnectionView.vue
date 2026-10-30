<script setup lang="ts">
import { computed } from "vue";
import { useServo } from "../composables/useServo";
import RegisterScanner from "../features/modbus/RegisterScanner.vue";
import EthercatConnectCard from "../features/ethercat/EthercatConnectCard.vue";
const { activePage, allowUnsupportedBaud, baudRate, baudRates, busy, cancelDiscovery, connect, connected, connectionBlockedReason, connectionMode, disconnect, discoverConnection, discovery, discoveryActive, discoveryBaudRates, discoveryEnd, discoveryMessage, discoveryStart, discoveryTimeout, ethercatConnected, handleProfileFile, host, parity, port, portName, ports, profile, protocol, refreshPorts, selectAllDiscoveryBaudRates, serialModeLabel, slaveId, stationMax, stopBits, summary, timeoutMs, toggleDiscoveryBaud } = useServo();
const modeLabels = { simulator: "配置模拟器", serial: "真实串口", tcp: "Modbus TCP", ethercat: "EtherCAT" } as const;
const modeLabel = computed(() => modeLabels[connectionMode.value]);
const modbusMode = computed(() => connectionMode.value !== "ethercat");
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
              <h2 class="ksu-page-title">{{ summary?.deviceName ?? (profile ? '设备 Profile 已加载' : '尚未导入设备 Profile') }}</h2>
              <p class="ksu-hint">
                <template v-if="summary">Profile {{ summary.profileVersion }} · {{ summary.parameterCount }} 参数 · {{ summary.statusCount }} 状态量</template>
                <template v-else-if="profile">Profile 已就绪，选择连接方式后连接。</template>
                <template v-else>导入设备 JSON Profile 后即可建立连接。</template>
              </p>
            </div>
            <div class="ksu-row-actions">
              <span class="ksu-pill" :class="connectionMode === 'simulator' ? 'ksu-pill--muted' : 'ksu-pill--ok'">{{ modeLabel }}</span>
              <span v-if="connectionMode === 'serial'" class="ksu-pill">{{ portName || '未选择串口' }}</span>
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

            <div class="ksu-field">
              <span class="ksu-field-label">搜索波特率（当前波特率必选，可多选）</span>
              <div class="button-row">
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

            <div class="button-row">
              <button class="ksu-btn ksu-btn--tonal" :disabled="busy || connected || !!connectionBlockedReason" :title="connectionBlockedReason" @click="discoverConnection">自动查找</button>
              <button v-if="discoveryActive" class="ksu-btn" @click="cancelDiscovery">取消查找</button>
            </div>
          </div>
        </section>

        <RegisterScanner />
        </div>
        <div class="ksu-col ksu-col--narrow">
            <section class="ksu-card">
              <div class="ksu-card-head">
                <div>
                  <h2 class="ksu-card-title">连接参数</h2>
                  <p class="ksu-card-sub">{{ connectionMode === 'ethercat' ? '选择专用网卡并接管 EtherCAT 总线' : connectionMode === 'tcp' ? '填写 Modbus TCP 主机与端口' : connectionMode === 'serial' ? '串口参数需与从站一致' : '使用 Profile 默认通讯参数' }}</p>
                </div>
              </div>
              <div class="ksu-card-body">
                <div class="ksu-field">
                  <span class="ksu-field-label">连接方式</span>
                  <div class="ksu-segmented ksu-segmented--wrap" role="group" aria-label="连接方式">
                    <button
                      type="button" class="ksu-segmented-item" :class="{ active: connectionMode === 'simulator' }"
                      :disabled="connected || busy || ethercatConnected" title="配置模拟器" @click="connectionMode = 'simulator'"
                    >模拟器</button>
                    <button
                      type="button" class="ksu-segmented-item" :class="{ active: connectionMode === 'serial' }"
                      :disabled="connected || busy || ethercatConnected" title="Modbus 串口" @click="connectionMode = 'serial'"
                    >串口</button>
                    <button
                      type="button" class="ksu-segmented-item" :class="{ active: connectionMode === 'tcp' }"
                      :disabled="connected || busy || ethercatConnected" title="Modbus TCP" @click="connectionMode = 'tcp'"
                    >TCP</button>
                    <button
                      type="button" class="ksu-segmented-item" :class="{ active: connectionMode === 'ethercat' }"
                      :disabled="connected || busy" title="EtherCAT 主站" @click="connectionMode = 'ethercat'"
                    >EtherCAT</button>
                  </div>
                </div>

                <EthercatConnectCard v-if="connectionMode === 'ethercat'" />
                <template v-else>
                  <div v-if="connectionMode === 'serial'" class="ksu-field">
                    <span class="ksu-field-label">串口</span>
                    <div class="inline-field">
                      <select class="ksu-select" v-model="portName" :disabled="connected || busy">
                        <option value="" disabled>请选择串口</option>
                        <option v-for="item in ports" :key="item.name" :value="item.name">{{ item.name }}</option>
                      </select>
                      <button class="ksu-btn ksu-btn--sm" :disabled="connected || busy" title="刷新串口" aria-label="刷新串口" @click="refreshPorts">刷新</button>
                    </div>
                  </div>
                  <div v-else-if="connectionMode === 'tcp'" class="ksu-form-grid">
                    <label class="ksu-field">
                      <span class="ksu-field-label">主机</span>
                      <input class="ksu-input" v-model="host" :disabled="connected || busy" placeholder="192.168.1.10" aria-label="Modbus TCP 主机" />
                    </label>
                    <label class="ksu-field">
                      <span class="ksu-field-label">端口</span>
                      <input class="ksu-input" v-model.number="port" type="number" min="1" max="65535" :disabled="connected || busy" aria-label="Modbus TCP 端口" />
                    </label>
                  </div>

                  <div class="ksu-form-grid">
                    <label class="ksu-field">
                      <span class="ksu-field-label">站号</span>
                      <input class="ksu-input" v-model.number="slaveId" type="number" min="1" :max="stationMax" :disabled="connected || busy" />
                    </label>
                    <template v-if="connectionMode === 'serial'">
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
                    </template>
                    <label class="ksu-field">
                      <span class="ksu-field-label">超时 ms</span>
                      <input class="ksu-input" v-model.number="timeoutMs" type="number" min="100" max="10000" :disabled="connected || busy" />
                    </label>
                  </div>

                  <div v-if="connectionMode === 'serial'" class="ksu-list">
                    <div class="ksu-row ksu-row--static">
                      <div class="ksu-row-body">
                        <span class="ksu-row-title">扩展波特率</span>
                        <span class="ksu-row-sub">允许连接与查找 Profile 未列出的常用波特率；仅本次应用会话，不修改 Profile</span>
                      </div>
                      <div class="ksu-row-control">
                        <button type="button" class="ksu-switch" role="switch" :aria-checked="allowUnsupportedBaud" :disabled="connected || busy" aria-label="允许 Profile 外波特率" @click="allowUnsupportedBaud = !allowUnsupportedBaud">
                          <span class="ksu-switch-knob"></span>
                        </button>
                      </div>
                    </div>
                  </div>
                </template>

                <div v-if="modbusMode && !connected && connectionBlockedReason" class="ksu-banner ksu-banner--info" role="status">
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
