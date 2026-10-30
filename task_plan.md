# Servo Assistant · FlClash Windows 视觉改版任务计划

> 2026-10-30 方向修正：用户否定了 VSCode 四区壳（P0–P3 旧成果）与 KSU-Miuix 复刻两版，明确「抄 FlClash 的 WIN 方案」。
> 参考：FlClash v0.9.2（https://github.com/chen08209/FlClash，本机已装）；精确规格见 `docs/flclash-win-spec.md`，实拍见 `docs/reference/flclash-windows-dashboard.png`。
> 旧计划（VSCode 壳 + 风格 A + Emil 动效）已作废，保留历史供追溯。

## 目标形态（FlClash Windows）

```
┌──────────────────────────────────────────────┐
│ 36px 拖拽条 ·················· 窗口按钮 ─ □ ✕ │
├────┬─────────────────────────────────────────┤
│48px│ 页面大标题（24/28）        [圆形动作按钮] │
│图标 │ ┌──────────┐ ┌──────────┐ ┌──────────┐ │
│栏   │ │ 卡片 #191A1C            │ │          │ │
│可展 │ └──────────┘ └──────────┘ └──────────┘ │
│220 │ ┌─────────────────────────────────────┐ │
│版本 │ │ 列表/图表大卡                         │ │
└────┴─────────────────────────────────────────┘
```

- 表面四级：page `#000000` / card `#191A1C` / block `#252528` / inset `#2F3134`（深色 pureBlack 实拍）；浅色 `#ECEDF9 / #F9F9FF / #E6E8F3 / #E0E2ED`。
- 主色 `#2F83FF`（深）/`#007AFF`（浅），成功 `#30D158`、警告 `#FF9F0A`、错误 `#FF453A`。
- 字阶：display 40、title1 28、pageTitle 24、title2 22、headline 17（卡片头）、body 17、subheadline 15、footnote 13、caption 12。
- 形状：卡片圆角 20（小卡 16），导航项 10，按钮 pill；无边框、无阴影、靠表面层级区分。
- 栅格：宽 ≥860 四列、<860 两列，单列上限 440，间距 16；整页滚动。

## 结构

- `src/shell/`：AppShell（拖拽条 + sidebar + main）、TitleBar（36px 拖拽条 + 窗口按钮）、RailNav（48/220 图标栏，仿 FlClash NavigationSidebar）、BottomNav（<840 底栏 5 项）、ui-mode（theme/rail/dock/字号缩放）、activities（7 页）。
- `src/styles/`：tokens-a.css（FlClash 两套明暗 + --sa-* 兼容别名）、ksu.css（壳 + 卡片/行/控件/栅格）、views.css（视图微调）、styles.css（旧组件遗留，冻结）。
- `src/components/ksu/`：KsuCard/KsuRow/KsuSwitch/KsuSegmented/KsuButton/KsuInput/KsuPill/KsuBadge/KsuEmpty/KsuSheet/KsuPage。
- `src/views/`：连接 / 参数 / 监控 / 诊断 / 通信 / 审计 / 设置，全部 FlClash 卡片仪表盘化。
- 窗口：`decorations:false` + 自绘拖拽条；Tauri 权限补 `core:window:allow-close/minimize/maximize/unmaximize/toggle-maximize/start-dragging/set-focus/is-maximized/set-title`；minWidth 720 / minHeight 480。

## 阶段

- [x] P0 规格：KSU 源码调研（docs/ksu-ui-spec.md，历史参考）+ FlClash 源码/实拍规格（docs/flclash-win-spec.md）
- [x] P1 窗口关闭修复：capabilities 补窗口写权限；minWidth 降 720
- [x] P2 KSU-Miuix 壳与视图第一版（已被 P3 替换）
- [x] P3 FlClash 壳：拖拽条 + 48/220 侧栏 + 页面内标题 + 四级表面令牌 + 栅格类（task-5）
- [x] P4 FlClash 视图：7 页卡片仪表盘重排（task-6、task-7 修缺）
- [x] P5 验收：lint:tokens / build / test 全绿 + 1440×900 七页 / 浅色 / 800×1100 截图对照 FlClash（Lead 复核）
- [ ] P6 收尾：task_plan/progress/docs 更新，原子化中文提交（进行中）

## 验收

- `pnpm lint:tokens` / `pnpm build` / `pnpm test` 全绿。
- 截图与 `docs/reference/flclash-windows-dashboard.png` 同语言：黑底、#191A1C 卡、蓝色主色、大标题、栅格卡片、行内控件右对齐。
- 窗口 ✕ 可关；1440/900/800 三档无横滚、无重叠、无「堆到一起」。
- 保留全部设备调试功能与 IPC 契约；测试直读的 `src/shell/activities.ts` 导出不变。

## 遇到的错误

| 错误 | 尝试 | 解决 |
|---|---|---|
| 自绘标题栏 ✕ 无效 | 检查 `getCurrentWindow().close()` | `core:window:default` 不含 `allow-close` 等写权限 → capabilities 显式补 9 条 |
| 两版风格反复被否 | KSU 安卓复刻 / Miuix 双皮肤 | 改为 FlClash Windows 单视觉体系（同一家族、现成桌面实现） |
