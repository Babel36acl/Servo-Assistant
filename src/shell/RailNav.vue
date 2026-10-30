<script setup lang="ts">
import { railPages, type PageId } from './activities';
import { APP_VERSION } from './app-info';
import { setRailExpanded, uiStore } from './ui-mode';

/** 左侧导航栏：仿 FlClash NavigationSidebar（折叠 48 / 展开 220，item 40×40 圆角 10，
 *  图标 24，选中填充 selected-fill + 图标 primary + 左侧 3×16 指示条，底部版本号）。 */
const props = defineProps<{
  active: PageId;
  badges?: Partial<Record<PageId, string | number>>;
}>();

const emit = defineEmits<{ select: [id: PageId] }>();

function toggle(): void {
  setRailExpanded(!uiStore.railExpanded);
}

function badgeOf(id: PageId): string | number | undefined {
  return props.badges?.[id];
}
</script>

<template>
  <nav class="ksu-rail" :class="{ expanded: uiStore.railExpanded }" aria-label="主导航">
    <button
      type="button"
      class="ksu-rail-toggle"
      :aria-label="uiStore.railExpanded ? '收起侧栏' : '展开侧栏'"
      :aria-expanded="uiStore.railExpanded"
      :title="uiStore.railExpanded ? '收起侧栏' : '展开侧栏'"
      @click="toggle"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M4 4h16v16H4zM9 4v16" />
      </svg>
    </button>

    <div class="ksu-rail-items">
      <button
        v-for="page in railPages"
        :key="page.id"
        type="button"
        class="ksu-rail-item"
        :class="{ active: page.id === props.active }"
        :aria-current="page.id === props.active ? 'page' : undefined"
        :title="page.label"
        @click="emit('select', page.id)"
      >
        <span class="ksu-rail-icon">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path :d="page.icon" />
          </svg>
        </span>
        <span class="ksu-rail-label">{{ page.label }}</span>
        <span v-if="badgeOf(page.id)" class="ksu-rail-badge">{{ badgeOf(page.id) }}</span>
      </button>
    </div>

    <div class="ksu-rail-foot">v{{ APP_VERSION }}</div>
  </nav>
</template>
