# Servo Assistant · 伺服参数调试器

[![CI](https://github.com/Babel36acl/Servo-Assistant/actions/workflows/ci.yml/badge.svg)](https://github.com/Babel36acl/Servo-Assistant/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Babel36acl/Servo-Assistant?display_name=tag)](https://github.com/Babel36acl/Servo-Assistant/releases)
[![License: GPL v3](https://img.shields.io/badge/license-GPL--3.0--only-blue.svg)](LICENSE)

设备中立的 Tauri 2 伺服调试器：程序不内置厂商、型号、参数表或手册内容，设备兼容性由用户导入的 JSON Profile 描述。支持 Modbus RTU / ASCII / TCP、EtherCAT 主站入口、参数安全写入、通信录制与离线解析。

> **验证状态**：EtherCAT 主站、从站诊断、在线对象字典等功能尚未在真实从站上验收，目前只有代码、ABI 与模拟测试；Modbus 实机行为也取决于用户 Profile 与现场设备，软件测试不等于设备兼容性认证。

## 功能

### 连接

- 安全模拟器：不需要硬件即可演练参数范围、缩放和写入流程。
- Modbus RTU / ASCII：站号/波特率/校验/停止位；自动查找支持多选波特率与会话级扩展波特率。
- Modbus TCP：MBAP + FC03/FC06，主机/端口可配；读取超时重试，写入不自动重发。
- EtherCAT 主站入口：Windows + Npcap，PRE-OP 下发现从站、CoE/SDO 配置、从站诊断与在线对象字典生成 Profile；不进入 OP。

### 参数与安全写入

- 读取、筛选、单项写入、快照导出/比较、选择性批量写入。
- 写前值比较 + 写后立即回读；高风险参数需输入 `写入 <parameterId>`。
- Profile 可声明应用/持久化命令；未声明时界面禁用、后端拒绝。
- 操作审计与参数快照保存在 `servo.db`。

### 诊断与录制

- 只读寄存器扫描：FC03 分块读取指定地址范围，列出非零值，支持 CSV 导出；支持串口与 TCP。
- 通讯诊断：首读/重试/失败统计、CRC/LRC/超时、最近失败帧。
- 通信工作台：串口/模拟器/EtherCAT 帧录制、分卷、索引检索、PCAP/PCAPNG 导入导出、故障触发录制。
- 历史曲线：按来源/协议/站号/地址和时间检索，可定位回原始事务。

### EtherCAT

- 从站列表：位置、站号、Vendor/Product/Revision、AL 状态/错误码。
- 从站诊断：活动端口、拓扑端口、DC 状态、ESC `0x0300..0x0311` 原始错误计数（只读）。
- 在线对象字典：SDO Info 只读发现，生成 `ethercat-1.0` Profile（整数对象，512 上限）。
- 对象监控：最多 6 个 SDO 轮询通道，非实时 PDO。

## 安装

从 [Releases](https://github.com/Babel36acl/Servo-Assistant/releases) 下载：

- Windows：安装版 MSI / NSIS EXE，或解压即用的便携 ZIP（需要 WebView2 Runtime）。
- Linux x64：DEB（Debian 12+）、RPM（Fedora）、AppImage（Arch 等）。

便携版升级时保留程序旁的 `data` 文件夹。在线 EtherCAT 需要单独安装 Npcap 并重启应用；未安装时仍可用 Modbus 与离线功能。

## 快速开始

1. 按 [Profile 1.0 契约](docs/profile-v1.md)准备设备 JSON。
2. 在“设备与连接 → 设备配置”导入；先选择“模拟器”验证范围、缩放和操作流程。
3. 实机前确认站号、串口/TCP 参数、接线和机械安全状态。
4. 在“参数工作区”读取/筛选/比较，在“实时监控”查看状态与曲线，再执行经确认的低风险写入。

[通用示例 Profile](examples/servo-profile.example.json) 仅展示格式，地址/范围/操作码均为虚构值，不得直接用于真实设备。

## 文档

| 文档 | 内容 |
|---|---|
| [Profile 1.0](docs/profile-v1.md) | Profile 字段、地址/缩放、写入风险、串口与查找规则 |
| [EtherCAT 与录制](docs/ethercat-and-recording.md) | 主站范围、对象字典、诊断、录制格式与验收边界 |
| [界面与样式](docs/ui.md) | 视图结构、样式入口与不能破坏的契约 |
| [FlClash Windows 规格](docs/flclash-win-spec.md) | 视觉规格与令牌来源 |
| [扩展路线图](docs/expansion-roadmap.md) | EtherCAT/Modbus 后续方向、库选型与阶段 |
| [第三方来源](src-tauri/native/README.md) | SOEM/WinPcap 许可证与本地补丁 |

## 开发

环境：Rust stable MSVC、Visual Studio 2022「Desktop development with C++」、LLVM（clang-cl）、WebView2、Node.js 和 pnpm。

```powershell
pnpm install
scripts\with-msvc.cmd pnpm tauri dev   # 桌面壳（Windows 必须经包装脚本）
pnpm dev                               # 纯网页预览 http://localhost:1420
```

验证门禁：

```powershell
pnpm lint:tokens
pnpm build
pnpm test
scripts\with-msvc.cmd cargo test --manifest-path src-tauri/Cargo.toml
scripts\with-msvc.cmd cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

当前基线：前端 25 项、Rust 71 项（另 1 项崩溃恢复子进程入口按设置忽略）。Windows 的 Rust/Tauri 命令直接跑 `cargo` 会因缺少 MSVC/Windows SDK 环境失败，必须经 `scripts\with-msvc.cmd`；VSCode 任务已内置这一层。

## 发布（维护者）

**发布只由版本标签触发，`main` 推送不再自动构建。**

1. 更新三处版本号并同步锁文件：

   ```powershell
   ./scripts/release-version.ps1 -Apply -Version 0.5.0
   ```

2. 更新 [发布说明](docs/release-notes.md)（Release 使用该文件作为说明）。
3. 提交并推送源码：

   ```powershell
   git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock docs/release-notes.md
   git commit -m "chore(release): v0.5.0"
   git push origin main
   ```

4. 打标签并只推送标签（这一步才触发构建）：

   ```powershell
   git tag v0.5.0
   git push origin v0.5.0
   ```

5. `Release` 工作流会在标签提交上应用/核对版本，跑测试与 clippy，构建 Windows MSI/NSIS/便携 ZIP 与 Linux DEB/RPM/AppImage，验证安装包后创建 GitHub Release。

`CI` 只在 PR 或手动触发时跑检查，不打包、不发布。`Release` 校验标签必须为 `v<主>.<次>.<修订>`；同一标签重跑会复用已有 Release，不覆盖已发布资产。

## 验证状态与边界

- 自动化测试与模拟器只验证软件契约；每份 Profile 仍需在目标设备上完成读取、缩放、写入、回读、应用和持久化验收。
- **EtherCAT 未实机验收**：诊断、对象字典与 SDO 读写尚未在真实从站验证；首次使用前必须独占总线、停用其他主站，并从只读操作开始。
- 主站不提供 OP、伺服使能、周期运动输出；不提供恢复出厂值、自动调参、EEPROM 写入或 FoE 固件升级。
- 不内置厂商参数库、手册摘录或资料链接；设备名称与参数定义由使用者从有权使用的资料制作。
- 录制/日志可能含现场信息，分享前请脱敏。

## 许可证

[GNU General Public License v3.0 only](LICENSE)。
