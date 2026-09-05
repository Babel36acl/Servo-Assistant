# EtherCAT 配置、监控与通信录制

通信工作台包含独立的录制/解析和 EtherCAT 主站入口。原有 Modbus Profile 1.0、
串口独占、FC03 有限重试和 FC06 不自动重发契约保持不变。

## 使用入口

1. 点击“开始录制”，再使用原有模拟器或串口功能。录制不依赖曲线显示开关。
2. 停止后，从历史会话打开录制；逐页查看原始字节、来源、方向、事务和错误。
3. 打开 PCAP/PCAPNG 文件时输入本机完整路径；支持分卷逐个打开，大文件流式读取。
4. EtherCAT 帧点选后显示标准字段。可导入解析映射，或从当前主站发现结果填入邮箱地址。
5. 在线网卡捕获单独启停，始终独立保存 PCAPNG；若统一录制同时开启，还会写入共同时间线。
   同时捕获主站自己的网卡会产生两个来源的观测，不做猜测性去重。

## 标准解析范围

- Ethernet、嵌套 VLAN、EtherType 0x88A4、EtherCAT Type 1、多 Datagram。
- NOP、APRD/APWR/APRW、FPRD/FPWR/FPRW、BRD/BWR/BRW、LRD/LWR/LRW、ARMW/FRMW。
- 地址、索引、长度、循环/后续标志、IRQ、WKC、原始负载；截断或长度矛盾显示错误。
- 已知物理地址的 AL 状态、AL 状态码、DC 系统时间。WKC 不自动推断预期值。
- 显式邮箱映射下的邮箱头、CoE SDO 快速传输/分段头/Abort、FoE、SoE、EoE、AoE 标准头。
  邮箱深层业务和离线跨帧重组尚未实现；分段保留原始字段，不冒充完整对象。
- 显式逻辑 PDO 映射的 1～64 位整数、小端位字段、有符号和缩放解码。
  64 位原始整数用字符串显示，工程值为浮点近似。
- 不内置厂商对象字典，不根据负载猜邮箱或 PDO，不自动导入 ESI 或重建 FMMU 配置。

解析映射参考 `examples/ethercat-decode.example.json`。邮箱项包含 `station`（配置站地址）和
`offset`（实际邮箱寄存器起始地址）；PDO 项包含 `logicalAddress`、`bitOffset`、
`bitLength`、`signed`、`scale` 和 `unit`。必须来自当前实际工程配置。

## 主站配置与监控

Windows 需要单独安装 Npcap 并重启应用。主站采用随源码提供的 SOEM，通过 MSVC 静态编译，
Npcap 动态加载；不需要用户额外构建或寻找 SOEM DLL。

连接前选择专用网卡并输入“接管 EtherCAT 总线”。此动作会初始化从站和邮箱、请求 PRE-OP，
属于主动总线操作。先停用其他主站并确认设备处于适合配置的状态。

发现列表显示位置、配置地址、Vendor/Product/Revision、AL 状态和错误码。
导入 `ethercat-1.0` Profile 后，只有 Vendor/Product 匹配且从站声明 CoE 时才能访问对象。
Profile 示例在 `examples/ethercat-profile.example.json`；示例标识和地址均虚构。

对象以 `index/subIndex` 标识，支持 1/2/4/8 字节整数；`min/max` 使用十进制原始整数
字符串，`scale` 只用于工程值显示。默认只读。写入要求：对象允许写入、值在范围内、
输入“写入 对象ID”、重新读取值与界面原始值一致、先持久记录写入尝试、再发送并回读。
超时或回读不一致会使显示值过期，不自动重复应用层写入。SOEM 保留标准链路级重试。
应用没有自动保存 EEPROM、恢复出厂值或执行厂商操作的路径；此类对象必须由使用者明确定义。

监控最多选择六个对象，以 SDO 轮询并绘制最近 300 点。轮询间隔为一轮读取后的等待，
不是实时 PDO 周期；主站不进入 OP、不使能伺服、不发送周期运动输出。

## 录制契约

统一录制位于应用数据目录 `recordings/session-*`：

- `session.json`：格式版本、时间单位、容量和采集范围。
- `records-NNNN.jsonl`：有界队列、后台单写者、16 MiB 分卷；每行保存原始字节及事件。
- `summary.json`：正常结束/失败状态、接收/写入/队列丢弃/未写入计数。
- 可导出其中以太网帧为 PCAPNG。串口和模拟器不伪装成以太网包。

默认容量 1024 MiB，允许 16～16384 MiB。达到上限停止录制，不删除旧文件。
队列溢出丢弃新记录并计数，通信事务继续；停止时先解除入口再排空队列。
断电/崩溃只能恢复已落盘的完整行，损坏尾行显示警告；没有完成摘要不能视为正常结束。
持续写入时由缓冲大小触发写盘，空闲时每 250 ms flush，正常结束执行 sync_all。

串口每次 read/write 的实际字节都记录，包含部分收发、超时、CRC 错误、每次事务和缓冲
清空事件。OS 接受写入不等于已在线缆发送；缓冲区清除掉的未读取字节无法追回。
不同串口连接通过来源和事务编号区分；模拟器记录语义事件，明确注明模拟来源。

在线捕获位于 `captures/capture-*`，每卷 `frames-NNNN.pcapng`，并保存最终摘要。
可录所有可见以太网协议或仅 EtherCAT，内核丢包和录制队列丢弃分别统计。
方向无法确定时标为 unknown，原始 PCAPNG 保留捕获长度和原长。
PCAPNG 默认微秒时间戳，导入纳秒时间戳在界面转换为微秒并标注精度；输入文件保持不变。
捕获范围取决于主站位置/TAP/镜像接线，不能保证看到其他设备所有通信。

录制索引目前按文件顺序扫描分页，筛选作用于当前页；未实现全会话数据库索引和倍速播放。
打开/翻页仅离线读取，绝不将历史报文重发到设备。

## 验证与设备验收

执行 `pnpm test`、`pnpm build`、`cargo test --manifest-path src-tauri/Cargo.toml --locked`、
`cargo clippy --manifest-path src-tauri/Cargo.toml --locked -- -D warnings`。
测试覆盖解析边界、PCAPNG 往返、录制恢复、串口实际故障注入路径以及前端写入失败失效处理。

实机仍需核对：独占总线上的发现和 PRE-OP 状态；匹配 Profile 的读取和低风险写入回读；
持续录制下帧数、驱动丢包、时序与 Wireshark 字段对照。软件测试不等于设备兼容性认证。

协议依据：

- https://www.ethercat.org/en/technology.html
- https://www.wireshark.org/docs/dfref/e/ecat.html
- https://www.wireshark.org/docs/dfref/e/ecat_mailbox.html
- https://npcap.com/guide/npcap-api.html
- https://github.com/OpenEtherCATsociety/SOEM/tree/v1.4.0

第三方来源、许可证和原生边界详见 `src-tauri/native/README.md`。
