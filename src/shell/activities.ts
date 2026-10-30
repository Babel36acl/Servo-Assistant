/* 一级导航页面表（壳与视图共享的唯一定义）。
   tests/communication.test.mjs 会直接读取本文件并在 vm 里执行，
   因此导出名 pages / PageId / pageById 不可改签名，也不要在此引入运行时依赖。 */

export const pages = [
  { id: 'connection', label: '设备与连接', caption: '导入配置 · 建立连接', icon: 'M7 3v4m10-4v4M5 7h14v4a7 7 0 0 1-14 0V7Zm7 11v3' },
  { id: 'parameters', label: '参数工作区', caption: '读取 · 比较 · 安全写入', icon: 'M4 5h16M4 12h16M4 19h16M9 3v4m6 3v4m-6 3v4' },
  { id: 'monitor', label: '实时监控', caption: '状态量 · 六通道曲线', icon: 'M3 12h4l3-8 4 16-3-8h4' },
  { id: 'diagnostics', label: '通讯诊断', caption: '采集设置 · 稳定性测试', icon: 'M5 20V10m7 10V4m7 16v-7' },
  { id: 'communication', label: '通信工作台', caption: '录制解析 · EtherCAT 主站', icon: 'M3 5h18v12H3zM8 21h8m-4-4v4M7 9h3m4 4h3' },
  { id: 'audit', label: '操作证据', caption: '操作记录 · 结果追溯', icon: 'M6 3h12v18H6zM9 7h6m-6 5h6m-6 5h4' },
  {
    id: 'settings',
    label: '设置',
    caption: '外观 · 关于',
    icon: 'M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1.08-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1.08 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9c.26.604.852.997 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z',
  },
] as const;

export type PageId = (typeof pages)[number]['id'];

/** 宽屏 rail 的全部入口（Miuix NavigationRail 放得下 7 项）。 */
export const railPages = pages;

/** 窄屏底部栏最多 5 项：4 个主页面 + 「更多」浮层。
 *  KSU 规则：bottom bar 不超过 5 项，多了会挤到不可读。 */
export const dockPageIds = ['connection', 'monitor', 'communication', 'parameters'] as const;

/** 「更多」浮层里的次级入口（窄屏借此可达全部 7 页）。 */
export const morePageIds = ['diagnostics', 'audit', 'settings'] as const;

export const morePageSet: ReadonlySet<string> = new Set(morePageIds);

/** 旧壳的分组定义：保留导出兼容旧引用，不再是布局依据。 */
export const navGroups = [
  { label: '工作', ids: ['connection', 'parameters', 'monitor'] },
  { label: '工具', ids: ['diagnostics', 'communication'] },
  { label: '追溯', ids: ['audit'] },
  { label: '应用', ids: ['settings'] },
] as const;

export function pageById(id: string): (typeof pages)[number] {
  return pages.find((page) => page.id === id) ?? pages[0];
}
