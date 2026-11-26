<script setup lang="ts">
import { useServo } from "../../composables/useServo";

/** 设备配置卡：JSON Profile 导入 / 摘要 / 跳转参数工作区。 */
const { activePage, busy, connected, handleProfileFile, profile, summary } = useServo();
</script>

<template>
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
      <div class="ksu-actions">
        <label class="ksu-btn ksu-btn--tonal" :class="{ disabled: busy || connected }">
          导入 JSON
          <input type="file" hidden accept="application/json,.json" :disabled="busy || connected" @change="handleProfileFile" />
        </label>
      </div>
      <p class="ksu-hint">{{ connected ? '连接中不可更换配置。' : '替换配置需要先断开设备。' }}</p>
    </div>
  </section>
</template>
