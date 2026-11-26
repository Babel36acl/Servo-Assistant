import { createApp } from "vue";
import App from "./App.vue";
// 样式顺序是硬契约：tokens（唯一色源）→ ksu（壳 / 组件 / 全局底座）→ views.css（视图微调）。
// 旧 styles.css 已退役；bridge-a 桥接层在下一步删除。
// 改顺序前先读 docs/ui.md：调换会让视图微调静默失效。
import "./styles/tokens-a.css";
import "./styles/ksu.css";
import "./styles/views.css";
import "./styles/bridge-a.css";
import { initUiMode } from "./shell/ui-mode";

// 先把 <html data-ui / data-theme> 写好，避免首帧用错色板。
initUiMode();

createApp(App).mount("#app");
