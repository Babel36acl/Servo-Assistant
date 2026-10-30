# 发布说明

本文件会被 `Release` 工作流用作 GitHub Release 说明。**打 `vX.Y.Z` 标签前请更新本文件**，并保持与标签版本一致。

## 本版内容（待发布）

- 连接方式菜单改为模拟器 / 串口 / Modbus TCP / EtherCAT 四选一；TCP 使用 MBAP + FC03/FC06，EtherCAT 复用 PRE-OP 主站接管流程。
- 自动查找支持多选搜索波特率与会话级扩展波特率；只读寄存器扫描支持串口与 TCP，可导出 CSV。
- EtherCAT 新增从站只读诊断：AL 状态、活动端口/拓扑、DC、ESC 0x0300..0x0311 原始错误计数。
- EtherCAT 新增在线对象字典发现：SDO Info 只读读取，生成整数对象 Profile（512 上限）。
- 文档新增扩展路线图，并同步 Profile、EtherCAT 与界面说明。

## 验证

- 前端：`pnpm lint:tokens`、`pnpm build`、`pnpm test`（25 项）通过。
- Rust：`cargo test --lib` 71 项通过（1 项崩溃恢复子进程入口按设置忽略）；`cargo clippy -- -D warnings` 通过。
- **EtherCAT 未在真实从站上验收**：诊断、对象字典与 SDO 读写目前只有代码/ABI/模拟验证；首次实机必须独占总线并先做只读操作。

## 安装包

- Windows：MSI、NSIS 安装 EXE，或解压即用的便携 ZIP（需要 WebView2 Runtime）。
- Linux x64：DEB（Debian 12+）、RPM（Fedora）、AppImage（Arch 等）。
- 在线 EtherCAT 需要单独安装 Npcap 并重启应用。

## 已知边界

- 主站仅 PRE-OP：不进入 OP、不使能伺服、不发送周期运动输出。
- 不提供恢复出厂值、自动调参、EEPROM 写入或 FoE 固件升级。
- 自动化测试与模拟器只验证软件契约，不代表任何具体设备的兼容性。
