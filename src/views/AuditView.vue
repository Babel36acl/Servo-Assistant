<script setup lang="ts">
import { computed, ref } from "vue";
import { useServo } from "../composables/useServo";

const { audit, formatTimestamp } = useServo();

const query = ref("");
const statusFilter = ref<"all" | "success" | "failed">("all");

const filtered = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  return [...audit.value]
    .reverse()
    .filter((entry) => {
      const ok = entry.status === "success" || entry.status === "completed";
      if (statusFilter.value === "success" && !ok) return false;
      if (statusFilter.value === "failed" && ok) return false;
      if (!keyword) return true;
      return `${entry.action} ${entry.detail} ${entry.status}`.toLowerCase().includes(keyword);
    })
    .slice(0, 20);
});
</script>

<template>
  <div class="ksu-page">
    <header class="ksu-page-head">
      <div>
        <h1 class="ksu-page-title">操作证据</h1>
        <p class="ksu-page-sub">操作记录与结果追溯</p>
      </div>
      <div class="ksu-page-actions">
        <span class="ksu-pill ksu-pill--muted">{{ audit.length }} 条</span>
      </div>
    </header>

    <div class="ksu-page-body">
      <div class="ksu-grid">
        <section class="ksu-card ksu-span-full">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">筛选</h2>
              <p class="ksu-card-sub">按动作、详情或结果过滤最近记录</p>
            </div>
            <div class="ksu-card-actions">
              <span class="ksu-pill ksu-pill--muted">{{ filtered.length }} 条匹配</span>
            </div>
          </div>
          <div class="ksu-card-body">
            <div class="ksu-form-grid">
              <label class="ksu-field">
                <span class="ksu-field-label">搜索</span>
                <input v-model="query" class="ksu-input" type="search" placeholder="动作 / 详情 / 结果" aria-label="搜索操作记录" autocomplete="off" spellcheck="false" />
              </label>
              <label class="ksu-field">
                <span class="ksu-field-label">结果</span>
                <select v-model="statusFilter" class="ksu-select" aria-label="按结果筛选">
                  <option value="all">全部记录</option>
                  <option value="success">仅成功 / 完成</option>
                  <option value="failed">仅失败</option>
                </select>
              </label>
            </div>
          </div>
        </section>

        <section class="ksu-card ksu-span-full">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">最近操作</h2>
              <p class="ksu-card-sub">按时间倒序，最多显示 20 条</p>
            </div>
            <div class="ksu-card-actions">
              <span class="ksu-pill ksu-pill--muted">{{ filtered.length }} 条</span>
            </div>
          </div>
          <div v-if="filtered.length" class="ksu-list">
            <div
              v-for="entry in filtered"
              :key="`${entry.timestampMs}-${entry.action}`"
              class="ksu-row ksu-row--static"
            >
              <div class="ksu-row-body">
                <span class="ksu-row-title">{{ entry.action === 'profile.import' && entry.status === 'success' ? '设备配置已导入' : entry.detail }}</span>
                <span class="ksu-row-sub"><code>{{ entry.action }}</code> · {{ formatTimestamp(entry.timestampMs) }}</span>
              </div>
              <div class="ksu-row-control">
                <span class="ksu-pill" :class="entry.status === 'success' || entry.status === 'completed' ? 'ksu-pill--ok' : 'ksu-pill--bad'">
                  {{ entry.status === 'success' ? '成功' : entry.status === 'completed' ? '完成' : entry.status === 'failed' || entry.status === 'failure' ? '失败' : entry.status }}
                </span>
              </div>
            </div>
          </div>
          <div v-else class="ksu-empty">
            <strong>没有匹配的操作记录</strong>
            <p>{{ audit.length ? '尝试清除筛选条件。' : '尚无操作记录。' }}</p>
            <button v-if="query || statusFilter !== 'all'" class="ksu-btn" @click="query = ''; statusFilter = 'all'">清除筛选</button>
          </div>
        </section>
      </div>
    </div>
    <div class="ksu-page-bottom" aria-hidden="true"></div>
  </div>
</template>
