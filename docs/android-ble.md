# 安卓平板 · BLE 桥接 485 使用与构建

## 链路

```
平板 / PC（BLE GATT 中央）
   │  NUS 透传：6E400001-B5A3-F393-E0A9-E50E24DCCA9E
   │      …0002 写（完整 Modbus 帧 = 单个 BLE write）
   │      …0003 notify（桥按 UART 空闲切帧回传）
   │  CTRL 配置：9F1D0002-6E4E-4B52-8A53-2A7F4F3E1002
   │      6 字节 [校验 0N/1E/2O][停止位 1..2][波特率 u32 LE]
ESP32-C6 桥（固件仓库：../ServoBridge，广播名 ServoBridge）
   │  UART0 19200 8E1 默认，参数经 CTRL 在线重配、NVS 掉电保持
TTL→RS485 隔离模块（自动流向）
   │
伺服驱动器
```

帧保真：下行整帧一次 `uart_write_bytes` 帧内零间隙；上行以 RX 空闲（>3.5 字符）为界整帧 notify，应用侧按字节流重组。自动查找切波特率走 CTRL 在线重配，**不重连 BLE**——247 站全扫不会变成几百次 GATT 连接。

## 应用内使用

连接方式选「蓝牙」→「扫描」→ 选 ServoBridge → 波特率/协议/校验/停止位照常选（随连接经 CTRL 下发到桥）→ 连接。寄存器扫描与自动查找均可用。桌面版（Windows/Linux/macOS）同样可用蓝牙通道，笔记本带蓝牙即可无线调试，不依赖 USB。

## Android 构建

仓库已含 `src-tauri/gen/android`（tauri 2 工程）。

前置：JDK 17、Android SDK（platform-tools、platforms;android-34、build-tools;34.0.0）、NDK r27c，`JAVA_HOME`/`ANDROID_HOME`/`ANDROID_NDK_HOME` 指好后：

```sh
pnpm tauri android build --debug   # 出 debug APK：src-tauri/gen/android/app/build/outputs/apk/
pnpm tauri android build           # 出签名 release（需配置 keystore）
```

Rust 侧交叉编译验证（无需 SDK，只要 NDK 的 clang 供 C 组件）：

```sh
rustup target add aarch64-linux-android
cargo check --target aarch64-linux-android
```

## 权限

Android 12+ 扫描/连接需要 `BLUETOOTH_SCAN`/`BLUETOOTH_CONNECT` 运行时授权；12 以下扫描需要位置权限。声明已写入 `gen/android/app/src/main/AndroidManifest.xml`（插件自带 merge，显式留档）。首次扫描系统弹授权，拒绝后错误会原样显示在界面提示区。

## 已知边界

- 平板上自绘标题栏（decorations:false + data-tauri-drag-region）尚未做 Android 安全区适配，会顶进系统状态栏；窄屏 BottomNav 形态本身可用。首次平板联调时按 `env(safe-area-inset-*)` 补一次壳层样式。
- BLE 吞吐按 2M PHY / MTU 247 / 连接间隔 15ms 估约 30~60 KB/s；一轮 FC03 读 16 寄存器 < 20ms，状态轮询 250ms 档绰绰有余。
- 若 BLE 不通先查 485 接线：桥固件按乐鑫 datasheet U0TXD=GPIO17 / U0RXD=GPIO16，与 Waveshare wiki 快查表标注相反，实测对调为准。
