# 发布说明

本文件会被 `Release` 工作流用作 GitHub Release 说明。**打 `vX.Y.Z` 标签前请更新本文件**，并保持与标签版本一致。

## v0.5.0（待发布）

- 新增蓝牙桥接通道：Modbus 传输层抽象为 `Transport`，BLE（ServoBridge / NUS 透传）与串口共用同一套帧逻辑，桌面与平板都能无线调试 485 总线；自动查找切波特率走桥的 CTRL 在线重配，不重连 GATT。
- 新增安卓平板端：`src-tauri/gen/android` 工程、BLE 扫描/连接权限声明，minSdk 26（`tauri-plugin-blec` 要求），窄屏底栏壳可直接使用。
- 依赖升级：rusqlite 0.43（SQLite 边界把 u64 转 i64）、Tauri 2.12 / tauri-build 2.7、TypeScript 6、Vite 8 等。
- 发布流程：`Release` 工作流新增安卓 job，随标签产出签名 universal APK；安卓 debug 构建由「安卓构建」工作流在 main 推送时冒烟。

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
- 平板自绘标题栏尚未做 Android 安全区适配，会顶进系统状态栏；窄屏底栏形态本身可用。
