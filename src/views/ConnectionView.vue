<script setup lang="ts">
import { useServo } from "../composables/useServo";
const { activePage, baudRate, baudRates, busy, cancelDiscovery, connect, connected, connectionBlockedReason, connectionMode, disconnect, discoverConnection, discovery, discoveryActive, discoveryEnd, discoveryMessage, discoveryStart, discoveryTimeout, handleProfileFile, parity, portName, ports, profile, protocol, refreshPorts, serialModeLabel, slaveId, stationMax, stopBits, summary, timeoutMs } = useServo();
</script>

<template>
        <div class="connection-layout">
        <section class="panel profile-panel">
          <div class="section-title"><span>01</span><h2>设备配置</h2></div>
          <div v-if="summary" class="profile-card">
            <strong>{{ summary.deviceName }}</strong>
            <span>Profile {{ summary.profileVersion }}</span>
            <small>{{ summary.parameterCount }} 参数 · {{ summary.statusCount }} 状态量</small>
          </div>
          <div v-else class="profile-empty"><span class="profile-symbol" aria-hidden="true">{ }</span><h3>从设备配置开始</h3><p>导入设备 JSON Profile，加载参数定义、寄存器地址和允许的操作。</p></div>
          <div class="button-row">
            <label class="file-button" :class="{ disabled: busy || connected }">
              导入 JSON
              <input type="file" accept="application/json,.json" :disabled="busy || connected" @change="handleProfileFile" />
            </label>
          </div>
        <ol class="setup-steps"><li :class="{ complete: profile }"><b>1</b><div><strong>导入设备配置</strong><p>使用对应设备的 JSON Profile</p></div></li><li :class="{ complete: connected }"><b>2</b><div><strong>选择模式并连接</strong><p>可先用模拟器安全演练</p></div></li><li><b>3</b><div><strong>读取后再修改</strong><p>写入前比较，写入后回读</p></div></li></ol>
          <button class="secondary full" :disabled="!profile" @click="activePage = 'parameters'">进入参数工作区 →</button>
        </section>
        <section class="panel connection-panel">
          <div class="section-title"><span>02</span><h2>连接设置</h2></div>
          <label>运行模式
            <select v-model="connectionMode" :disabled="connected || busy">
              <option value="simulator">配置模拟器（安全演练）</option>
              <option value="serial">真实 Modbus 串口</option>
            </select>
          </label>
          <template v-if="connectionMode === 'serial'">
            <label>串口
              <div class="inline-field">
                <select v-model="portName" :disabled="connected || busy">
                  <option value="" disabled>请选择串口</option>
                  <option v-for="port in ports" :key="port.name" :value="port.name">{{ port.name }}</option>
                </select>
                <button class="icon-button" :disabled="connected || busy" title="刷新串口" aria-label="刷新串口" @click="refreshPorts">↻</button>
              </div>
            </label>
          </template>
          <div class="field-grid">
            <label>站号<input v-model.number="slaveId" type="number" min="1" :max="stationMax" :disabled="connected || busy" /></label>
            <label>波特率
              <select v-model.number="baudRate" :disabled="connected || busy">
                <option v-for="baud in baudRates" :key="baud" :value="baud">{{ baud }}</option>
              </select>
            </label>
            <label>协议
              <select v-model="protocol" :disabled="connected || busy"><option value="rtu">Modbus RTU</option><option value="ascii">Modbus ASCII</option></select>
            </label>
            <label>停止位
              <select v-model.number="stopBits" :disabled="connected || busy"><option :value="1">1</option><option :value="2">2</option></select>
            </label>
            <label>校验
              <select v-model="parity" :disabled="connected || busy">
                <option value="none">N · 无校验</option><option value="even">E · 偶校验</option><option value="odd">O · 奇校验</option>
              </select>
            </label>
            <label>超时 ms<input v-model.number="timeoutMs" type="number" min="100" max="10000" :disabled="connected || busy" /></label>
          </div>
          <div v-if="!connected && connectionBlockedReason" class="connection-requirement" role="status">
            <p>{{ connectionBlockedReason }}</p>
            <label v-if="!profile" class="file-button full" :class="{ disabled: busy }">
              导入设备 Profile
              <input type="file" accept="application/json,.json" :disabled="busy" @change="handleProfileFile" />
            </label>
          </div>
          <button v-if="!connected" class="primary full" :disabled="busy || !!connectionBlockedReason" :title="connectionBlockedReason" @click="connect">连接</button>
          <template v-if="connectionMode === 'serial'">
            <fieldset class="discovery-settings" :disabled="busy || connected">
              <legend>自动查找通讯配置</legend>
              <div class="field-grid">
                <label>起始站号<input v-model.number="discoveryStart" type="number" min="1" :max="stationMax" /></label>
                <label>结束站号<input v-model.number="discoveryEnd" type="number" min="1" :max="stationMax" /></label>
                <label>探测超时 ms<input v-model.number="discoveryTimeout" type="number" min="100" max="2000" /></label>
              </div>
              <p class="field-help">优先当前配置，再查找 RTU / ASCII 的 8E1、8N1、8O1。每个配置连续两次只读响应后确认。</p>
              <button class="secondary full" :disabled="!!connectionBlockedReason" :title="connectionBlockedReason" @click="discoverConnection">自动查找</button>
            </fieldset>
            <p v-if="discoveryActive && discovery">{{ discovery.completed }}/{{ discovery.total }} · 站号 {{ discovery.slaveId }} · {{ discovery.baudRate }} baud · {{ discovery.serialMode ? serialModeLabel(discovery.serialMode) : '' }}</p>
            <p v-if="discoveryMessage" class="field-help" role="status">{{ discoveryMessage }}</p>
            <button v-if="discoveryActive" class="secondary full" @click="cancelDiscovery">取消查找</button>
          </template>
          <button v-if="connected" class="danger-outline full" :disabled="busy" @click="disconnect">断开连接</button>
        </section>
        </div>
</template>
