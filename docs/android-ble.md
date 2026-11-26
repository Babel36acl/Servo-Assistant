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

前置：JDK 17、Android SDK（platform-tools、platforms;android-37、build-tools;37.0.0）、NDK r27，`JAVA_HOME`/`ANDROID_HOME`/`ANDROID_NDK_HOME` 指好后：

```sh
pnpm install
pnpm tauri android build --debug --target aarch64            # 出 debug APK：src-tauri/gen/android/app/build/outputs/apk/
pnpm tauri android build --apk --target aarch64 armv7 x86_64 # 出 release APK，三个 ABI 合并为 universal
```

release 签名由 `gen/android/app/build.gradle.kts` 的 `signingConfigs.release` 从环境变量读取，仓库内不存任何密钥文件：

```sh
export ANDROID_KEYSTORE_PATH=/绝对路径/release.jks
export ANDROID_KEYSTORE_PASSWORD=...   # 值只在维护者本机/仓库 secrets
export ANDROID_KEY_ALIAS=...
export ANDROID_KEY_PASSWORD=...
pnpm tauri android build --apk --target aarch64
```

未设置 `ANDROID_KEYSTORE_PATH`（或文件不存在）时签名配置不挂载，release 构建照常完成，只是产出未签名 APK（`*-release-unsigned.apk`），不能直接分发。

GitHub Actions 两条流水线：

- 「安卓构建」（`.github/workflows/android.yml`）：main 推送到 `src/**`、`src-tauri/**` 或该工作流自身时跑 aarch64 debug APK 冒烟，artifact 名 `servo-assistant-debug-apk`。
- 「Release」（`.github/workflows/release.yml`）的 `android` job：推送 `vX.Y.Z` 标签时用仓库 secrets 里的 keystore 构建**签名** universal APK，`apksigner verify --print-certs` 校验后以 `Servo-Assistant_<版本>_android_universal.apk` 上传 artifact `release-android`。版本号由 `scripts/release-version.ps1` 改写 `tauri.conf.json`，Tauri 构建时再生成 `app/tauri.properties`（versionName = 该版本，versionCode = 主*1000000 + 次*1000 + 修订）。

不打标签也能预演：手动 dispatch `Release` 并填 `version`（如 `0.4.99`），只构建 artifact，publish 被跳过、不会创建 Release。

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

## 本仓实测过的构建坑（fnOS NAS 上）

- `tauri-plugin-blec` 要求 **minSdk 26**，已在 `gen/android/app/build.gradle.kts` 固化；重新 `tauri android init` 会重置它，记得改回。
- /vol4 的 trim ZFS 上 tauri CLI 往 `jniLibs` 建软链会报 `Invalid cross-device link`（同卷也复现）。绕法：CLI 跑完 Rust 交叉编译后失败无妨，手动把 `target/aarch64-linux-android/debug/libservo_assistant_lib.so` **copy** 进 `gen/android/app/src/main/jniLibs/arm64-v8a/`，再裸 gradle 打包：

  ```sh
  cd src-tauri/gen/android
  ./gradlew assembleUniversalDebug -x rustBuildArm64Debug -x rustBuildArmDebug -x rustBuildX86Debug -x rustBuildX86_64Debug
  ```

  （gradle 的 rust 任务要求 `tauri android build` CLI 驻留运行，所以直接排除它们；产物在 `app/build/outputs/apk/universal/debug/`。）
- debug APK 约 200MB 是 Rust debug 符号所致，属正常；正式发布走 `Release` 工作流的三 ABI universal release + proguard。
- gradle 走代理：`GRADLE_OPTS="-Dhttps.proxyHost=… -Dhttps.proxyPort=…" pnpm tauri android build`。
