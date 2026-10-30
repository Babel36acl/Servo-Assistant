# Servo Assistant · 界面改版任务计划

> 视觉基准：**FlClash Windows 方案**（`docs/flclash-win-spec.md`，实拍 `docs/reference/flclash-windows-dashboard.png`）。
> 改界面前先读 `docs/ui.md`（结构、入口、不能破坏的契约）。

## 目标形态

```
┌──────────────────────────────────────────────┐
│ 36px 拖拽条 ·················· 窗口按钮 ─ □ ✕ │
├──────┬───────────────────────────────────────┤
│ 图标栏│ 页面大标题（24/28）      [动作区]       │
│ 220px│ ┌──────────┐ ┌──────────┐ ┌─────────┐ │
│ 可收 │ │ 卡片 #191A1C            │ │         │ │
│ 48px │ └──────────┘ └──────────┘ └─────────┘ │
│ 版本  │ ┌───────────────────────────────────┐ │
└──────┴─┴───────────────────────────────────┴─┘
```

- 表面四级：page `#000` / card `#191A1C` / block `#252528` / inset `#2F3134`；浅色 `#ECEDF9 / #F9F9FF / #E6E8F3 / #E0E2ED`。
- 主色 `#2F83FF`（深）/`#007AFF`（浅）；成功 `#30D158`、警告 `#FF9F0A`、错误 `#FF453A`。
- 字阶（Apple Dynamic Type）：display 40、title1 28、pageTitle 24、headline 17、body 17、subheadline 15、footnote 13、caption 12；支持 90/100/110% 缩放。
- 卡片圆角 20、无边框无阴影；栅格间距 16；`<840px` 侧栏收为底部 5 项导航。

## 已完成

- [x] 令牌：tokens-a.css 重写为 FlClash 四级表面 + Dynamic Type 字阶 + 字号缩放
- [x] 壳：36px 自绘拖拽条（右上窗口按钮）、图标栏（默认展开 220，可收 48）、窄屏底部导航；旧应用栏/状态栏/命令面板/二级侧栏删除
- [x] 七页视图卡片化（连接/参数/监控/诊断/通信/审计/设置），表格改 `.ksu-list` 行卡，全局提示改右下 toast
- [x] 窗口关闭修复：`core:window:default` 缺写权限 → capabilities 显式补 9 条（实机点 ✕ 验证通过）
- [x] 移除悬浮底栏（开启后宽屏走悬浮坞有布局 bug 且无使用场景）
- [x] 侧栏默认展开，设置页不再提供该开关（侧栏顶部按钮可收起）
- [x] Windows 调试链路：`scripts/with-msvc.cmd`（VS2022 + clang-cl + 仓库内 TMP）；VSCode 任务/调试配置可直接跑
- [x] 文档：README / CONTRIBUTING / AGENTS / docs/ui.md / docs/flclash-win-spec.md；旧 KSU/Material/动效方案文档与 findings/animation-plans 删除

## 待办

- [ ] 实机验收拖拽区、最大化/还原（关闭已验证；本机沙箱跑 Tauri 需 portable.marker，正常桌面直接 `pnpm tauri dev`）
- [ ] 旧 `src/styles.css` 存量裸字号/散写色值清理（`lint:tokens` 目前仅告警；旧组件删完再动）
- [ ] Linux 界面冒烟：DEB/RPM/AppImage 启动后确认无 WebView2 相关假设泄漏到前端
- [ ] 每次改界面后补 1440×900 + 800×1100 截图对照（命令见 docs/ui.md）

## 验收

- `pnpm lint:tokens` / `pnpm build` / `pnpm test` 全绿；改到 Rust 侧另跑 `scripts\with-msvc.cmd cargo test/clippy`。
- 1440×900 与 800×1100 无横滚、卡片栅格无空洞、页面标题不挤、窗口按钮可点。
- 保留全部设备调试功能、IPC 契约与测试直读的导出。

## 已知坑

| 现象 | 根因 | 处理 |
|---|---|---|
| 自绘标题栏 ✕ / 最小化 / 最大化无效 | `core:window:default` 只有只读能力 | capabilities 显式补 `core:window:allow-*` 写权限，别删 |
| VSCode 里 cargo / tauri dev 全挂 | 普通终端缺 MSVC/SDK 环境；本机 cl/c1.dll D8050 | 一律走 `scripts\with-msvc.cmd`（VS2022 + clang-cl），见 CONTRIBUTING |
| 卡片栅格出现大片空洞 | 混合高度卡片同处一行，行高被最高卡撑开 | 用 `.ksu-columns` 双列各自堆叠，或让短卡跨列/整行 |
| 提示条把页面顶下去 | 常驻 notice 占据文档流 | 全局横幅固定为右下 toast，不参与布局 |
