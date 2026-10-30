<script setup lang="ts">
/** 壳 UI 空态：图标 / 标题 / 说明 / 操作按钮。 */
const props = defineProps<{ title?: string; desc?: string }>();
</script>

<template>
  <div class="ksu-empty">
    <slot name="icon" />
    <strong v-if="props.title">{{ props.title }}</strong>
    <p v-if="props.desc">{{ props.desc }}</p>
    <slot />
  </div>
</template>
