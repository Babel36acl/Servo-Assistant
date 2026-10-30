<script setup lang="ts">
import { useServo } from "../composables/useServo";
import ScopeChart from "../components/ScopeChart.vue";
const { activePage, communicationError, connected, headlineStatuses, hex, pollingEnabled, sampleInterval, scopeChannels, scopeData, scopeOptions, scopeRunning, scopeSeries, statusUpdatedAt, statuses } = useServo();
</script>

<template>
        <div class="page-stack">
          <div class="monitor-status"><span>更新于 {{ statusUpdatedAt ? new Date(statusUpdatedAt).toLocaleTimeString() : '尚未读取' }} · {{ !connected ? '已断开' : !pollingEnabled ? '轮询已暂停，显示旧值' : communicationError ? '读取失败，显示旧值' : '状态轮询中' }}</span><button @click="activePage = 'diagnostics'">采集设置 →</button></div>
        <section class="status-strip">
          <article v-for="(status, index) in headlineStatuses" :key="status.id" :style="{ '--i': Math.min(index, 8) }">
            <span>{{ status.name }}</span>
            <strong>{{ status.value }} <small>{{ status.unit }}</small></strong>
            <code>{{ hex(status.address ?? 0) }}</code>
          </article>
          <article v-if="!statuses.length" class="empty-status">
            <span>实时状态</span><strong>—</strong><small>连接后自动刷新</small>
          </article>
        </section>
        <section class="panel scope-panel">
          <div class="scope-toolbar">
            <div>
              <p class="eyebrow">LIVE SCOPE</p>
              <h2>实时曲线</h2>
            </div>
            <div class="scope-actions">
              <label>采样间隔
                <select v-model.number="sampleInterval" :disabled="!connected">
                  <option :value="200">200 ms</option>
                  <option :value="250">250 ms</option>
                  <option :value="500">500 ms</option>
                  <option :value="1000">1 s</option>
                  <option :value="3000">3 s</option>
                  <option v-if="![200,250,500,1000,3000].includes(sampleInterval)" :value="sampleInterval">{{ sampleInterval }} ms（自定义）</option>
                </select>
              </label>
              <button class="secondary" :disabled="!connected" :aria-pressed="scopeRunning" @click="scopeRunning = !scopeRunning">{{ scopeRunning ? '暂停曲线采集' : '继续曲线采集' }}</button>
              <button class="secondary" @click="scopeData = {}">清空</button>
            </div>
          </div>
          <div class="channel-picker">
            <label v-for="channel in scopeOptions" :key="channel.id">
              <input v-model="scopeChannels" type="checkbox" :value="channel.id" :disabled="scopeChannels.length >= 6 && !scopeChannels.includes(channel.id)" />
              {{ channel.name }}
            </label>
          </div>
          <ScopeChart :series="scopeSeries" />
        </section>
        </div>
</template>
