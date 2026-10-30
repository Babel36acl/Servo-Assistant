<script setup lang="ts">
import { computed, ref } from 'vue';
import KsuButton from '../components/ksu/KsuButton.vue';
import KsuCard from '../components/ksu/KsuCard.vue';
import KsuPage from '../components/ksu/KsuPage.vue';
import KsuPill from '../components/ksu/KsuPill.vue';
import KsuRow from '../components/ksu/KsuRow.vue';
import KsuSegmented from '../components/ksu/KsuSegmented.vue';
import KsuSwitch from '../components/ksu/KsuSwitch.vue';
import { APP_REPO, APP_VERSION } from '../shell/app-info';
import {
  resolvedTheme,
  setColorMode,
  setRailExpanded,
  setTextScale,
  toggleFloatingDock,
  uiStore,
  type ColorMode,
  type TextScale,
} from '../shell/ui-mode';

/** 设置页（FlClash 语言）：分组卡 + 设置行，
 *  外观 = 主题模式 / 侧栏展开 / 悬浮底栏 / 字号缩放；关于 = 版本 / 仓库 / 当前主题。
 *  视觉只有一套体系，不再有 Miuix/Material 切换。 */
const themeModeOptions = [
  { value: 'dark', label: '深色' },
  { value: 'light', label: '浅色' },
  { value: 'system', label: '跟随系统' },
] as const;

const textScaleOptions = [
  { value: '0.9', label: '90%' },
  { value: '1', label: '100%' },
  { value: '1.1', label: '110%' },
] as const;

const copied = ref(false);

const themeLabel = computed(() =>
  uiStore.colorMode === 'system' ? '跟随系统' : uiStore.colorMode === 'light' ? '浅色' : '深色',
);

const resolvedLabel = computed(() => (resolvedTheme.value === 'dark' ? '深色' : '浅色'));

const textScaleLabel = computed(() => `${Math.round(uiStore.textScale * 100)}%`);

function onThemeMode(value: string): void {
  setColorMode(value as ColorMode);
}

function onTextScale(value: string): void {
  setTextScale(Number(value) as TextScale);
}

function onRailExpanded(value: boolean): void {
  setRailExpanded(value);
}

function onFloatingDock(value: boolean): void {
  if (value !== uiStore.floatingDock) toggleFloatingDock();
}

async function copyRepo(): Promise<void> {
  try {
    await navigator.clipboard.writeText(APP_REPO);
    copied.value = true;
    window.setTimeout(() => {
      copied.value = false;
    }, 1600);
  } catch {
    copied.value = false;
  }
}
</script>

<template>
  <KsuPage title="设置" sub="外观 · 关于">
    <template #actions>
      <KsuPill tone="muted">{{ themeLabel }} · {{ textScaleLabel }}</KsuPill>
    </template>

    <div class="ksu-grid">
      <KsuCard title="外观" sub="主题与布局即时生效，不重建页面状态" class="ksu-span-2">
        <div class="ksu-list">
          <KsuRow title="主题模式" sub="跟随系统时随系统深浅色变化">
            <template #control>
              <KsuSegmented
                :model-value="uiStore.colorMode"
                :options="themeModeOptions"
                label="主题模式"
                @update:modelValue="onThemeMode"
              />
            </template>
          </KsuRow>

          <KsuRow title="展开侧栏" sub="图标栏展开为图标 + 文字（48 → 220）">
            <template #control>
              <KsuSwitch
                :model-value="uiStore.railExpanded"
                label="展开侧栏"
                @update:modelValue="onRailExpanded"
              />
            </template>
          </KsuRow>

          <KsuRow title="悬浮底栏" sub="开启后宽屏也使用悬浮胶囊底栏（默认关闭，走左侧图标栏）">
            <template #control>
              <KsuSwitch
                :model-value="uiStore.floatingDock"
                label="悬浮底栏"
                @update:modelValue="onFloatingDock"
              />
            </template>
          </KsuRow>

          <KsuRow title="字号缩放" sub="Apple Dynamic Type 基准，整站字号按档位缩放">
            <template #control>
              <KsuSegmented
                :model-value="String(uiStore.textScale)"
                :options="textScaleOptions"
                label="字号缩放"
                @update:modelValue="onTextScale"
              />
            </template>
          </KsuRow>
        </div>
      </KsuCard>

      <KsuCard title="关于" sub="构建信息与开源仓库">
        <div class="ksu-list">
          <KsuRow title="版本" :sub="`Servo-Assistant ${APP_VERSION}`">
            <template #control>
              <KsuPill tone="muted">v{{ APP_VERSION }}</KsuPill>
            </template>
          </KsuRow>

          <KsuRow title="仓库地址" :sub="APP_REPO">
            <template #control>
              <KsuButton sm variant="ghost" @click="copyRepo">{{ copied ? '已复制' : '复制' }}</KsuButton>
            </template>
          </KsuRow>

          <KsuRow title="当前主题" :sub="`选择 ${themeLabel}，实际渲染 ${resolvedLabel}`">
            <template #control>
              <KsuPill>{{ resolvedLabel }}</KsuPill>
            </template>
          </KsuRow>
        </div>
      </KsuCard>
    </div>
  </KsuPage>
</template>
