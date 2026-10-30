<script setup lang="ts">
import TitleBar from './TitleBar.vue';
import ActivityBar from './ActivityBar.vue';
import SideBar from './SideBar.vue';
import StatusBar from './StatusBar.vue';
import type { PageId } from './activities';

const props = defineProps<{ active: PageId; title: string; subtitle?: string }>();
const emit = defineEmits<{ select: [id: PageId]; palette: [] }>();
</script>

<template>
  <div class="sa-shell sa-shard-bg">
    <a class="sa-skip-link" href="#main">跳到主内容</a>
    <TitleBar :title="props.title" :subtitle="props.subtitle" />
    <div class="sa-body">
      <ActivityBar :active="props.active" @select="emit('select', $event)" @palette="emit('palette')" />
      <SideBar :active="props.active" />
      <main id="main" class="sa-main" tabindex="-1"><slot /></main>
    </div>
    <StatusBar @go="emit('select', $event)" @palette="emit('palette')" />
  </div>
</template>
