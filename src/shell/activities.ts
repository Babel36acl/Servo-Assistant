export const pages = [
  { id: 'connection', label: '设备与连接', caption: '导入配置 · 建立连接', icon: 'M7 3v4m10-4v4M5 7h14v4a7 7 0 0 1-14 0V7Zm7 11v3' },
  { id: 'parameters', label: '参数工作区', caption: '读取 · 比较 · 安全写入', icon: 'M4 5h16M4 12h16M4 19h16M9 3v4m6 3v4m-6 3v4' },
  { id: 'monitor', label: '实时监控', caption: '状态量 · 六通道曲线', icon: 'M3 12h4l3-8 4 16-3-8h4' },
  { id: 'diagnostics', label: '通讯诊断', caption: '采集设置 · 稳定性测试', icon: 'M5 20V10m7 10V4m7 16v-7' },
  { id: 'communication', label: '通信工作台', caption: '录制解析 · EtherCAT 主站', icon: 'M3 5h18v12H3zM8 21h8m-4-4v4M7 9h3m4 4h3' },
  { id: 'audit', label: '操作证据', caption: '操作记录 · 结果追溯', icon: 'M6 3h12v18H6zM9 7h6m-6 5h6m-6 5h4' },
] as const;

export type PageId = (typeof pages)[number]['id'];

export const navGroups = [
  { label: '工作', ids: ['connection', 'parameters', 'monitor'] },
  { label: '工具', ids: ['diagnostics', 'communication'] },
  { label: '追溯', ids: ['audit'] },
] as const;

export function pageById(id: string): (typeof pages)[number] {
  return pages.find((page) => page.id === id) ?? pages[0];
}
