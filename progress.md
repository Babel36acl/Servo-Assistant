# progress · 变更日志（界面与开发链路）

> 只记与当前实现相关的里程碑；更早的 VSCode 四区壳 / 风格 A / 动效实验已废弃，见 git 历史。

## 2026-10-30 · FlClash Windows 改版

- 视觉基准定为 FlClash Windows（`docs/flclash-win-spec.md` + 实拍 `docs/reference/flclash-windows-dashboard.png`）：纯黑页 + `#191A1C` 卡片四级表面、Dynamic Type 字阶、主色 `#2F83FF`、卡片圆角 20、栅格间距 16。
- 壳重写：36px 自绘拖拽条 + 右上窗口按钮；图标栏默认展开 220（可收 48）；`<840px` 收底部 5 项导航。删除应用栏 / 状态栏 / 命令面板 / 二级侧栏。
- 七页视图卡片化；表格改行卡；全局提示改右下 toast；设置页保留主题模式 / 字号缩放 / 关于。
- 窗口关闭修复：`core:window:default` 不含写权限，capabilities 显式补 `allow-close/minimize/maximize/unmaximize/toggle-maximize/start-dragging/set-focus/is-maximized/set-title`；Tauri 实机点 ✕ 进程退出验证通过。
- 移除悬浮底栏（开启后宽屏悬浮坞有布局 bug 且无使用场景）；侧栏改为默认展开、设置页不再暴露开关。
- 文档整理：新增 `docs/ui.md`（结构 / 改动入口 / 常见坑）；README/CONTRIBUTING/AGENTS 同步；删除 KSU 调研、Material 色板、动效方案、旧 findings/animation-plans。

## 2026-10-30 · Windows 调试链路

- `scripts/with-msvc.cmd`：优先 VS2022 → `VsDevCmd` → `CC/CXX=clang-cl` → `TMP` 指向仓库 `.tmp`；解决普通终端缺 SDK 头、cl/c1.dll D8050、cargo test 临时目录被拒三类失败。
- `.vscode/tasks.json` / `launch.json`：Rust/Clippy/Tauri dev 全部经包装脚本；F5 网页调试先起 Vite 后台任务并等 1420；两份配置纳入版本库。
- SOEM 两处 clang-cl 兼容补丁（`osal.h` 线程原型 `void **`；`osal_win32.h` 去掉被误报冲突的 `osal_gettimeofday` 原型），记录在 `src-tauri/native/README.md`。
- 验证：`cargo build` / `cargo test --lib` 60 passed / `cargo clippy -D warnings` 全过；`tauri dev` 实机起窗并可关闭。

## 2026-10-30 · 前端门禁

- `pnpm lint:tokens` / `pnpm build` / `pnpm test`（23/23）保持全绿；旧 `styles.css` 散写告警仍在（冻结，待旧组件清理）。
