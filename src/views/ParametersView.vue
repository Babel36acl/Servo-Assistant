<script setup lang="ts">
import { useServo } from "../composables/useServo";
const { activePage, applyChanges, busy, canApply, canPersist, changedDifferences, clearComparison, comparison, comparisonById, connected, drafts, exportSnapshot, failedGroups, filteredParameters, groups, handleSnapshotFile, hex, isDirty, persistChanges, profile, query, readAll, riskLabel, selectedBatch, selectedGroup, snapshotExportPath, staleIds, values, writeOne, writeSelectedBatch } = useServo();
</script>

<template>
  <div class="ksu-page">
    <header class="ksu-page-head">
      <div>
        <h1 class="ksu-page-title">参数工作区</h1>
        <p class="ksu-page-sub">逐项写入并立即回读校验</p>
      </div>
      <div class="ksu-page-actions">
        <input v-model="query" class="ksu-input" type="search" aria-label="搜索参数" placeholder="搜索参数 ID / 名称… 如 Pn001" autocomplete="off" spellcheck="false" />
        <span class="ksu-pill" :class="connected ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ connected ? '已连接' : '未连接' }}</span>
      </div>
    </header>

    <div class="ksu-page-body">
      <div class="ksu-grid">
        <section class="ksu-card ksu-span-full">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">参数表</h2>
              <p class="ksu-card-sub">
                <template v-if="profile">{{ filteredParameters.length }} 项 · 写前比较，写后回读</template>
                <template v-else>尚未加载参数定义</template>
              </p>
            </div>
            <div class="ksu-card-actions">
              <button class="ksu-btn ksu-btn--sm ksu-btn--tonal" :disabled="busy || !connected" @click="readAll()">读取全部</button>
              <button class="ksu-btn ksu-btn--sm" :disabled="busy || !connected || !failedGroups.length" @click="readAll(true)">重读失败（{{ failedGroups.length }}）</button>
            </div>
          </div>

          <div v-if="profile" class="ksu-card-body">
            <nav class="ksu-tabs" aria-label="参数分组">
              <button
                v-for="group in groups" :key="group" type="button"
                class="ksu-tab" :class="{ active: selectedGroup === group }"
                :aria-pressed="selectedGroup === group" @click="selectedGroup = group"
              >{{ group }}</button>
            </nav>
          </div>

          <div v-if="comparison.length" class="ksu-banner ksu-banner--info" role="status">
            <div class="ksu-banner-body">
              <strong>快照差异 {{ changedDifferences.length }} 项</strong>
              <span>已选 {{ selectedBatch.length }} 项将按当前值选择性写入；只读参数不可批量写入</span>
            </div>
            <div class="ksu-banner-actions">
              <button class="ksu-btn ksu-btn--sm" @click="clearComparison">取消比较</button>
              <button class="ksu-btn ksu-btn--sm ksu-btn--primary" :disabled="busy || !selectedBatch.length" @click="writeSelectedBatch">选择性批量写入</button>
            </div>
          </div>

          <div v-if="!profile" class="ksu-empty">
            <strong>尚未加载参数定义</strong>
            <p>先导入设备 Profile，再连接模拟器或真实设备读取参数。</p>
            <button class="ksu-btn ksu-btn--primary" @click="activePage = 'connection'">前往设备与连接</button>
          </div>
          <div v-else-if="!filteredParameters.length" class="ksu-empty">
            <strong>没有匹配的参数</strong>
            <p>尝试其他关键词，或清除筛选条件。</p>
            <button class="ksu-btn" @click="query = ''; selectedGroup = '全部'">清除筛选</button>
          </div>
          <div v-else class="ksu-list">
            <div v-for="parameter in filteredParameters" :key="parameter.parameterId" class="ksu-row ksu-row--static">
              <div class="ksu-row-body">
                <span class="ksu-row-title">
                  {{ parameter.name }}
                  <span v-if="isDirty(parameter)" class="ksu-pill ksu-pill--warn">已修改</span>
                  <span v-if="staleIds.has(parameter.parameterId)" class="ksu-pill ksu-pill--muted">未读取</span>
                </span>
                <span class="ksu-row-sub mono">{{ parameter.parameterId }} · {{ hex(parameter.address) }} · {{ parameter.min }} … {{ parameter.max }} {{ parameter.unit }}</span>
                <span class="ksu-row-sub">
                  当前
                  <template v-if="values[parameter.parameterId]"><b class="mono">{{ values[parameter.parameterId].value }} {{ parameter.unit }}</b></template>
                  <template v-else>—</template>
                  · {{ riskLabel(parameter.risk) }}
                  <template v-if="parameter.description"> · {{ parameter.description }}</template>
                </span>
              </div>
              <div class="ksu-row-control">
                <input
                  v-if="comparison.length && comparisonById.get(parameter.parameterId)?.changed && comparisonById.get(parameter.parameterId)?.writable"
                  v-model="selectedBatch" type="checkbox" :value="parameter.parameterId" :aria-label="`选择 ${parameter.name}`"
                />
                <span v-else-if="comparison.length && comparisonById.get(parameter.parameterId)?.changed" class="ksu-pill ksu-pill--muted">只读</span>
                <select v-if="parameter.enumValues.length" class="ksu-select" :aria-label="`${parameter.name} 目标值`" v-model.number="drafts[parameter.parameterId]" :disabled="!connected">
                  <option v-for="choice in parameter.enumValues" :key="choice.value" :value="choice.value">{{ choice.label }}</option>
                </select>
                <input v-else class="ksu-input" :aria-label="`${parameter.name} 目标值`" v-model.number="drafts[parameter.parameterId]" type="number" :min="parameter.min" :max="parameter.max" :step="1 / 10 ** parameter.decimals" :disabled="!connected" />
                <button class="ksu-btn ksu-btn--sm" :disabled="busy || !connected || staleIds.has(parameter.parameterId) || !isDirty(parameter)" @click="writeOne(parameter)">写入并回读</button>
              </div>
            </div>
          </div>
        </section>

        <section class="ksu-card ksu-span-2">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">参数快照</h2>
              <p class="ksu-card-sub">保存设备原始值和工程值，用于备份、差异比较和选择性恢复</p>
            </div>
            <div class="ksu-card-actions">
              <span class="ksu-pill" :class="comparison.length ? 'ksu-pill--warn' : 'ksu-pill--muted'">{{ comparison.length ? '比较中' : '未比较' }}</span>
            </div>
          </div>
          <div class="ksu-card-body">
            <div class="ksu-list">
              <div class="ksu-row ksu-row--static">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">读取并导出快照</span>
                  <span class="ksu-row-sub">选择保存位置和文件名，读取当前参数值</span>
                </div>
                <div class="ksu-row-control">
                  <button class="ksu-btn ksu-btn--sm ksu-btn--tonal" :disabled="busy || !connected" @click="exportSnapshot">导出</button>
                </div>
              </div>
              <label class="ksu-row">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">导入快照并比较</span>
                  <span class="ksu-row-sub">选择 .servo-snapshot.json 文件与当前值做差异</span>
                </div>
                <div class="ksu-row-control">
                  <span class="ksu-btn ksu-btn--sm">选择文件</span>
                  <input type="file" hidden accept="application/json,.json" :disabled="busy || !connected" @change="handleSnapshotFile" />
                </div>
              </label>
            </div>
            <label v-if="snapshotExportPath" class="ksu-field">
              <span class="ksu-field-label">最近成功导出的文件（可复制路径）</span>
              <input class="ksu-input" readonly :value="snapshotExportPath" aria-label="最近导出的参数快照路径" />
            </label>
          </div>
        </section>

        <section class="ksu-card">
          <div class="ksu-card-head">
            <div>
              <h2 class="ksu-card-title">参数事务</h2>
              <p class="ksu-card-sub">仅在 Profile 定义对应命令时可用，且需输入确认短语</p>
            </div>
          </div>
          <div class="ksu-card-body">
            <div class="ksu-list">
              <div class="ksu-row ksu-row--static">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">应用参数</span>
                  <span class="ksu-row-sub">让已写入参数在设备上生效</span>
                </div>
                <div class="ksu-row-control">
                  <button class="ksu-btn ksu-btn--sm ksu-btn--tonal" :disabled="busy || !connected || !canApply" @click="applyChanges">应用</button>
                </div>
              </div>
              <div class="ksu-row ksu-row--danger">
                <div class="ksu-row-body">
                  <span class="ksu-row-title">持久化参数</span>
                  <span class="ksu-row-sub">覆盖旧配置，操作后需重新上电确认</span>
                </div>
                <div class="ksu-row-control">
                  <button class="ksu-btn ksu-btn--sm ksu-btn--danger" :disabled="busy || !connected || !canPersist" @click="persistChanges">持久化</button>
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
