<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window';

const props = defineProps<{ title: string; subtitle?: string }>();

const flag = window as unknown as Record<string, unknown>;
const isTauri = 'TauriInternals' in flag || '__TAURI_INTERNALS__' in flag || '__TAURI__' in flag;

async function minWin(): Promise<void> {
  try {
    await getCurrentWindow().minimize();
  } catch {
    /* 浏览器预览：没有原生窗口可收 */
  }
}

async function maxWin(): Promise<void> {
  try {
    await getCurrentWindow().toggleMaximize();
  } catch {
    /* 浏览器预览：没有原生窗口可切 */
  }
}

async function closeWin(): Promise<void> {
  try {
    await getCurrentWindow().close();
  } catch {
    /* 浏览器预览：没有原生窗口可关 */
  }
}
</script>

<template>
  <header class="sa-titlebar">
    <div class="sa-titlebar-drag" data-tauri-drag-region>
      <span class="sa-titlebar-mark" aria-hidden="true">S<span>∿</span></span>
      <span class="sa-titlebar-text" data-tauri-drag-region>
        <strong data-tauri-drag-region>{{ props.title }}</strong>
        <small v-if="props.subtitle" data-tauri-drag-region>{{ props.subtitle }}</small>
      </span>
    </div>
    <div v-if="isTauri" class="sa-titlebar-winctl">
      <button type="button" aria-label="最小化" title="最小化" @click="minWin">─</button>
      <button type="button" aria-label="最大化或还原" title="最大化或还原" @click="maxWin">▢</button>
      <button type="button" aria-label="关闭" title="关闭" class="sa-winctl-close" @click="closeWin">✕</button>
    </div>
  </header>
</template>
