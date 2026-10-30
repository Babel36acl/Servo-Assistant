<script setup lang="ts">
import { useServo } from "../composables/useServo";
const { busy, cancelRequested, communicationError, communicationStats, connected, hex, maxRegisters, normalizeSampleInterval, pollingEnabled, probeRange, protocol, readActive, readProgress, retries, runStabilityTest, sampleInterval, saveCommunicationSettings, statusUpdatedAt, testActive, testCycles, testProgress, testResults } = useServo();
</script>

<template>
  <div class="ksu-page">
    <header class="ksu-page-head">
      <div>
        <h1 class="ksu-page-title">通讯诊断</h1>
        <p class="ksu-page-sub">链路统计与短帧 / 长帧对照</p>
      </div>
      <div class="ksu-page-actions">
        <span class="ksu-pill" :class="!connected ? 'ksu-pill--muted' : communicationError ? 'ksu-pill--bad' : 'ksu-pill--ok'">
          {{ !connected ? '已断开' : communicationError ? '通讯异常' : '通讯正常' }}
        </span>
      </div>
    </header>

    <div class="ksu-page-body">

        <section class="ksu-card">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">通讯状态</h2>
              <p class="ksu-card-sub">
                状态更新：{{ statusUpdatedAt ? new Date(statusUpdatedAt).toLocaleTimeString() : '尚未读取' }} · {{ !connected ? '已断开' : !pollingEnabled ? '轮询已暂停，保留旧值' : communicationError ? '读取失败，保留旧值' : '轮询已启用' }}
              </p>
            </div>
            <div class="ksu-card-actions">
              <span class="ksu-pill" :class="communicationError ? 'ksu-pill--bad' : connected ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ communicationError ? '异常' : connected ? '正常' : '未连接' }}</span>
            </div>
          </div>
          <div class="ksu-card-body">
            <div v-if="communicationError" class="ksu-banner ksu-banner--error" role="alert">
              <div class="ksu-banner-body"><strong>通讯异常</strong><span>{{ communicationError }}</span></div>
            </div>
            <div class="ksu-list">
              <div class="ksu-row ksu-row--static">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">状态轮询</span>
                  <span class="ksu-row-sub">{{ pollingEnabled ? '按间隔读取状态量' : '轮询已暂停，保留旧值' }}</span>
                </div>
                <span class="ksu-pill" :class="pollingEnabled && connected ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ pollingEnabled && connected ? '运行中' : '暂停' }}</span>
              </div>
              <div class="ksu-row ksu-row--static">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">最近成功</span>
                  <span class="ksu-row-sub">{{ communicationStats?.lastSuccessMs ? new Date(communicationStats.lastSuccessMs).toLocaleString() : '—' }}</span>
                </div>
              </div>
              <div v-if="communicationStats" class="ksu-row ksu-row--static">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">串口统计</span>
                  <span class="ksu-row-sub">读取 {{ communicationStats.transactions }} · 首次成功 {{ communicationStats.firstSuccesses }} · 重试恢复 {{ communicationStats.recovered }} · 最终失败 {{ communicationStats.failed }}</span>
                </div>
              </div>
            </div>
            <details v-if="communicationStats?.lastFailure" class="ksu-field">
              <summary class="ksu-field-label">最近失败地址与响应帧</summary>
              <pre class="mono">{{ communicationStats.lastFailure }}</pre>
            </details>
            <p v-if="!communicationStats" class="ksu-hint">连接真实串口后显示读取统计；模拟器不产生串口统计。</p>
          </div>
        </section>
      <div class="ksu-columns">
        <div class="ksu-col ksu-col--wide">

        <section class="ksu-card">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">采集设置</h2>
              <p class="ksu-card-sub">仅保留于当前应用会话</p>
            </div>
          </div>
          <div class="ksu-card-body">
            <div class="ksu-form-grid">
              <label class="ksu-field">
                <span class="ksu-field-label">校验 / 超时额外重试次数</span>
                <input class="ksu-input" v-model.number="retries" type="number" min="0" max="3" />
              </label>
              <label class="ksu-field">
                <span class="ksu-field-label">每组读取寄存器上限</span>
                <input class="ksu-input" v-model.number="maxRegisters" type="number" min="1" max="100" />
              </label>
              <label class="ksu-field">
                <span class="ksu-field-label">轮询等待间隔（ms）</span>
                <input class="ksu-input" v-model.number="sampleInterval" type="number" min="50" max="60000" step="50" @change="normalizeSampleInterval" />
              </label>
            </div>
            <div class="ksu-list">
              <div class="ksu-row ksu-row--static">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">状态轮询</span>
                  <span class="ksu-row-sub">暂停曲线采集不会停止通讯轮询</span>
                </div>
                <div class="ksu-row-control">
                  <button type="button" class="ksu-switch" role="switch" :aria-checked="pollingEnabled" aria-label="状态轮询" @click="pollingEnabled = !pollingEnabled">
                    <span class="ksu-switch-knob"></span>
                  </button>
                </div>
              </div>
            </div>
            <div class="button-row">
              <button class="ksu-btn ksu-btn--tonal" @click="saveCommunicationSettings">应用通讯设置</button>
            </div>
          </div>
        </section>
        </div>
        <div class="ksu-col ksu-col--narrow">

        <section class="ksu-card">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">稳定性测试</h2>
              <p class="ksu-card-sub">使用 Profile 中最长连续状态区，短帧 / 长帧只读对照，长帧不拆组；执行期间暂停日常轮询</p>
            </div>
            <div class="ksu-card-actions">
              <span class="ksu-pill" :class="testActive ? 'ksu-pill--warn' : 'ksu-pill--muted'">{{ testActive ? '测试中' : '未测试' }}</span>
            </div>
          </div>
          <div class="ksu-card-body">
            <div class="ksu-form-grid">
              <label class="ksu-field">
                <span class="ksu-field-label">测试轮数</span>
                <input class="ksu-input" v-model.number="testCycles" type="number" min="1" max="1000" />
              </label>
            </div>
            <p class="ksu-hint">
              探测区间：{{ probeRange.length ? hex(probeRange[0].address) : '—' }}，短帧 1 / 长帧 {{ probeRange.length }} 个寄存器。
              {{ protocol === 'ascii' ? 'ASCII' : 'RTU' }} 帧长按当前协议换算。
            </p>
            <div class="button-row">
              <button class="ksu-btn ksu-btn--tonal" :disabled="!connected || probeRange.length < 2 || busy" @click="runStabilityTest">短帧 / 长帧只读测试</button>
              <button v-if="readActive || testActive" class="ksu-btn" :disabled="cancelRequested" @click="cancelRequested = true">{{ cancelRequested ? '正在等待当前事务结束' : '取消当前读取 / 测试' }}</button>
            </div>
            <p v-if="readProgress || testProgress" class="ksu-hint" role="status">{{ readProgress }} {{ testProgress }}</p>

            <div v-if="testResults.length" class="ksu-list">
              <div v-for="row in testResults" :key="row.label" class="ksu-row ksu-row--static">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">{{ row.label }} · {{ protocol === 'ascii' ? 11 + row.count * 4 : 5 + row.count * 2 }} 字节</span>
                  <span class="ksu-row-sub">已测 {{ row.total }} · 重试恢复 {{ row.recovered }} · 最终失败 {{ row.failed }} · 累计 {{ row.elapsed }} ms</span>
                </div>
                <div class="ksu-row-control">
                  <span class="ksu-pill" :class="row.failed ? 'ksu-pill--warn' : 'ksu-pill--ok'">
                    {{ row.total ? (100 * row.first / row.total).toFixed(1) : '—' }}% 首次成功
                  </span>
                </div>
              </div>
            </div>
            <div v-else class="ksu-empty">尚无测试结果，连接设备后运行一次只读对照。</div>
          </div>
        </section>
        </div>
      </div>

        <section class="ksu-card">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">通讯统计</h2>
              <p class="ksu-card-sub">本次串口连接累计（模拟器不计）</p>
            </div>
            <div class="ksu-card-actions">
              <span v-if="communicationStats" class="ksu-pill ksu-pill--muted">读取 {{ communicationStats.transactions }} 次</span>
            </div>
          </div>
          <div v-if="communicationStats" class="ksu-card-body">
            <div class="ksu-grid">
              <div class="ksu-stat">
                <span class="ksu-stat-label">读取事务</span>
                <span class="ksu-stat-value ksu-display">{{ communicationStats.transactions }}</span>
                <span class="ksu-stat-sub">总请求数</span>
              </div>
              <div class="ksu-stat">
                <span class="ksu-stat-label">首次成功</span>
                <span class="ksu-stat-value ksu-display">{{ communicationStats.firstSuccesses }}</span>
                <span class="ksu-stat-sub">无需重试</span>
              </div>
              <div class="ksu-stat">
                <span class="ksu-stat-label">重试恢复</span>
                <span class="ksu-stat-value ksu-display">{{ communicationStats.recovered }}</span>
                <span class="ksu-stat-sub">重试后成功</span>
              </div>
              <div class="ksu-stat">
                <span class="ksu-stat-label">最终失败</span>
                <span class="ksu-stat-value ksu-display">{{ communicationStats.failed }}</span>
                <span class="ksu-stat-sub">重试耗尽</span>
              </div>
              <div class="ksu-stat">
                <span class="ksu-stat-label">CRC 错误</span>
                <span class="ksu-stat-value ksu-display">{{ communicationStats.crcErrors }}</span>
                <span class="ksu-stat-sub">RTU 帧校验</span>
              </div>
              <div class="ksu-stat">
                <span class="ksu-stat-label">LRC 错误</span>
                <span class="ksu-stat-value ksu-display">{{ communicationStats.lrcErrors }}</span>
                <span class="ksu-stat-sub">ASCII 帧校验</span>
              </div>
              <div class="ksu-stat">
                <span class="ksu-stat-label">超时</span>
                <span class="ksu-stat-value ksu-display">{{ communicationStats.timeouts }}</span>
                <span class="ksu-stat-sub">无响应</span>
              </div>
              <div class="ksu-stat">
                <span class="ksu-stat-label">重试次数</span>
                <span class="ksu-stat-value ksu-display">{{ communicationStats.retries }}</span>
                <span class="ksu-stat-sub">累计重发</span>
              </div>
            </div>
          </div>
          <div v-else class="ksu-empty">连接真实串口后显示读取统计；模拟器不产生串口统计。</div>
        </section>
    </div>
    <div class="ksu-page-bottom" aria-hidden="true"></div>
  </div>
</template>
