# 伺服参数调试器

[![CI](https://github.com/Babel36acl/Servo-Assistant/actions/workflows/ci.yml/badge.svg)](https://github.com/Babel36acl/Servo-Assistant/actions/workflows/ci.yml)

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
- 支持安全模拟器、Modbus RTU / ASCII 串口、Modbus TCP，以及 EtherCAT 主站连接入口。
- 自动查找可多选搜索波特率，并显式允许 Profile 之外的常用波特率（仅本次应用会话，不修改 Profile）。
- 提供只读保持寄存器扫描：指定站号与地址范围，按 FC03 分块读取，列出非零值并可导出 CSV；扫描期间独占链路（串口或 TCP），不写入设备。
- 支持参数读取、单项安全写入、快照导出、差异比较和选择性批量写入。
- 高风险参数需要输入 `写入 <parameterId>`；所有写入都执行写前值比较和写后回读。
- Profile 可选择定义应用和持久化命令；未定义时 GUI 不允许执行对应操作。
- 操作审计和参数快照保存在应用数据目录的 `servo.db`。
- “读取并导出快照”先弹出原生保存窗口，选择目录和文件名后读取参数并写入 `.servo-snapshot.json`；取消时不执行本次读取或文件导出。保存成功后显示完整路径，参数快照区保留最近成功导出的路径供复制。选择已有文件时由保存窗口确认覆盖，写入失败不会提前清空原备份。导入比较请选择参数快照文件，通信录制目录中的 `session.json` 和 `summary.json` 不属于参数快照。
- Profile 可声明任意状态寄存器，GUI 最多显示 6 个实时曲线通道。
- 不提供恢复出厂值、自动调参、站号穷举或通讯参数在线修改。

本仓库不包含任何特定厂商的参数库、说明书摘录、品牌资源或资料链接。设备名称、参数定义及操作码应由使用者从有权使用的资料中制作并导入。项目与任何设备制造商不存在隶属、授权或背书关系。

## EtherCAT 与通信录制

通信工作台支持原始串口收发、模拟器事件及 EtherCAT 帧的后台录制、分卷、离线浏览和以太网 PCAPNG 导出。
录制绑定当时的设备配置，支持会话名称与备注、文件选择、全会话时间/协议筛选、实时跟随、历史曲线定位事务，以及保留故障前后窗口的一次性触发录制。
PCAP/PCAPNG 可按文件或分卷目录统一检索；录制与网卡捕获的写入、摘要错误会显示完整性告警。
支持 PCAP/PCAPNG 导入、EtherCAT 标准 Datagram、邮箱头和显式 PDO 映射解析。
Windows 主站通过 SOEM/Npcap 提供从站发现、PRE-OP 下的 CoE/SDO 参数配置及最多六通道监控，写入执行确认、写前比较和回读校验。从站诊断提供 AL 状态、活动端口/拓扑、DC 状态和 ESC 0x0300..0x0311 原始错误计数，全部只读。“设备与连接”的连接方式菜单也可直接接管 EtherCAT 网卡；主站状态与通信工作台共享，参数工作区仍使用 Modbus Profile。

Npcap 需单独安装；未安装时仍可使用 Modbus 和离线功能。主站不提供 OP/运动输出。
详细范围、Profile 契约、录制完整性和设备验收见 [EtherCAT 与录制说明](docs/ethercat-and-recording.md)。
SOEM 与 WinPcap SDK 头文件的许可证及修改说明见 [第三方来源](src-tauri/native/README.md)。

## 开发

环境要求：Rust stable MSVC、Visual Studio 2022「Desktop development with C++」、LLVM（clang-cl）、WebView2、Node.js 和 pnpm。

Windows 本机的 Rust / Tauri 命令必须经 `scripts\with-msvc.cmd`（载入 VS2022 环境、C 编译走 clang-cl、临时目录放仓库 `.tmp`）；直接 `cargo` 会因缺 Windows SDK 环境而失败。VSCode 的 `.vscode/tasks.json` 已内置这一层。

```powershell
pnpm install
scripts\with-msvc.cmd pnpm tauri dev   # 桌面壳
pnpm dev                               # 纯网页预览 http://localhost:1420
```

验证命令：

```powershell
pnpm lint:tokens
pnpm build
pnpm test
scripts\with-msvc.cmd cargo test --manifest-path src-tauri/Cargo.toml
scripts\with-msvc.cmd cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

界面结构、样式改动入口与“不要动”的约束见 [界面与样式指南](docs/ui.md)。

## 使用

1. 按 [Profile 1.0 契约](docs/profile-v1.md)准备设备 JSON。
2. 在“设备与连接”的“设备配置”中导入 JSON；建议先选择配置模拟器验证范围、缩放和操作流程。
3. 实机连接前确认串口参数、站号、接线和机械安全状态。
4. 在“参数工作区”读取、筛选和比较参数，在“实时监控”查看状态与曲线，再执行经确认的低风险写入。

[通用示例 Profile](examples/servo-profile.example.json)仅用于展示格式和模拟器测试，其中地址、范围和操作码均为虚构值，不得直接用于真实设备。

界面仿 FlClash Windows 方案（视觉规格见 [FlClash Windows 视觉方案](docs/flclash-win-spec.md)）：36px 自绘拖拽条 + 右上窗口按钮，左侧图标栏默认展开（220px，可收成 48px 图标栏），页面内大标题 + 卡片栅格仪表盘；深色为纯黑页 + `#191A1C` 卡片四级表面，主色 `#2F83FF`。设置页提供深色/浅色/跟随系统与 90/100/110% 字号。七个工作区为设备与连接、参数工作区、实时监控、通讯诊断、通信工作台（录制解析 / EtherCAT 主站双 Tab）、操作证据和设置；窄窗口（<840px）图标栏收为底部 5 项导航。自绘标题栏的最小化/最大化/关闭走 `core:window:*` 权限调用原生窗口。

## 关键文件

- `examples/servo-profile.example.json`：不对应任何真实设备的格式示例
- `src-tauri/src/profile.rs`：Profile 反序列化、唯一性、范围和缩放校验
- `src-tauri/src/modbus.rs`：FC03/FC06、RTU CRC / ASCII LRC、异常响应和帧间隔
- `src-tauri/src/modbus_tcp.rs`：Modbus TCP / MBAP、FC03/FC06、事务号校验与超时重试
- `src-tauri/src/runtime.rs`：串口 / TCP 独占、模拟器、安全写入、自动查找与寄存器扫描命令
- `src-tauri/src/discovery.rs`：查找候选排序、多波特率校验与串口格式遍历
- `src-tauri/src/scan.rs`：只读寄存器扫描的分块、取消与结果汇总
- `src-tauri/src/audit.rs`：SQLite 持久审计与快照存储
- `src/App.vue`：页面切换、全局状态横幅、确认弹层
- `src/shell/`：应用壳（自绘拖拽条 / 图标栏 / 窄屏底栏 / 主题与字号偏好）
- `src/components/ksu/`：卡片、行、开关、分段、按钮等 UI 原语
- `src/composables/useServo.ts`：应用状态单例
- `src/views/`：七个工作区视图（含设置）
- `src/features/{recording,ethercat}/`：录制解析 / EtherCAT 主站
- `src/features/ethercat/EthercatConnectCard.vue`：连接页的 EtherCAT 网卡接管入口
- `src/features/modbus/RegisterScanner.vue`：只读寄存器扫描卡片（地址范围 / 进度 / CSV）
- `src/styles/tokens-a.css`：设计令牌唯一色源（`pnpm lint:tokens` 门禁）
- `src/styles/ksu.css`：壳与组件的布局/交互样式；`views.css` 只放视图微调
- [docs/ui.md](docs/ui.md)：界面结构、改动入口与不能破坏的契约

## 验证边界

### 自动查找

导入对应设备 Profile，选择串口、校验位和停止位，保持断开状态后点击“自动查找”。先在当前站号上按所选波特率与 RTU / ASCII 的 8E1、8N1、8O1 依次探测，再遍历指定的站号范围（默认 1～247）及所选波特率。搜索波特率可多选；默认来自 Profile，打开“扩展波特率”后可加入常用档位（1200–115200），该选项只影响本次应用会话。每个组合只读取一个已定义寄存器，连续两次有效响应后回填站号、波特率及串口格式；点击“连接”后才开始常规读取。

查找不修改设备配置、不发送广播，支持取消并释放串口。默认探测超时为 200 ms；慢设备可增大到 2000 ms。查找期间不能连接或修改 Profile。未找到不代表设备不存在，还需核对校验位、停止位、Profile 地址和接线。

### 寄存器扫描

“设备与连接”页的寄存器扫描只使用 FC03 读取，不写入设备。地址范围为 0..65535，单次最多 4096 个地址，每块 1～100 个寄存器；失败块计数后继续，取消在当前事务结束后生效，结果可导出 CSV。扫描需要断开设备连接并独占链路（串口或 Modbus TCP），期间的报文仍进入现有录制。未知设备的部分读操作可能有厂商自定义副作用，应先核对手册；扫描结果不代表设备已通过兼容性验收。

### Modbus TCP

连接方式选择“TCP”后填写主机与端口，从站号作为 MBAP 单元标识；只使用 FC03/FC06，帧格式为 MBAP + PDU。TCP 不使用波特率、校验位和停止位，但仍沿用 Profile 的参数地址、缩放和写入风险规则。读取支持超时重试，写入与串口一样不会自动重发；事务号、协议标识、单元标识、长度或回显不一致都会返回诊断。原始收发按 `modbus-tcp` 协议进入通信录制；TCP 录制不包含串口级 CRC/LRC 统计。

自动查找和“扩展波特率”只适用于串口；TCP 设备需要已知主机/端口并使用固定站号。软件测试与本地环回测试不能代替真实网关/设备的兼容性验收。

### 便携版与 Linux

Windows 便携 ZIP 解压后运行 `servo-assistant.exe`，需要 WebView2 Runtime；随包的 `portable.marker` 使数据保存在程序旁的 `data` 文件夹，升级时保留该目录。安装版继续使用系统应用数据目录。

Linux x64 在 Ubuntu 22.04 构建：Debian 12 及以上使用 DEB，Fedora 使用 RPM，Arch Linux 使用 AppImage。发布流程在 Debian 12、Fedora 和 Arch 容器中检查安装与动态库依赖；桌面显示和真实串口仍需目标系统验证。串口使用前需确保当前用户有目标设备节点的读写权限。Linux 依赖参见 [Tauri 官方说明](https://v2.tauri.app/start/prerequisites/#linux)。

### 读取与采集行为

- 通讯设置在当前应用会话内生效：默认每组最多 16 个寄存器（1～100），CRC / LRC（TCP 无校验）/ 超时额外重试 2 次（0～3）。仅 FC03 读取可重试，FC06 写入不会自动重发；设备异常响应、错误站号和格式错误直接返回诊断。
- 参数读取逐组显示进度，成功组即时更新，失败或取消时保留旧值并标记过期；可以只重读失败 / 未完成组。刷新保留未提交编辑；过期值须重新读取后才能单项写入。
- 状态轮询与曲线采集分别控制。默认 250 ms 是一次状态读取完成后的等待时间，不是精确采样周期；支持 200/250/500/1000/3000 ms 及 50～60000 ms 自定义。通讯恢复自动恢复正常间隔，不清除其他操作的错误。
- 只读稳定性测试交替读取 Profile 中最长连续状态区的 1 个和最多 100 个寄存器。长帧对照有意不按分组上限拆分；默认 100 轮，可选 1～1000 轮。测试期间暂停日常轮询；取消等待当前事务（含有限重试）结束，随后保留统计并按原开关恢复轮询。
- 首次成功、重试恢复、最终失败分别统计；CRC、超时、重试次数和最近失败地址/数量/尝试次数/接收字节可见。真实串口或 TCP 每次读取成功、失败和重试写入现有 SQLite 操作证据；模拟器测试结果不能证明实机链路稳定。
- 前端状态回归运行 `pnpm test`；串口故障注入、TCP 环回与原有核心测试运行 `cargo test --manifest-path src-tauri/Cargo.toml`。故障测试覆盖部分帧超时、CRC 恢复、重试上限、设备异常不重试、写入不重发、长短帧读取边界和 MBAP 事务校验。

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
