# 界面结构与修改指南

> 视觉基准：FlClash Windows 版（`docs/flclash-win-spec.md` + `docs/reference/flclash-windows-dashboard.png`）。
> 代码里的 `ksu-*` 类名前缀是历史命名（早期按 KernelSU 复刻时的前缀），现在指 **FlClash 皮肤**，不要按名字理解。
> 本文列的是**改界面前必须知道的入口和不能破坏的契约**。

## 1. 文件分工

| 位置 | 职责 | 改动规则 |
|---|---|---|
| `src/styles/tokens-a.css` | 唯一色源 + 字阶/间距/形状/尺寸 | 只在这里写色值；新颜色先加 token；`pnpm lint:tokens` 会拦其他文件的 hex/rgb/hsl |
| `src/styles/ksu.css` | 壳与组件的布局/交互样式（`.ksu-*` 公共类） | 类名是 `views/`、`shell/`、`components/ksu/` 的公共契约，改名先 `rg` 全仓 |
| `src/styles/views.css` | 只补视图局部排版 | 不覆盖核心 `.ksu-*` 规则，不写色值，字号走 `--ksu-fs-*` |
| `src/styles/styles.css` | 旧组件遗留样式 | **冻结：只删不加**；新样式写上面三个文件 |
| `src/styles/bridge-a.css` | 旧变量名 → 新 token 的桥接 | 旧组件删完前不要删；只许 `var(...)` |
| `src/components/ksu/` | UI 原语（页面/卡片/行/开关/分段/按钮/弹层等） | 新页面优先复用，不要自己拼接一套 |
| `src/shell/` | 应用壳：拖拽条、图标栏、窄屏底栏、主题/字号偏好 | 见第 3 节契约 |
| `src/views/` | 七个工作区视图 | 页面级组合放这里；业务状态一律走 `src/composables/useServo.ts` |

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
3. **样式引入顺序**：`src/main.ts` 里 `tokens-a → styles.css → ksu.css → views.css → bridge-a` 是硬顺序，`bridge-a.css` 必须最后；调换会让旧组件或视图微调失效。
4. **测试是源码直测**：`tests/*.test.mjs` 用 `readFileSync` 读 `src/` 源码在 vm 里跑。**移动/重命名源码文件必须同步改夹具路径**；`src/shell/activities.ts` 被 `communication.test.mjs` 直读，`pages / PageId / pageById` 导出不可改签名；`recording / ethercat / history` 组件里被断言的状态名与 `defineExpose` 不要改名。
5. **`ksu-*` 类名是跨文件契约**：`views/` 写类名、`ksu.css` 实现、`components/ksu/` 复用。改类名前先 `rg "ksu-xxx" src`，否则会出现“页面还在用、样式没了”。
6. **`--sa-*` 兼容别名**：`tokens-a.css` 同时导出旧 `--sa-*` 名给 `styles.css` / `bridge-a.css` / 少量旧组件用；删之前确认 `rg -- "--sa-" src` 无引用。
7. **Windows 构建走包装脚本**：`scripts/with-msvc.cmd`（VS2022 + clang-cl + 仓库内 TMP）。`.vscode/tasks.json` 已内置；直接 `cargo` 会挂在 `native/bridge.c` / `libsqlite3-sys` 上。SOEM 的两处源码补丁见 `src-tauri/native/README.md`，升级 SOEM 时要重打。
8. **字号缩放**：新样式字号必须用 `var(--ksu-fs-*)`，写裸 `px` 会导致 90/110% 档位下大小不一。

## 4. 加一个页面 / 改一个视图

1. 页面表：`src/shell/activities.ts` 加 `{ id, label, caption, icon }`，`PageId` 自动扩展；宽度栏 7 项以内不用改导航结构。
2. 注册视图：`src/App.vue` 的 `v-else-if` 链加组件（视图无 props，内部用 `useServo()`）。
3. 视图骨架：用 `.ksu-page > .ksu-page-head(标题/副标题/actions) + .ksu-page-body > .ksu-grid/.ksu-card`；行用 `.ksu-list/.ksu-row`。
4. 需要新样式：先看 `ksu.css` 是否已有类；没有再加，写在对应区块并保持不写色值；视图专属微调放 `views.css`。
5. 别在卡片里堆操作：每卡最多 1 个主操作，其余进卡头动作或行内。

## 5. 调试与截图

- 网页预览：`pnpm dev`（http://localhost:1420）。
- 桌面壳：`scripts\with-msvc.cmd pnpm tauri dev`。
- 截图/深链参数（仅首次进入读取，不写回 URL）：`?page=settings&theme=light&textScale=1.1&rail=1`，`page` 取 `activities.ts` 里的 id。
- 无头截图（本机 Edge）：

  ```powershell
  msedge --headless=new --disable-gpu --no-sandbox --disable-features=CalculateNativeWinOcclusion `
    --window-size=1440,900 --screenshot=shot.png "http://localhost:1420/?page=monitor"
  ```

- 验收门禁：`pnpm lint:tokens`、`pnpm build`、`pnpm test`；改到 Rust 侧再加 `scripts\with-msvc.cmd cargo test / clippy`。
