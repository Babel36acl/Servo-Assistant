<script setup lang="ts">
import { useServo } from "../composables/useServo";
import ScopeChart from "../components/ScopeChart.vue";
const { activePage, communicationError, connected, headlineStatuses, hex, pollingEnabled, sampleInterval, scopeChannels, scopeData, scopeOptions, scopeRunning, scopeSeries, statusUpdatedAt, statuses } = useServo();

function toggleChannel(id: string): void {
  const picked = new Set(scopeChannels.value);
  if (picked.has(id)) picked.delete(id);
  else if (picked.size < 6) picked.add(id);
  scopeChannels.value = [...picked];
}
</script>

<template>
  <div class="ksu-page">
    <header class="ksu-page-head">
      <div>
        <h1 class="ksu-page-title">实时监控</h1>
        <p class="ksu-page-sub">
          更新于 {{ statusUpdatedAt ? new Date(statusUpdatedAt).toLocaleTimeString() : '尚未读取' }} · {{ !connected ? '已断开' : !pollingEnabled ? '轮询已暂停，显示旧值' : communicationError ? '读取失败，显示旧值' : '状态轮询中' }}
        </p>
      </div>
      <div class="ksu-page-actions">
        <span class="ksu-pill" :class="!connected ? 'ksu-pill--muted' : communicationError ? 'ksu-pill--warn' : 'ksu-pill--ok'">
          {{ !connected ? '已断开' : communicationError ? '读取失败' : pollingEnabled ? '轮询中' : '已暂停' }}
        </span>
        <button class="ksu-btn ksu-btn--ghost ksu-btn--sm" @click="activePage = 'diagnostics'">采集设置</button>
      </div>
    </header>

    <div class="ksu-page-body">
      <div class="ksu-grid">
        <section v-for="status in headlineStatuses" :key="status.id" class="ksu-card ksu-card--sm">
          <div class="ksu-card-body">
            <div class="ksu-stat">
              <span class="ksu-stat-label">{{ status.name }}</span>
              <span class="ksu-stat-value ksu-display">{{ status.value }}<span class="ksu-stat-unit">{{ status.unit }}</span></span>
              <span class="ksu-stat-sub mono">{{ hex(status.address ?? 0) }}</span>
            </div>
          </div>
        </section>

        <template v-if="!headlineStatuses.length">
          <section v-for="label in ['实际速度', '实际转矩', '电机电流', '报警代码', '母线电压', '驱动温度']" :key="label" class="ksu-card ksu-card--sm">
            <div class="ksu-card-body">
              <div class="ksu-stat">
                <span class="ksu-stat-label">{{ label }}</span>
                <span class="ksu-stat-value ksu-display">—</span>
                <span class="ksu-stat-sub">{{ statuses.length ? '当前 Profile 未定义' : '等待连接' }}</span>
              </div>
            </div>
          </section>
        </template>

        <section class="ksu-card ksu-span-full">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">实时曲线</h2>
              <p class="ksu-card-sub">{{ scopeRunning ? '采样中 · 每 ' + sampleInterval + ' ms · 保留最近 300 点 · 各通道独立缩放' : '曲线采集已暂停，通讯轮询不受影响' }}</p>
            </div>
            <div class="ksu-card-actions">
              <span class="ksu-pill" :class="scopeRunning ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ scopeRunning ? '采集中' : '已暂停' }}</span>
            </div>
          </div>
          <div class="ksu-chart">
            <ScopeChart :series="scopeSeries" />
          </div>
        </section>

        <section class="ksu-card ksu-span-2">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">曲线通道</h2>
              <p class="ksu-card-sub">最多选择 6 个状态量，各通道独立缩放</p>
            </div>
            <div class="ksu-card-actions">
              <span class="ksu-pill ksu-pill--muted">{{ scopeChannels.length }}/6</span>
            </div>
          </div>
          <div v-if="scopeOptions.length" class="ksu-list">
            <div v-for="channel in scopeOptions" :key="channel.id" class="ksu-row ksu-row--static">
              <div class="ksu-row-body">
                <span class="ksu-row-title">{{ channel.name }}</span>
                <span class="ksu-row-sub mono">{{ hex(channel.address) }} · {{ channel.unit || '无单位' }}</span>
              </div>
              <div class="ksu-row-control">
                <button
                  type="button" class="ksu-switch" role="switch"
                  :aria-checked="scopeChannels.includes(channel.id)"
                  :disabled="scopeChannels.length >= 6 && !scopeChannels.includes(channel.id)"
                  :aria-label="`曲线通道 ${channel.name}`" @click="toggleChannel(channel.id)"
                ><span class="ksu-switch-knob"></span></button>
              </div>
            </div>
          </div>
          <div v-else class="ksu-card-body">
            <p class="ksu-hint">导入并连接设备后，可在此选择曲线通道。</p>
          </div>
        </section>

        <section class="ksu-card">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">采样设置</h2>
              <p class="ksu-card-sub">暂停曲线采集不停止通讯轮询</p>
            </div>
          </div>
          <div class="ksu-card-body">
            <div class="ksu-form-grid">
              <label class="ksu-field">
                <span class="ksu-field-label">采样间隔</span>
                <select class="ksu-select" v-model.number="sampleInterval" :disabled="!connected">
                  <option :value="200">200 ms</option>
                  <option :value="250">250 ms</option>
                  <option :value="500">500 ms</option>
                  <option :value="1000">1 s</option>
                  <option :value="3000">3 s</option>
                  <option v-if="![200, 250, 500, 1000, 3000].includes(sampleInterval)" :value="sampleInterval">{{ sampleInterval }} ms（自定义）</option>
                </select>
              </label>
            </div>
            <div class="ksu-list">
              <div class="ksu-row ksu-row--static">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">曲线采集</span>
                  <span class="ksu-row-sub">{{ scopeRunning ? '按采样间隔持续记录' : '采集已暂停，保留已有点位' }}</span>
                </div>
                <div class="ksu-row-control">
                  <button type="button" class="ksu-switch" role="switch" :aria-checked="scopeRunning" :disabled="!connected" aria-label="曲线采集" @click="scopeRunning = !scopeRunning">
                    <span class="ksu-switch-knob"></span>
                  </button>
                </div>
              </div>
              <div class="ksu-row ksu-row--static">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">清空数据</span>
                  <span class="ksu-row-sub">仅清除本页曲线采样</span>
                </div>
                <div class="ksu-row-control">
                  <button class="ksu-btn ksu-btn--sm" @click="scopeData = {}">清空</button>
                </div>
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>
    <div class="ksu-page-bottom" aria-hidden="true"></div>
  </div>
</template>
