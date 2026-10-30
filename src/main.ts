import { createApp } from "vue";
import App from "./App.vue";
// 样式顺序是硬契约：tokens（唯一色源）→ styles.css（旧组件兜底）→ ksu（新壳）
// → views.css（视图微调）→ bridge-a（旧变量名映射，必须最后覆盖）。
// 改顺序前先读 docs/ui.md：调换会让旧组件或视图微调静默失效。
import "./styles/tokens-a.css";
import "./styles.css";
import "./styles/ksu.css";
import "./styles/views.css";
import "./styles/bridge-a.css";
import { initUiMode } from "./shell/ui-mode";

// 先把 <html data-ui / data-theme> 写好，避免首帧用错色板。
initUiMode();

createApp(App).mount("#app");
