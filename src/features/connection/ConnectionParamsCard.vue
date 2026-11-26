<script setup lang="ts">
import { computed } from "vue";
import { useServo } from "../../composables/useServo";
import EthercatConnectCard from "../ethercat/EthercatConnectCard.vue";

/** 连接参数卡：连接方式分段 + 各传输专属字段（串口 / BLE / TCP / EtherCAT）。 */
const {
  allowUnsupportedBaud, baudRate, baudRates, bleDeviceAddress, bleDevices, bleScanning, busy,
  connected, connectionBlockedReason, connectionMode, ethercatConnected, handleProfileFile, host,
  parity, port, portName, ports, profile, protocol, refreshBleDevices, refreshPorts, slaveId,
  stationMax, stopBits, timeoutMs,
} = useServo();

const modbusMode = computed(() => connectionMode.value !== "ethercat");
</script>

<template>
  <section class="ksu-card">
    <div class="ksu-card-head">
      <div>
        <h2 class="ksu-card-title">连接参数</h2>
        <p class="ksu-card-sub">{{ connectionMode === 'ethercat' ? '选择专用网卡并接管 EtherCAT 总线' : connectionMode === 'tcp' ? '填写 Modbus TCP 主机与端口' : connectionMode === 'serial' ? '串口参数需与从站一致' : connectionMode === 'ble' ? 'BLE 桥接 485，串口参数随连接下发到桥' : '使用 Profile 默认通讯参数' }}</p>
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
                type="button" class="ksu-segmented-item" :class="{ active: connectionMode === 'ble' }"
                :disabled="connected || busy || ethercatConnected" title="BLE 桥接 485" @click="connectionMode = 'ble'"
                >蓝牙</button>
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
                    <div class="ksu-inline">
                      <select class="ksu-select" v-model="portName" :disabled="connected || busy">
                        <option value="" disabled>请选择串口</option>
                        <option v-for="item in ports" :key="item.name" :value="item.name">{{ item.name }}</option>
                      </select>
                      <button class="ksu-btn ksu-btn--sm" :disabled="connected || busy" title="刷新串口" aria-label="刷新串口" @click="refreshPorts">刷新</button>
                    </div>
                  </div>
                  <div v-else-if="connectionMode === 'ble'" class="ksu-field">
                    <span class="ksu-field-label">蓝牙桥设备</span>
                    <div class="ksu-inline">
                      <select class="ksu-select" v-model="bleDeviceAddress" :disabled="connected || busy">
                        <option value="" disabled>请扫描并选择 ServoBridge</option>
                        <option v-for="item in bleDevices" :key="item.address" :value="item.address">{{ item.name || item.address }}</option>
                      </select>
                      <button class="ksu-btn ksu-btn--sm" :disabled="bleScanning || connected || busy" title="扫描蓝牙桥" aria-label="扫描蓝牙桥" @click="refreshBleDevices">{{ bleScanning ? '扫描中…' : '扫描' }}</button>
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
                    <template v-if="connectionMode === 'serial' || connectionMode === 'ble'">
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

                  <div v-if="connectionMode === 'serial' || connectionMode === 'ble'" class="ksu-list">
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
</template>
