# 伺服参数调试器

[![CI](https://github.com/AKCX2002/Servo-Assistant/actions/workflows/ci.yml/badge.svg)](https://github.com/AKCX2002/Servo-Assistant/actions/workflows/ci.yml)

一个设备中立的 Tauri 2 GUI 调试器。程序不内置厂商、型号、参数表或手册内容；设备兼容性完全由用户导入的 JSON Profile 描述。

```text
JSON Profile 导入
  → Modbus RTU / ASCII 连接
  → FC03 读取和缩放
  → 写前值比较
  → FC06 安全写入
  → 立即回读校验
  → 可选应用/持久化操作
```

## 当前能力

- 导入并校验设备 JSON Profile，不限制参数 ID 命名方式或寄存器地址布局。
- 支持安全模拟器和真实 Modbus RTU / ASCII 串口连接。
- 支持参数读取、单项安全写入、快照导出、差异比较和选择性批量写入。
- 高风险参数需要输入 `写入 <parameterId>`；所有写入都执行写前值比较和写后回读。
- Profile 可选择定义应用和持久化命令；未定义时 GUI 不允许执行对应操作。
- 操作审计和参数快照保存在应用数据目录的 `servo.db`。
- Profile 可声明任意状态寄存器，GUI 最多显示 6 个实时曲线通道。
- 不提供恢复出厂值、自动调参、站号穷举或通讯参数在线修改。

本仓库不包含任何特定厂商的参数库、说明书摘录、品牌资源或资料链接。设备名称、参数定义及操作码应由使用者从有权使用的资料中制作并导入。项目与任何设备制造商不存在隶属、授权或背书关系。

## EtherCAT 与通信录制

通信工作台支持原始串口收发、模拟器事件及 EtherCAT 帧的后台录制、分卷、离线浏览和以太网 PCAPNG 导出。
支持 PCAP/PCAPNG 导入、EtherCAT 标准 Datagram、邮箱头和显式 PDO 映射解析。
Windows 主站通过 SOEM/Npcap 提供从站发现、PRE-OP 下的 CoE/SDO 参数配置及最多六通道监控，写入执行确认、写前比较和回读校验。

Npcap 需单独安装；未安装时仍可使用 Modbus 和离线功能。主站不提供 OP/运动输出。
详细范围、Profile 契约、录制完整性和设备验收见 [EtherCAT 与录制说明](docs/ethercat-and-recording.md)。
SOEM 与 WinPcap SDK 头文件的许可证及修改说明见 [第三方来源](src-tauri/native/README.md)。

## 开发

环境要求：Rust stable MSVC、Microsoft C++ Build Tools、WebView2、Node.js 和 pnpm。

```powershell
pnpm install
pnpm tauri dev
```

验证命令：

```powershell
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

## 使用

1. 按 [Profile 1.0 契约](docs/profile-v1.md)准备设备 JSON。
2. 在“设备配置”中导入 JSON；建议先选择配置模拟器验证范围、缩放和操作流程。
3. 实机连接前确认串口参数、站号、接线和机械安全状态。
4. 先只读核对参数及状态，再执行经确认的低风险写入。

[通用示例 Profile](examples/servo-profile.example.json)仅用于展示格式和模拟器测试，其中地址、范围和操作码均为虚构值，不得直接用于真实设备。

## 关键文件

- `examples/servo-profile.example.json`：不对应任何真实设备的格式示例
- `src-tauri/src/profile.rs`：Profile 反序列化、唯一性、范围和缩放校验
- `src-tauri/src/modbus.rs`：FC03/FC06、RTU CRC / ASCII LRC、异常响应和帧间隔
- `src-tauri/src/runtime.rs`：串口独占、模拟器、安全写入和可选设备操作
- `src-tauri/src/audit.rs`：SQLite 持久审计与快照存储
- `src/App.vue`：配置导入、连接、差异比较和确认界面

## 验证边界

### 自动查找

导入对应设备 Profile，选择串口、校验位和停止位，保持断开状态后点击“自动查找”。先尝试当前站号和波特率，再遍历指定的站号范围（默认 1～247）及 Profile 允许的波特率。每个组合只读取一个已定义寄存器，遍历 RTU / ASCII 的 8E1、8N1、8O1，连续两次有效响应后回填站号、波特率及串口格式；点击“连接”后才开始常规读取。

查找不修改设备配置、不发送广播，支持取消并释放串口。默认探测超时为 200 ms；慢设备可增大到 2000 ms。查找期间不能连接或修改 Profile。未找到不代表设备不存在，还需核对校验位、停止位、Profile 地址和接线。

### 便携版与 Linux

Windows 便携 ZIP 解压后运行 `servo-assistant.exe`，需要 WebView2 Runtime；随包的 `portable.marker` 使数据保存在程序旁的 `data` 文件夹，升级时保留该目录。安装版继续使用系统应用数据目录。

Linux x64 在 Ubuntu 22.04 构建：Debian 12 及以上使用 DEB，Fedora 使用 RPM，Arch Linux 使用 AppImage。发布流程在 Debian 12、Fedora 和 Arch 容器中检查安装与动态库依赖；桌面显示和真实串口仍需目标系统验证。串口使用前需确保当前用户有目标设备节点的读写权限。Linux 依赖参见 [Tauri 官方说明](https://v2.tauri.app/start/prerequisites/#linux)。

### 读取与采集行为

- 通讯设置在当前应用会话内生效：默认每组最多 16 个寄存器（1～100），CRC / LRC / 超时额外重试 2 次（0～3）。仅 FC03 读取可重试，FC06 写入不会自动重发；设备异常响应、错误站号和格式错误直接返回诊断。
- 参数读取逐组显示进度，成功组即时更新，失败或取消时保留旧值并标记过期；可以只重读失败 / 未完成组。刷新保留未提交编辑；过期值须重新读取后才能单项写入。
- 状态轮询与曲线采集分别控制。默认 250 ms 是一次状态读取完成后的等待时间，不是精确采样周期；支持 200/250/500/1000/3000 ms 及 50～60000 ms 自定义。通讯恢复自动恢复正常间隔，不清除其他操作的错误。
- 只读稳定性测试交替读取 Profile 中最长连续状态区的 1 个和最多 100 个寄存器。长帧对照有意不按分组上限拆分；默认 100 轮，可选 1～1000 轮。测试期间暂停日常轮询；取消等待当前事务（含有限重试）结束，随后保留统计并按原开关恢复轮询。
- 首次成功、重试恢复、最终失败分别统计；CRC、超时、重试次数和最近失败地址/数量/尝试次数/接收字节可见。真实串口每次读取成功、失败和重试写入现有 SQLite 操作证据；模拟器测试结果不能证明实机链路稳定。
- 前端状态回归运行 `pnpm test`；串口故障注入与原有核心测试运行 `cargo test --manifest-path src-tauri/Cargo.toml`。故障测试覆盖部分帧超时、CRC 恢复、重试上限、设备异常不重试、写入不重发和长短帧读取边界。

自动化测试和模拟器只能验证软件契约。每份设备 Profile 仍需针对其目标设备分别完成读取、缩放、写入、回读、应用和持久化验收；未定义或未实测的设备能力不能视为已兼容。

## 自动构建与发布

- Pull Request 或手动运行 `CI` 时，执行 Windows 检查和打包，不发布。
- 每次推送到 `main`，或在 `main` 上手动运行 `Release`，都会测试、构建 Windows EXE/MSI/便携 ZIP 和 Linux DEB/RPM/AppImage，执行 Linux 优化模式 glib 安全回归测试，并验证 Debian/Fedora/Arch 包。全部成功后自动创建标签和 GitHub Release，说明取自 `docs/release-notes.md`。
- 自动版本取源码版本与现有稳定标签的下一补丁版本中的较大值；同一已发布提交重跑时复用版本并保留既有发布。源码中的版本是发布起始版本，流水线会在构建副本中同步更新 package、Tauri、Cargo 和锁文件的实际发布版本，标签指向触发构建的源码提交，不向 main 写入机器人版本提交。

立即构建并发布当前 main：

```powershell
gh workflow run release.yml --ref main
```

构建时核对三个项目版本与选定发布版本；任一平台构建、测试或包验证失败都不会创建 Release。后续无需手工推送标签。自动生成版本的源码复现：检出发布标签后，运行 `./scripts/release-version.ps1 -Apply -Version <标签去掉v>`，再执行正常构建命令。

Linux GTK3 依赖的 glib 0.18.5 使用仓库内的上游安全修复回移，见 [补丁来源与移除条件](src-tauri/vendor/glib/PATCH.md)。没有添加告警忽略规则；版本扫描器仍可能按 0.18.5 报警。

当前流水线未配置 Windows 代码签名证书，安装包能够正常构建，但从浏览器下载后可能触发 SmartScreen 提示。

## 许可证

本项目采用 [GNU General Public License v3.0 only](LICENSE)。

通讯支持 RTU / ASCII 与 N/E/O 校验，自动查找会回填协议、校验位和停止位。P300–P302 手册预设及扫描范围见 [串口协议与查找](docs/profile-v1.md#串口协议与查找)。
