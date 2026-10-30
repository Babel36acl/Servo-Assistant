# Servo Assistant 扩展方向与路线图

> 2026-10-30 · 基于 `40cfcf4`（v0.4.3）只读盘点
> 范围：EtherCAT 主站能力、Modbus 设备层、调试/验收工作流、库选型
> 性质：分析文档，不含实现；本文不改变任何现有 Profile、录制、IPC 或界面契约

## 1. 结论摘要

1. **最大缺口在 EtherCAT 主站**：当前只走到 PRE-OP + CoE/SDO 轮询。vendored 的 SOEM v1.4.0 已经包含对象字典浏览、过程数据（PDO）、分布式时钟（DC）、EEPROM/SII、FoE 等能力，但 `bridge.c` 只暴露了适配器/打开/从站列表/SDO 读写/捕获等少量入口。扩展瓶颈在 C ABI 与 UI，不在库能力。
2. **优先做只读三件套**：从站诊断与拓扑、CoE 对象字典在线发现、ESI（XML）导入。三件事都不改变总线状态，能把“手写对象表/手工猜映射”变成“标准设备描述 + 在线确认”，符合项目“不内置厂商表、设备中立”的定位。
3. **不建议当前整体迁移 EtherCrab**。它的价值在纯 Rust/async/跨平台（Linux），代价是重写主站桥接、pre-1.0、只有 CoE（无 FoE/EoE/冗余）、自动配置只读。SOEM 已 vendored 且带本地补丁，增量扩展成本低得多；EtherCrab 留作 Linux/跨平台成为硬需求时的 PoC。
4. **Modbus 侧**优先补寄存器扫描/搜索、FC16/FC23、Profile 编辑器/CSV 导入；协议层扩展比再堆设备 Profile 更有杠杆。

## 2. 现状盘点

### 2.1 已具备

| 模块 | 能力 |
|---|---|
| 应用 | Tauri 2 + Vue 3；7 个工作区；48 个 Tauri 命令；FlClash 视觉体系；浅/深色与字号缩放 |
| Modbus | RTU / ASCII；FC03/FC06；自动查找（站号范围、多选波特率、RTU/ASCII × 8E1/8N1/8O1，可显式扩展 Profile 外波特率）；只读寄存器扫描（FC03，CSV 导出）；分组读取、重试、写前比较、写后回读、批量写、快照导出/比较、SQLite 审计 |
| 录制 | JSONL 有界队列/分卷/摘要；全会话索引与检索；故障触发录制；PCAP/PCAPNG 导入导出；Modbus 事务与 EtherCAT 帧解析 |
| EtherCAT | Windows + Npcap；SOEM 1.4.0 静态编译；PRE-OP 下发现从站、CoE SDO 读写、6 通道轮询曲线（300 点） |
| 测试 | 前端 `node --test` 24 项；Rust 66 项 + 1 个崩溃恢复子进程入口（忽略） |

### 2.2 明确边界

| 边界 | 现状 |
|---|---|
| EtherCAT 总线状态 | 只请求 PRE-OP；不映射 PDO、不进入 OP、不使能伺服、不发送周期输出 |
| EtherCAT 设备描述 | 不内置厂商对象字典；不导入 ESI；不自动猜 PDO/邮箱；Profile 由用户手写 JSON |
| EtherCAT 维护操作 | 不写 EEPROM、不做 FoE 固件升级、不提供厂商专有操作 |
| 平台 | 主站与在线捕获仅 Windows；Modbus 与离线功能跨平台 |
| 实时性 | SDO 轮询与状态读取不是硬实时；Windows + Npcap 存在毫秒级抖动 |

### 2.3 可扩展的底层能力（源码已存在，未暴露）

`src-tauri/native/soem`（v1.4.0，commit `abbf0d42`）已包含：

| 能力 | SOEM API | 是否已接出 |
|---|---|---|
| 从站状态/AL 状态码/端口/DC 信息 | `ec_readstate`、`ec_slave[]` 结构体 | 部分（只读状态与 AL 码） |
| 对象字典浏览（SDO Info） | `ec_readODlist` / `ec_readODdescription` / `ec_readOEsingle` | 否 |
| 过程数据（PDO/OP） | `ec_config_map` / `ec_send_processdata` / `ec_receive_processdata` | 否 |
| 分布式时钟 | `ec_configdc` / `ec_dcsync0` / `ec_dcsync01` | 否 |
| EEPROM/SII | `ec_readeeprom` / `ec_writeeeprom` / `ec_sii*` | 否 |
| 固件传输 | `ec_FOEwrite` / `ec_FOEread` | 否 |
| 邮箱隧道 | `ec_EOE*`、SoE | 否 |

这决定了当前阶段的合理做法：**扩展 bridge 的 C ABI 与前端，而不是先换主站库**。

## 3. 业内常用功能对标

| 常用功能 | 商业工具/资料 | 本项目 | 建议 |
|---|---|---|---|
| 从站扫描、AL 状态/错误码、端口状态、拓扑、错误计数 | [TwinCAT 诊断](https://download.beckhoff.com/download/Document/automation/twincat3/TwinCAT_3_IO_EN.pdf)、[Kollmorgen EtherCAT 排障清单](https://www.kollmorgen.cn/sites/default/files/public_downloads/AKD2G%20EtherCAT%20Troubleshooting%20List_RevA.pdf)、[EC-Lyser](http://motrotech.com/ethercat/EC-Lyser.html) | 从站列表 + AL 状态码；无端口/计数/拓扑 | P0 |
| CoE 对象字典在线浏览 | TwinCAT “CoE Online”、各类主站配置工具 | 无；对象靠手写 JSON | P0 |
| ESI/XML 设备描述导入、Revision 比对 | TwinCAT、EC-Lyser、ZLG 从站工具 | 明确不做 | P0 |
| PDO 过程数据 / 实时示波器 | TwinCAT Scope、伺服调试软件 | 只有 SDO 轮询，非周期采样 | P1 |
| DC 诊断（SYNC0 抖动、传播延迟） | TwinCAT、EC-Lyser | 无 | P1 |
| CiA 402 状态机/使能/故障复位 | 各家调参软件 | 明确不做 | P1/P2（需安全门） |
| 惯量辨识、自动调谐、频率响应、陷波器 | [三菱 MR Configurator2](https://fa-dic.mitsubishielectric.co.jp/fa/products/drv/servo/smerit/mr2/05.html)、[Beckhoff TE5960](https://document.beckhoff.com/te5960.pdf)、[Veichi](https://www.veichi.cn/Uploads/20250205014412_988107.pdf)、[Servotronix](https://www.servotronix.cn/file/product/1049/20260817115718185846490.pdf)、[Omron](https://www.fa.omron.com.cn/data_pdf/mnu/dilm-cn5-01c_final.pdf) | 无 | P2（优先驱动侧自整定） |
| 波形记录/触发、FFT、阶跃指标、报告 | 各家伺服调试软件 | 曲线 300 点；无分析、无报告 | P1 |
| FoE 固件升级、EEPROM 写 | [EC-Lyser](http://motrotech.com/ethercat/EC-Lyser.html)、TwinCAT、ZLG | 明确不做 | P2/不做 |
| Modbus TCP、FC16/FC23、寄存器扫描 | 通用 Modbus 调试器 | 仅 RTU/ASCII FC03/06；无原始寄存器扫描 | P1/P2 |
| 自动更新、i18n、插件化 | 桌面软件常备 | 无 | P2 |

## 4. 扩展方向

### 4.1 P0：只读能力（不改变总线状态）

#### 4.1.1 从站诊断与拓扑

- 目标：定位掉站、错帧、链路异常、DC 异常，接近商业主站的基础诊断能力。
- 数据项：AL 状态/状态码、`activeports` / `parentport` / `entryport`（可推导物理拓扑）、SII 身份（序列号、别名）、DC 能力与偏移、邮箱协议、0x0300 起的错误计数寄存器（`ec_FPRD` 读）、单次广播读的 WKC。
- 落点：`bridge.c` 新增固定宽度诊断结构；`master.rs` 新增查询命令；从站列表行展开诊断详情；只读，不写总线。
- 验收：拔插/断链时状态与计数变化可见；错误码与实际 AL 状态一致；操作可录制、可审计。

#### 4.1.2 CoE 对象字典在线发现

- 目标：从在线从站读取对象字典，自动生成 `ethercat-1.0` Profile，替代手工维护 `index/subIndex/size/scale`。
- 做法：`ec_readODlist` → `ec_readODdescription` → `ec_readOEsingle`，分页、限流、可取消；生成对象表供用户确认后保存；读取失败或超时明确报错，不猜测。
- 落点：`master.rs` 新命令 + `EthercatMasterView` 的“从设备生成 Profile”入口。
- 验收：对真实从站导出的对象表与厂商 ESI/手册抽样一致；只读；不覆盖用户既有 Profile，除非显式确认。

#### 4.1.3 ESI（XML）导入

- 目标：导入用户自己的 ESI 文件，生成 Profile、PDO 解码映射，并与在线设备 Revision 比对。
- 解析内容：Vendor/Product/Revision、对象字典、RxPdo/TxPdo、SyncManager、DC、邮箱能力。
- 依赖：Rust 侧 ESI 解析库，例如 [`ethercat-esi`](https://crates.io/crates/ethercat-esi) 或 [`taktora-ethercat-esi`](https://crates.io/crates/taktora-ethercat-esi)；只解析用户导入的文件，不随包分发厂商资料。
- 落点：新增 `src-tauri/src/esi.rs`；`ethercat_profile` 接受“由 ESI 生成”的来源标记；UI 导入入口。
- 验收：导入 ESI 后能生成可读 Profile；PDO 映射与 ESI 一致；Revision 不一致时告警但不阻断。

#### 4.1.4 波形分析与调试报告

- 目标：把现有曲线和审计数据变成可交付的验收材料。
- 内容：FFT/频谱、游标、阶跃指标（上升时间/超调/稳定时间）、数学通道、多通道叠加、CSV/PNG 导出；由审计 + 快照 + 曲线 + 录制摘要生成 HTML/PDF 报告。
- 落点：前端图表组件与导出模块；Rust 侧报告模板；不改变录制格式。
- 验收：导出结果可复现；报告中每个结论能追溯到审计/快照/录制记录。

### 4.2 P1：周期总线（需独立安全门）

#### 4.2.1 PDO 只读示波器

- 目标：读取位置/速度/转矩/跟随误差等过程数据，形成真正的伺服调试观测能力。
- 做法：`ec_config_map` + 输出区强制清零 → SAFE-OP/OP → 1–4 ms 线程 `ec_send_processdata` / `ec_receive_processdata`；输入 PDO 解析复用现有解码逻辑；环形缓冲、抽稀到 UI 50–100 Hz；原始采样写入现有录制。
- 安全设计：独立“实时总线”模式 + 确认短语；进入前确认无其他主站；断线/看门狗回 SAFE-OP/INIT；动作全部进审计。
- 限制：Windows 非硬实时，周期抖动为毫秒级，只做观测与配置，不做运动控制；文档中必须写明。
- 验收：与驱动器内部波形/示波器对照，采样值正确；拔线后能安全退出；录制可回放。

#### 4.2.2 CiA 402 状态机（可选）

- 内容：状态字/控制字显示、shutdown/switch on/enable/fault reset、模式设置。
- 前提：PDO 通路稳定；独立授权层、默认关闭、双确认；不在普通调试流程中默认使能。

### 4.3 P2：调谐与维护（谨慎）

#### 4.3.1 驱动侧自整定/惯量辨识向导

- 多数伺服驱动器自带 autotune、惯量辨识、陷波器设置；PC 工具只需触发、读回结果并可视化。
- 做法：在 Profile 中声明“动作对象/结果对象”，复用现有 SDO 写前比较、确认短语、写后回读与审计管线；不自研控制环、不依赖 PDO 输出。
- 边界：没有厂商对象就不提供；不内置厂商参数。

#### 4.3.2 FoE 固件升级 / EEPROM 写

- 与项目“不自动写 EEPROM、不提供厂商操作”的原则冲突。建议默认关闭；如确需，作为独立“维护模式”，编译/配置关闭、双确认、全程审计。

### 4.4 Modbus 设备层

- **寄存器扫描/搜索（已落地，2026-10-30）**：`RegisterScanner` 按 FC03 分块读取指定站号与地址区间，列出非零值，支持包含 0 值、取消与 CSV 导出；单次最多 4096 个地址、每块 1～100 个寄存器。仅读、不写；扫描期间独占串口，需要断开设备连接；失败块计数后继续；报文进入现有录制，结果写入审计。
- **自动查找多波特率（已落地，2026-10-30）**：搜索波特率可多选，默认来自 Profile；新增会话级“扩展波特率”开关，可显式启用 Profile 之外的常用档位（1200～115200），不修改 Profile。候选顺序改为先在当前站号上遍历所选波特率与串口格式，再扫描其余站号，避免为了换一个波特率先扫完整站号范围。
- **FC16 / FC23**：批量写由逐条 FC06 改为一次多寄存器；保持写前比较与回读策略。
- **Modbus TCP**：传输层抽象为 Serial / TCP，扩大设备覆盖；Profile 1.0 契约不变，新增字段可选。
- **Profile 编辑器 / CSV 导入**：`housine/` 下现有 214/136 条参数为手工维护；提供 CSV/Excel → JSON 转换、校验、diff，降低 Profile 制作成本。

### 4.5 工程化

- 自动更新（`tauri-plugin-updater`）、崩溃与错误上报；
- 中英双语（当前界面硬编码中文）；
- 多设备/多轴会话（当前单连接、单 Profile）；
- 传输层 trait 化，为后续 EtherCAT/Modbus TCP/串口统一接口。

## 5. 库选型

| 库 | 语言/许可 | 能力 | 对本项目 |
|---|---|---|---|
| SOEM 1.4.0（已 vendored） | C，GPLv2 + linking exception | CoE/SDO Info/PDO/DC/EEPROM/FoE/EoE；Windows 经 Npcap | 继续使用；增量扩展 |
| [EtherCrab 0.7.x](https://docs.rs/ethercrab/latest/ethercrab/) | Rust，MIT/Apache-2.0 | 纯 Rust、async、CoE/PDO/DC、SDO Info；Windows 走 Npcap | 长期备选；先 PoC |
| IgH EtherLab | C，Linux 内核模块 | 能力最全 | Windows 桌面不适用 |
| ethercat crate | Rust，绑定 IgH | 继承 Linux 约束 | 不适用 |
| ESI 解析 crate | Rust | 离线 XML → 类型化 IR | 建议引入 |

EtherCrab 的已知边界（第三方评估，0.7.1）：只有 CoE，无 FoE/EoE/线缆冗余；自动配置只读、不解析 ESI、不支持模块化耦合器；仍处 pre-1.0。迁移成本不只是换 crate，还包括重写 `bridge.c` + `master.rs` 的同步状态机、录制 hook、权限审计与回归测试。

**触发迁移的条件**（满足任一再做 PoC）：Linux 主站成为硬需求；需要 async 集成；要求纯 Rust 供应链；SOEM 维护停滞。

## 6. 路线图

| 阶段 | 内容 | 粗略工作量 | 风险 |
|---|---|---|---|
| P0 | 从站诊断/拓扑；SDO Info 对象字典发现；ESI 导入（全部只读） | 2–3 周 | 低 |
| P1 | PDO 只读示波器；波形分析/报告；Modbus FC16/FC23、TCP、Profile 编辑器 | 4–8 周 | 中 |
| P2 | 驱动侧自整定向导；CiA 402 可选；EtherCrab PoC；FoE/EEPROM 维护模式 | 按需 | 高 |

每阶段沿用现有门禁：

```text
pnpm lint:tokens
pnpm build
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

另需实机总线验收；自动化测试与模拟器只能验证软件契约，不等于设备兼容性。

## 7. 边界与不做

- 不内置厂商对象字典、不猜 PDO/邮箱映射、不自动写 EEPROM；
- 不承诺硬实时；
- 未完成“接管总线”授权时不进入 OP；未独立授权时不使能伺服；
- FoE/EEPROM 默认关闭，避免把调试器变成维护工具；
- 库迁移不改变现有 Profile/录制/IPC 契约。

## 8. 已确认与后续

- **Modbus 搜索已确认并落地（2026-10-30）**：只读寄存器扫描 + 自动查找多波特率选择/会话级扩展波特率。实现见 `src/features/modbus/RegisterScanner.vue`、`src-tauri/src/scan.rs`、`src-tauri/src/discovery.rs`、`src-tauri/src/runtime.rs`。
- 后续候选：站号结果列表/多设备枚举、扫描结果生成 Profile 草稿、FC16/FC23、Modbus TCP、Profile 编辑器（见 4.4）。
- 本轮未改动 EtherCAT 侧；4.1–4.3 仍按原优先级评估。

## 9. 本轮验证（2026-10-30）

- `pnpm lint:tokens`：通过（仅 `styles.css` 存量冻结告警）。
- `pnpm build`：通过（vue-tsc 0 错误，79 模块）。
- `pnpm test`：24 项通过（新增自动查找多波特率请求断言）。
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`：66 项通过，1 项崩溃恢复子进程入口按设置忽略。
- `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`：通过。
- 未做：实机串口上的多波特率查找与寄存器扫描。当前环境没有可确认的目标设备，软件测试与模拟器不等于实机验收；首次实机使用应先断开其他主站/软件，从小区间、低风险站号开始。

## 附录 A：SOEM 能力与暴露对照

| 能力 | API | 当前暴露 | 计划 |
|---|---|---|---|
| 从站状态 | `ec_readstate`、`ec_slave[]` | 是 | 扩展诊断字段 |
| 对象字典 | `ec_readODlist` 等 | 否 | P0 接出 |
| 过程数据 | `ec_config_map`、`ec_send/receive_processdata` | 否 | P1 接出 |
| DC | `ec_configdc`、`ec_dcsync0` | 否 | P1 诊断/配置 |
| EEPROM/SII | `ec_readeeprom`、`ec_writeeeprom`、`ec_sii*` | 否 | 只读先行 |
| FoE | `ec_FOEwrite`、`ec_FOEread` | 否 | 默认不做 |

## 附录 B：参考链接

- EtherCAT 技术：https://www.ethercat.org/en/technology.html
- EtherCAT 帧解析（Wireshark）：https://www.wireshark.org/docs/dfref/e/ecat.html
- Npcap 指南：https://npcap.com/guide/npcap-api.html
- EtherCrab 文档：https://docs.rs/ethercrab/latest/ethercrab/
- 开源 EtherCAT 主站对比：https://www.mutexer.com/blog/ethercat-open-source-libraries
- TwinCAT 3 I/O 诊断：https://download.beckhoff.com/download/Document/automation/twincat3/TwinCAT_3_IO_EN.pdf
- Beckhoff TwinCAT Autotuning：https://document.beckhoff.com/te5960.pdf
- Kollmorgen EtherCAT 排障清单：https://www.kollmorgen.cn/sites/default/files/public_downloads/AKD2G%20EtherCAT%20Troubleshooting%20List_RevA.pdf
- 三菱 MR Configurator2：https://fa-dic.mitsubishielectric.co.jp/fa/products/drv/servo/smerit/mr2/05.html
- Servotronix HD 控制器手册：https://www.servotronix.cn/file/product/1049/20260817115718185846490.pdf
