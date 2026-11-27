# AGENTS.md — 给 AI 协作者的仓库速查

## 一句话

设备中立的 Tauri 2 调试器（Vue3 前端 + Rust 后端）。改代码前先读 `CONTRIBUTING.md` 的门禁和铁律。

## 常用命令（都在仓库根跑）

```sh
pnpm install
pnpm lint:tokens   # 色值门禁，新样式禁 hex
pnpm build         # vue-tsc + vite 打包
pnpm test          # node --test tests/*.test.mjs（26 用例）
# Windows 本机：Rust/Tauri 必须经 MSVC 包装脚本（VS2022 + clang-cl），
# 直接 cargo 会因缺 SDK 环境/本机 cl D8050 失败；VSCode 任务已内置。
scripts\with-msvc.cmd cargo test --manifest-path src-tauri/Cargo.toml
scripts\with-msvc.cmd cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
scripts\with-msvc.cmd pnpm tauri dev
# 本机没有 VS Build Tools 时：Linux 目标的 cargo test / clippy 改在 WSL 里跑
# （CARGO_TARGET_DIR 放 WSL 原生盘，别放 /mnt/c）；Windows 专属代码交给 CI 的 Windows job。
wsl -d archlinux -- bash -lc 'cd /mnt/c/<仓库> && CARGO_TARGET_DIR=~/target-sa cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings'
```

## 架构（FlClash 改版之后）

- `src/shell/` 壳（自绘拖拽条 `TitleBar` + 图标栏 `RailNav` + 窄屏 `BottomNav`，主题/侧栏/字号在 `ui-mode.ts`）→ `src/composables/useServo.ts` 聚合单例（首个调用方必须是 App.vue；领域实现拆在 `src/composables/servo/`）→ `src/views/` 七视图（含设置）→ `src/features/` 连接卡片/录制/EtherCAT/Modbus 扫描。
- 样式：`tokens-a.css` 是唯一色源（FlClash 四级表面 + Dynamic Type 字阶），`chart-palette.ts` 是唯一 JS 色源；`ksu.css` 写全局底座 + 壳/卡片/行/控件，`views.css` 只放视图微调。旧 `styles.css` 与 `bridge-a.css` 已退役、`--sa-*` 别名已删，`pnpm lint:tokens` 会拦裸字号 / 散写色值 / 旧变量名回流。尺寸/颜色规格见 `docs/flclash-win-spec.md`。
- 测试是源码直测：`tests/*.test.mjs` 把目标源码 transpile 后在 vm 里跑，夹具只 mock vue / tauri / api，其余相对导入按路径递归读真实源码。**搬文件必改夹具里的入口路径**，加 `defineExpose`/`defineEmits` 要同步补 mock。
- Tauri 窗口 `decorations:false` + 前端自绘标题栏（`data-tauri-drag-region`）；窗口按钮依赖 `src-tauri/capabilities/default.json` 里的 `core:window:*` 写权限，改标题栏先确认拖拽区不断、能力没删。

## 约定

- 提交中文、原子化、显式路径暂存（不用 `git add -A`）；先 `git status --short` 再动手。
- 发布由推送 `vX.Y.Z` 标签触发，`main` 推送不自动构建；普通 PR 不改版本号，发版前更新 `docs/release-notes.md`。
- 无界面环境用 `vite build` + `node --test` 验证，不跑 `tauri dev`。
- 旧 Profile/录制格式是兼容性契约，改格式先更新 `docs/`；Profile 契约唯一来源是 `docs/profile-v1.md`。
- Profile 制作 Skill 只保留 `.agents/skills/servo-profile-from-pdf/SKILL.md` 一份，不要在 docs/README 复制第二份可执行提示词。
