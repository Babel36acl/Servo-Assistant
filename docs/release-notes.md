# 发布说明

本文件会被 `Release` 工作流用作 GitHub Release 说明。**打 `vX.Y.Z` 标签前请更新本文件**，并保持与标签版本一致。

## v0.5.0（2026-11-26）

- 新增蓝牙桥接通道：Modbus 传输层抽象为 `Transport`，BLE（ServoBridge / NUS 透传）与串口共用同一套帧逻辑，桌面与平板都能无线调试 485 总线；自动查找切波特率走桥的 CTRL 在线重配，不重连 GATT。
- 新增安卓平板端：`src-tauri/gen/android` 工程、BLE 扫描/连接权限声明，minSdk 26（`tauri-plugin-blec` 要求），窄屏底栏壳可直接使用。
- 依赖升级：rusqlite 0.43（SQLite 边界把 u64 转 i64）、Tauri 2.12 / tauri-build 2.7、TypeScript 6、Vite 8 等。
- 发布流程：`Release` 工作流新增安卓 job，随标签产出签名 universal APK；安卓 debug 构建由「安卓构建」工作流在 main 推送时冒烟。修复 Windows 产物文件名平台段为空的问题；新增 `workflow_dispatch` 预演入口（只出 artifact，不发布）。
- 界面：退役旧 `styles.css` 与 `bridge-a.css`，样式全部走 FlClash 令牌，`lint:tokens` 对裸字号与散写色值零容忍；连接方式分段改为网格圆角块，平板竖屏 / 横屏与窄屏底栏排版补齐（触控目标 ≥40px、safe-area 内距、消除横向滚动与右侧黑边）。
- 代码结构：前端 `useServo` 按连接 / 参数 / 监控 / 通讯拆成领域 composable，连接页与录制页拆出子组件；Rust 侧 `runtime.rs`、`master.rs` 拆为目录模块。IPC 命令、Profile 与录制格式均未变化。
- ServoBridge 固件同步发布 v0.1.0（见该仓库 Release），新增只读版本特性 `9F1D0003`，应用侧协议不变。

## 验证

- 每次 PR 与 main 推送由 `CI` 跑 `pnpm lint:tokens`、`pnpm build`、`pnpm test`，并在 Windows 与 Ubuntu 22.04 各跑一遍 `cargo test --locked` 与 `cargo clippy --locked --all-targets -- -D warnings`（Linux 侧专门覆盖 `#[cfg(windows)]` 分支不参与编译时的 dead_code）。
- 发版前 `Release` 工作流对同一版本号重跑上述门禁后才打包。
- 蓝牙链路目前只有模拟器与自建 ServoBridge 验证；首次接真机驱动器仍需按 `docs/android-ble.md` 核对 485 接线与串口参数。

## 安装包

- Windows x64：MSI、NSIS 安装 EXE，或解压即用的便携 ZIP（需要 WebView2 Runtime）。
- Linux x64：DEB（Debian 12+）、RPM（Fedora）、AppImage（Arch 等）。
- 安卓：签名 universal APK（arm64-v8a / armeabi-v7a / x86_64 三合一），minSdk 26（Android 8.0 及以上）。首次使用需授予蓝牙与附近设备权限；平板经 BLE 连 ServoBridge 桥再接 485。
- 在线 EtherCAT 需要单独安装 Npcap 并重启应用。

## 已知边界

- 主站仅 PRE-OP：不进入 OP、不使能伺服、不发送周期运动输出。
- 不提供恢复出厂值、自动调参、EEPROM 写入或 FoE 固件升级。
- 自动化测试与模拟器只验证软件契约，不代表任何具体设备的兼容性。
- 平板安全区内距只在浏览器内用 CDP 模拟验证过，尚未在真机平板上确认状态栏 / 手势条区域。
