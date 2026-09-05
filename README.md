# 伺服参数调试器

[![CI](https://github.com/AKCX2002/Servo-Assistant/actions/workflows/ci.yml/badge.svg)](https://github.com/AKCX2002/Servo-Assistant/actions/workflows/ci.yml)

一个设备中立的 Tauri 2 GUI 调试器。程序不内置厂商、型号、参数表或手册内容；设备兼容性完全由用户导入的 JSON Profile 描述。

```text
JSON Profile 导入
  → Modbus RTU 连接
  → FC03 读取和缩放
  → 写前值比较
  → FC06 安全写入
  → 立即回读校验
  → 可选应用/持久化操作
```

## 当前能力

- 导入并校验设备 JSON Profile，不限制参数 ID 命名方式或寄存器地址布局。
- 支持安全模拟器和真实 Modbus RTU 串口连接。
- 支持参数读取、单项安全写入、快照导出、差异比较和选择性批量写入。
- 高风险参数需要输入 `写入 <parameterId>`；所有写入都执行写前值比较和写后回读。
- Profile 可选择定义应用和持久化命令；未定义时 GUI 不允许执行对应操作。
- 操作审计和参数快照保存在应用数据目录的 `servo.db`。
- Profile 可声明任意状态寄存器，GUI 最多显示 6 个实时曲线通道。
- 不提供恢复出厂值、自动调参、站号穷举或通讯参数在线修改。

本仓库不包含任何特定厂商的参数库、说明书摘录、品牌资源或资料链接。设备名称、参数定义及操作码应由使用者从有权使用的资料中制作并导入。项目与任何设备制造商不存在隶属、授权或背书关系。

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
- `src-tauri/src/modbus.rs`：FC03/FC06、CRC、异常响应和 RTU 帧间隔
- `src-tauri/src/runtime.rs`：串口独占、模拟器、安全写入和可选设备操作
- `src-tauri/src/audit.rs`：SQLite 持久审计与快照存储
- `src/App.vue`：配置导入、连接、差异比较和确认界面

## 验证边界

自动化测试和模拟器只能验证软件契约。每份设备 Profile 仍需针对其目标设备分别完成读取、缩放、写入、回读、应用和持久化验收；未定义或未实测的设备能力不能视为已兼容。

## 自动构建与发布

- 推送到 `main`、提交 Pull Request 或手动运行 `CI` 工作流时，会在 Windows runner 上执行前端构建、Rust 测试、Clippy 检查和 Tauri 安装包构建。MSI/NSIS 制品可从对应的 Actions 运行记录下载。
- 推送格式为 `v<SemVer>` 的标签时，`Release` 工作流会先核对 `package.json`、`src-tauri/tauri.conf.json` 和 `src-tauri/Cargo.toml` 的版本，再构建 Windows 安装包并发布 GitHub Release。

发布 `0.1.0` 的示例：

```powershell
git tag v0.1.0
git push origin v0.1.0
```

如果三个项目版本与标签不一致，发布会在生成 Release 前失败，不会留下半成品发布。

当前流水线未配置 Windows 代码签名证书，安装包能够正常构建，但从浏览器下载后可能触发 SmartScreen 提示。

## 许可证

本项目采用 [GNU General Public License v3.0 only](LICENSE)。
