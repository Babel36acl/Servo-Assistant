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
- 遗留：styles.css 64 裸字号 + 间距 px（P5 统一收）；`tauri dev` 实机验拖拽/最大化；平板 1024 真机验抽屉 + 44 触控
- 验证：每阶段 `pnpm build`，P4 起加 `pnpm test` + `cargo test --manifest-path src-tauri/Cargo.toml` + clippy `-D warnings`
