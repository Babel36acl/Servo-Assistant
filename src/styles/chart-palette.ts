/* 图表序列配色唯一来源（数据色，非 UI 铬）。
   取值与 tokens-a.css 的 --sa-chart-* / --sa-hist-* 保持一致，
   改色两边一起改，不要在组件里另起数组。 */

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
