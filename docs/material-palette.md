# Material 风格色板（由种子离线生成）

> 用途：KSU 的 Material 风格没有静态 hex（运行时由系统壁纸种子 + materialkolor 生成）。
> 我方桌面端无壁纸，故按 KSU 同款参数固定种子离线生成一套，供 `src/styles/tokens-a.css` 落盘。
>
> - 生成参数：`SchemeTonalSpot(Hct.fromInt(seed), isDark, contrast=0.0)`，KSU 对应 `paletteStyle=TonalSpot`（`K/data/repository/SettingsRepositoryImpl.kt:66`）、`SPEC_2021`（JS 版 material-color-utilities 0.4.0 的 SchemeTonalSpot 即 Spec 2021；KSU 默认 SPEC_2025，色相/明度差异 ≤ 2~4 tone，不影响观感）
> - 种子：`#3482FF`（与 Miuix `primary` 同色，两风格强调色一致）
> - 生成命令：`npm pack @material/material-color-utilities` 解包后跑 `SchemeTonalSpot` + `MaterialDynamicColors`
> - 参考文献：`docs/ksu-ui-spec.md` §2.3 / 附录 B

## 浅色（Material light）

| 角色 | 值 | 用途 |
|---|---|---|
| primary | `#455E91` | 强调、选中、开关开态 |
| onPrimary | `#FFFFFF` | 强调上的字 |
| primaryContainer | `#D8E2FF` | 强调容器、选中底 |
| onPrimaryContainer | `#2C4678` | 强调容器上的字 |
| secondary | `#575E71` | 次强调 |
| onSecondary | `#FFFFFF` | |
| secondaryContainer | `#DBE2F9` | 次容器 |
| onSecondaryContainer | `#3F4759` | |
| tertiary | `#715573` | 第三色 |
| onTertiary | `#FFFFFF` | |
| tertiaryContainer | `#FCD7FB` | |
| onTertiaryContainer | `#583E5A` | |
| background | `#FAF9FF` | 页面底 |
| onBackground | `#1A1B20` | 正文 |
| surface | `#FAF9FF` | 表面 |
| onSurface | `#1A1B20` | |
| surfaceVariant | `#E1E2EC` | 行/输入底 |
| onSurfaceVariant | `#44474F` | 副标题 |
| surfaceDim | `#DAD9E0` | |
| surfaceBright | `#FAF9FF` | 分组列底（KSU Material 用它） |
| surfaceContainerLowest | `#FFFFFF` | |
| surfaceContainerLow | `#F3F3FA` | |
| surfaceContainer | `#EEEDF4` | 底栏/rail 容器 |
| surfaceContainerHigh | `#E8E7EF` | |
| surfaceContainerHighest | `#E2E2E9` | 开关关闭轨道 |
| outline | `#75777F` | 描边 |
| outlineVariant | `#C5C6D0` | 分割线 |
| error | `#BA1A1A` | 危险 |
| onError | `#FFFFFF` | |
| errorContainer | `#FFDAD6` | 危险容器 |
| onErrorContainer | `#93000A` | |
| inverseSurface | `#2F3036` | 反色面 |
| inverseOnSurface | `#F0F0F7` | |
| scrim | `#000000` | 遮罩 |

## 深色（Material dark）

| 角色 | 值 | 用途 |
|---|---|---|
| primary | `#AEC6FF` | 强调、开关开态 |
| onPrimary | `#122F60` | 强调上的字 |
| primaryContainer | `#2C4678` | 强调容器 |
| onPrimaryContainer | `#D8E2FF` | |
| secondary | `#BFC6DC` | |
| onSecondary | `#293041` | |
| secondaryContainer | `#3F4759` | |
| onSecondaryContainer | `#DBE2F9` | |
| tertiary | `#DFBBDE` | |
| onTertiary | `#402843` | |
| tertiaryContainer | `#583E5A` | |
| onTertiaryContainer | `#FCD7FB` | |
| background | `#111318` | 页面底 |
| onBackground | `#E2E2E9` | 正文 |
| surface | `#111318` | 表面 |
| onSurface | `#E2E2E9` | |
| surfaceVariant | `#44474F` | 行/输入底 |
| onSurfaceVariant | `#C5C6D0` | 副标题 |
| surfaceDim | `#111318` | |
| surfaceBright | `#37393E` | 分组列底（KSU Material 用它） |
| surfaceContainerLowest | `#0C0E13` | |
| surfaceContainerLow | `#1A1B20` | |
| surfaceContainer | `#1E1F25` | 底栏/rail 容器 |
| surfaceContainerHigh | `#282A2F` | |
| surfaceContainerHighest | `#33353A` | 开关关闭轨道 |
| outline | `#8E9099` | 描边 |
| outlineVariant | `#44474F` | 分割线 |
| error | `#FFB4AB` | 危险 |
| onError | `#690005` | |
| errorContainer | `#93000A` | |
| onErrorContainer | `#FFDAD6` | |
| inverseSurface | `#E2E2E9` | |
| inverseOnSurface | `#2F3036` | |
| scrim | `#000000` | 遮罩 |

## 落盘建议

- 在 `tokens-a.css` 里为 Material 二套写 `:root[data-ui='material'][data-theme='light']` / `[data-theme='dark']`。
- 语义映射：页面底 `--bg`=background；卡片/分组列 `--card`=surfaceBright（KSU Material `SegmentedColumn` 容器色）；行底 `--row`=surfaceBright；rail/底栏 `--chrome`=surfaceContainer；开关关闭轨道=surfaceContainerHighest；副标题=onSurfaceVariant；分割线=outlineVariant。
- Miuix 侧直接用 `docs/ksu-ui-spec.md` §2.2 的 53 角色表，不要用本文件的 Material 值。
- 两风格强调色虽同种子，但 Material 的 primary 是 tone-40（`#455E91` 浅色），比 Miuix 的 `#3482FF` 更沉——这是 MD3 的正常结果，不要为了"更亮"改成 Miuix 蓝。
