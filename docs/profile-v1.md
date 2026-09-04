# Servo Profile 1.0

Profile 是声明式 JSON 数据，用于把设备参数 ID、Modbus 地址、通讯整数和工程单位连接起来。运行时不会执行 Profile 中的脚本，也不包含任何厂商专属地址推导规则。

## 顶层结构

| 字段 | 必填 | 含义 |
| --- | --- | --- |
| `schemaVersion` | 是 | 当前固定为 `1.0` |
| `device` | 是 | 设备 ID、显示名称和 Profile 版本 |
| `transport` | 是 | Modbus RTU 默认参数及允许的波特率 |
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

当前传输层实现为 Modbus RTU，读取使用 FC03，参数写入使用 FC06。使用其他功能码、多寄存器数据、32 位值、浮点数、Modbus TCP 或厂商私有协议的设备，不能只靠当前 Profile 1.0 兼容，需先扩展运行时和契约。

只有在有权使用的设备资料明确给出地址、数据类型、缩放、访问权限和操作语义后，才应制作可写 Profile。示例文件中的数值均为虚构值，仅用于格式和模拟器演练。
