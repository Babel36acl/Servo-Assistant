# 001 · 壳动效加固（已部分落地，剩余验收）

> 依据：`review-animations` 十项非协商标准 + `remove-animations` Gate；数值全部内联，不依赖上文。

## 现状（已在本轮改完）

| # | 位置 | 改前 | 改后 | 验收 |
|---|---|---|---|---|
| 1 | `src/styles/tokens-a.css` | 无 drawer/stagger/press | 新增 `--sa-ease-drawer: cubic-bezier(0.32,0.72,0,1)`、`--sa-stagger: 50ms`、`--sa-press: 0.97` | `lint:tokens` 过，无 hex |
| 2 | `src/styles/bridge-a.css` | 旧变量只到 `--ease-in-out` | 追加 `--ease-drawer/--stagger/--press` 三重指向 | 同上 |
| 3 | `src/styles/shell.css` 标题栏/图标栏 | 裸 `:hover`（触屏粘滞）+ 无 `:active` | `hover` 包进 `@media (hover:hover) and (pointer:fine)`；新增 `:active { transform: scale(var(--sa-press)) }` + `transition: transform var(--sa-fast) var(--sa-ease-out)` | 桌面 hover 正常、触屏无粘滞、按压有 140ms 回弹 |

## 有意不改（决定）

- `src/shell/CommandPalette.vue`：键盘唤起（Ctrl+K / Ctrl+Shift+P）保持零动画。频率 100+/天，按 Gate 直接 DELETE 任何进入/退出动画。
- `styles.css` 遗留 keyframes（`banner-in/page-in/pill-in/modal-*`）：冻结只删不加，不在本 plan 内重构。

## 后续执行（按杠杆排序，需另起提交）

1. **平板抽屉曲线**：`shell.css` 侧栏 `transition: transform var(--sa-panel) var(--sa-ease-out)` → 同值改 `var(--sa-ease-drawer)`，真机验 240ms 跟手感。
2. **退出对称性抽查**：toast/抽屉若后续加退出，必须同边退出；删退出时同步删 `transitionend/onAnimationEnd/setTimeout` 等待。
3. **stagger 预留启用**：偶发列表首次进入可用 `transition-delay: calc(var(--index) * var(--sa-stagger))`，上限 80ms/项，总量不超 300ms；高频列表禁用。

## 验证

```sh
pnpm lint:tokens
pnpm build   # vue-tsc --noEmit + vite
pnpm test    # 23 用例
```

真机（`tauri dev`）：标题栏按钮按压回弹、图标栏 hover 不粘滞、平板抽屉 240ms、命令面板瞬开；`prefers-reduced-motion` 下动效压到 `0.01ms`。
