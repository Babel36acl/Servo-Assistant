# KSU 平板模式 + Miuix / Material 双 UI 风格 —— 精确视觉规格

> **状态（2026-10-30）：已被 FlClash Windows 方案取代。** 用户最终口径是「抄 FlClash 的 WIN 方案（和 KSU 同一视觉体系、现成 win 版）」，
> 落地基准改看 [FlClash Windows 视觉方案](flclash-win-spec.md) / [FlClash 参考截图](reference/flclash-windows-dashboard.png)。
> 本文保留作为 KSU/Miuix 的源码调研资料（色板、尺寸、组件出处仍然准确），不再作为实现依据。

> 用途：把现有 Vue3 前端从「VSCode 四区壳」改造为「完全仿照 KernelSU Manager 平板模式」，且同时支持 KSU 设置里可切换的 **Miuix** 与 **Material** 两种 UI 风格。
> 本文所有数值均取自源码，每项后标注 `文件:行号`。可直接复制到 tokens / CSS 中。

---

## 0. 文档约定与源码基线

### 0.1 源码基线

| 代号 | 来源 | 本地克隆 | 提交 / 版本 |
|---|---|---|---|
| `K/` | tiann/KernelSU master，`manager/app/src/main/java/me/weishu/kernelsu/` | `/tmp/ksu-src` | master |
| `M/` | compose-miuix-ui/miuix（KSU 依赖的 UI 库 `top.yukonga.miuix.kmp`） | `/tmp/miuix-src` | tag **v0.9.4**（见 `K/../gradle/libs.versions.toml:20` `miuix = "0.9.4"`） |
| `A/` | androidx.compose.material3（KSU 直接使用，未改） | 无本地副本 | M3 Expressive |

即：`K/ui/theme/Theme.kt:14` 表示 `/tmp/ksu-src/manager/app/src/main/java/me/weishu/kernelsu/ui/theme/Theme.kt` 第 14 行；
`M/miuix-ui/src/commonMain/kotlin/top/yukonga/miuix/kmp/theme/Colors.kt:342` 同理。

### 0.2 三种数值可信度（重要，避免误用）

1. **KSU 自己写的值** —— 最可信，直接引用，行号可查。本文绝大部分属于此类。
2. **Miuix 库内建值** —— KSU 的 Miuix 风格组件尺寸实际由库决定，KSU 只传少量覆写。标注 `（Miuix 库默认）` 并给出 `M/...` 行号。
3. **M3 库内建值** —— KSU 的 Material 风格完全依赖 `androidx.compose.material3`（含 Expressive API），KSU 只覆写容器色与少量 padding。标注 `（M3 库默认）`，KSU 源码里只有「使用位置」的行号，没有尺寸字面量。

> 不要按「两套版式」理解需求：Miuix / Material 是**同一套平板版式的两种皮肤**，共用 pager + rail/bottom bar 结构，差异在配色来源、卡片圆角体系、行组件、导航条实现。

### 0.3 dp/px 换算

Android 下 `px = dp × density`（density 通常 2.0 / 2.625 / 3.0）。本文一律给 dp；换 px 时乘以目标设备 density。
例：rail 阈值 840dp，density 2.0 → 1680px；density 2.625（典型平板）→ 2205px。
KSU 自己也是这么算的：`widthDp = windowInfo.containerSize.width / density`（`K/ui/util/WindowSize.kt:11`）。

### 0.4 TL;DR（先看这 8 条）

| # | 关键结论 | 出处 |
|---|---|---|
| 1 | UiMode 默认 **Miuix**，存储键 `ui_mode`，值 `"miuix"` / `"material"` | `K/ui/UiMode.kt:5-16`、`K/data/repository/SettingsRepositoryImpl.kt:41-43` |
| 2 | 切风格是**纯重组合**，不重建 Activity（`LocalUiMode` + `KernelSUTheme`） | `K/ui/MainActivity.kt:166-177` |
| 3 | 宽屏判定：`widthDp ≥ 840`，或 `widthDp ≥ 600 且 高宽比 < 1.2` | `K/ui/util/WindowSize.kt:8-13` |
| 4 | rail 与 bottom bar **互斥**；唯一例外：Miuix + 开悬浮底栏时不走 rail | `K/ui/component/bottombar/BottomBar.kt:107-110` |
| 5 | **颜色是运行时生成的，不是静态色板**：Material 由系统取色种子 + materialkolor 生成；Miuix 有库内建精确色板 | `K/ui/theme/MaterialTheme.kt:25-31`、`K/ui/theme/ThemeExt.kt:37-48`、`M/.../theme/Colors.kt:341,451` |
| 6 | 卡片圆角：Miuix 16dp；Material 16dp（外）/4dp（内）/段间 2dp | `M/.../basic/Card.kt:190`、`K/ui/component/material/SegmentedList.kt:76-77` |
| 7 | 设置行最小高 **56dp**、内边距 **16dp**、标题 17sp/Medium、摘要 14sp | `M/.../basic/Component.kt:161,266,92-101` |
| 8 | 页面内容**不限最大宽**，只有 12dp（Miuix）/16dp（Material）水平边距 | `K/ui/screen/settings/SettingsMiuix.kt:105`、`K/ui/screen/settings/SettingsMaterial.kt:99`、全文 grep 无 `widthIn(max=)` |

---

## 1. UiMode 配置

### 1.1 枚举与默认值

```kotlin
enum class UiMode(val value: String) {
    Miuix("miuix"),
    Material("material");
    companion object {
        fun fromValue(value: String): UiMode = when (value) {
            Material.value -> Material
            else -> Miuix
        }
        val DEFAULT_VALUE = Miuix.value
    }
}
val LocalUiMode = staticCompositionLocalOf { UiMode.Miuix }
```
`K/ui/UiMode.kt:5-19`

| 项 | 值 | 出处 |
|---|---|---|
| 可选值 | `miuix` / `material` | `K/ui/UiMode.kt:6-7` |
| 默认值 | **`miuix`** | `K/ui/UiMode.kt:15` |
| 非法值回退 | → `Miuix` | `K/ui/UiMode.kt:10-13` |
| CompositionLocal 缺省 | `Miuix` | `K/ui/UiMode.kt:19` |

### 1.2 存储方式

SharedPreferences 文件名 `settings`，MODE_PRIVATE（`K/data/repository/SettingsRepositoryImpl.kt:22,37-39`）。

| 设置键 | 类型 | 默认值 | 含义 | 出处 |
|---|---|---|---|---|
| `ui_mode` | String | `"miuix"` | UI 风格 | `K/data/repository/SettingsRepositoryImpl.kt:41-43` |
| `color_mode` | Int | `0` | 颜色模式（见 §2.6） | `:53-55` |
| `miuix_monet` | Boolean | `false` | Miuix 是否走动态取色 | `:57-59` |
| `key_color` | Int(ARGB) | `0` | 取色种子；`0` = 跟随系统 | `:61-63` |
| `color_style` | String | `TonalSpot` | 调色风格 | `:65-67` |
| `color_spec` | String | `SPEC_2025` | M3 规范版本 | `:69-71` |
| `enable_blur` | Boolean | `false` | 顶栏/底栏模糊 | `:85-87` |
| `enable_floating_bottom_bar` | Boolean | `false` | 悬浮底栏 | `:89-91` |
| `enable_floating_bottom_bar_blur` | Boolean | `false` | 悬浮底栏玻璃效果 | `:93-95` |
| `enable_navigation_badge` | Boolean | `true` | 导航角标 | `:97-99` |
| `nav_rail_expanded` | Boolean | `false` | rail 展开态 | `:101-103` |
| `page_scale` | Float | `1.0` | 整页缩放（乘到 density 上） | `:105-107` |

### 1.3 在设置页的位置

两套风格的设置页里都是**第二个分组的第一行**（第一分组是「检查更新」），文案 `settings_ui_mode`：

- Miuix：`OverlayDropdownPreference`（下拉浮层），选项文字直接取 `UiMode.entries.map { it.name }` → 显示 `Miuix` / `Material`，选中索引 `if (uiState.uiMode == Material.value) 1 else 0`
  `K/ui/screen/settings/SettingsMiuix.kt:152-166`
- Material：`SegmentedDropdownItem`（列表行 + 弹出菜单），参数同上
  `K/ui/screen/settings/SettingsMaterial.kt:127-134`

紧跟其后的是「主题 / 配色」入口行（`settings_theme`），点击进入独立的**外观页** `ColorPaletteScreen`——模糊、悬浮底栏、角标、翻页手势、页面缩放、取色种子、调色风格/规范都在那一页，**不在主设置页**：
`K/ui/screen/settings/SettingsMiuix.kt:167-179`、`K/ui/screen/colorpalette/ColorPaletteScreenMiuix.kt:120,182,288,363,466`

### 1.4 切换后如何生效

**不重建 Activity，直接重组整棵树。** uiMode 从 ViewModel 流出 → `CompositionLocalProvider(LocalUiMode provides uiMode)` → `KernelSUTheme(appSettings, uiMode)` 内部 `when(uiMode)` 选择 `MiuixKernelSUTheme` 或 `MaterialKernelSUTheme`：

```
K/ui/MainActivity.kt:142        val uiMode = uiState.uiMode
K/ui/MainActivity.kt:166-176    CompositionLocalProvider(..., LocalUiMode provides uiMode, ...)
K/ui/MainActivity.kt:177        KernelSUTheme(appSettings = appSettings, uiMode = uiMode) {
K/ui/theme/Theme.kt:110-120     when (uiMode) { Miuix -> MiuixKernelSUTheme(...); Material -> MaterialKernelSUTheme(...) }
```

推论（对我方实现的要求）：切换风格必须是**纯 CSS/token 级切换**（换 `data-ui-mode` 属性），不能触发整页刷新或重新拉数据；滚动位置、连接状态、正在进行的通信会话都必须保持。

### 1.5 UiMode 与「动态取色」的联动（易漏）

当 `ui_mode == "miuix"` 时，`ThemeController.getAppSettings()` 会强制把 colorMode 与 `miuix_monet` 对齐：
- `miuix_monet == false` 且当前是 Monet 模式 → 降级为非 Monet；
- `miuix_monet == true` 且当前非 Monet → 升级为 Monet。
`K/ui/theme/Theme.kt:72-82`

Material 风格**没有**这个开关：它始终由种子+materialkolor 生成（`key_color == 0` 时种子取系统动态主色）。
`K/ui/theme/MaterialTheme.kt:23-31`

---

## 2. 颜色角色与色板

### 2.1 必须先理解：KSU 的颜色是「生成式」的

- Material：`MaterialKernelSUTheme` → `rememberKernelSUColorScheme(seedColor = if (keyColor==0) Color.Unspecified else Color(keyColor), isDark, isAmoled, paletteStyle, colorSpec)`
  `K/ui/theme/MaterialTheme.kt:25-31`
  其中 `seedColor == Unspecified` 时取 `dynamicLight/DarkColorScheme(context).primary`（即**系统壁纸取色**），再交给 materialkolor 的 `rememberDynamicColorScheme(...)` 生成整套 scheme：
  `K/ui/theme/ThemeExt.kt:36-49`
- Miuix：同样接受种子；但**种子为 null 时回落到库内建的固定色板** `lightColorScheme()` / `darkColorScheme()`
  `K/ui/theme/MiuixTheme.kt:45-67`（`resolvedKeyColor` 三段判断）、`M/.../theme/ThemeController.kt:219-220`
- minSdk = **31**，动态取色恒定可用，**KSU 没有任何 Pre-Monet 兜底色板**
  `manager/build.gradle.kts:7`（`androidMinSdkVersion = 31`）

结论：**Miuix 有一张精确静态色板可抄（默认配置下就是它在生效）；Material 没有，它的运行值取决于设备壁纸。** 下面分别处理。

### 2.2 表 A —— Miuix 深浅色板（精确 hex，可直接落盘）

生效条件：`ui_mode=miuix`、`miuix_monet=false`、`key_color=0`、`color_mode` 为系统/浅/深（即默认安装状态 + 大多数使用场景）。
出处：`M/miuix-ui/src/commonMain/kotlin/top/yukonga/miuix/kmp/theme/Colors.kt`，**浅色在 341-393 行，深色在 451-503 行**（同一行号顺序一一对应，下面按浅色行号列出）。

| 角色 | 浅色 | 浅色行 | 深色 | 深色行 | 说明 / KSU 用途 |
|---|---|---|---|---|---|
| `primary` | `#3482FF` | 342 | `#277AF7` | 452 | 强调色：开关、按钮、滑杆、选中项 |
| `onPrimary` | `#FFFFFF` | 343 | `#FFFFFF` | 453 | 主色上的文字 |
| `primaryVariant` | `#3482FF` | 344 | `#0073DD` | 454 | 映射到 CSS `inversePrimary` |
| `onPrimaryVariant` | `#AECDFF` | 345 | `#99C7F1` | 455 | |
| `primaryContainer` | `#5D9BFF` | 355 | `#338FE4` | 465 | 填充卡片容器 |
| `onPrimaryContainer` | `#FFFFFF` | 356 | `#FFFFFF` | 466 | |
| `error` | `#E94634` | 346 | `#F12522` | 456 | 错误 |
| `onError` | `#FFFFFF` | 347 | `#FFFFFF` | 457 | |
| `errorContainer` | `#FDF6F4` | 348 | `#2E0603` | 458 | 错误提示卡背景 |
| `onErrorContainer` | `#410002` | 349 | `#FFDAD6` | 459 | |
| `disabledPrimary` | `#C2D9FF` | 350 | `#253E64` | 460 | 禁用开关 |
| `disabledOnPrimary` | `#F3F8FF` | 351 | `#677993` | 461 | |
| `disabledPrimaryButton` | `#C2D9FF` | 352 | `#253E64` | 462 | 禁用按钮 |
| `disabledOnPrimaryButton` | `#FFFFFF` | 353 | `#677893` | 463 | |
| `disabledPrimarySlider` | `#B8CFF5` | 354 | `#44587C` | 464 | 禁用滑杆 |
| `secondary` | `#E6E6E6` | 357 | `#505050` | 467 | 次级容器（首页工作状态卡） |
| `onSecondary` | `#FFFFFF` | 358 | `#FFFFFF` | 468 | |
| `secondaryVariant` | `#F0F0F0` | 359 | `#434343` | 469 | |
| `onSecondaryVariant` | `#303030` | 360 | `#D9D9D9` | 470 | |
| `disabledSecondary` | `#F0F0F0` | 361 | `#3F3F3F` | 471 | 禁用次级 |
| `disabledOnSecondary` | `#FCFCFC` | 362 | `#797979` | 472 | |
| `disabledSecondaryVariant` | `#F2F2F2` | 363 | `#404040` | 473 | |
| `disabledOnSecondaryVariant` | `#B2B2B2` | 364 | `#707170` | 474 | **禁用行标题/副标题色** |
| `secondaryContainer` | `#F0F0F0` | 365 | `#434343` | 475 | **CSS `filledTonalButtonContainerColor` / 首页状态卡** |
| `onSecondaryContainer` | `#A9A9A9` | 366 | `#7C7C7C` | 476 | |
| `secondaryContainerVariant` | `#F0F0F0` | 367 | `#4F4F4F` | 477 | |
| `onSecondaryContainerVariant` | `#A8A8A8` | 368 | `#959595` | 478 | |
| `tertiaryContainer` | `#EAF2FF` | 369 | `#2B3B54` | 479 | 通知/警告卡（Notice 级） |
| `onTertiaryContainer` | `#3482FF` | 370 | `#4788FF` | 480 | |
| `tertiaryContainerVariant` | `#EAF2FF` | 371 | `#505050` | 481 | 映射到 CSS `tertiary` |
| `background` | `#FFFFFF` | 372 | `#242424` | 482 | 底色角色（**页面容器实际用 `surface`**，见 §4.1） |
| `onBackground` | `#000000` | 373 | `#E6FFFFFF`（90% 白） | 483 | 正文 |
| `onBackgroundVariant` | `#8C93B0` | 374 | `#787E96` | 484 | 弱化文字 |
| `surface` | `#F7F7F7` | 375 | `#000000` | 485 | **页面 / 顶栏 / 普通底栏容器色（Scaffold 默认）** |
| `onSurface` | `#000000` | 376 | `#F2F2F2` | 486 | |
| `surfaceVariant` | `#FFFFFF` | 377 | `#242424` | 487 | |
| `onSurfaceSecondary` | `#CC000000`（80% 黑） | 378 | `#CCFFFFFF` | 488 | |
| `onSurfaceVariantSummary` | `#99000000`（60% 黑） | 379 | `#80FFFFFF`（50% 白） | 489 | **设置行副标题 / CSS `onSurfaceVariant`** |
| `onSurfaceVariantActions` | `#66000000`（40% 黑） | 380 | `#66FFFFFF` | 490 | |
| `disabledOnSurface` | `#B2B2B2` | 381 | `#666666` | 491 | |
| `surfaceContainer` | `#FFFFFF` | 382 | `#242424` | 492 | **卡片容器 / 悬浮底栏底色** |
| `onSurfaceContainer` | `#000000` | 383 | `#E6FFFFFF` | 493 | |
| `onSurfaceContainerVariant` | `#959595` | 384 | `#737373` | 494 | |
| `surfaceContainerHigh` | `#E8E8E8` | 385 | `#242424` | 495 | rail 选中指示器底色 |
| `onSurfaceContainerHigh` | `#A2A2A2` | 386 | `#666666` | 496 | |
| `surfaceContainerHighest` | `#E8E8E8` | 387 | `#2D2D2D` | 497 | |
| `onSurfaceContainerHighest` | `#000000` | 388 | `#E9E9E9` | 498 | |
| `outline` | `#D9D9D9` | 389 | `#404040` | 499 | 描边 |
| `dividerLine` | `#E0E0E0` | 390 | `#393939` | 500 | 分割线 / CSS `outlineVariant` |
| `windowDimming` | `#000000` α0.30 | 391 | `#000000` α0.60 | 501 | 对话框遮罩 / CSS `scrim` |
| `sliderKeyPoint` | `#4DA3B3CD` | 392 | `#4D7A8AA6` | 502 | 滑杆刻度点 |
| `sliderKeyPointForeground` | `#6EB5FF` | 393 | `#5DAAFF` | 503 | |
| `sliderBackground` | `#0F000000`（6% 黑） | 394 | `#26FFFFFF`（15% 白） | 504 | 滑杆轨道底 |

**注意**：Miuix 的角色名与 M3 **不同名**（如 `onSurfaceVariantSummary` vs `onSurfaceVariant`、`tertiaryContainerVariant` vs `tertiary`）。做 CSS token 时按 §2.5 的映射表，不要按名字直译。

### 2.3 表 B —— Material 侧色板（生成式，无静态 hex）

Material 风格的角色集合就是 M3 `ColorScheme` 全套（`K/ui/theme/ThemeExt.kt:60-110` 逐字段列举了 46 个）。KSU 让它动态生成，**源码里没有一行 Material hex**（全仓库 grep `Color(0x` 仅剩 §2.4 那些功能色）。

生成参数（默认态）：

| 参数 | 默认值 | 出处 |
|---|---|---|
| 种子 `seedColor` | `key_color == 0` → 系统动态主色；否则 `Color(key_color)` | `K/ui/theme/MaterialTheme.kt:23-31` |
| `paletteStyle` | `TonalSpot` | `K/data/repository/SettingsRepositoryImpl.kt:66` |
| `colorSpec` | `SPEC_2025`（对不支持 2025 的 style 自动降级 2021） | `:70`、`K/ui/theme/Theme.kt:54-65` |
| 深色 | `colorMode.isDark` 或（系统模式 && 系统深色） | `K/ui/theme/MaterialTheme.kt:21` |
| AMOLED | `color_mode == 6` → 全部 surface 系强制纯黑 | `K/ui/theme/ThemeExt.kt:15-26` |
| 动画 | 主题切换时颜色 spring 过渡 | `K/ui/theme/ThemeExt.kt:52-58` |
| 动态取色种子候选（用户可选 15 色） | `#F44336 #E91E63 #9C27B0 #673AB7 #3F51B5 #2196F3 #00BCD4 #009688 #4FAF50 #FFEB3B #FFC107 #FF9800 #795548 #607D8F #FF9CA8` | `K/ui/theme/Colors.kt:6-22` |

**对我方的可执行结论**（见附录 B 脚本）：
1. 若要「视觉上像 KSU 的 Material」，必须实现同一套取色算法（HCT + materialkolor），或
2. 退一步：选一个固定种子，一次性生成浅/深两套并落盘。推荐种子 `#3482FF`（= Miuix 默认 primary，保证两风格强调色一致）或 `#4FAF50`（KSU 候选里的品牌绿）。
3. **不要**把 M3 baseline（紫 `#6750A4`）当 KSU 色值写进文档/tokens——KSU 从不显示它。

### 2.4 表 C —— KSU 硬编码功能色（精确，可直接落盘）

这些是 KSU 仅有的非派生色，两套风格共用逻辑（Miuix 只在「非动态取色」时用固定值）：

**(1) 警告卡** `K/ui/component/miuix/WarningCard.kt`

| 状态 | 容器色 浅 / 深 | 内容色（文字+图标） | 动态取色时改用 | 出处 |
|---|---|---|---|---|
| Error | `#F8E2E2` / `#310808` | `#F72727` | `errorContainer` / `onErrorContainer` | `:64,69,82,59,77` |
| Notice | `#FFF0DB` / `#3E2F1B` | `#F5A623` | `tertiaryContainer` / `onTertiaryContainer` | `:65,70,83,60,78` |

卡片内边距 16dp、文案 14sp：`K/ui/component/miuix/WarningCard.kt:43,49`

**(2) 运行状态卡（成功绿）** `K/ui/screen/home/HomeMiuix.kt`

| 用途 | 浅色 | 深色 | 出处 |
|---|---|---|---|
| 状态卡背景 | `#DFFAE4` | `#1A3825` | `:277-278` |
| 大对勾图标 | `#36D167` | `#36D167` | `:302` |
| 徽标内容色 | `#164A29` | `#B8E8C5` | `:350,352` |
| 徽标背景色 | `#B8E8C5` | `#315D3E` | `:357,359` |

动态取色时改用 `secondaryContainer` / `primary@80%` / `tertiaryContainer`：`:276,300,355`。
背景色同样用于外观页预览：`K/ui/screen/colorpalette/ColorPaletteScreenMiuix.kt:553-554`。

**(3) 日志语法高亮**（我方「通信/审计」页直接可用）`K/ui/util/ShellHighlight.kt`

| Token | 浅色 | 深色 | 行号 |
|---|---|---|---|
| comment | `#6A737D` | `#8B949E` | `:25,31` |
| string | `#032F62` | `#A5D6FF` | `:26,32` |
| variable | `#E36209` | `#FFA657` | `:27,33` |
| keyword | `#D73A49` | `#FF7B72` | `:28,34` |

### 2.5 CSS 变量契约（45 个，两套风格的映射不同）

KSU 把当前配色导出成 webview 的 `:root` CSS 变量，**这就是最贴近我方需求的现成契约**：
`K/ui/webui/MonetColorsProvider.kt:40-86`（Miuix）与 `:95-141`（Material），输出格式 `:root { --name: #rrggbb; }`（`:147-155`，带 alpha 时输出 `#rrggbbaa`，`:157-166`）。

| CSS 变量 | Miuix 取值来源 | Material 取值来源 |
|---|---|---|
| `--primary` | `primary` | `primary` |
| `--onPrimary` | `onPrimary` | `onPrimary` |
| `--primaryContainer` | `primaryContainer` | `primaryContainer` |
| `--onPrimaryContainer` | `onPrimaryContainer` | `onPrimaryContainer` |
| `--inversePrimary` | `primaryVariant` | `inversePrimary` |
| `--secondary` | `secondary` | `secondary` |
| `--onSecondary` | `onSecondary` | `onSecondary` |
| `--secondaryContainer` | `secondaryContainer` | `secondaryContainer` |
| `--onSecondaryContainer` | `onSecondaryContainer` | `onSecondaryContainer` |
| `--tertiary` | `tertiaryContainerVariant` | `tertiary` |
| `--onTertiary` | `tertiaryContainer` | `onTertiary` |
| `--tertiaryContainer` | `tertiaryContainer` | `tertiaryContainer` |
| `--onTertiaryContainer` | `onTertiaryContainer` | `onTertiaryContainer` |
| `--background` | `background` | `background` |
| `--onBackground` | `onBackground` | `onBackground` |
| `--surface` | `surface` | `surface` |
| `--tonalSurface` | `surfaceContainer` | `surfaceColorAtElevation(1.dp)` |
| `--onSurface` | `onSurface` | `onSurface` |
| `--surfaceVariant` | `surfaceVariant` | `surfaceVariant` |
| `--onSurfaceVariant` | `onSurfaceVariantSummary` | `onSurfaceVariant` |
| `--surfaceTint` | `primary` | `surfaceTint` |
| `--inverseSurface` | `disabledOnSurface` | `inverseSurface` |
| `--inverseOnSurface` | `surfaceContainer` | `inverseOnSurface` |
| `--error` | `error` | `error` |
| `--onError` | `onError` | `onError` |
| `--errorContainer` | `errorContainer` | `errorContainer` |
| `--onErrorContainer` | `onErrorContainer` | `onErrorContainer` |
| `--outline` | `outline` | `outline` |
| `--outlineVariant` | `dividerLine` | `outlineVariant` |
| `--scrim` | `windowDimming` | `scrim` |
| `--surfaceBright` | `surface` | `surfaceBright` |
| `--surfaceDim` | `surface` | `surfaceDim` |
| `--surfaceContainer` | `surfaceContainer` | `surfaceContainer` |
| `--surfaceContainerHigh` | `surfaceContainerHigh` | `surfaceContainerHigh` |
| `--surfaceContainerHighest` | `surfaceContainerHighest` | `surfaceContainerHighest` |
| `--surfaceContainerLow` | `surfaceContainer` | `surfaceContainerLow` |
| `--surfaceContainerLowest` | `surfaceContainer` | `surfaceContainerLowest` |
| `--filledTonalButtonContentColor` | `onPrimaryContainer` | `onPrimaryContainer` |
| `--filledTonalButtonContainerColor` | `secondaryContainer` | `secondaryContainer` |
| `--filledTonalButtonDisabledContentColor` | `onSurfaceVariantSummary` | `onSurfaceVariant` |
| `--filledTonalButtonDisabledContainerColor` | `surfaceVariant` | `surfaceVariant` |
| `--filledCardContentColor` | `onPrimaryContainer` | `onPrimaryContainer` |
| `--filledCardContainerColor` | `primaryContainer` | `primaryContainer` |
| `--filledCardDisabledContentColor` | `onSurfaceVariantSummary` | `onSurfaceVariant` |
| `--filledCardDisabledContainerColor` | `surfaceVariant` | `surfaceVariant` |

（`tonalSurface` 的差异是两套风格最明显的体积感差别：Miuix 直接用 `surfaceContainer`，Material 用 surface + 1dp 高度叠加。）

### 2.6 ColorMode 枚举与深色判定

```kotlin
enum class ColorMode(val value: Int) {
    SYSTEM(0), LIGHT(1), DARK(2), MONET_SYSTEM(3), MONET_LIGHT(4), MONET_DARK(5), DARK_AMOLED(6);
    val isSystem; val isDark; val isAmoled; val isMonet
}
```
`K/ui/theme/Theme.kt:14-45`

| 判定 | 规则 | 出处 |
|---|---|---|
| 是否深色 | `1,4 → 浅`；`2,5,6 → 深`；`0,3 → 跟随系统` | `K/ui/theme/Theme.kt:123-131` |
| 是否 AMOLED | `value == 6`，8 个 surface 角色强制 `#000000` | `K/ui/theme/Theme.kt:29`、`ThemeExt.kt:15-26` |
| 外观页的分段选项 | 系统 / 浅色 / 深色（三段） | `K/ui/screen/colorpalette/ColorPaletteScreenMiuix.kt:170-172` |

---

## 3. 导航规格

### 3.1 宽屏阈值与 rail/bottom bar 判定

```kotlin
// K/ui/util/WindowSize.kt:8-13
fun shouldShowSplitPane(): Boolean {
    val deviceDensity = LocalResources.current.displayMetrics.density
    val widthDp = windowInfo.containerSize.width / deviceDensity
    val heightDp = windowInfo.containerSize.height / deviceDensity
    return widthDp >= 840f || (widthDp >= 600f && heightDp / widthDp < 1.2f)
}

// K/ui/component/bottombar/BottomBar.kt:107-110
fun useNavigationRail(enableFloatingBottomBar: Boolean): Boolean =
    shouldShowSplitPane() && !(LocalUiMode.current == UiMode.Miux && enableFloatingBottomBar)
```

| 条件 | 结果 |
|---|---|
| `widthDp ≥ 840` | 用 rail（无论高宽比） |
| `600 ≤ widthDp < 840` 且 `height/width < 1.2`（横屏平板） | 用 rail |
| `600 ≤ widthDp < 840` 且 `height/width ≥ 1.2`（竖屏平板/大屏手机） | 用 bottom bar |
| `widthDp < 600` | 用 bottom bar |
| 例外 | Miuix + `enable_floating_bottom_bar = true` → **强制走 bottom bar（悬浮）**，即使宽屏 |

换算示例：840dp @density 2.0 = 1680px，@2.625 = 2205px；600dp @2.0 = 1200px。
我方 Web 端 density 恒为 1（CSS px = dp），所以直接用 **840px / 600px** 阈值 + 视口宽高比判定即可。

布局落地（二选一互斥）：

```
宽屏（rail）:  Row { SideRail(); Box(weight=1f) { Pager内容 } }
   K/ui/MainActivity.kt:425-457
窄屏（bottom）: Scaffold(bottomBar = BottomBar) { Pager内容 }
   K/ui/MainActivity.kt:459-484
```

rail 模式下不使用 bottomBar；bottom 模式下不渲染 rail。二者都只由 `useNavigationRail` 一个布尔决定（`K/ui/MainActivity.kt:292`），**不要同时显示**。

### 3.2 Miuix rail

KSU 侧：
- 组件：`NavigationRail` + `NavigationRailItem`（Miuix 库）
- 容器色 = `MiuixTheme.colorScheme.surface`（`K/ui/component/bottombar/NavigationRailMiuix.kt:46`）
- 初始展开态读 `nav_rail_expanded`，变化时回写（`:32-41`）
- 4 项，顺序 `Home / SuperUser / Module / Setting`，图标来自 `BottomBarDestination`（`K/ui/component/bottombar/BottomBarMiuix.kt:135-143`）
- 项带角标 `badge`（`:58`）

Miuix 库默认尺寸（`M/miuix-ui/.../basic/NavigationRail.kt`）：

| 项 | 值 | 行号 |
|---|---|---|
| 折叠宽 `MinWidth` | **80dp** | 603 |
| 展开宽 `ExpandedWidth` | **240dp** | 606 |
| 容器纵向内边距 `VerticalPadding` | 24dp | 609 |
| 头部间距 `HeaderSpacing` | 24dp | 612 |
| 图标 `IconSize` | **28dp** | 615 |
| 图标-文字间距 | 4dp | 618 |
| item 纵向内边距 `ItemVerticalPadding` | 12dp | 621 |
| 折叠态 label 字号 | **12sp** | 624 |
| 展开态 label 字号 | **16sp** | 627 |
| 展开态 item 左右外边距 | 12dp | 630 |
| 展开态 item 圆角 | **16dp** | 633 |
| 展开态 item 内容左内边距 | 14dp | 639 |
| 展开态 item 内容纵向内边距 | 14dp | 642 |
| 展开态图标-文字间距 | 16dp | 645 |
| 选中指示器色 | `surfaceContainerHigh` | 341（`NavigationRailItem`）/ 442 |
| item 内容默认色 | `onSurfaceContainer`，字重 Medium | 340-342 |

### 3.3 Material rail

KSU 侧：`WideNavigationRail`（M3 Expressive）+ `WideNavigationRailItem`
`K/ui/component/bottombar/NavigationRailMaterial.kt:77-125`

| 项 | 值 | 出处 |
|---|---|---|
| 容器色 | `surfaceContainer`（覆写 M3 默认） | `:80-83` |
| 内容色 | `onSurface` | `:82` |
| 内容纵向内边距 | **20dp** | `:87` |
| 头部展开/收起按钮 | 左内边距 **24dp** 的 `IconButton` | `:89-103` |
| insets | `systemBars ∪ displayCutout`，仅 Start+Vertical 边 | `:84-86` |
| 选中/未选中图标 | Filled / Outlined 双套切换 | `:116-122`（Home/SuperUser/Module/Setting，`:55-60`） |
| 展开态持久化 | 写 `nav_rail_expanded`（基于 targetValue） | `:71-75` |
| 折叠宽 / 展开宽 | M3 库默认（WideNavigationRail：80dp / 220dp，M3 库默认未覆写） | — |

### 3.4 Miuix bottom bar（普通，非悬浮）

`K/ui/component/bottombar/BottomBarMiuix.kt:63-83`

| 项 | 值 | 出处 |
|---|---|---|
| 组件 | `NavigationBar` + `NavigationBarItem` + `NavigationItem` | `:65-79` |
| 容器色 | 无模糊时 `surface`；有模糊 `Transparent`（内容画在 backdrop 上） | `:67` |
| 项权重 | 每项 `weight(1f)` | `:71` |
| 项数 | 4 | `:57`、`:135-143` |
| item 高度 | **64dp** | `M/.../basic/NavigationBar.kt:442` |
| icon 尺寸 | **26dp** | `:445` |
| icon 上内边距 | 8dp | `:451` |
| label 字号 | **12sp** | `:448` |
| label 下内边距 | 8dp | `:454` |
| 选中字重 | Bold；未选中 Normal | `:160` |
| 未选中透明度 | 0.4 | `:463`（`UnselectedAlpha`）；按下态 0.5/0.6 见 `:457,460` |
| 容器最小高 | 52dp（其他显示模式） | `:315` |
| 底部 insets | 有导航条时 `26dp + inset`，否则 `36dp` | `:289-293` |
| 角标 | `Badge`，Accent 用 `primary/onPrimary` | `K/ui/component/bottombar/BottomBarMiuix.kt:163-167` |

顶部模糊条：`blurRadius = 25f`（`K/ui/util/BlurExt.kt:36`），由 `enable_blur` 控制。

### 3.5 Material bottom bar

`K/ui/component/bottombar/BottomBarMaterial.kt:47-78`

| 项 | 值 | 出处 |
|---|---|---|
| 组件 | `ShortNavigationBar` + `ShortNavigationBarItem`（M3 Expressive） | `:47,55` |
| 容器色 | `surfaceContainer` | `:48` |
| insets | `systemBars ∪ displayCutout`，Horizontal+Bottom | `:49-51` |
| 项数 | 4（Filled/Outlined 图标切换） | `:40-45,64` |
| label | 单行，`Ellipsis` | `:70-74` |
| 容器高度 | M3 Expressive `ShortNavigationBar` 默认 64dp（库内建，KSU 未覆写） | — |
| 角标 Accent 色 | `primary` / `onPrimary` | `:96-97` |

### 3.6 悬浮底栏（Miuix 专属）

仅 `ui_mode=miuix` 且 `enable_floating_bottom_bar=true` 时启用；此时即使宽屏也**不走 rail**（§3.1）。
`K/ui/component/bottombar/BottomBarMiuix.kt:84-131` + `K/ui/component/FloatingBottomBar.kt`

| 项 | 值 | 出处 |
|---|---|---|
| 自身高度 | **64dp** | `FloatingBottomBar.kt:388` |
| 自身内边距 | **4dp** | `:389` |
| 形状 | `CircleShape`（全圆角胶囊） | `:216` |
| 外层左右边距 | **28dp** | `BottomBarMiuix.kt:92` |
| 外层底部边距 | 有系统导航条：`8dp + inset`；无：**28dp** | `BottomBarMiuix.kt:85-86` |
| 每个 tab 最小宽 | **76dp** | `BottomBarMiuix.kt:105` |
| 内部指示器高 | **56dp** | `FloatingBottomBar.kt:424,478,491` |
| 指示器宽度 | `(总宽 - 8dp) / tabsCount` | `:342-343` |
| 指示器左右内边距 | 4dp | `:425,437,484` |
| 容器色 | `surfaceContainer`；开模糊时 `surfaceContainer` @ **α0.4** | `:219-220` |
| 投影 | `radius=10dp`，黑；深色 α0.2 / 浅色 α0.1 | `:347-354` |
| 模糊 | `blur(4dp, 4dp)` + `vibrancy()` + `lens(refractionHeight=24dp, refractionAmount=24dp)`，effect padding `max(40dp)` | `:357-368` |
| 高光描边 | `1dp`，白 α0.12，内模糊 2dp，`dualPeak` | `:96-114`（`iosIndicatorSpecular`） |
| 指示器（未开模糊） | `primary` @ **α0.15** 背景 | `:490` |
| 按下缩放 | tab 内容 1.0 → **1.2**；面板 `56→78` 比例 | `:399-401`、`:262` |
| tab 内 icon+label 间距 | 1dp，垂直居中 | `:199` |
| tab label 字号 | **11sp**，行高 **14sp**，单行不换行 | `BottomBarMiuix.kt:123-124` |
| 角标（悬浮态） | `primaryContainer` / `onPrimaryContainer` | `BottomBarMiuix.kt:163-164` |
| 拖动/点击 | 支持拖拽切换 + 弹性回弹（rubber band 4dp） | `FloatingBottomBar.kt:231-241,286-297` |

### 3.7 角标尺寸

| 项 | 值 | 出处 |
|---|---|---|
| 纯圆点角标 | 6dp | `M/.../basic/Badge.kt:182` |
| 大号圆点 | 16dp | `:185` |
| 数字角标水平内边距 | 4dp | `:220` |
| 数字角标行高 | 16sp | `:164` |
| 相对锚点水平偏移 | 12dp | `:226` |
| 相对锚点垂直重叠 | 14dp | `:227` |

### 3.8 两套导航差异速查

| 维度 | Miuix | Material |
|---|---|---|
| rail 库 | `NavigationRail`（80 / 240dp） | `WideNavigationRail`（80 / 220dp，M3） |
| rail 容器色 | `surface` | `surfaceContainer` |
| rail 展开控制 | 库内置展开按钮 | 自定义 `IconButton`，左内边距 24dp |
| rail 内容纵向内边距 | 24dp | 20dp |
| bottom bar 库 | `NavigationBar`（item 64dp，icon 26dp，label 12sp） | `ShortNavigationBar`（M3 Expressive，64dp） |
| bottom bar 容器色 | `surface` | `surfaceContainer` |
| bottom bar 选中态 | 图标填充 + 文字加粗，未选中 α0.4 | Filled / Outlined 图标切换 |
| 悬浮底栏 | 有（64dp 胶囊 + 模糊/镜头） | **无** |
| 角标 | `Badge`（6dp 点 / 数字） | `Badge`（M3 默认） |

---

## 4. 组件规格

### 4.1 页面壳

| 项 | Miuix | Material | 出处 |
|---|---|---|---|
| 顶部栏 | `TopAppBar`（小标题，可滚动收起） | `LargeFlexibleTopAppBar`（大标题，滚动折叠） | `K/ui/screen/settings/SettingsMiuix.kt:88-93`；`K/ui/screen/settings/SettingsMaterial.kt:393-398` |
| 顶栏容器色 | `surface`；开模糊则 Transparent | `surfaceContainer` | 同上 / `K/ui/component/material/ExpressiveScaffold.kt:45-50` |
| 顶栏收起高度 | 52dp（Miuix 库） | M3 默认 | `M/.../basic/TopAppBar.kt:409` |
| 顶栏标题左内边距 | 26dp（Miuix 库） | M3 默认 | `M/.../basic/TopAppBar.kt:400` |
| 页面容器色 | `surface`（`#F7F7F7` / 深 `#000000`） | `surfaceContainer`（Scaffold containerColor） | `M/.../basic/Scaffold.kt:89`；`K/ui/MainActivity.kt:432,476`、`ExpressiveScaffold.kt:24` |
| 滚动容器 | `LazyColumn` | `Column + verticalScroll` | `SettingsMiuix.kt:99`；`SettingsMaterial.kt:91-95` |
| 内容水平内边距 | **12dp** | **16dp**（分组自身 padding） | `SettingsMiuix.kt:105`；`SettingsMaterial.kt:99` |
| 分组间距 | 卡片 `padding(top = 12dp)` | 分组 `padding(bottom = 13dp)` | `SettingsMiuix.kt:112`；`SettingsMaterial.kt:99` |
| 底部留白 | `bottomInnerPadding`（= 底栏高度） | 同 | `SettingsMiuix.kt:481`；`SettingsMaterial.kt:384` |
| 内容最大宽 | **无限制**（只有水平 padding） | **无限制** | 全仓 grep 无 `widthIn(max=)` 用于页面容器 |

### 4.2 分组卡（Card / SegmentedColumn）

**Miuix**

| 项 | 值 | 出处 |
|---|---|---|
| 圆角 | **16dp** | `M/.../basic/Card.kt:190` |
| 内边距 | 默认 `0dp`（由行自己带 16dp） | `M/.../basic/Card.kt:195` |
| 容器色 | 未显式指定 → 库默认（`surfaceContainer` 系） | `SettingsMiuix.kt:110-114` |
| 卡间距 | 12dp（`padding(top = 12dp)`） | `SettingsMiuix.kt:112` |
| 最大宽 | 无 | — |
| 描边/阴影 | 无显式描边；按压反馈 `PressFeedbackType.Sink/Tilt` | `WarningCard.kt:38`、`HomeMiuix.kt:287` |

**Material**

| 项 | 值 | 出处 |
|---|---|---|
| 分组列 `SegmentedColumn` | `Column(spacedBy(2.dp))` 包多行 | `K/ui/component/material/SegmentedList.kt:119` |
| 行外圆角 | **16dp**（`SegmentedOuterRadius`） | `:76` |
| 行内圆角 | **4dp**（`SegmentedInnerRadius`） | `:77` |
| 单行分组 | 整卡用 `MaterialTheme.shapes.large` | `:92-99` |
| 段间隙 | **2dp**（`SegmentedGap`） | `:119` |
| 容器色 | `surfaceBright` | `:85-89,278` |
| 禁用容器色 | `surfaceBright` | `:87` |
| 副标题色 | `onSurfaceVariant` | `:88` |
| 分组标题（可选） | `titleSmallEmphasized`，色 `primary`，左内边距 16dp、下 8dp | `:112-117` |
| 分组外边距 | 左右 16dp、下 13dp | `SettingsMaterial.kt:99` |
| 折叠/展开动画 | 弹簧 stiffness 800 / damping 0.9 | `:78-79` |

### 4.3 设置行

**Miuix（`BasicComponent` 家族：`SwitchPreference` / `ArrowPreference` / `OverlayDropdownPreference`）**

| 项 | 值 | 出处 |
|---|---|---|
| 行最小高 | **56dp** | `M/.../basic/Component.kt:161` |
| 行内边距 | **16dp**（四周） | `M/.../basic/Component.kt:266` |
| 左图标与文字间距 | **8dp** | `M/.../basic/Component.kt:200`（`spacerPx = 8.dp`） |
| 右侧控件与文字间距 | 8dp，右侧控件最大占可用宽 60% | `Component.kt:217-218`、`SwitchPreference.kt:73` |
| 标题字号/字重 | **17sp / Medium** | `Component.kt:92-93` |
| 标题色 | `onBackground` | `Component.kt:273` |
| 副标题字号 | **14sp** | `Component.kt:100` |
| 副标题色 | `onSurfaceVariantSummary`（浅 `#99000000` / 深 `#80FFFFFF`） | `Component.kt:287` |
| 禁用文字色 | `disabledOnSecondaryVariant` | `Component.kt:274,288` |
| 左图标尺寸（KSU 用法） | 默认 `Icon` 尺寸（24dp），仅加 `padding(end = 6dp)` | `SettingsMiuix.kt:119-125`（全篇一致） |
| 左图标着色 | `colorScheme.onBackground`；不可用时 `disabledOnSecondaryVariant` | `SettingsMiuix.kt:123,298` |
| 行按压效果 | `pressFeedbackType = Sink/Tilt` | `WarningCard.kt:38`、`HomeMiuix.kt:287` |

**Material（`SegmentedListItem` → M3 `ListItem`）**

| 项 | 值 | 出处 |
|---|---|---|
| 行高 | 单行 **56dp** / 双行 **72dp**（M3 `ListItem` 默认，KSU 未覆写） | `SegmentedList.kt:288-380`（仅传内容槽） |
| 左右内边距 | M3 默认（16dp） | 同上 |
| 前导图标 | M3 默认 **24dp** | `SegmentedList.kt:406,453,339` |
| 标题 | M3 `ListItem` headline（默认 `bodyLarge` 16sp） | `SegmentedList.kt:405` |
| 副标题 | M3 supporting（默认 `bodyMedium` 14sp，色 `onSurfaceVariant`） | `:415`、`:88` |
| 尾部箭头 | 用 `Icons.AutoMirrored.Filled.KeyboardArrowRight` 显式放在 `trailingContent` | `SettingsMaterial.kt:142-147` |
| 行按压 | 触觉反馈 `VirtualKey` + M3 波纹 | `SegmentedList.kt:399` |
| 选中态（下拉行尾部文字） | `primary`，右对齐，占 30% 宽 | `SegmentedList.kt:457-462` |

### 4.4 开关

| 项 | Miuix | Material | 出处 |
|---|---|---|---|
| 轨道尺寸 | **49 × 28dp** | 52 × 32dp（M3 默认） | `M/.../basic/Switch.kt:143`；M3 库默认 |
| 拇指尺寸 | **20dp** | 28dp（选中）/ 16dp（未选中）（M3 默认） | `M/.../basic/Switch.kt:156` |
| 拇指位移 | unchecked 4dp → checked 25dp | M3 默认 | `Switch.kt:92` |
| 开启色 | `primary`（轨道） | `primary`（轨道）+ `onPrimary`（拇指） | 库默认 / `K/ui/component/material/ExpressiveSwitch.kt:59` |
| 关闭轨道 | 库默认 `secondaryContainer` 系 | `surfaceContainerHighest`（M3 默认） | `Switch.kt` 默认 |
| 拇指内图标 | 无 | 有：选中 `Check`、未选中 `Close`，尺寸 `SwitchDefaults.IconSize` | `ExpressiveSwitch.kt:33-41` |
| 图标着色（选中） | — | `primary` | `ExpressiveSwitch.kt:50` |
| 禁用 | `disabledPrimary` / `disabledOnPrimary`（Miuix 色板） | α0.12 轨道 / α0.38 拇指 | `ExpressiveSwitch.kt:52-58` |
| 行尾内边距 | 8dp | M3 默认 | `M/.../preference/SwitchPreference.kt:73` |

### 4.5 滑杆

| 项 | 值 | 出处 |
|---|---|---|
| 最小高度（Miuix） | **28dp** | `M/.../basic/Slider.kt:1352` |
| 刻度点半径 | 3.855dp | `:1357` |
| 刻度点颜色 | `sliderKeyPoint`（浅 `#4DA3B3CD` / 深 `#4D7A8AA6`） | `Colors.kt:392,502` |
| 刻度点前景 | `sliderKeyPointForeground`（浅 `#6EB5FF` / 深 `#5DAAFF`） | `:393,503` |
| 轨道底 | `sliderBackground`（浅 `#0F000000` / 深 `#26FFFFFF`） | `:394,504` |
| 激活色 | `primary` | 库默认 |
| Material 侧 | M3 `Slider` 默认（轨道 16dp 高、拇指 4dp×44dp 条状，Expressive 有 M3 默认尺寸），KSU 未覆写 | `K/ui/screen/colorpalette/*` 使用处 |

### 4.6 下拉行与浮层

**Miuix（`OverlayDropdownPreference`）**：行本体 = `BasicComponent`（56dp/16dp/17sp/14sp，§4.3）；点开为列表浮层，浮层内每行用 KSU 自己的 `DropdownItem`：

| 项 | 值 | 出处 |
|---|---|---|
| 首行上内边距 | 20dp | `K/ui/component/miuix/DropdownItem.kt:28` |
| 中间行上/下内边距 | 12dp | `:28-29` |
| 末行下内边距 | 20dp | `:29` |
| 左右内边距 | 20dp | `:35` |
| 库级下拉尺寸 | 最小高 56dp、最小宽 200dp、勾选图标 20dp、左右内边距 20dp、图标尾部内边距 12dp、勾选前内边距 12dp、最大文本宽 216dp | `M/.../basic/Dropdown.kt:364-397` |
| 箭头/雪佛龙 | 10 × 16dp | `:373,376` |

**Material（`SegmentedDropdownItem`）**：行本体 = `SegmentedListItem`（§4.3）；尾部显示当前值文字（`primary`，占 30% 宽）；点开 `OffsetAnchoredExpressiveMenu`，选项 `SelectableDropdownMenuItem`，形状 `MenuDefaults.itemShape(index, count)`，勾选图标 `MenuDefaults.LeadingIconSize`
`K/ui/component/material/SegmentedList.kt:420-493`（尾部文字 `:457-462`，菜单 `:465-491`）

### 4.7 分段控件 / Tab

KSU 的「分段」有两层含义，注意区分：

1. **列表分段（Material 专属）**：一列 `SegmentedListItem` 用 2dp 间隙 + 外 16dp/内 4dp 圆角拼成一个「分段列表」（§4.2）。这不是 tab。
2. **真正的 Tab（颜色模式三选一等）**：Miuix 用 `TabRow`，Material 用 M3 `TabRow`。

| 项 | 值 | 出处 |
|---|---|---|
| Miuix `TabRowHeight` | **42dp** | `M/.../basic/TabRow.kt:511` |
| 带轮廓变体高度 | 45dp | `:516` |
| 圆角 | **12dp** | `:521` |
| 带轮廓变体圆角 | 8dp | `:526` |
| 颜色模式三选一（系统/浅/深） | 分段 3 项 | `K/ui/screen/colorpalette/ColorPaletteScreenMiuix.kt:167-175` |

### 4.8 按钮

| 项 | Miuix | Material | 出处 |
|---|---|---|---|
| 最小宽 | **58dp** | M3 默认（Filled 58dp） | `M/.../basic/Button.kt:158` |
| 最小高 | **40dp** | M3 默认 40dp | `M/.../basic/Button.kt:164` |
| 圆角 | **16dp** | M3 Expressive 胶囊（默认） | `M/.../basic/Button.kt:169` |
| 内边距 | 水平 **16dp** / 垂直 **13dp** | M3 默认 | `M/.../basic/Button.kt:174` |
| 主要按钮 | 容器 `primary`，内容 `onPrimary` | `filled`：容器 `primary` | 库默认 |
| 次要/文字按钮 | — | `TextButton`，主色变体 `ButtonDefaults.textButtonColorsPrimary()` | `K/ui/component/miuix/SuperEditArrow.kt:122` |
| 禁用 | `disabledPrimaryButton` / `disabledOnPrimaryButton` | M3 α0.12 / α0.38 | `Colors.kt:352-353` |
| 对话框按钮排布 | 两枚 `weight(1f)` + 中间 20dp 空隙 | 同 | `K/ui/component/miuix/SuperEditArrow.kt:95-123` |

### 4.9 输入框

| 项 | Miuix（`BasicComponent` 风格行内编辑） | Miuix `TextField` | Material |
|---|---|---|---|
| 最小高 | **56dp** | 库默认 | M3 默认（`OutlinedTextField` 56dp） |
| 内边距 | **16dp** | 16 × 16dp（`InsideMargin`） | M3 默认 |
| 圆角 | 跟随卡片 16dp | **16dp** | M3 默认 |
| 边框宽（聚焦） | — | 2dp | `M/.../basic/TextField.kt:382` |
| 浮动标签字号 | — | 10sp（浮动）/ 17sp（常态） | `:385,388` |
| KSU 自定义 EditText 行 | 最小高 56dp、内边距 16dp、左右各半并用 16dp 分隔 | | `K/ui/component/miuix/EditText.kt:68,136,151` |

### 4.9b 搜索栏（页内筛选，两风格实现不同）

**Miuix（自定义 `SuperSearchBar`）** `K/ui/component/miuix/SuperSearchBar.kt`

| 项 | 值 | 出处 |
|---|---|---|
| 顶部内边距 | 12dp（默认参数 `searchBarTopPadding`） | `:117,242,325` |
| 左右内边距 | 12dp | `:269,343` |
| 底部内边距 | 6dp | `:270,344` |
| 输入区最小高 | **45dp** | `:271` |
| 输入字号 | **17sp** | `:262` |
| 前导/尾随图标 | 44dp 占位盒，图标内边距 16dp/8dp（左右对称） | `:283-284,300-301,336-337` |
| 顶部额外留白 | 向下取 max(offsetY, 0dp)，另加系统栏 + 5dp | `:124-126` |

**Material（M3 `SearchBar` + `LargeFlexibleTopAppBar` 组合）** `K/ui/component/material/SearchBar.kt`

| 项 | 值 | 出处 |
|---|---|---|
| 外层容器色 | `surfaceContainer` | `:183` |
| 搜索栏左右内边距 | **16dp** | `:200` |
| 搜索栏底部内边距 | **13dp** | `:201` |
| 搜索栏容器色 | `surfaceContainerHighest` | `:205` |
| 展开态前导按钮 | `IconButton` 容器 `surfaceContainerHighest` / 内容 `onSurfaceVariant`，右内边距 8dp | `:156-165` |
| 展开方式 | `ExpandedFullScreenContainedSearchBar`（全屏展开） | `:210-215` |
| 库默认输入高/字号 | 45dp / 17sp（M3 `SearchBar` 默认，与 Miuix 一致） | — |

### 4.10 列表 / 日志行（我方「表格」的替代）

**KSU 没有表格组件**（全仓无 table / DataGrid）。所有结构化数据都用「卡片 + 行」或「日志条目卡」表达。抄这个语言，不要给审计/通信页发明表格皮肤。

日志条目（`SulogMiuix`，供我方审计/通信页直接照搬）：

| 项 | 值 | 出处 |
|---|---|---|
| 外卡 | `Card`，外边距左右 12dp、下 12dp | `K/ui/screen/sulog/SulogMiuix.kt:321-325,452-456` |
| 卡内边距 | **16dp** | `:459` |
| 条目间距 | **2dp** | `:467` |
| 顶部预留 | `innerPadding.top + 6dp` | `:289` |
| 滚动时标题位移 | `12dp × (1 - collapsedFraction)` | `:115` |
| 等宽/高亮 | 用 `ShellHighlight` 四色（§2.4） | `K/ui/util/ShellHighlight.kt:25-34` |
| 行内状态标签 | `StatusTag`（Miuix/Material 两版） | `K/ui/component/statustag/StatusTag.kt` |

### 4.11 对话框 / 弹层

| 项 | Miuix | Material | 出处 |
|---|---|---|---|
| 遮罩 | `windowDimming`（浅 α0.3 / 深 α0.6） | `scrim`（M3 默认） | `Colors.kt:391,501` |
| 内容内边距 | 16dp（标题下 16dp、按钮行左右 16dp） | M3 默认 | `K/ui/component/miuix/ScaleDialog.kt:41,47` |
| 按钮间距 | 20dp | M3 默认 | `ScaleDialog.kt:68` |
| 发送日志 | `SendLogDialog`（对话框） | `SendLogBottomSheet`（底部弹层） | `SettingsMiuix.kt:462-466`；`SettingsMaterial.kt:378-383` |
| Snackbar | 有 | `SnackBarHost`，底部 padding = `bottomInnerPadding` | `SettingsMaterial.kt:88` |

### 4.12 两套风格差异总表（实现时按此分支）

| 维度 | Miuix | Material |
|---|---|---|
| 卡片/行圆角 | 16dp 单卡 | 16dp 外 + 4dp 内 + 2dp 段间隙 |
| 页面水平边距 | 12dp | 16dp |
| 分组间距 | `top:12dp` | `bottom:13dp` |
| 设置行实现 | `BasicComponent`（56dp/16dp/17sp+14sp） | M3 `ListItem`（56/72dp，16sp+14sp） |
| 行尾箭头 | 库内置（`ArrowPreference`） | 手放 `KeyboardArrowRight` |
| 开关 | 49×28，拇指 20dp，无图标 | 52×32，拇指 28dp，带 Check/Close |
| 顶栏 | 小标题 `TopAppBar` | `LargeFlexibleTopAppBar` 大标题折叠 |
| 容器色 | 页面 `surface`，卡片 `surfaceContainer` | 页面 `surfaceContainer`，分段卡 `surfaceBright` |
| 底部弹层 | 对话框 | BottomSheet |
| 悬浮底栏 | 有 | 无 |
| 配色来源 | 库内建固定板（并可由种子重算） | 系统动态种子生成 |

---

## 5. 设置页信息结构（照抄用）

### 5.1 分组顺序（两套风格**完全一致**，只是组件不同）

| # | 分组内容 | 行（标题 → 组件） |
|---|---|---|
| 1 | 更新检查 | 检查更新 → Switch；检查模块更新 → Switch（仅 full-featured） |
| 2 | 界面 | **界面风格 → Dropdown（Miuix/Material）**；主题 → Arrow（进外观页） |
| 3 | 配置模板 | 配置模板 → Arrow |
| 4 | Kernel 功能（仅 full-featured） | SU 兼容模式 → Dropdown（3 项）；内核 umount → Switch；SELinux 隐藏 → Switch；SU 日志 → Switch；ADB Root → Switch；仅 ADB 的 shell su → Switch；软重启 → Switch |
| 5 | 通用 | 默认卸载模块 → Switch；Web 调试 → Switch；自动越狱 → Switch |
| 6 | LKM 专属（仅 `isLkmMode`） | 卸载 → Arrow |
| 7 | 关于 | 发送日志 → Arrow（Miuix 弹对话框 / Material 弹 BottomSheet）；关于 → Arrow |

出处：Miuix `K/ui/screen/settings/SettingsMiuix.kt:109-482`（分组 1 `:110-145`、2 `:147-180`、3 `:182-203`、4 `:205-364`、5 `:366-415`、6 `:418-443`、7 `:445-480`）；
Material `K/ui/screen/settings/SettingsMaterial.kt:97-374`（一一对应）。

行文案键：`settings_check_update`、`settings_module_check_update`、`settings_ui_mode`(+`_summary`)、`settings_theme`(+`_summary`)、`settings_profile_template`、`settings_sucompat`、`settings_kernel_umount`、`settings_selinux_hide`、`settings_sulog`、`settings_adb_root`、`settings_shell_su_adb_only`、`settings_soft_reboot`、`settings_umount_modules_default`、`enable_web_debugging`、`settings_auto_jailbreak`、`settings_uninstall`、`send_log`、`about`。

**每个功能行都有统一的四态摘要**（可直接复用为我方「能力状态」文案）：

| 状态 | 摘要文案键 |
|---|---|
| unsupported | `feature_status_unsupported_summary` |
| managed | `feature_status_managed_summary` |
| forced | `feature_status_forced_summary` |
| 正常 | 各功能自己的 `_summary` |

见 `SettingsMiuix.kt:217-222` 等（每行一处）。

### 5.2 外观页（`ColorPaletteScreen`）分组

| # | 分组 | 行 |
|---|---|---|
| 1 | 主题 | 颜色模式（系统/浅/深 三段）；Monet 开关（仅 Miuix）；取色种子（默认 + 15 色）；调色风格（仅非默认）；M3 规范版本 |
| 2 | 效果 | 模糊开关；悬浮底栏开关；悬浮底栏玻璃效果 |
| 3 | 导航 | 导航角标开关 |
| 4 | 交互 | 预测性返回；滑动返回；翻页手势模式；页面缩放 |

出处：`K/ui/screen/colorpalette/ColorPaletteScreenMiuix.kt:120,167-175,182-288,288-363,363-466`（分组序号对应 `Card` 出现行 182/288/363/466）；Material 同结构 `ColorPaletteScreenMaterial.kt`。

---

## 6. 平板布局判定与行为

### 6.1 判定链

```
window 尺寸
  └─ shouldShowSplitPane(): width≥840 || (width≥600 && aspect<1.2)   K/ui/util/WindowSize.kt:8-13
        └─ useNavigationRail(floatingBar): splitPane && !(Miuix && floatingBar)   BottomBar.kt:107-110
              ├─ true  → Row { SideRail; Pager }                          MainActivity.kt:425-457
              └─ false → Scaffold(bottomBar=BottomBar) { Pager }          MainActivity.kt:459-484
```

### 6.2 内容区宽度与边距

- **没有最大宽约束**：平板宽屏时内容铺满 rail 右侧全部空间，只有页级水平边距 12dp（Miuix）/16dp（Material）。
  证据：`SettingsMiuix.kt:105`、`SettingsMaterial.kt:99`，且全仓 `widthIn(`/`fillMaxWidth(0.` 仅用于零散控件（如 `SegmentedList.kt:460` 的 30% 尾部文字）。
- 宽屏时不额外加左右居中容器、不做「最大 1200px 居中」处理——**这是 KSU 与常见 Web 后台最不一样的一点，必须照抄**（否则平板观感立刻不像）。
- `page_scale` 通过放大 `LocalDensity` 实现整页缩放，不是改字号：
  `K/ui/MainActivity.kt:161-164`（`Density(systemDensity.density * uiState.pageScale, fontScale)`）。
  Web 端等价做法：根字号/CSS `zoom` 或 `rem` 基准缩放，且必须与所有 dp 值同比例。

### 6.3 系统栏与 insets

| 区域 | Miuix | Material |
|---|---|---|
| 页面 insets | `systemBars + displayCutout`，仅 Horizontal | `safeDrawing`，Top+Horizontal |
| rail insets | — | `systemBars ∪ displayCutout`，Start+Vertical |
| bottom bar insets | 库内处理（26+inset / 36dp） | `systemBars ∪ displayCutout`，Horizontal+Bottom |
| 状态栏图标明暗 | `isAppearanceLightStatusBars = !darkTheme` | 同 |

出处：`SettingsMiuix.kt:96`、`SettingsMaterial.kt:89`、`NavigationRailMaterial.kt:84-86`、`BottomBarMaterial.kt:49-51`、`NavigationBar.kt:289-293`、`MiuixTheme.kt:72-78`、`MaterialTheme.kt:33-39`。

---

## 7. 我方 7 页映射建议（遵循 KSU 语言）

总体原则（全部来自 KSU 现有页面结构）：

1. 每页 = `Scaffold` + `TopAppBar(页面标题)` + 一个可滚动容器；**不做左右分栏、不做卡片仪表盘拼贴**。
2. 页面主体 = 若干个「分组卡」，卡内是行；分组之间 12dp（Miuix）/13dp（Material）。
3. 行 = 「左图标 24dp + 标题 17sp/16sp + 副标题 14sp + 右侧控件」；行最小高 56dp（双行 72dp）。
4. 需要「表格」时用「行 + 分割线」或「日志条目卡」，不引入表格视觉。
5. 只有真的切换对象（4 个以内）才用底部导航/rail；其余都是页内分组。
6. 危险/异常状态用 `WarningCard` 语言（error/notice 两色），不要用彩色 Alert 条、不要 toast 弹窗堆叠。

| 页面 | 顶栏标题 | 首屏结构（分组顺序） | 关键组件 / 两风格差异 |
|---|---|---|---|
| **连接 connection** | 「连接」 | ①**主状态卡**（照 `StatusCard`：大号状态字 22sp SemiBold + 版本 15sp + 右侧 110dp 状态图标，背景成功绿/`secondaryContainer`）②连接参数分组（主机/端口/从站 行）③连接动作分组（连接/断开 按钮行） | 主状态卡 = KSU 首页工作状态卡（`HomeMiuix.kt:266-368`）：Miuix 圆角 16dp 单卡；Material 用 `SegmentedColumn` 单行 + `surfaceBright`。禁止改成仪表盘 |
| **参数 parameters** | 「参数」 | ①参数分组卡（每行：参数名 + 当前值 + 右侧「编辑」箭头）②写参数动作行 ③危险参数单独一个 Warning 分组 | 行 = Miuix `ArrowPreference` / Material `SegmentedListItem`+`KeyboardArrowRight`；数值编辑用 `SuperEditArrow` 模式（行内摘要显示当前值 → 点开对话框输入，`K/ui/component/miuix/SuperEditArrow.kt:41-64`） |
| **监控 monitor** | 「监控」 | ①实时状态分组（关键量行：名称 + 数值 + 单位 + 状态点）②曲线分组（1 张卡内 1 个折线图，卡内边距 16dp）③采样设置行 | 状态点语义色 = §2.4 成功绿/警告橙/错误红；图表用卡内区域，不给图表卡片加彩色渐变。两风格只换卡片底色与圆角，图表绘制逻辑不分叉 |
| **诊断 diagnostics** | 「诊断」 | ①警告卡列表（`WarningCard`，error 在上、notice 在下）②检查项分组（每行：检查名 + 结果标签 `StatusTag` + 右侧「查看详情」）③一键诊断动作行 | 直接用 `WarningCard` 两色（§2.4）+ `StatusTag`。Material 侧同结构但卡片是 `surfaceBright` 分段行 |
| **通信 communication** | 「通信」 | ①连接/端口选择行 ②收发日志（**日志条目卡**，等宽字体 + `ShellHighlight` 四色语法色）③输入行（`EditText` 56dp）+ 发送按钮（40dp 高、16dp 圆角） | 日志 = §4.10 模式：卡内边距 16dp、条目间距 2dp；HEX/文本切换用 `TabRow`（42dp 高、12dp 圆角）或分段行 |
| **审计 audit** | 「审计」 | ①筛选分组（时间范围 / 级别 / 关键字，各一行）②审计条目列表（每卡 16dp 内边距，2dp 条目间距：时间 14sp 弱化 + 事件 17sp + 详情 14sp）③导出/清空动作行 | 与 `SulogScreen` 同构（`K/ui/screen/sulog/SulogMiuix.kt:321-467`）。**不要**做带表头的多列表格；时间戳用弱化色 |
| **设置 settings** | 「设置」 | 严格照 §5.1 的 7 组顺序：更新/界面/模板/功能/通用/专属/关于。我方特有项（主题、风格、悬浮底栏等）放「界面」组 + 独立外观页 | 分组卡与行完全照 §4.2/§4.3；「界面风格」行 = 下拉（Miuix `OverlayDropdownPreference` / Material `SegmentedDropdownItem`），切换后**只重着色不刷新**（§1.4） |

### 7.1 导航映射（7 页 → 4 项导航？）

KSU 是 4 个导航项（Home/SuperUser/Module/Setting），我方 7 页放不下。建议（保持 KSU 语言的最小偏离）：

- 导航项收敛为 4–5 个：`连接 / 监控 / 通信 / 参数 / 设置`，诊断并入监控页内的分组、审计并入通信页（或作为顶栏次级入口）。
- 若必须 7 个一级页：宽屏用 rail（80/240dp 足够放 7 项），窄屏用 bottom bar 会超载 → 按 KSU 规则，窄屏下不超过 5 项，其余收进「更多」分组页。**不要让 bottom bar 出现 7 个 item**（KSU item 高 64dp、label 12sp 单行，7 项会挤压到不可读）。

### 7.2 Token 命名建议（供 shell-dev 落盘）

顶层切风格：`<html data-ui-mode="miuix|material" data-theme="light|dark">`，两组变量同名不同值，组件 CSS 只写一套：

```
--bg --surface --surface-container --surface-container-high --on-surface --on-surface-variant
--primary --on-primary --primary-container --on-primary-container --secondary --error --success
--outline --divider --scrim
--radius-card: 16px      (两风格相同)
--radius-inner: 4px      (Material 分段内角；Miuix 恒 16px)
--page-pad-x: 12px|16px
--group-gap: 12px|13px
--row-min-h: 56px
--row-pad: 16px
--icon-size: 24px
--title-size: 17px|16px  (Miuix 17sp / Material bodyLarge 16sp)
--subtitle-size: 14px
--nav-rail-w: 80px|240px (折叠|展开，Miuix 240 / Material 220)
--nav-bar-h: 64px
--float-bar-h: 64px --float-bar-tab-min-w: 76px --float-bar-pad: 4px
```

---

## 附录 A：本文档的已知缺口（给 Lead 决策）

1. **Material 静态 hex 不存在**。KSU 的 Material 配色 100% 由「系统种子 + materialkolor(TonalSpot, SPEC_2025)」运行时生成（`ThemeExt.kt:37-48`）。要给出精确 4 套 hex，必须离线跑一次生成脚本（附录 B），或接受「我方自选种子」的近似。这是唯一无法从源码直接抄出数值的部分。
2. **M3 库内建尺寸**（ListItem 行高、Switch 52×32、ShortNavigationBar 64dp、WideNavigationRail 220dp、M3 形状 token）不是 KSU 源码字面量。KSU 依赖 `androidx.compose.material3` Expressive 版，其 shape/尺寸 token 随版本变动；本文给的 M3 值属该库稳定默认，落地时如与实机差 2–4dp 属版本差异，不是抄错。
3. **rail 展开/收起动画曲线**：Miuix 用库内 `expandProgress` 时间线（`M/.../basic/NavigationRail.kt:193-205`），Material 用 M3 `WideNavigationRailState`；我方 Web 侧建议统一 220ms `ease-out`，不影响观感一致性。
4. **字体**：Miuix 文本样式用系统中文字体（`FontFamily.Default`，M3 `Typography` 仅覆写 `bodyLarge`，`K/ui/theme/Type.kt:9-17`）。我方沿用系统 UI 字体栈即可，不要引入第三方字体。

## 附录 B：从种子生成 Material 色板（可复现步骤）

`materialkolor` 是 material-color-utilities 的 Kotlin 封装；Web 侧可用官方 JS 版 `@material/material-color-utilities` 得到数值一致的结果：

```js
// npm i @material/material-color-utilities  （我方环境已禁装依赖时，请在离线机器生成后把结果粘进 tokens）
import { argbFromHex, hexFromArgb, Hct, SchemeTonalSpot, MaterialDynamicColors } from '@material/material-color-utilities';

const seed = argbFromHex('#3482FF');            // 与 Miuix primary 一致，两风格强调色统一
for (const isDark of [false, true]) {
  const scheme = new SchemeTonalSpot(Hct.fromInt(seed), isDark, 0.0);
  const out = {};
  for (const [role, dyn] of Object.entries(MaterialDynamicColors)) {
    if (dyn instanceof Object && 'getArgb' in dyn) out[role] = hexFromArgb(dyn.getArgb(scheme));
  }
  console.log(isDark ? 'dark' : 'light', out);  // 输出即为 §2.3 表 B 的取值
}
```

对应 KSU 侧参数：`paletteStyle = TonalSpot`（`SettingsRepositoryImpl.kt:66`）、`isDark = colorMode.isDark || 系统深色`（`MaterialTheme.kt:21`）、`SPEC_2025`（`:70`）。注意 JS 版 `SchemeTonalSpot` 对应 Spec 2021 的动态色；要严格对齐 `SPEC_2025` 需用 materialkolor 的 2025 实现（`K/ui/theme/Theme.kt:60-65` 说明 2025 仅对 TonalSpot/Neutral/Vibrant/Expressive 生效）。

---

*本文档为只读调研产物，仅新增 `docs/ksu-ui-spec.md`，未修改任何源码。*
