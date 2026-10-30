<script setup lang="ts">
import { pages, type PageId } from './activities';

const props = defineProps<{ active: PageId }>();
const emit = defineEmits<{ select: [id: PageId]; palette: [] }>();
</script>

<template>
  <nav class="sa-activity" aria-label="活动栏">
    <button
      v-for="page in pages"
      :key="page.id"
      type="button"
      :class="{ active: props.active === page.id }"
      :aria-current="props.active === page.id ? 'page' : undefined"
      :title="page.label + ' · ' + page.caption"
      :aria-label="page.label"
      @click="emit('select', page.id)"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true"><path :d="page.icon" /></svg>
    </button>
    <span class="sa-activity-sep" aria-hidden="true"></span>
    <button type="button" title="命令面板 · Ctrl+K" aria-label="打开命令面板" @click="emit('palette')">⌘K</button>
  </nav>
</template>
