# findings · 壳重构依据（实测为准）

## 1. App.vue：一个文件就是半个应用
- 970 行 / 48111 字节；template 里 54 个 div、9 个 section、38 个 v-if、126 处 class
- 6 页全 `v-show` 常驻（connection / parameters / monitor / diagnostics / communication / audit），切页不卸载，状态全留在 33 个 ref 里
- script：34 个 function、22 个 async（pollDiscovery / discover / probeRead / stabilityTest / applyCommunication…），串口独占 + 轮询 + 录制状态全耦合
- 结论：必须按域拆 composables，视图改懒加载；App.vue 最终只剩壳

## 2. styles.css：token 架子有了，散写没收住
- 1658 行，29 个独立 hex（#0e0e12 / #eba937 / #8ab4f8 / #4cc38a / #e07b39 / #ef6461 …），64 处 font-size，367 处 px
- 注释头写着 Apple HIG + Material/Vercel + Emil 动效 + Obsidian Blue Topaz + 香槟金 + Inter，与本次风格 A（#0A1628 + 碎玻璃 + Maple Mono）直接冲突——旧审美要整块换掉，不能渐变
- 章节：外壳窄轨 / 悬浮顶栏 / 横幅 / 面板按钮表单 / 连接双栏 / 监控曲线 / 参数吸顶表 / 诊断审计通信 / 确认弹窗 / 响应式（14 个 @media）/ 降级
- 门禁方案：`tokens-a.css` 唯一允许 hex；其余 css 用 `var(--sa-*)`；CI 加 `grep -rnE '#[0-9a-fA-F]{3,8}|rgb\(|hsl\(' src --include='*.css' --include='*.vue'` 排除 tokens 文件

## 3. CommunicationWorkbench：杂糅重灾区（quirky 里最乱的一屉）
- 23KB：串口收发 + 模拟器事件 + EtherCAT 帧后台录制 + 分卷 + 离线浏览 + PCAPNG 导出全在一页
- 内嵌 EthercatMaster（SOEM/Npcap：发现 / PRE-OP CoE-SDO / 六通道）+ RecordingLibrary（文件/分卷）+ RecordingHistory（曲线定位事务）
- 拆法：CommunicationView 只做 Tab 头；`features/ethercat/` 管主站，`features/recording/` 管录制会话/库/历史/PcapExport；backgroundActivity 改 status bar 常驻提示

## 4. Tauri2 自绘标题栏前提
- 现 `tauri.conf.json`：width 1360 / min 1080x700，无 decorations/transparent 配置，默认原生标题栏
- 改法：`decorations: false` + 前端 TitleBar（左 activity 缩略/标题/拖拽区/右 min-max-close，调用 `@tauri-apps/api/window`）；拖拽区用 `data-tauri-drag-region`，按钮区排除；Linux/Win 需实机验拖拽与双击最大化；保留原生 fallback 开关

## 5. 1024 断点与触控
- 旧阈值是窄窗切顶部导航 + 表格自滚，没有单列/抽屉/底部 Tab 概念；minWidth 360 保留
- 新阈：>=1024 四区全开；<1024 单列，ActivityBar 下移为底部 Tab，SideBar 变 overlay 抽屉，StatusBar 收纳；所有可点元素 min-height/min-width 44px，表单控件同高；沿用 safe-area padding

## 6. Google/M3 对照点
- Navigation rail（图标栏 48-56px）+ drawer（二级栏 260-300px）即 VSCode 壳的 M3 名字
- Tokens 五层：color roles / type scale / shape scale / elevation / motion；state layer（hover/focus/pressed opacity）替代现在各处手写 opacity
- 命令面板对 M3 Search + keyboard shortcut（Ctrl+K / Ctrl+Shift+P），首项默认选中、Esc 关闭、箭头导航、全键盘可达
