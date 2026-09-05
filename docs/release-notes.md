新增 Modbus ASCII 通信与 RTU/ASCII 串口格式自动查找，支持 P300–P302 连接预设，保留 FC03 读取与 FC06 写前比较、写后回读。

- 新增 EtherCAT 标准帧解析、Windows PRE-OP CoE/SDO 配置与监控。
- 新增串口、以太网和模拟器统一录制、离线浏览与以太网 PCAPNG 导出。
- 回移 glib 上游 VariantStrIter 修复（GHSA-wrw7-89jp-8q8g），兼容 Tauri Linux GTK3 依赖，并在 Linux 优化构建中执行回归测试。
- main 构建、测试与跨发行版包验证全部成功后自动发布新版本。

在线 EtherCAT 需要用户安装 Npcap；真实 Modbus 串口与 EtherCAT 从站仍需实机验收。当前 EtherCAT 监控为非实时 SDO 轮询，不含运动控制。

- Windows：安装版 EXE / MSI，或解压即用的便携 ZIP。
- Debian：DEB；Fedora：RPM；Arch Linux：AppImage（均为 x64）。

便携版的数据保存在程序旁的 `data` 文件夹；升级时请保留。Windows 便携版需要 WebView2 Runtime。
