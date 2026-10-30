<script setup lang="ts">
/** FlClash 分组卡：20 圆角（小卡 16），card 级表面，无边框无阴影。
 *  - head 插槽：标题区（默认用 title/sub props，色 onSurfaceVariant）
 *  - actions 插槽：卡头右侧控件
 *  - list 插槽：满宽列表（行自带 16 内距）
 *  - default 插槽：卡体（padding 4 20 20）
 *  - flush：卡体不补水平内边距（列表/指标/图表自撑满宽）
 *  - sm：小卡圆角 16 */
const props = defineProps<{ title?: string; sub?: string; flush?: boolean; sm?: boolean }>();
</script>

<template>
  <section
    class="ksu-card"
    :class="{ 'ksu-card--flush': props.flush, 'ksu-card--sm': props.sm }"
  >
    <div v-if="props.title || props.sub || $slots.head || $slots.actions" class="ksu-card-head">
      <div>
        <slot name="head">
          <h2 v-if="props.title" class="ksu-card-title">{{ props.title }}</h2>
          <p v-if="props.sub" class="ksu-card-sub">{{ props.sub }}</p>
        </slot>
      </div>
      <div v-if="$slots.actions" class="ksu-card-actions">
        <slot name="actions" />
      </div>
    </div>

    <div v-if="$slots.default" class="ksu-card-body">
      <slot />
    </div>

    <slot name="list" />
  </section>
</template>
