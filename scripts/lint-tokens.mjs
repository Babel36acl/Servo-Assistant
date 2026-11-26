// lint:tokens — 设计令牌门禁（P0，零告警）。
// 规则（全部为错误，不再有冻结豁免）：
//   1. 除 src/styles/tokens-a.css 与 src/styles/chart-palette.ts 外，
//      *.css / *.vue <style> / *.ts 内禁止 hex（#abc / #aabbcc）与 rgb()/rgba()/hsl()/hsla()。
//   2. 字号禁止裸写（px / rem / em / pt / % / vw / vh），只能 var(--ksu-fs-*)。
//   3. 禁止引用已退役的旧变量名（--sa-*、bridge-a 时代的 --bg/--panel/--ink/...）。
// 旧 styles.css 与 bridge-a.css 已删除，所以违规一律阻断提交，不再区分存量。
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, extname, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', 'src');
// 唯一允许写色值的两个文件：CSS 令牌源 + 图表调色板（JS 色源）。
const ALLOW = new Set(['src/styles/tokens-a.css', 'src/styles/chart-palette.ts']);
const HEX = /#[0-9a-fA-F]{3,8}\b/g;
const FUNC = /\b(?:rgb|rgba|hsl|hsla)\s*\(/g;
// font-size 后面必须是 var(--ksu-fs-*)，或显式继承语义。
const FONT_SIZE = /font-size\s*:\s*([^;}]+)/g;
const FONT_VALUE_OK = /^(?:var\(|inherit\b|initial\b|unset\b|revert\b|0\s*$)/;
// 已退役的旧变量：--sa-* 整族 + bridge-a 映射的短名。
const LEGACY_VAR = new RegExp(
  'var\\(\\s*--(?:sa-[a-z0-9-]+|bg|panel|panel-2|glass|glass-strong|glass-wash|line|line-soft|rim|ink|muted|faint|accent|accent-deep|accent-ink|info|green|warning|red|grad-brand|font-sans|font-mono|r-sm|r-md|r-lg|r-xl|r-pill|shadow-soft|shadow-float|ease-out|ease-in-out|fast|base|panel-t)\\b',
  'g',
);

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) walk(p, out);
    else if (['.css', '.vue', '.ts'].includes(extname(p))) out.push(p);
  }
  return out;
}

function styleOfVue(text) {
  const blocks = [...text.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1]);
  return blocks.join('\n');
}

const files = walk(ROOT);
let errors = 0;

for (const file of files) {
  const rel = ('src' + file.slice(ROOT.length)).replace(/\\/g, '/');
  if (ALLOW.has(rel)) continue;
  const raw = readFileSync(file, 'utf8');
  const isVue = file.endsWith('.vue');
  const isTs = file.endsWith('.ts');
  const text = isVue ? styleOfVue(raw) : raw;
  if (!text.trim()) continue;

  const detail = [];
  const hexes = [...new Set([...text.matchAll(HEX)].map((m) => m[0]))];
  const funcs = [...new Set([...text.matchAll(FUNC)].map((m) => m[0].replace(/\s*\($/, '(')))];
  if (hexes.length || funcs.length) {
    detail.push(`散写色值：${[...hexes, ...funcs].slice(0, 5).join(' ')}`);
  }

  if (!isTs) {
    const badFonts = [];
    for (const match of text.matchAll(FONT_SIZE)) {
      const value = match[1].trim();
      if (!FONT_VALUE_OK.test(value)) badFonts.push(`font-size: ${value.slice(0, 24)}`);
    }
    if (badFonts.length) {
      detail.push(`裸字号 ${badFonts.length} 处（改 var(--ksu-fs-*)）：${badFonts.slice(0, 3).join(' / ')}`);
    }
  }

  const legacy = [...new Set([...text.matchAll(LEGACY_VAR)].map((m) => m[0]))];
  if (legacy.length) {
    detail.push(`已退役变量 ${legacy.length} 处：${legacy.slice(0, 3).join(' ')}`);
  }

  if (detail.length) {
    errors++;
    console.error(`[tokens] ${rel}\n  - ${detail.join('\n  - ')}`);
  }
}

if (errors) {
  console.error(`\n${errors} 个文件违反设计令牌门禁。色值只能进 src/styles/tokens-a.css 与 src/styles/chart-palette.ts，`);
  console.error('字号只能 var(--ksu-fs-*)，旧 --sa-* / bridge-a 变量名已退役。');
  process.exit(1);
}
console.log(`令牌门禁通过：${files.length} 个样式/脚本文件，无散写色值、无裸字号、无退役变量引用。`);
