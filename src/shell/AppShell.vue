<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import BottomNav from './BottomNav.vue';
import RailNav from './RailNav.vue';
import TitleBar from './TitleBar.vue';
import type { PageId } from './activities';
import { uiStore } from './ui-mode';

/** FlClash 整窗壳：36px 拖拽条 + 主体（侧栏 48/220 + 整页滚动内容）。
 *  页面标题在内容区顶部（各视图自己的 .ksu-page-head），壳里不再有 appbar。
 *  窄屏 <840 走底栏；设置里开启悬浮底栏时宽屏也强制走悬浮坞。 */
const props = defineProps<{
  active: PageId;
  badges?: Partial<Record<PageId, string | number>>;
}>();

const emit = defineEmits<{ select: [id: PageId] }>();

const mainRef = ref<HTMLElement | null>(null);
const isDock = computed(() => uiStore.floatingDock);

watch(
  () => props.active,
  () => {
    mainRef.value?.scrollTo({ top: 0 });
  },
);
</script>

<template>
  <div class="ksu-app" :class="{ 'is-dock': isDock }">
    <a class="ksu-skip-link" href="#main">跳到主内容</a>

    <TitleBar />

    <div class="ksu-body">
      <RailNav :active="props.active" :badges="props.badges" @select="emit('select', $event)" />
      <main id="main" ref="mainRef" class="ksu-main" tabindex="-1">
        <slot />
      </main>
    </div>

    <BottomNav :active="props.active" :badges="props.badges" @select="emit('select', $event)" />
  </div>
</template>
