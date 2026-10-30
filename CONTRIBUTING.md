# 贡献指南

## 环境

- Node.js 22+、pnpm、Rust stable（Windows 另需 MSVC + WebView2 Runtime）
- Linux 串口：当前用户需有设备节点的读写权限（见 README 验收边界）

```sh
pnpm install
pnpm dev        # 网页预览
pnpm tauri dev  # 桌面壳（含自绘标题栏）
```

## 门禁（按顺序跑，全绿才能提 PR）

```sh
pnpm lint:tokens   # 设计令牌：新增样式禁散写色值
pnpm build         # 含 vue-tsc --noEmit
pnpm test          # 前端 23 用例（源码直测，无需真机）
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

## 三条铁律

1. **颜色只进 `src/styles/tokens-a.css`**（图表序列色进 `src/styles/chart-palette.ts`，两边取值保持一致）。新文件出现 hex 直接被 `lint:tokens` 拦下。
2. **移动 `src/` 下的文件，必须同步改测试夹具**：`tests/*.test.mjs` 是把对应源码 transpile 后在 vm 里跑的，路径写死在夹具里（见 `readFileSync(new URL(...))` 那行）。
3. **提交原子化**：一次提交只做一件事，中文信息，暂存时显式指定路径，不用 `git add -A`。

## 目录地图

- `src/shell/`：应用壳（标题栏/图标栏/侧栏/状态栏/命令面板）
- `src/composables/useServo.ts`：应用状态单例（首个调用方必须是 App.vue，生命周期绑应用壳）
- `src/views/`：六个工作区视图（懒加载）
- `src/features/{recording,ethercat}/`：录制解析 / EtherCAT 主站
- `src/styles/`：`tokens-a.css`（唯一色源）/ `chart-palette.ts` / `shell.css` / `bridge-a.css`
- `scripts/`：`lint-tokens.mjs`（门禁）、`split-app.py` / `split-workbench.py`（历史拆分记录，可考据，不用再跑）
