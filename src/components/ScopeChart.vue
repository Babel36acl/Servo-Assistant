<script setup lang="ts">
import { computed } from "vue";

export interface ScopeSeries {
  id: string;
  name: string;
  unit: string;
  color: string;
  values: number[];
}

const props = defineProps<{ series: ScopeSeries[] }>();
const width = 1000;
const height = 230;
const left = 24;
const top = 16;
const plotWidth = 952;
const plotHeight = 178;

const paths = computed(() => props.series.map((series) => {
  const values = series.values;
  const min = values.length ? Math.min(...values) : 0;
  const max = values.length ? Math.max(...values) : 0;
  const span = max - min || 1;
  const points = values.map((value, index) => {
    const x = left + (index / Math.max(values.length - 1, 1)) * plotWidth;
    const y = top + plotHeight - ((value - min) / span) * plotHeight;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(" ");
  return { ...series, min, max, current: values.length ? values[values.length - 1] : null, points };
}));
</script>

<template>
  <div class="scope-chart">
    <svg :viewBox="`0 0 ${width} ${height}`" role="img" aria-label="实时状态曲线">
      <g class="grid">
        <line v-for="row in 5" :key="`h${row}`" :x1="left" :x2="left + plotWidth" :y1="top + (row - 1) * plotHeight / 4" :y2="top + (row - 1) * plotHeight / 4" />
        <line v-for="column in 11" :key="`v${column}`" :x1="left + (column - 1) * plotWidth / 10" :x2="left + (column - 1) * plotWidth / 10" :y1="top" :y2="top + plotHeight" />
      </g>
      <polyline v-for="path in paths" :key="path.id" :points="path.points" :stroke="path.color" />
      <text x="24" y="220">各通道独立缩放 · 最近 300 个采样点</text>
    </svg>
    <p v-if="!series.length" class="scope-empty">连接设备并选择状态通道后显示曲线</p>
    <div v-else class="scope-legend">
      <div v-for="path in paths" :key="path.id">
        <i :style="{ background: path.color }"></i>
        <strong>{{ path.name }}</strong>
        <span>{{ path.current ?? '—' }} {{ path.unit }}</span>
        <small>{{ path.min }} … {{ path.max }}</small>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 颜色只走 tokens-a.css 的 --ksu-*，字号走 --ksu-fs-*（跟随 90/100/110% 字号缩放）。 */
.scope-chart { display: grid; grid-template-columns: minmax(0, 1fr) 230px; gap: 12px; align-items: stretch; }
svg { width: 100%; min-height: 230px; background: var(--ksu-inset); border: 1px solid var(--ksu-divider); border-radius: var(--ksu-radius-12); }
.grid line { stroke: var(--ksu-divider); stroke-width: 1; }
polyline { fill: none; stroke-width: 2; vector-effect: non-scaling-stroke; }
text { fill: var(--ksu-on-surface-variant); font-size: var(--ksu-fs-caption); }
.scope-legend { display: flex; flex-direction: column; gap: 7px; }
.scope-legend > div { display: grid; grid-template-columns: 8px 1fr auto; gap: 7px; align-items: center; padding: 8px; border-radius: var(--ksu-radius-10); background: var(--ksu-inset); }
.scope-legend i { width: 8px; height: 8px; border-radius: 50%; }
.scope-legend strong { font-size: var(--ksu-fs-caption); font-weight: var(--ksu-weight-semibold); }
.scope-legend span { font-size: var(--ksu-fs-caption); font-variant-numeric: tabular-nums; }
.scope-legend small { grid-column: 2 / -1; color: var(--ksu-on-surface-variant); font-size: var(--ksu-fs-caption2); }
@media (max-width: 1180px) { .scope-chart { grid-template-columns: 1fr; } .scope-legend { display: grid; grid-template-columns: repeat(3, 1fr); } }

.scope-empty { align-self: center; padding: var(--ksu-space-6); color: var(--ksu-on-surface-variant); font-size: var(--ksu-fs-caption); line-height: var(--ksu-leading-loose); text-align: center; }
@media (max-width: 640px) { .scope-legend { grid-template-columns: 1fr; } }
</style>
