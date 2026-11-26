<script setup lang="ts">
import { ref } from "vue";
import { timeLabel } from "./format";
import type { RecordItem } from "./types";

/** 报文列表卡：虚拟分页后的可见行 + 空态 / 警告；滚动与聚焦都暂停跟随。
 *  列表滚动容器由父视图通过 listEl 取用（跟随最新时滚到底）。 */
const props = defineProps<{
  rows: RecordItem[];
  total: number | null;
  pageBusy: boolean;
  follow: boolean;
  browseError: string;
  pageWarning: string;
  selected: RecordItem | null;
}>();

const emit = defineEmits<{ select: [record: RecordItem]; pause: [] }>();

const listEl = ref<HTMLElement | null>(null);
defineExpose({ listEl });
</script>

<template>
  <section class="ksu-card ksu-span-full">
    <div class="ksu-card-head">
      <div>
        <h2 class="ksu-card-title">报文列表</h2>
        <p class="ksu-card-sub">丢包、清空缓冲和未完整落盘的数据均不能视为完整总线记录 · {{ props.total === null ? '本页 ' + props.rows.length + ' 条' : '共 ' + total + ' 条匹配' }}</p>
      </div>
      <div class="ksu-card-actions">
        <span v-if="follow" class="ksu-pill ksu-pill--ok">自动跟随</span>
        <span v-if="pageBusy" class="ksu-pill ksu-pill--warn">检索中</span>
      </div>
    </div>
    <div v-if="browseError" class="ksu-banner ksu-banner--error" role="alert">
      <div class="ksu-banner-body"><span>{{ browseError }}</span></div>
    </div>
    <div v-if="pageWarning" class="ksu-banner ksu-banner--info" role="status">
      <div class="ksu-banner-body"><span>{{ pageWarning }}</span></div>
    </div>
    <div ref="listEl" class="ksu-list" @wheel.passive="emit('pause')" @focusin="emit('pause')">
      <button
        v-for="r in props.rows" :key="r.sequence" type="button" class="ksu-row"
        :aria-current="selected === r ? 'true' : undefined" @click="emit('select', r)"
        >
        <div class="ksu-row-body">
          <span class="ksu-row-title mono">#{{ r.sequence }} · {{ r.source }}</span>
          <span class="ksu-row-sub">{{ timeLabel(r.timestampUs) }} · {{ r.protocol }} · {{ r.direction }} / 事务 {{ r.transaction || '—' }}</span>
          <span class="ksu-row-sub">{{ r.bytes.length }} 字节 · {{ r.detail }}</span>
        </div>
        <span class="ksu-row-chevron" aria-hidden="true">›</span>
      </button>
      <div v-if="!props.rows.length" class="ksu-empty">{{ pageBusy ? '正在建立索引或检索报文…' : '没有匹配的报文。打开会话、清除筛选或开启跟随查看新记录。' }}</div>
    </div>
  </section>
</template>
