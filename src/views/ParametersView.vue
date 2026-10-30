<script setup lang="ts">
import { useServo } from "../composables/useServo";
const { activePage, applyChanges, busy, canApply, canPersist, changedDifferences, clearComparison, comparison, comparisonById, connected, drafts, exportSnapshot, failedGroups, filteredParameters, groups, handleSnapshotFile, hex, isDirty, persistChanges, profile, query, readAll, riskLabel, selectedBatch, selectedGroup, snapshotExportPath, staleIds, values, writeOne, writeSelectedBatch } = useServo();
</script>

<template>
        <div class="page-stack">
        <section class="panel parameter-panel">
          <div class="parameter-toolbar">
            <div>
              <p class="eyebrow">PARAMETER WORKSPACE</p>
              <h2>参数读取与安全写入</h2>
            </div>
            <div class="toolbar-actions">
              <input v-model="query" class="search" type="search" aria-label="搜索参数" placeholder="搜索参数 ID / 名称… 如 Pn001" autocomplete="off" spellcheck="false" />
              <button class="secondary" :disabled="busy || !connected" @click="readAll()">读取全部</button>
              <button class="secondary" :disabled="busy || !connected || !failedGroups.length" @click="readAll(true)">重读失败 / 未完成组（{{ failedGroups.length }}）</button>
            </div>
          </div>

          <nav class="group-tabs" aria-label="参数分组">
            <button v-for="group in groups" :key="group" :class="{ active: selectedGroup === group }" :aria-pressed="selectedGroup === group" @click="selectedGroup = group">{{ group }}</button>
          </nav>

          <div v-if="comparison.length" class="comparison-bar">
            <span>快照差异 <strong>{{ changedDifferences.length }}</strong> 项；已选 <strong>{{ selectedBatch.length }}</strong> 项</span>
            <div>
              <button class="secondary" @click="clearComparison">取消比较</button>
              <button class="warning" :disabled="busy || !selectedBatch.length" @click="writeSelectedBatch">选择性批量写入</button>
            </div>
          </div>

          <div v-if="!profile" class="workspace-empty"><h3>尚未加载参数定义</h3><p>先导入设备 Profile，再连接模拟器或真实设备读取参数。</p><button class="primary" @click="activePage = 'connection'">前往设备与连接</button></div>
          <div v-else-if="!filteredParameters.length" class="workspace-empty"><h3>没有匹配的参数</h3><p>尝试其他关键词，或清除筛选条件。</p><button @click="query = ''; selectedGroup = '全部'">清除筛选</button></div>
          <div v-else class="table-wrap">
            <table>
              <thead><tr><th scope="col">选择</th><th scope="col">参数</th><th scope="col">当前值</th><th scope="col">写入目标</th><th scope="col">范围</th><th scope="col">风险</th><th scope="col">操作</th></tr></thead>
              <tbody>
                <tr v-for="parameter in filteredParameters" :key="parameter.parameterId" :class="{ dirty: isDirty(parameter) }">
                  <td v-if="comparison.length" class="select-cell">
                    <input v-if="comparisonById.get(parameter.parameterId)?.changed && comparisonById.get(parameter.parameterId)?.writable" v-model="selectedBatch" type="checkbox" :value="parameter.parameterId" />
                    <span v-else-if="comparisonById.get(parameter.parameterId)?.changed" title="只读参数不能批量写入">只读</span>
                    <span v-else>—</span>
                  </td>
                  <td class="parameter-id"><strong>{{ parameter.parameterId }}</strong><code>{{ hex(parameter.address) }}</code></td>
                  <td class="parameter-name"><span>{{ parameter.name }}</span><small v-if="parameter.description">{{ parameter.description }}</small></td>
                  <td class="current-value">
                    <template v-if="values[parameter.parameterId]">{{ values[parameter.parameterId].value }} {{ parameter.unit }} <small v-if="staleIds.has(parameter.parameterId)">（已过期 / 本次未读取）</small></template>
                    <span v-else>未读取</span>
                  </td>
                  <td>
                    <select :aria-label="`${parameter.name} 目标值`" v-if="parameter.enumValues.length" v-model.number="drafts[parameter.parameterId]" :disabled="!connected">
                      <option v-for="choice in parameter.enumValues" :key="choice.value" :value="choice.value">{{ choice.label }}</option>
                    </select>
                    <div v-else class="number-field">
                      <input :aria-label="`${parameter.name} 目标值`" v-model.number="drafts[parameter.parameterId]" type="number" :min="parameter.min" :max="parameter.max" :step="1 / 10 ** parameter.decimals" :disabled="!connected" />
                      <span>{{ parameter.unit }}</span>
                    </div>
                  </td>
                  <td><span class="range">{{ parameter.min }} … {{ parameter.max }}</span></td>
                  <td><span class="risk" :class="parameter.risk">{{ riskLabel(parameter.risk) }}</span></td>
                  <td><button class="write-button" :disabled="busy || !connected || staleIds.has(parameter.parameterId) || !isDirty(parameter)" @click="writeOne(parameter)">写入并回读</button></td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>
          <div class="parameter-utilities">
        <section class="panel snapshot-panel">
          <div class="section-title"><span>SNAP</span><h2>参数快照</h2></div>
          <p>快照保存设备原始值和工程值，可用于备份、差异比较和选择性恢复。</p>
          <button class="secondary full" :disabled="busy || !connected" @click="exportSnapshot">读取并导出快照</button>
          <p>导出时选择保存位置和文件名；导入比较请选择 .servo-snapshot.json 文件。</p>
          <label v-if="snapshotExportPath">最近成功导出的文件（可复制路径）
            <input class="full" readonly :value="snapshotExportPath" aria-label="最近导出的参数快照路径" />
          </label>
          <label class="file-button full" :class="{ disabled: busy || !connected }">
            导入快照并比较
            <input type="file" accept="application/json,.json" :disabled="busy || !connected" @change="handleSnapshotFile" />
          </label>
        </section>
        <section class="panel safety-panel">
          <div class="section-title"><span>WRITE</span><h2>参数事务</h2></div>
          <p>应用和持久化仅在 Profile 明确定义对应命令时可用，并且必须分别确认。</p>
          <button class="secondary full" :disabled="busy || !connected || !canApply" @click="applyChanges">应用参数</button>
          <button class="warning full" :disabled="busy || !connected || !canPersist" @click="persistChanges">持久化参数</button>
        </section>
          </div>
        </div>
</template>
