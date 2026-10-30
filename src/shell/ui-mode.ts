/* UI 偏好 —— 响应式单例 store（FlClash Windows 方案）。
   颜色只有深/浅/跟随系统（data-theme），字号缩放 0.9/1/1.1（data-text-scale），
   侧栏展开状态持久化。
   ⚠ 防御：localStorage 键 sa.colorMode / sa.textScale / sa.railExpanded 与
   <html data-theme|data-text-scale> 都被 CSS 直接消费，改名/改属性会让用户偏好
   静默重置或样式整块失效；uiMode 字段仅为旧配置兼容，不再影响视觉。 */

import { computed, reactive, watch } from 'vue';

export type UiMode = 'miuix' | 'material';
export type ColorMode = 'system' | 'light' | 'dark';
export type ThemeName = 'light' | 'dark';
export type TextScale = 0.9 | 1 | 1.1;

const KEY_UI_MODE = 'sa.uiMode';
const KEY_COLOR_MODE = 'sa.colorMode';
const KEY_RAIL_EXPANDED = 'sa.railExpanded';
const KEY_TEXT_SCALE = 'sa.textScale';

const COLOR_MODES: readonly ColorMode[] = ['system', 'light', 'dark'];
const TEXT_SCALES: readonly TextScale[] = [0.9, 1, 1.1];

function readString(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeString(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    /* 隐私模式 / 非浏览器环境：忽略，仅本次会话生效 */
  }
}

function readUiMode(): UiMode {
  // 兼容旧配置：不再影响视觉，只保留字段。
  return readString(KEY_UI_MODE) === 'material' ? 'material' : 'miuix';
}

function readColorMode(): ColorMode {
  const value = readString(KEY_COLOR_MODE);
  return value === 'light' || value === 'dark' ? value : 'system';
}

function readTextScale(): TextScale {
  const value = Number(readString(KEY_TEXT_SCALE));
  return value === 0.9 || value === 1.1 ? value : 1;
}

function readBool(key: string, fallback: boolean): boolean {
  const value = readString(key);
  if (value === '1' || value === 'true') return true;
  if (value === '0' || value === 'false') return false;
  return fallback;
}

/** 系统深色偏好；非浏览器环境按深色处理。 */
const darkMedia: MediaQueryList | null =
  typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia('(prefers-color-scheme: dark)')
    : null;

export const uiStore = reactive({
  /** 兼容字段：FlClash 单视觉体系，不再暴露切换 UI */
  uiMode: readUiMode(),
  colorMode: readColorMode(),
  /** 侧栏展开态（FlClash NavigationSidebar 48 ⇄ 220）；默认展开，侧栏顶部按钮可收起 */
  railExpanded: readBool(KEY_RAIL_EXPANDED, true),
  /** 字号缩放（Apple Dynamic Type 基准 1.0） */
  textScale: readTextScale(),
});

/** 当前实际生效的明暗（colorMode=system 时跟随系统）。 */
export const resolvedTheme = computed<ThemeName>(() =>
  uiStore.colorMode === 'system'
    ? darkMedia?.matches
      ? 'dark'
      : 'light'
    : uiStore.colorMode,
);

/** 把 store 写到 <html>：data-theme 决定明暗，data-text-scale 决定字号缩放。 */
export function applyUiMode(): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  root.dataset.ui = 'flclash';
  root.dataset.theme = resolvedTheme.value;
  root.dataset.textScale = String(uiStore.textScale);
  root.style.colorScheme = resolvedTheme.value;
}

/** 截图/调试用 URL 覆盖：?theme=light&textScale=1.1&rail=1（不改常态逻辑）。 */
function applyUrlOverrides(): void {
  if (typeof window === 'undefined') return;
  const params = new URLSearchParams(window.location.search);
  const theme = params.get('theme');
  if (theme === 'light' || theme === 'dark' || theme === 'system') uiStore.colorMode = theme;
  const scale = Number(params.get('textScale'));
  if (scale === 0.9 || scale === 1.1) uiStore.textScale = scale;
  if (params.get('rail') === '1') uiStore.railExpanded = true;
}

/** 仅供 main.ts 调用：补齐初始 dataset 并挂系统明暗监听。 */
export function initUiMode(): void {
  applyUrlOverrides();
  applyUiMode();
  darkMedia?.addEventListener?.('change', () => {
    if (uiStore.colorMode === 'system') applyUiMode();
  });
}

/** 兼容导出：不再有视觉分支，仅持久化旧字段。 */
export function setUiMode(mode: UiMode): void {
  uiStore.uiMode = mode === 'material' ? 'material' : 'miuix';
  writeString(KEY_UI_MODE, uiStore.uiMode);
}

export function setColorMode(mode: ColorMode): void {
  uiStore.colorMode = COLOR_MODES.includes(mode) ? mode : 'system';
  writeString(KEY_COLOR_MODE, uiStore.colorMode);
  applyUiMode();
}

/** 快捷键：深色 ⇄ 浅色（脱离 system，写入显式值）。 */
export function toggleColorMode(): void {
  setColorMode(resolvedTheme.value === 'dark' ? 'light' : 'dark');
}

export function toggleRailExpanded(): void {
  uiStore.railExpanded = !uiStore.railExpanded;
  writeString(KEY_RAIL_EXPANDED, uiStore.railExpanded ? '1' : '0');
}

export function setRailExpanded(expanded: boolean): void {
  if (uiStore.railExpanded === expanded) return;
  uiStore.railExpanded = expanded;
  writeString(KEY_RAIL_EXPANDED, expanded ? '1' : '0');
}

export function setTextScale(scale: TextScale): void {
  uiStore.textScale = TEXT_SCALES.includes(scale) ? scale : 1;
  writeString(KEY_TEXT_SCALE, String(uiStore.textScale));
  applyUiMode();
}

// 外部直接改 store 字段时保持 dataset 同步；持久化只在 set* 函数里做，
// 这样 ?theme= / ?textScale= 这类截图覆盖不会污染 localStorage。
watch(
  () => uiStore.colorMode,
  () => applyUiMode(),
);
watch(
  () => uiStore.textScale,
  () => applyUiMode(),
);

// 模块加载即把 dataset 写好（main.ts 也会显式 init 一次，幂等）。
applyUiMode();
