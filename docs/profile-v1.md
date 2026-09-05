# Servo Profile 1.0

Profile 是声明式 JSON 数据，用于把设备参数 ID、Modbus 地址、通讯整数和工程单位连接起来。运行时不会执行 Profile 中的脚本，也不包含任何厂商专属地址推导规则。

## 顶层结构

| 字段 | 必填 | 含义 |
| --- | --- | --- |
| `schemaVersion` | 是 | 当前固定为 `1.0` |
| `device` | 是 | 设备 ID、显示名称和 Profile 版本 |
| `transport` | 是 | Modbus RTU / ASCII 默认参数及允许的波特率 |
| `parameters` | 是 | 可读取或写入的参数，至少一项 |
| `statuses` | 否 | 只读状态寄存器，缺省为空 |
| `operations` | 否 | 参数应用和持久化命令，可只定义其中一项 |

`device` 不保存厂商、手册版本或资料链接。项目不会内置或下载设备资料。

## 参数标识与地址

`parameterId` 是 Profile 内唯一的非空字符串，只用于查找、确认和审计。它可以是面板编号、寄存器助记符或项目自定义名称，例如 `gain-main`、`Pr1.01` 或 `speed_limit`。

`address` 是 0..65535 的 Modbus 保持寄存器地址，与 `parameterId` 相互独立。导入时会拒绝重复的 `semanticId`、`parameterId` 或地址。

## 数值编码

`decimals` 表示通讯整数相对于显示值放大的十进制位数。例如显示值 `12.3`、`decimals=1` 时，通讯整数为 `123`。

`rawType=i16` 使用 16 位二补数，`rawType=u16` 使用无符号 16 位整数。不能被指定小数位精确表示、越过工程范围或越过 16 位通讯范围的数值会在写入前被拒绝。

## 写入风险

- `low` / `medium`：显示差异并确认后写入。
- `high` / `critical`：还必须输入 `写入 <parameterId>`。
- `access=ro` 的参数永远不能写入。
- 参数写入使用 FC06；写前读取当前原始值，写后再次读取并校验一致。
- 批量写入先对全部选中项执行写前比较，再逐项写入和回读。

## 可选设备操作

`operations` 可包含共享的 `commandRegister`、`statusRegister`，以及可选的 `apply`、`persist`。每项操作分别定义 `command`、`successStatus` 和 `timeoutMs`。

缺少 `operations` 或对应子项时，GUI 会禁用该按钮，后端也会拒绝调用。操作码不得从其他型号猜测或复用。

## 适配边界

当前传输层支持 Modbus RTU / ASCII（8 数据位），读取使用 FC03，参数写入使用 FC06。使用其他功能码、多寄存器数据、32 位值、浮点数、Modbus TCP 或厂商私有协议的设备，不能只靠当前 Profile 1.0 兼容，需先扩展运行时和契约。

只有在有权使用的设备资料明确给出地址、数据类型、缩放、访问权限和操作语义后，才应制作可写 Profile。示例文件中的数值均为虚构值，仅用于格式和模拟器演练。

## 串口协议与查找

`transport.kind` 可为 `modbus-rtu` 或 `modbus-ascii`，导入后作为默认协议；旧 RTU Profile 无需修改。校验位支持 none/even/odd，停止位支持 1/2。ASCII 使用冒号、十六进制数据、LRC 与 CRLF，详见 [Modbus 串行规范](https://www.modbus.org/file/secure/modbusoverserial.pdf)。

连接面板的“P300–P302 手册预设”来自本次提供的参数说明，只调整上位机设置，不生成寄存器地址，也不写入设备：

| 参数 | 范围与含义 |
| --- | --- |
| P300 | 站号 1～32，缺省 1；同一总线上不可重复 |
| P301 | 0 关闭 MODBUS、启用 USB；1～6 分别为 4800、9600、19200、38400、57600、115200 bit/s |
| P302 | 0/1/2 为 ASCII 8N1/8E1/8O1；3/4/5 为 RTU 8N1/8E1/8O1；缺省 4 |
| P305 | 0 普通模式、1 Motion 模式，缺省 0；与串口格式查找独立，不自动切换 |

该预设将查找范围设为 1～32，并使用上述六档波特率；P301=0 不参与查找。默认仍使用 Profile 的波特率列表和 1～247 范围。优先当前协议/格式及波特率、站号，再遍历 RTU/ASCII 的 8E1、8N1、8O1；当前设置若为 2 停止位，也先探测该配置。每个候选须连续两次 FC03 读取成功，找到首个响应配置后填回全部连接设置，不自动连接。扫描地址来自已导入 Profile，因此仍需正确的寄存器映射。
