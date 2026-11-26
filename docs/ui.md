# 界面结构与修改指南

> 视觉基准：FlClash Windows 版（`docs/flclash-win-spec.md` + `docs/reference/flclash-windows-dashboard.png`）。
> 代码里的 `ksu-*` 类名前缀是历史命名（早期按 KernelSU 复刻时的前缀），现在指 **FlClash 皮肤**，不要按名字理解。
> 本文列的是**改界面前必须知道的入口和不能破坏的契约**。

## 1. 文件分工

| 位置 | 职责 | 改动规则 |
|---|---|---|
| `src/styles/tokens-a.css` | 唯一色源 + 字阶/间距/形状/尺寸 | 只在这里写色值；新颜色先加 token；`pnpm lint:tokens` 会拦其他文件的 hex/rgb/hsl |
| `src/styles/ksu.css` | 全局底座（body / 重置 / 可访问性）+ 壳与组件的布局/交互样式（`.ksu-*` 公共类） | 类名是 `views/`、`shell/`、`components/ksu/` 的公共契约，改名先 `rg` 全仓 |
| `src/styles/views.css` | 只补视图局部排版 | 不覆盖核心 `.ksu-*` 规则，不写色值，字号走 `--ksu-fs-*` |
| `src/components/ksu/` | UI 原语（页面/卡片/行/开关/分段/按钮/弹层等） | 新页面优先复用，不要自己拼接一套；横排按钮组用 `.ksu-actions`、行内控件用 `.ksu-inline` |
| `src/shell/` | 应用壳：拖拽条、图标栏、窄屏底栏、主题/字号偏好 | 见第 3 节契约 |
| `src/composables/useServo.ts` | 对外聚合单例（首个调用方必须是 App.vue） | 只做聚合与生命周期，领域实现写 `src/composables/servo/` |
| `src/composables/servo/` | 领域 composable：`core` / `confirm` / `communication` / `monitoring` / `parameters` / `connection` / `format` | 新增跨页状态加到对应领域，不要再往 `useServo.ts` 堆字段 |
| `src/views/` | 七个工作区视图 | 页面级组合放这里；业务状态一律走 `src/composables/useServo.ts` |
| `src/features/` | 连接卡片、录制、EtherCAT、寄存器扫描 | 视图拆出的卡片放这里，卡片内部同样只读 `useServo()` |

> `src/styles.css` 与 `src/styles/bridge-a.css` 已删除，`--sa-*` 兼容别名同时移除；
> `pnpm lint:tokens` 会把裸字号、散写色值和旧变量名当作错误拦下，不要再加回“最后覆盖”的老变量层。

## 2. 结构与主题

```
.ksu-app
├─ .ksu-titlebar   36px 自绘拖拽条 + 右上最小化/最大化/关闭
├─ .ksu-body
│  ├─ .ksu-rail    左侧图标栏：默认展开 220px，可收成 48px（顶部按钮切换）
│  └─ .ksu-main    整页滚动区，页面自己放 .ksu-page（标题 + 卡片栅格）
└─ .ksu-dock       <840px 时才显示的底部 5 项导航（4 主 + 更多弹层）
```

- 主题：`<html data-theme="dark|light" data-text-scale="0.9|1|1.1">`，由 `src/shell/ui-mode.ts` 写入；设置页只暴露「深色/浅色/跟随系统」和「字号缩放」。
- 侧栏默认展开（220px），收起状态记在 `localStorage.sa.railExpanded`；顶部按钮可切换，设置页不再提供开关。
- 图标右栏宽度阈值：`>=840px` 显示侧栏，`<840px` 显示底部导航，两者互斥。

## 3. 改动前确认（容易踩的坑）

1. **窗口按钮依赖权限**：`src-tauri/capabilities/default.json` 必须保留
   `core:window:allow-close / allow-minimize / allow-maximize / allow-unmaximize / allow-toggle-maximize / allow-start-dragging / allow-set-focus / allow-is-maximized / allow-set-title`。
   `core:window:default` 只有只读能力，删掉这些会让自绘标题栏的 ✕ / 最小化 / 最大化**静默失效**。
2. **拖拽区**：`.ksu-titlebar` / 子元素上的 `data-tauri-drag-region` 与 `-webkit-app-region: no-drag` 是配对关系，改标题栏先确认窗口还能拖。
3. **样式引入顺序**：`src/main.ts` 里 `tokens-a → ksu.css → views.css` 是硬顺序；`tokens-a` 必须先（唯一色源），`views.css` 必须最后（视图微调）。
4. **测试是源码直测**：`tests/*.test.mjs` 用 `readFileSync` 读 `src/` 源码在 vm 里跑，`communication.test.mjs` 只 mock vue / tauri / api，其余相对导入按路径递归加载真实源码。**移动/重命名源码文件必须同步改夹具入口路径**；`src/shell/activities.ts` 被直读，`pages / PageId / pageById` 导出不可改签名；`recording / ethercat / history` 组件里被断言的状态名与 `defineExpose` 不要改名。
5. **`ksu-*` 类名是跨文件契约**：`views/` 写类名、`ksu.css` 实现、`components/ksu/` 复用。改类名前先 `rg "ksu-xxx" src`，否则会出现“页面还在用、样式没了”。
6. **旧变量名已退役**：`--sa-*` 与 bridge-a 的 `--bg/--panel/--ink/--line` 等别名全部删除，新样式只读 `--ksu-*`；`lint:tokens` 会拦住回流。
7. **Windows 构建走包装脚本**：`scripts/with-msvc.cmd`（VS2022 + clang-cl + 仓库内 TMP）。`.vscode/tasks.json` 已内置；直接 `cargo` 会挂在 `native/bridge.c` / `libsqlite3-sys` 上。SOEM 的两处源码补丁见 `src-tauri/native/README.md`，升级 SOEM 时要重打。
8. **字号缩放**：新样式字号必须用 `var(--ksu-fs-*)`，写裸 `px` 会导致 90/110% 档位下大小不一。

## 4. 加一个页面 / 改一个视图

1. 页面表：`src/shell/activities.ts` 加 `{ id, label, caption, icon }`，`PageId` 自动扩展；宽度栏 7 项以内不用改导航结构。
2. 注册视图：`src/App.vue` 的 `v-else-if` 链加组件（视图无 props，内部用 `useServo()`）。
3. 视图骨架：用 `.ksu-page > .ksu-page-head(标题/副标题/actions) + .ksu-page-body > .ksu-grid/.ksu-card`；行用 `.ksu-list/.ksu-row`。
4. 需要新样式：先看 `ksu.css` 是否已有类；没有再加，写在对应区块并保持不写色值；视图专属微调放 `views.css`。
5. 别在卡片里堆操作：每卡最多 1 个主操作，其余进卡头动作或行内。

## 5. 排版与响应式基线

- 卡片栅格两种：`.ksu-grid`（auto-fit 340 起排，同排等高，跨列用 `.ksu-span-2` / `.ksu-span-full`）
  与 `.ksu-kpi-strip`（KPI 小卡专用，固定 2 / 3 / 6 列分档：`<700` 两列、`≥700` 三列、`≥1360` 一行 6 个）。
- 双列板式用 `.ksu-columns > .ksu-col.ksu-col--wide + .ksu-col.ksu-col--narrow`：
  两列各自纵向堆叠，短卡不会在另一列留下空洞；<900 自动退化为单列。
- 行就用 `.ksu-list > .ksu-row`（最小高 56，左右 16），卡内横排按钮用 `.ksu-actions`，
  「输入 + 小按钮」用 `.ksu-inline`，不要再用旧的 `button-row` / `inline-field`。
- 触控：可点控件 ≥40（小按钮 40、分段/标签页 46 含 3 内距、行 56）；
  开关是 52×32 轨道，`pointer: coarse` 或 ≤1024 时靠透明伪元素补到 44 命中区。
- 平板与刘海屏：`.ksu-app` 用 `env(safe-area-inset-*)` 让位，底栏 `is-bar` 横屏也让左右；
  页尾留白 = 底栏高度 + safe-area，`.ksu-main` 带 `scroll-padding-bottom`，
  输入框聚焦时不会被底栏或软键盘压住。
- 验收尺寸：1440×900、1280×800、800×1100、800×1280（平板竖屏）；四个尺寸都不许出现横向滚动。

## 6. 调试与截图

- 网页预览：`pnpm dev`（http://localhost:1420，`vite.config.ts` 里 strictPort）。
  若本机 1420 落在 Hyper-V 端口排除区间（`netsh int ipv4 show excludedportrange protocol=tcp` 可查），
  临时用 `pnpm dev --port 1650 --host 127.0.0.1` 截图，不要把端口改动提交。
- 桌面壳：`scripts\with-msvc.cmd pnpm tauri dev`。
- 截图/深链参数（仅首次进入读取，不写回 URL）：`?page=settings&theme=light&textScale=1.1&rail=1`，`page` 取 `activities.ts` 里的 id。
- 无头截图（本机 Edge）：

  ```powershell
  msedge --headless=new --disable-gpu --no-sandbox --disable-features=CalculateNativeWinOcclusion `
    --window-size=1440,900 --screenshot=shot.png "http://localhost:1420/?page=monitor"
  ```

- 验收门禁：`pnpm lint:tokens`、`pnpm build`、`pnpm test`；改到 Rust 侧再加 `scripts\with-msvc.cmd cargo test / clippy`。
