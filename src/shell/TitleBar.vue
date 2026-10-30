<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';

/** FlClash Windows 顶部拖拽条（36px）：
 *  - 整条可拖拽（data-tauri-drag-region），双击最大化
 *  - 右上角窗口按钮 44×36（关闭 hover 变红），不占用标题/设置图标
 *  - 浏览器预览下 Tauri API 会抛错，统一 try/catch 兜底 */
const maximized = ref(false);

async function minimize(): Promise<void> {
  try {
    await getCurrentWindow().minimize();
  } catch {
    /* 浏览器预览：无 Tauri 运行时 */
  }
}

async function toggleMaximize(): Promise<void> {
  try {
    const win = getCurrentWindow();
    await win.toggleMaximize();
    maximized.value = await win.isMaximized();
  } catch {
    /* 浏览器预览：无 Tauri 运行时 */
  }
}

async function closeWindow(): Promise<void> {
  try {
    await getCurrentWindow().close();
  } catch {
    /* 浏览器预览：无 Tauri 运行时 */
  }
}

async function syncMaximized(): Promise<void> {
  try {
    maximized.value = await getCurrentWindow().isMaximized();
  } catch {
    maximized.value = false;
  }
}

onMounted(() => {
  void syncMaximized();
  window.addEventListener('resize', syncMaximized);
});

onUnmounted(() => {
  window.removeEventListener('resize', syncMaximized);
});
</script>

<template>
  <header class="ksu-titlebar">
    <div class="ksu-titlebar-drag" data-tauri-drag-region @dblclick="toggleMaximize"></div>

    <div class="ksu-winctl">
      <button type="button" title="最小化" aria-label="最小化" @click="minimize">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M5 12h14" />
        </svg>
      </button>
      <button
        type="button"
        :title="maximized ? '还原' : '最大化'"
        :aria-label="maximized ? '还原窗口' : '最大化窗口'"
        @click="toggleMaximize"
      >
        <svg v-if="!maximized" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M6 6h12v12H6z" />
        </svg>
        <svg v-else viewBox="0 0 24 24" aria-hidden="true">
          <path d="M9 9h10v10H9z" />
          <path d="M5 15V5h10" />
        </svg>
      </button>
      <button type="button" class="ksu-winctl-close" title="关闭" aria-label="关闭窗口" @click="closeWindow">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M6 6l12 12M18 6 6 18" />
        </svg>
      </button>
    </div>
  </header>
</template>
