# progress · 会话日志

## 2026-10-30（规划）
- 通读 App.vue / styles.css / components / api.ts / tauri.conf.json，确认 monolith + 散写 + EtherCAT/录制杂糅三宗罪
- 落盘 task_plan.md / findings.md / progress.md；风格 A 与 VSCode 四区 + palette + 1024 单列 + 44px 方案已定，未动手改代码
## 2026-10-30（P0 落地）
- 建 `src/styles/tokens-a.css`：71 个 `--sa-*` 变量，色/字/距/角/影/动效六表 + 壳尺寸/断点/触控/层级；地基 #0A1628，Maple Mono 全局，圆角主 12，阴影两档
- 建 `scripts/lint-tokens.mjs` + `pnpm lint:tokens`：新文件散写色值直接拦下；存量 4 文件冻结放行待迁（styles.css / CommunicationWorkbench / RecordingHistory / ScopeChart）
- `index.html` theme-color 切到 #0A1628；旧 styles.css 冻结只删不加
- 验证：`pnpm lint:tokens` 通过（EXIT:0），`node --test` 23/23 绿
## 2026-10-30（P1–P3 落地，一次到底）
- P1 壳：`src/shell/`（activities/TitleBar/ActivityBar/SideBar/StatusBar/CommandPalette/AppShell）+ `shell.css` + `bridge-a.css`（旧变量重指向风格 A）；`main.ts` 追加三层 import；`tauri.conf.json` 开 `decorations:false`；`index.html` 摘 Google 字体外链
- P2 拆页：`scripts/split-app.py` 机械切分 → `src/composables/useServo.ts`（109 名单例）+ `src/views/` 六视图 + 瘦 App.vue（壳/横幅/弹窗/面板）；communication 测试夹具改直测 `createServo()`
- P3 分家：`scripts/split-workbench.py` → `features/recording/RecordingSessionView.vue`（录制+浏览，`applyMailboxes` expose）；`features/ethercat/EthercatMasterView.vue`（+activity 带从站数）；`views/CommunicationView.vue` 变两 Tab 编排 + 事件拼接；`RecordingLibrary/History` 搬家
- 调色板收口：`src/styles/chart-palette.ts` 唯一 JS 色源（CHART/HISTORY），tokens-a.css 镜像 `--sa-chart-*/--sa-hist-*` + `--sa-text-10/11`；lint 扩到 `.ts`；`scopePalette`/`colors`/内联数组三处并入
- 验证：`lint:tokens` 过（存量仅 styles.css），`vue-tsc` 零错，`node --test` 23/23，`vite build` 66 模块 1.31s
## 2026-10-30（依赖与协作收尾）
- 全部依赖升最新：vue 3.5.44 / vite 8.3.5 / plugin-vue 6 / vue-tsc 3.3.13 / tauri 2.12.4 / TS 退 5.9.3（TS 7 与工具链不兼容）；`cargo update`（锁文件跟进）
- 新工具链兼容修三处：chartSeries 进 computed、confirmField 显式读、调色板标 readonly string[]
- Dependabot 5 个 open（vite/launch-editor 高危×1 中×3 低×1）随 vite 8.3.5 推送自动关闭，现 10/10 fixed
- 协作配置：dependabot 周检、PR/Issue 模板、CONTRIBUTING、SECURITY、AGENTS、.editorconfig、CI 加 lint:tokens、README 认领 Babel36acl
- 本机缺 glib 系统库，`cargo test`/`clippy` 走 CI Windows；两提交已推：`2a95645`（deps）、`2193c2f`（repo）
- 遗留：styles.css 64 裸字号 + 间距 px（P5 统一收）；`tauri dev` 实机验拖拽/最大化；平板 1024 真机验抽屉 + 44 触控
- 验证：每阶段 `pnpm build`，P4 起加 `pnpm test` + `cargo test --manifest-path src-tauri/Cargo.toml` + clippy `-D warnings`
## 2026-10-30（方向修正：FlClash Windows 视觉体系）
- 用户否定 VSCode 四区壳与 KSU-Miuix 复刻两版，指定「抄 FlClash（chen08209/FlClash）的 WIN 方案」：同属 KSU 视觉体系、现成 Windows 实现
- 规格：`docs/flclash-win-spec.md` + `docs/reference/flclash-windows-dashboard.png`（本机 FlClash v0.9.2 实拍取色）；KSU 调研文档 `docs/ksu-ui-spec.md` 标注为历史参考
- 令牌重写：深色「纯黑页 #000 / 卡 #191A1C / 块 #252528 / 内嵌 #2F3134」四级表面，主色 #2F83FF、成功 #30D158、警告 #FF9F0A、错误 #FF453A；浅色 #ECEDF9/#F9F9FF/#E6E8F3；Apple Dynamic Type 字阶全部 token 化并支持 90/100/110% 缩放
- 壳重写：36px 自绘拖拽条 + 右上窗口按钮（关闭失败根因 `core:window:default` 不含写权限，已补 allow-close/minimize/maximize/unmaximize/toggle-maximize/start-dragging/set-focus/is-maximized/set-title）；左侧 48px 图标栏可展开 220px（仿 FlClash NavigationSidebar）；删除应用栏/命令面板/状态栏；窄屏 <840 收 5 项底栏
- 视图：7 页全部 FlClash 卡片仪表盘化（`.ksu-grid/.ksu-span-*/.ksu-stat/.ksu-display/.ksu-columns`），连接页双列、诊断双列、监控 6 指标卡 + 全宽曲线、表格改行卡、全局横幅改右下 toast
- 设置页：主题模式 深/浅/跟随系统、侧栏展开、悬浮底栏、字号缩放 90/100/110%；关于信息
- 验证：`pnpm lint:tokens` / `pnpm build`（vue-tsc 0 错）/ `pnpm test` 23/23 全绿；Edge 无头截图覆盖 1440×900 七页深色、浅色、800×1100 底栏；`cargo build` 仍卡在 native/bridge.c 的 cl.exe（本机 MSVC/依赖头问题，非本次改动），Tauri 实机点击关闭待 `pnpm tauri dev` 验
