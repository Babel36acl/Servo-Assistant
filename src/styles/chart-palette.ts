/* 图表序列配色唯一来源（数据色，非 UI 铬）。
   这是唯一允许写 hex 的 JS 文件（lint:tokens 白名单）；
   数据序列色不进 tokens-a.css，组件里不要在本地另起数组。 */

export const CHART_COLORS: readonly string[] = [
  '#eba937',
  '#8ab4f8',
  '#4cc38a',
  '#ef6461',
  '#c792ea',
  '#f28bc1',
];

export const HISTORY_COLORS: readonly string[] = [
  '#45d4b0',
  '#f7b955',
  '#73a9ff',
  '#d59aff',
];
