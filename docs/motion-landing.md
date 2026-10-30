# Motion 落地：Emil skills → Servo-Assistant（Vue3 + Tauri2）

> 翻译原则：数值照搬 Emil，调用方式译成 Vue/CSS/WAAPI；React 专属库不直装。

## Token 映射（唯一真相源仍是 `src/styles/tokens-a.css`）

| Emil 原值 | 本仓 token | 用途 |
|---|---|---|
| `cubic-bezier(0.23,1,0.32,1)` 强 ease-out | `--sa-ease-out` | 进入/退出、按压回弹 |
| `cubic-bezier(0.77,0,0.175,1)` | `--sa-ease-in-out` | 屏内移动/形变 |
| `cubic-bezier(0.32,0.72,0,1)` iOS 抽屉曲线 | `--sa-ease-drawer` | 侧栏抽屉（平板 `<1024px`） |
| press 100–160ms | `--sa-fast: 140ms` | `:active scale`、skip-link、modal-fade |
| dropdown 150–250ms | `--sa-base: 180ms` | 抽屉外其他浮层 |
| modal/drawer 200–500ms | `--sa-panel: 240ms` | 平板侧栏 `translateX` |
| stagger 30–80ms/项 | `--sa-stagger: 50ms` | 后续列表分组进入（预留，未启用） |
| press scale 0.95–0.98 | `--sa-press: 0.97` | 所有可点按钮 `:active` |

旧变量经 `src/styles/bridge-a.css` 重指向：`--fast/--base/--panel-t/--ease-out/--ease-in-out`
+ 新增 `--ease-drawer/--stagger/--press`。`styles.css` 为冻结遗留，不新增动效。

## 硬规则（已落进 `shell.css`）

1. 只动 `transform` + `opacity`；`transition: all` 禁止（逐属性列出）。
2. `hover` 动效一律包在 `@media (hover:hover) and (pointer:fine)` 内；触屏走 `:active`。
3. 可点元素必有 `:active { transform: scale(var(--sa-press)) }`，与 `transition: transform var(--sa-fast) var(--sa-ease-out)` 同发。
4. `prefers-reduced-motion: reduce` 下壳内动效压到 `0.01ms`（`shell.css` 已有，保留）。
5. 键盘唤起面零动画：`CommandPalette`（Ctrl+K / Ctrl+Shift+P）保持瞬时出现，不加进入/退出动画。

## React → Vue 翻译表

| Emil（React/Motion/Base-UI） | 本仓做法 |
|---|---|
| `motion.div animate={{ x }}` / `x/y/scale` 简写 | 不用；用全量 `transform: translateX()` 字符串或 CSS transition（硬件加速） |
| `var(--transform-origin)`（Base-UI） | 触发锚定浮层用 `transform-origin` 指到触发点；居中模态豁免居中 |
| `AnimatePresence` 退出驻留 | Vue `<Transition>`；若删退出动画，同步删 `onAnimationEnd/transitionend` 等待与 `setTimeout(duration)` |
| `whileInView` 全屏滚动显现 | 禁止逐节 reveal；偶发页最多一组淡入 |
| Sonner-React / cmdk / base-ui / NumberFlow / recharts（`pick-ui-library` 清单） | 不直装；toast/命令面板/虚拟列表按 Vue 生态另选，决策记录在 plan 内 |

## 只读审计摘要（2026-10-30）

- `shell.css` 已修：标题栏/图标栏 `hover` 设门 + `:active` 按压（本次提交）。
- `CommandPalette.vue`：无 transition/animation，符合键盘零动画，保持。
- `styles.css`（冻结）：`banner-in/page-in/pill-in/modal-in` 等遗留 keyframes 只删不加，后续随 P5 字号工程清。
- 待办见 `animation-plans/001-shell-motion-hardening.md`（执行顺序与验收在内）。
