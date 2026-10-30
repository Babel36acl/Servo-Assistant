# Servo Assistant · VSCode 壳重构任务计划

> 目标：Vue3 + Tauri2，抄 VSCode 那套：左侧图标栏 / 二级侧栏 / 中间主区 / 底下状态栏，命令面板全局呼出。
> 风格 A：深蓝底 #0A1628、碎玻璃 + 发光线条、Maple Mono、圆角 12、阴影收敛。
> 铁律：所有颜色字号间距全部收进 design tokens，禁止散写色值。

## 现状（实测数字）
- `src/App.vue` 970 行 / ~48KB：6 个页面全用 v-show 常驻，33 个 ref、34 个 function、22 个 async 全挤在一个 setup 里
- `src/styles.css` 1658 行：29 个独立 hex、64 处 font-size、367 处 px；:root 有 token 架子，但色值/字号/间距仍在各处散写
- `CommunicationWorkbench.vue` ~23KB：原始串口 + 模拟器事件 + EtherCAT 帧录制 + 分卷 + PCAPNG 导出 + 内嵌 EthercatMaster/RecordingLibrary/RecordingHistory，一个文件扛了两条主线
- 导航是 208px 窄轨 + 顶栏，窄窗才切顶部导航；没有图标栏/二级栏/状态栏/命令面板的概念

## 要去哪里：新壳
```
桌面 >=1024px（VSCode 式四区）
┌─────────────────────────────────────────┐
│ TitleBar（自绘：拖拽区 + 窗口按钮）      │
├────┬──────────┬─────────────────────────┤
│图标 │ 二级侧栏  │ 主区（面包屑/页头+内容）│
│栏  │ 260-300px│                         │
│48px│          │                         │
├────┴──────────┴─────────────────────────┤
│ StatusBar（连接/模式/统计/后台任务/报错） │
└─────────────────────────────────────────┘
浮层：CommandPalette（Ctrl+K / Ctrl+Shift+P 全局）
```
```
平板 <1024px（单列）
┌──────────────────┐
│ TitleBar（精简）  │
│ 主区（单列滚动）  │
│ 底部 Tab（图标栏  │
│ 下移，>=44px）    │
└──────────────────┘
二级侧栏变抽屉，按需拉出；StatusBar 收成一行关键态。
```

## Google 规范映射（M3 为准绳，不照抄皮）
- 导航模式对 M3 Navigation rail + Navigation drawer：图标栏 = rail，二级栏 = drawer，主区不变
- Tokens 分层对 M3：color / type / shape / elevation / motion / state，全部落进 `tokens-a.css`
- 可达性：对比 4.5:1、focus-visible 常在、skip-link 保留、reduced-motion / 高对比降级、触控目标 >=44px、间距 8px 起跳
- 动效收敛：只留 140/180/240 三档 + 强 ease-out，进入错峰、按下回馈；玻璃 blur 只给铬（栏/条/浮层），内容面板实色保可读

## 风格 A · Token 草案（唯一真相源）
- 地基：`--sa-bg: #0A1628`，向上做 3 级 panel（#0E1E38 / #122641 / #162E4F 方向，具体值 P0 定稿时锁死）
- 碎玻璃：只在 TitleBar/ActivityBar/SideBar/StatusBar/CommandPalette 用 `backdrop-filter: blur(20px) saturate(160%)` + 对角碎片渐变 + 1px 发光描边（`box-shadow: 0 0 12px rgba(90,180,255,.25)` 量级，具体 P0 收敛）
- 发光线：1px 高亮线 `inset 0 1px 0 rgba(160,220,255,.18)`，不做多重外发光
- 字体：`--sa-font-mono: "Maple Mono NF CN", "Maple Mono", ui-monospace...` 全局主字体；数字 tabular-nums
- 字阶：12 / 13 / 14 / 16 / 20 / 24，只许用这六档；行高 1.5/1.6 两档
- 间距：4 / 8 / 12 / 16 / 24 / 32；圆角：8 / 12 / 16 / pill，主圆角 12；阴影只留 soft / float 两档
- 语义色：info / success / warning / danger 各一枚，全部走 token；Accent 从香槟金切到冷蓝-冰青系（P0 出 4-6 个命名 hex）
- 禁散写：除 `styles/tokens-a.css` 外禁止出现 `#[0-9a-f]`、`rgb(`/`hsl(`；`pnpm lint:tokens` grep 门禁进 CI

## 页面拆分（把倒在一起的抽屉分开）
- `src/shell/`：AppShell.vue、TitleBar.vue、ActivityBar.vue、SideBar.vue、StatusBar.vue、CommandPalette.vue
- `src/views/`：ConnectionView / ParametersView / MonitorView / DiagnosticsView / CommunicationView（薄编排，只做切换）/ AuditView
- `src/features/ethercat/`：EthercatMasterView（SOEM/Npcap 发现 + PRE-OP CoE/SDO + 六通道监控，只管主站）
- `src/features/recording/`：RecordingSessionView（录制/分卷/筛选/跟随/触发）、RecordingLibrary / RecordingHistory（原样搬入瘦身）、PcapExport（文件/分卷统一检索导出）
- `src/features/parameters/`：参数表 + 快照 + 事务（含 grouping.ts，原 communication.ts 搬家）
- `src/features/monitor/`：状态卡片 + ScopeChart（原样）
- `src/composables/`：useProfile / useConnection / useParams / useMonitor / useDiagnostics / useRecordingBridge / useCommandPalette / useStatusBar（把 App.vue 里 33 个 ref 按域拆出去）
- `src/styles/`：tokens-a.css（唯一色字距源）/ base.css / shell.css / views.css / components.css；旧 styles.css 冻结只删不加
- `src/api.ts`、`src/types.ts` 原位保留；先不引 vue-router / pinia，用组合式 + activeView 状态撑起导航和 palette

## 阶段
- [x] P0 tokens + 门禁：建 tokens-a.css，定死色/字/距/角/影/动效六表，接 lint:tokens + stylelint（只许 tokens 文件有 hex）
- [x] P1 壳 + 标题栏 + 状态栏 + 命令面板：ActivityBar 六入口（沿用现有 pages id/caption/icon），SideBar 按 activity 切换内容，TitleBar 自绘 + `data-tauri-drag-region` 留出拖拽区，StatusBar 接 connected/mode/stats/backgroundActivity，palette 接页面跳转 + 连接/断开/读取快照
- [x] P2 视图迁移：connection → parameters → monitor → diagnostics → audit 逐个从 App.vue 搬进 views/，v-show 改懒加载（v-if），每搬一个跑 `pnpm build`
- [x] P3 拆 EtherCAT 与录制：CommunicationView 只剩 Tab 切换；EtherCAT 主站 vs 录制会话彻底分家，backgroundActivity 改走 status bar 提示 + 一键返回
- [ ] P4 1024 断点 + 触控 + 可达性：单列 + 底部 Tab + 抽屉式 SideBar，全按钮 >=44px，focus/对比/动效降级扫一遍，跑 `pnpm test` + `cargo test` + clippy
- [ ] P5 收尾：删旧 styles.css 散写，截图对照风格 A，更新 README「界面」段 + docs 快照

## 验收
- grep `#[0-9a-fA-F]` 只命中 tokens-a.css；字号/间距只有 token 变量
- 桌面四区 + 平板单列 + 自绘标题栏可拖 + palette 全局呼出 + 状态栏常显连接态
- EtherCAT 与录制不再同文件；App.vue 只剩壳 <150 行
- `pnpm build` / `pnpm test` / `cargo test` 全绿

## 遇到的错误
| 错误 | 尝试 | 解决 |
|------|------|------|
| — | — | — |
