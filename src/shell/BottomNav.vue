<script setup lang="ts">
import { computed, ref } from 'vue';
import KsuSheet from '../components/ksu/KsuSheet.vue';
import { dockPageIds, morePageIds, morePageSet, pageById, type PageId } from './activities';

/** 窄屏导航：最多 5 项（KSU 规则），4 个主页面 + 「更多」。
 *  贴底普通导航（64 高，图标 26 + 12sp 标签），
 *  「更多」用底部弹层补足诊断 / 审计 / 设置。 */
const props = defineProps<{
  active: PageId;
  badges?: Partial<Record<PageId, string | number>>;
}>();

const emit = defineEmits<{ select: [id: PageId] }>();

const moreOpen = ref(false);

const dockPages = computed(() => dockPageIds.map((id) => pageById(id)));
const morePages = computed(() => morePageIds.map((id) => pageById(id)));
const moreActive = computed(() => morePageSet.has(props.active));
const moreBadgeCount = computed(
  () => morePageIds.filter((id) => props.badges?.[id] !== undefined).length,
);

function badgeOf(id: PageId): string | number | undefined {
  return props.badges?.[id];
}

function pick(id: PageId): void {
  moreOpen.value = false;
  emit('select', id);
}

const moreIcon =
  'M6 12h.01M12 12h.01M18 12h.01';
</script>

<template>
  <nav class="ksu-dock is-bar" aria-label="主导航">
    <button
      v-for="page in dockPages"
      :key="page.id"
      type="button"
      class="ksu-dock-item"
      :class="{ active: page.id === props.active }"
      :aria-current="page.id === props.active ? 'page' : undefined"
      :title="page.label"
      @click="emit('select', page.id)"
    >
      <span class="ksu-dock-icon">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path :d="page.icon" />
        </svg>
      </span>
      <span class="ksu-dock-label">{{ page.label }}</span>
      <span v-if="badgeOf(page.id)" class="ksu-dock-badge">{{ badgeOf(page.id) }}</span>
    </button>

    <button
      type="button"
      class="ksu-dock-item"
      :class="{ active: moreActive }"
      :aria-current="moreActive ? 'page' : undefined"
      :aria-expanded="moreOpen"
      title="更多"
      @click="moreOpen = true"
    >
      <span class="ksu-dock-icon">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path :d="moreIcon" />
        </svg>
      </span>
      <span class="ksu-dock-label">更多</span>
      <span v-if="moreBadgeCount" class="ksu-dock-badge">{{ moreBadgeCount }}</span>
    </button>
  </nav>

  <KsuSheet
    :open="moreOpen"
    variant="bottom"
    title="更多"
    sub="诊断 / 审计 / 设置"
    @close="moreOpen = false"
  >
    <div class="ksu-sheet-list">
      <button
        v-for="page in morePages"
        :key="page.id"
        type="button"
        class="ksu-row ksu-row--clickable"
        :class="{ 'ksu-row--active': page.id === props.active }"
        @click="pick(page.id)"
      >
        <span class="ksu-row-icon">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path :d="page.icon" />
          </svg>
        </span>
        <div class="ksu-row-body">
          <span class="ksu-row-title">{{ page.label }}</span>
          <span class="ksu-row-sub">{{ page.caption }}</span>
        </div>
        <span class="ksu-row-chevron" aria-hidden="true">›</span>
      </button>
    </div>
  </KsuSheet>
</template>
