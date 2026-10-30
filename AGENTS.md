# AGENTS.md — 给 AI 协作者的仓库速查

## 一句话

设备中立的 Tauri 2 调试器（Vue3 前端 + Rust 后端）。改代码前先读 `CONTRIBUTING.md` 的门禁和铁律。

## 常用命令（都在仓库根跑）

```sh
pnpm install
pnpm lint:tokens   # 色值门禁，新样式禁 hex
pnpm build         # vue-tsc + vite 打包
pnpm test          # node --test tests/*.test.mjs（23 用例）
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

## 架构（P1–P3 之后的样子）

- `src/shell/` 壳 → `src/composables/useServo.ts` 单例（109 名，首个调用方必须是 App.vue）→ `src/views/` 六视图 → `src/features/` 录制/EtherCAT。
- 样式：`tokens-a.css` 是唯一色源（83+ token），`chart-palette.ts` 是唯一 JS 色源；`shell.css` 写新壳，`bridge-a.css` 把旧变量名重指向风格 A；旧 `styles.css` 冻结只删不加。
- 测试是源码直测：`tests/*.test.mjs` 把目标源码 transpile 后在 vm 里跑，`require` 只 mock 了 vue/tauri/同级模块。**搬文件必改夹具里的 `readFileSync` 路径**，加 `defineExpose`/`defineEmits` 要同步补 mock。
- Tauri 窗口 `decorations:false` + 前端自绘标题栏（`data-tauri-drag-region`），改标题栏先确认拖拽区不断。

## 约定

- 提交中文、原子化、显式路径暂存（不用 `git add -A`）；先 `git status --short` 再动手。
- 无界面环境用 `vite build` + `node --test` 验证，不跑 `tauri dev`。
- 旧 Profile/录制格式是兼容性契约，改格式先更新 `docs/`。
