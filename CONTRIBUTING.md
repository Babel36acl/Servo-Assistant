# 贡献指南

## 环境

- Node.js 22+、pnpm、Rust stable（Windows 另需 MSVC + WebView2 Runtime）
- Linux 串口：当前用户需有设备节点的读写权限（见 README 验收边界）
- **Windows 本机跑 Rust / Tauri 必须经 `scripts\with-msvc.cmd`**：普通终端没有 MSVC/Windows SDK 的 `INCLUDE`/`LIB`/`PATH`，直接 `cargo` 会在 `native/bridge.c`（`winsock2.h`）和 `libsqlite3-sys` 上失败。该脚本会加载 VS 2022 环境并改用 `clang-cl`（本机 `cl.exe`/`c1.dll` 在 cargo 下会报 D8050）。VSCode 的 `.vscode/tasks.json` 已内置这一层。

```sh
pnpm install
pnpm dev        # 网页预览 http://localhost:1420
pnpm tauri dev  # 桌面壳（含自绘标题栏）——Windows 请改用下一行的任务/命令
scripts\with-msvc.cmd pnpm tauri dev
```

## 门禁（按顺序跑，全绿才能提 PR）

```sh
pnpm lint:tokens   # 设计令牌：新增样式禁散写色值
pnpm build         # 含 vue-tsc --noEmit
pnpm test          # 前端 23 用例（源码直测，无需真机）
# Windows 本机（Linux/CI 可去掉 with-msvc.cmd 前缀）：
scripts\with-msvc.cmd cargo test --manifest-path src-tauri/Cargo.toml
scripts\with-msvc.cmd cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

## 三条铁律

1. **颜色只进 `src/styles/tokens-a.css`**（图表序列色进 `src/styles/chart-palette.ts`，两边取值保持一致）。新文件出现 hex 直接被 `lint:tokens` 拦下。
2. **移动 `src/` 下的文件，必须同步改测试夹具**：`tests/*.test.mjs` 是把对应源码 transpile 后在 vm 里跑的，路径写死在夹具里（见 `readFileSync(new URL(...))` 那行）。
3. **提交原子化**：一次提交只做一件事，中文信息，暂存时显式指定路径，不用 `git add -A`。

## 目录地图

- `src/shell/`：FlClash 式壳（自绘拖拽条 `TitleBar`、图标栏 `RailNav`、窄屏底栏 `BottomNav`、`ui-mode.ts` 主题/字号/侧栏偏好）
- `src/components/ksu/`：卡片/行/开关/分段/按钮等 UI 原语
- `src/composables/useServo.ts`：应用状态单例（首个调用方必须是 App.vue，生命周期绑应用壳）
- `src/views/`：七个工作区视图（连接/参数/监控/诊断/通信/审计/设置）
- `src/features/{recording,ethercat}/`：录制解析 / EtherCAT 主站
- `src/styles/`：`tokens-a.css`（唯一色源）/ `chart-palette.ts` / `ksu.css`（壳与组件）/ `views.css` / `bridge-a.css`
- `scripts/`：`lint-tokens.mjs`（门禁）、`with-msvc.cmd`（Windows Rust/Tauri 环境包装）、`split-app.py` / `split-workbench.py`（历史拆分记录，可考据，不用再跑）

界面结构、样式改动入口与防御性契约见 [界面与样式指南](docs/ui.md)。
