修复设备连接及通信、录像功能被 Tauri ACL 拒绝的问题。

- 补齐 `configure_communication` 权限，修复点击连接时出现 `Command configure_communication not allowed by ACL` 的错误。
- 同步补齐自动查找、通信统计、探测读取及录像事务、历史、上下文与会话管理等共 14 个现有命令的权限。
- 权限继续仅授予本地主窗口，保留明确的命令白名单。
- 新增前端 IPC 调用、后端注册与权限白名单一致性回归检查，防止同类遗漏。

本次修复通过软件测试与构建验证，真实串口连接仍需设备验收。

在线 EtherCAT 需要用户安装 Npcap；当前监控为非实时 SDO 轮询，不含运动控制。
暂无设备，实机长时间录制、真实磁盘满/断电与独立抓包对照尚未验收。子进程强退恢复测试不等同于物理断电验证。
历史曲线重建完整成功的 Modbus FC03 事务或模拟器状态采样，不补造丢失数据；外部 PCAP 暂不重建 EtherCAT 过程数据曲线。

- Windows：安装版 EXE / MSI，或解压即用的便携 ZIP。
- Debian：DEB；Fedora：RPM；Arch Linux：AppImage（均为 x64）。

便携版的数据保存在程序旁的 `data` 文件夹；升级时请保留。Windows 便携版需要 WebView2 Runtime。
