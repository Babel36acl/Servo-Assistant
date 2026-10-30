// lint:tokens — 色值散写门禁（P0）。
// 规则：除 src/styles/tokens-a.css 与 src/styles/chart-palette.ts 外，
// *.css / *.vue <style> / *.ts 内禁止出现 hex（#abc / #aabbcc）与 rgb()/hsl()。
// 字号/间距裸写只告警（WARN），P2 迁移时再收紧为错误。
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, extname } from "node:path";

const ROOT = new URL("../src", import.meta.url).pathname;
const ALLOW = new Set(['src/styles/tokens-a.css', 'src/styles/chart-palette.ts']);
// 存量散写（P2/P3 迁移时清零）：冻结只删不加，门禁放行但单独计数。
// P1 新建的 shell/views/features/composables 一律不许进这个名单。
// P3 已清：RecordingSessionView / EthercatMasterView / RecordingHistory / ScopeChart。
const LEGACY = new Set([
  'src/styles.css',
]);
const HEX = /#[0-9a-fA-F]{3,8}\b/g;
const FUNC = /\b(?:rgb|rgba|hsl|hsla)\s*\(/g;
const FONT_PX = /font-size\s*:\s*\d+px/g;

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) walk(p, out);
    else if ([".css", ".vue", ".ts"].includes(extname(p))) out.push(p);
  }
  return out;
}

function styleOfVue(text) {
  const blocks = [...text.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1]);
  return blocks.join("\n");
}

function styleOfTs(text) {
  // 调色板模块是色值的唯一 JS 落点，其余 TS 不许散写
  return text;
}

let errors = 0;
let warns = 0;
let legacy = 0;
for (const file of walk(ROOT)) {
  const rel = "src" + file.slice(ROOT.length);
  if (ALLOW.has(rel)) continue;
  const raw = readFileSync(file, "utf8");
  const text = file.endsWith(".vue") ? styleOfVue(raw) : file.endsWith(".ts") ? styleOfTs(raw) : raw;
  if (!text.trim()) continue;
  // TS 只查色值，不查字号（逻辑文件无 font-size 语义）
  const skipFontCheck = file.endsWith(".ts");
  const hexes = [...text.matchAll(HEX)].map((m) => m[0]).slice(0, 5);
  const funcs = text.match(FUNC);
  if (hexes.length || funcs) {
    if (LEGACY.has(rel)) {
      legacy++;
      console.warn(`[tokens-legacy] ${rel} 存量散写（冻结，P2/P3 清）：${[...hexes, ...(funcs ? [funcs[0]] : [])].join(" ")}`);
    } else {
      errors++;
      console.error(`[tokens] ${rel} 散写色值：${[...hexes, ...(funcs ? [funcs[0]] : [])].join(" ")}`);
    }
  }
  const fonts = skipFontCheck ? null : text.match(FONT_PX);
  if (fonts) {
    warns++;
    console.warn(`[tokens-warn] ${rel} 裸字号 ${fonts.length} 处（P2 再收紧）：${fonts.slice(0, 3).join(" / ")}`);
  }
}

if (errors) {
  console.error(`\n散写色值 ${errors} 个文件，请全部改成 var(--sa-*)，定义只许进 tokens-a.css / chart-palette.ts。`);
  process.exit(1);
}
console.log(`色值门禁通过（存量 ${legacy} 个文件冻结待迁）。${warns ? `裸字号告警 ${warns} 个文件，P2 收紧。` : "无裸字号。"}`);
