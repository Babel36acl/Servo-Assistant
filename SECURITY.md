# 安全政策

## 报告漏洞

请走 GitHub **Private vulnerability reporting**（仓库 Security 页 → Report a vulnerability），不要直接开公开 Issue。内容请包含：影响版本、复现路径、影响范围评估。

## 已知边界

- 依赖漏洞由 Dependabot 每周扫描（`npm` + `cargo` 分组），合并前必须跑完全部门禁。
- 本程序会经串口/TCP 向设备**写入参数**：任何写入路径必须保留“写前比较 + 回读校验”（见参数事务实现），新增写入入口需在 PR 中说明校验链。
- Modbus TCP 只连接用户填写的主机/端口，不监听端口、不做设备发现广播；EtherCAT 主站会独占所选网卡并接管总线，需要 Npcap 权限，使用前必须停用其他主站。
- Profile 是声明式 JSON，不执行脚本；程序不内置厂商资料，也不会自动从网络下载设备描述。
- 录制/日志可能含现场信息：提 Issue 或传录制文件前，先脱敏（去掉真实设备地址与路径）。
