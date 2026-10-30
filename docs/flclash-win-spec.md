# FlClash Windows 视觉方案（Servo-Assistant 复刻基准）

> 用户口径（2026-10-30）：「抄 FlClash 的 WIN 方案」「这个和 KSU 是一个视觉体系，现成的 win 版本」。
> 参考：FlClash（https://github.com/chen08209/FlClash ，本地安装 v0.9.2，源码已 clone 到 /tmp/flclash）。
> 截图：`docs/reference/flclash-windows-dashboard.png`（本机 FlClash Windows 仪表盘实拍）。
> 视觉体系 = Apple Dynamic Type 字阶 + Material 3 Expressive 表面/形状 + iOS 风蓝主色，与 KSU 同源，但**按桌面窗口组织**（左侧图标栏 + 页面大标题 + 卡片栅格）。
> 本文数值均出自 FlClash 源码，路径相对 `/tmp/flclash`。

## 1. 窗口与布局

| 项 | 值 | 出处 |
|---|---|---|
| 窗口 | 无系统标题栏，自绘 32–40px 顶部拖拽条；窗口按钮（最小化/最大化/关闭）在右上角 | 截图；`lib/ui/sidebar.dart` `windowControls` 约定 |
| 左侧导航栏 compact | **48px** 宽 | `lib/ui/sidebar.dart:14` |
| 左侧导航栏 expanded | **220px** 宽（250ms 展开） | `lib/ui/sidebar.dart:15,22` |
| 导航 item | **40×40**，间距 4，圆角 **10**，图标 **24** | `sidebar.dart:16-19,25` |
| 导航侧内缩 | ≤12px（由 48 与 40 推算：`(48-40)/2`） | `sidebar.dart:276-279` |
| 导航选中态 | item 填充 `surfaceContainerHigh`，图标 `primary`；左侧 3×16 指示条（可选） | `sidebar.dart:19-20,413` |
| 导航顶部 | 让出窗口按钮覆盖区（Win 上即空白可拖拽区）；底部放版本号小字 | `sidebar.dart` `windowControls` |
| 页面标题区 | 高 **72px**（pageToolbarHeight）；标题 `pageTitle 24/600`（大屏可 `title1 28/700`）；右侧动作按钮区 | `lib/ui/metrics.dart:23` |
| 页面水平内边距 | 16（卡片区）/ 24（大屏） | 截图；`metrics.dart:15-16` |
| 卡片栅格 | 宽 ≥**860** 四列；<860 两列；单列上限 **440**；间距 **16** | `lib/views/dashboard/layout.dart:3-8`、`dashboard.dart:25` |
| 页面滚动 | 整页滚动，卡内不滚动 | `dashboard.dart` 注释 |
| 窗口最小宽 | 我们保留 720px（Tauri 配置） | 本仓库 |

## 1.5 本机实拍取色（FlClash 深色 + pureBlack 配置，直接可用）

| 角色 | 值 | 说明 |
|---|---|---|
| page | `#000000` | 纯黑页面底（screenshot 采样 `card_gap` = #000000） |
| card | `#191A1C` | 卡片底（采样 `card_bg`） |
| block | `#252528` | 卡内嵌块/分段容器/分隔线（采样 `divider`、`chip_bg`） |
| inset | `#2F3134` | 最内层（输入/轨道） |
| on-surface | `#F5F6FF` | 主文字 |
| on-surface-variant | `#A6A9B2` | 副标题/卡片头 |
| outline | `#3A3D42` | 描边 |
| primary | `#2F83FF` | 选中分段/开关/主按钮（采样 `inner_seg`） |
| on-primary | `#FFFFFF` | |
| success | `#30D158` | 运行中/绿灯 |
| warning | `#FF9F0A` | 未加载/警示 |
| error | `#FF453A` | 错误 |
| 浅色 page | `#ECEDF9` | 取自 SchemeContent(#007AFF) light 的 surfaceContainer |
| 浅色 card | `#F9F9FF` | surfaceBright |
| 浅色 primary | `#007AFF` | 种子本色 |

> 深色走「黑底 + #191A1C 卡 + #252528 内嵌」三阶，不要用带蓝味的 M3 深色容器（截图是纯黑 pureBlack 配置）。

## 2. 表面层级（关键）

FlClash 用四级表面，深色只升不降（`lib/ui/surface.dart:9-38`）：

| 层 | 深色 | 浅色 | 用途 |
|---|---|---|---|
| page | `surface`（近黑 `#111318` 量级） | `surfaceContainer` | 页面底 |
| card | `surfaceContainer`（`#1E1F25` 量级） | `surfaceBright` | 卡片 |
| block | `surfaceContainerHigh`（`#282A2F`） | `surfaceContainer` | 卡内嵌块/分段容器/悬浮 |
| inset | `surfaceContainerHighest`（`#33353A`） | `surfaceContainerHighest` | 最内层（输入、轨道） |

分隔线 = `onSurface` @ 14% α（`lib/ui/surface.dart:150-152`）。卡片无边框、无阴影（靠层级色区分，`cardTheme elevation: 0`）。

## 3. 颜色

| 项 | 值 | 出处 |
|---|---|---|
| 默认主色种子 | **`#007AFF`**（iOS 蓝） | `lib/common/constant.dart:166` |
| 取色算法 | material_color_utilities：`ColorStyle.vivid = Variant.content`，色相取种子、chroma 收紧到 primaryContainer | `lib/ui/color.dart:14-70` |
| 深色截图观感 | 页面 `#0F1011` 左右，卡片 `#1B1D20`，内嵌 `#26282B`，主色 `#4C8DFF` 附近 | 截图取样 |
| 成功/警告/错误 | 绿 `#30D158`/`#34C759` 系、橙 `#FF9F0A`、红 `#FF453A`/`#FF3B30` | `lib/views/dashboard/style.dart` success/warning 扩展 |
| 开关开态 | `primary` 轨道 + `onPrimary` 拇指；关态 `surfaceContainerHighest` | 截图 + M3 默认 |

> 落地：实际取值已完成并写入 `tokens-a.css`；如需按种子重新生成，用 `@material/material-color-utilities` 的 `SchemeContent(seed=#007AFF)` 生成深/浅两套再替换 token。

## 4. 字阶（Apple Dynamic Type，1dp=1px）

`lib/ui/typography.dart`：

| token | 字号/行高 | 用途 |
|---|---|---|
| display | 40/44 | 仪表盘大数字 |
| largeTitle | 34/41 | — |
| title1 | 28/34 | 大页标题 |
| pageTitle | **24/30**，600 | 页面标题 |
| title2 | 22/28 | 卡片大标题 |
| title3 | 20/25 | 区块标题 |
| headline | **17/22**，600 | 卡片头 |
| body | 17/22 | 正文 |
| callout | 16/21 | — |
| subheadline | **15/20** | 副标题/行副文本 |
| footnote | 13/18 | 辅助信息 |
| caption1 | 12/16 | 标签/单位 |
| caption2 | 11/13 | 角标 |

卡片头颜色 = `onSurfaceVariant` @ 80%（`lib/ui/context.dart:70-72`）。

## 5. 组件

| 组件 | 规格 | 出处 |
|---|---|---|
| 卡片 | 圆角 **16**（大卡 20–24），底色 `card`，padding 16–20，无边框无阴影 | `lib/ui/card.dart:195`、`metrics.dart:15` |
| 卡片头 | 左图标 20 + 12 间距 + 标题 `headline 17`；右侧动作区 8 内缩 | `card.dart:26-110` |
| 设置/列表行 | 最小高 **56**，上下 padding 10，左右 16 | `metrics.dart:20-21` |
| 分段控件 | 容器 `block` 圆角 pill，高 40–44；选中项填充 `primary`，文字 `onPrimary`；未选中 `onSurfaceVariant` | 截图 |
| 开关 | M3 52×32（拇指 28，选中带勾） | 截图 |
| 主按钮 | 高 44–48，pill 圆角，`primary` 填充 | 截图 |
| 圆形动作按钮 | 48–56 直径，`primary`/`success` 填充，白色图标（仪表盘右上角绿勾即此） | 截图 |
| 图标按钮 | 40×40，圆角 10–12，hover 填充 block | 截图 |
| 输入框 | 高 40–44，`inset` 底，圆角 12，focus 主色 1.5px 描边 | 截图 |
| 滑杆 | 轨道高 16，拇指条状 4×44（M3 Expressive） | `lib/ui/theme.dart` SliderDefaultsM3 |

## 6. 页面结构范式（照截图）

```
[顶部 32-40px 拖拽条 ····························· 窗口按钮]
┌────────┬──────────────────────────────────────────────┐
│ 48px   │ 页面标题（24-28）              [圆形动作按钮] │
│ 图标栏  │ ┌────────────┐ ┌────────────┐ ┌──────────┐ │
│        │ │ 卡片        │ │ 卡片        │ │ 卡片      │ │
│        │ └────────────┘ └────────────┘ └──────────┘ │
│        │ ┌──────────────────────────┐ ┌────────────┐ │
│        │ │ 宽卡（列表/图表）          │ │ 卡片        │ │
│        │ └──────────────────────────┘ └────────────┘ │
│ 版本    │                                              │
└────────┴──────────────────────────────────────────────┘
```
- 仪表盘页 = 卡片栅格（4/2 列），卡片可跨列；大数字用 `display/title1`（例：IP 地址、流量）。
- 列表页 = 单张大卡内嵌 `.ksu-list` 行；行内右侧数值等宽、行尾操作按钮。
- 设置页 = 多张分组卡，行内左图标 + 标题 17 + 副标题 15 + 右侧控件。

## 7. Servo-Assistant 映射（7 页）

| 页面 | FlClash 化结构 |
|---|---|
| 连接 | 状态大卡（设备名/连接状态 22–28 + 大号动作圆钮）+ 连接参数卡 + 自动查找卡；栅格 2 列 |
| 参数 | 页头搜索 + 参数大卡（行：名称/地址 左，当前值 mono 右，行尾目标值输入 + 写入）+ 快照卡 + 事务卡 |
| 监控 | 6 个指标卡（大数字 display）+ 曲线卡（跨 2–4 列）+ 通道开关卡 |
| 诊断 | 警告/状态卡 + 采集设置卡 + 稳定性结果大卡 + 统计指标卡 |
| 通信 | Tab 分段（pill）+ 录制卡组 / EtherCAT 卡组，日志块用 `block` 底、行距 2 |
| 审计 | 筛选卡 + 日志行大卡（时间 footnote 弱化） |
| 设置 | 外观卡（主题深/浅/跟随系统、字号缩放）+ 关于卡 |

## 8. 落地状态（实现对照）

已按本规格落地：

- 36px 自绘拖拽条 + 右上窗口按钮；左侧图标栏默认展开 220px、顶部按钮可收成 48px；页面大标题在内容区（24/28）。
- FlClash 四级表面（page/card/block/inset）、圆角 20、卡间距 16、无边框无阴影；字阶与 90/100/110% 字号缩放落进 `tokens-a.css`。
- 七页卡片化；`<840px` 收底部 5 项导航。
- 设置页只保留 **深色 / 浅色 / 跟随系统** 与 **字号缩放**：FlClash 的主色种子、模糊开关、悬浮底栏均未做（悬浮底栏试过后因布局 bug 移除，用户无使用场景）。
- 代码里的 `ksu-*` 类名是早期命名，现指 FlClash 皮肤；扩展/维护指引见 `docs/ui.md`。
- 不在卡片里堆按钮：每卡 ≤1 主操作，其余进卡头动作或行内。
