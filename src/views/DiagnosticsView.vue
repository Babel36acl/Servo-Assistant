<script setup lang="ts">
import { useServo } from "../composables/useServo";
const { busy, cancelRequested, communicationError, communicationStats, connected, hex, maxRegisters, normalizeSampleInterval, pollingEnabled, probeRange, protocol, readActive, readProgress, retries, runStabilityTest, sampleInterval, saveCommunicationSettings, statusUpdatedAt, testActive, testCycles, testProgress, testResults } = useServo();
</script>

<template>
        <div class="page-stack">
        <section class="panel communication-panel">
          <h2>通讯与采集设置</h2>
          <fieldset :disabled="busy" class="communication-controls">
            <label>校验 / 超时额外重试次数<input v-model.number="retries" type="number" min="0" max="3" /></label>
            <label>每组读取寄存器上限<input v-model.number="maxRegisters" type="number" min="1" max="100" /></label>
            <label>轮询等待间隔（ms）<input v-model.number="sampleInterval" type="number" min="50" max="60000" step="50" @change="normalizeSampleInterval" /></label>
            <label class="toggle-field"><input v-model="pollingEnabled" type="checkbox" /> 状态轮询</label>
            <button class="secondary" @click="saveCommunicationSettings">应用通讯设置</button>
            <label>稳定性测试轮数<input v-model.number="testCycles" type="number" min="1" max="1000" /></label>
            <button class="secondary" :disabled="!connected || probeRange.length < 2" @click="runStabilityTest">短帧 / 长帧只读测试</button>
          </fieldset>
          <p>设置仅保留于当前应用会话。间隔是每次状态读取完成后的等待时间；暂停曲线采集不停止通讯。</p>
          <p>稳定性测试使用 Profile 中最长连续状态区：{{ probeRange.length ? hex(probeRange[0].address) : '—' }}，短帧 1 / 长帧 {{ probeRange.length }} 个寄存器；长帧对照不拆组。执行期间暂停日常轮询。</p>
          <p v-if="communicationError" class="banner error">通讯异常：{{ communicationError }}</p>
          <p v-if="communicationStats">本次串口连接统计（模拟器不计）：读取 {{ communicationStats.transactions }} · 首次成功 {{ communicationStats.firstSuccesses }} · 重试恢复 {{ communicationStats.recovered }} · 最终失败 {{ communicationStats.failed }} · CRC {{ communicationStats.crcErrors }} · LRC {{ communicationStats.lrcErrors }} · 超时 {{ communicationStats.timeouts }} · 重试 {{ communicationStats.retries }}</p>
          <p>状态更新：{{ statusUpdatedAt ? new Date(statusUpdatedAt).toLocaleTimeString() : '尚未读取' }} · {{ !connected ? '已断开' : !pollingEnabled ? '轮询已暂停，保留旧值' : communicationError ? '读取失败，保留旧值' : '轮询已启用' }}</p>
          <p v-if="communicationStats">最近成功：{{ communicationStats.lastSuccessMs ? new Date(communicationStats.lastSuccessMs).toLocaleString() : '—' }}</p>
          <details v-if="communicationStats?.lastFailure"><summary>最近失败地址与响应帧</summary><pre>{{ communicationStats.lastFailure }}</pre></details>
          <p>{{ readProgress }} {{ testProgress }}</p>
          <button v-if="readActive || testActive" class="secondary" :disabled="cancelRequested" @click="cancelRequested = true">{{ cancelRequested ? '正在等待当前事务结束' : '取消当前读取 / 测试' }}</button>
          <table v-if="testResults.length">
            <thead><tr><th>帧</th><th>已测</th><th>首次成功率</th><th>重试恢复</th><th>最终失败</th><th>累计耗时</th></tr></thead>
            <tbody><tr v-for="row in testResults" :key="row.label"><td>{{ row.label }}（{{ protocol === 'ascii' ? 11 + row.count * 4 : 5 + row.count * 2 }} 字节）</td><td>{{ row.total }}</td><td>{{ row.total ? (100 * row.first / row.total).toFixed(1) : '—' }}%</td><td>{{ row.recovered }}</td><td>{{ row.failed }}</td><td>{{ row.elapsed }} ms</td></tr></tbody>
          </table>
        </section>
        </div>
</template>
