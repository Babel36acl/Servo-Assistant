# 安全政策

## 报告漏洞

请走 GitHub **Private vulnerability reporting**（仓库 Security 页 → Report a vulnerability），不要直接开公开 Issue。内容请包含：影响版本、复现路径、影响范围评估。

## 已知边界

- 依赖漏洞由 Dependabot 每周扫描（`npm` + `cargo` 分组），合并前必须跑完全部门禁。
- 本程序会经串口向设备**写入参数**：任何写入路径必须保留"写前比较 + 回读校验"（见参数事务实现），新增写入入口需在 PR 中说明校验链。
- 录制/日志可能含现场信息：提 Issue 或传录制文件前，先脱敏（去掉真实设备地址与路径）。
