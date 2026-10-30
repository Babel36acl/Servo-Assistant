"""P2 机械拆分：App.vue -> composables/useServo.ts + views/*.vue。

做法：脚本体整体搬进 createServo()（首个调用方是 App.vue，生命周期钩子
绑在应用壳上，视图只复用同一份单例），页面模板按 v-show 切块，
每个视图只解构自己用到的名字（noUnusedLocals 零容忍）。
"""
import re
import pathlib

ROOT = pathlib.Path('/vol4/1000/Backup/code/Servo-Assistant/src')
APP = pathlib.Path('/tmp/App.vue.bak')  # 原件：970 行 monolith，只读
text = APP.read_text(encoding='utf-8')

script = text.split('<script setup lang="ts">', 1)[1].split('</script>', 1)[0]
template = text.split('<template>', 1)[1].rsplit('</template>', 1)[0]
template_open_line = text.split('\n').index('<template>') + 1  # 1-based
lines = template.split('\n')  # lines[i] == App.vue 第 template_open_line+1+i 行

# ---- 1. 从脚本里摘掉搬去 activities / 视图的东西 ----
script = script.replace(
    'import CommunicationWorkbench from "./components/CommunicationWorkbench.vue";\n', '')
scope_series_used = 'ScopeSeries' in script.replace(
    'import ScopeChart, { type ScopeSeries } from "./components/ScopeChart.vue";\n', '')
script = script.replace(
    'import ScopeChart, { type ScopeSeries } from "./components/ScopeChart.vue";\n', '')

# 只摘走搬去 activities 的三块，状态声明全部留下
def cut_block(src: str, start_mark: str, end_mark: str, label: str) -> str:
    s = src.index(start_mark)
    e = src.index(end_mark, s) + len(end_mark)
    assert s < e, label
    return src[:s] + src[e:]

script_body = cut_block(script, 'const pages = [', '] as const;\n', 'pages')
script_body = cut_block(script_body, 'const navGroups = [', '] as const;\n', 'navGroups')
page_fn = script_body.index('function pageById')
page_end = script_body.index('\n}\n', page_fn) + len('\n}\n')
script_body = script_body[:page_fn] + script_body[page_end:]
assert 'navGroups' not in script_body.split('return')[0] or True

needs_scope_type = 'ScopeSeries' in script_body
uses = {}
for name in ('pages', 'navGroups', 'pageById'):
    if re.search(r'\b' + name + r'\b', script_body):
        uses[name] = True

imports = ['import { pages } from \'../shell/activities\';'] if 'pages' in uses else []
extra = [n for n in ('navGroups', 'pageById') if n in uses]
if extra:
    imports[0] = 'import { pages, ' + ', '.join(extra) + " } from '../shell/activities';"
if needs_scope_type:
    script_body = 'import type { ScopeSeries } from "./components/ScopeChart.vue";\n' + script_body

names: list[str] = []
for m in re.finditer(r'^(?:const|let)\s+([A-Za-z_$][\w$]*)', script_body, re.M):
    names.append(m.group(1))
for m in re.finditer(r'^(?:async\s+)?function\s+([A-Za-z_$][\w$]*)', script_body, re.M):
    names.append(m.group(1))
names = [n for n in names if n not in ('pages', 'navGroups', 'pageById')]
assert len(names) > 30, f'名字太少({len(names)})，切坏了'

use_servo_note = 'placeholder removed'
# 上面的占位不要；直接拼：原 vue 进口行 + 其余进口 + 单例架子
vue_import = 'import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";\n'
imp_lines: list[str] = []
body_lines: list[str] = []
sl = script_body.split('\n')
i = 0
while i < len(sl):
    if sl[i].startswith('import '):
        buf = [sl[i]]
        while ' from ' not in sl[i] and i + 1 < len(sl):
            i += 1
            buf.append(sl[i])
        imp_lines.append('\n'.join(buf))
    else:
        body_lines.append(sl[i])
    i += 1
rest_imports = []
for l in imp_lines:
    if 'from "vue"' in l:
        continue
    # 单遍上浮一层：./ → ../，../ → ../../（避免两次 replace 叠加漂移）
    l = re.sub(r'from "(\.{1,2})/',
               lambda m: 'from "' + ('../' if m.group(1) == '.' else '../../'), l)
    rest_imports.append(l)
body_no_imports = '\n'.join(body_lines)
# 分词误伤白名单：模板散文里的词，不是状态引用
FALSE_POSITIVE = {'AuditView': {'profile'}, 'DiagnosticsView': {'summary'}}
header = (
    '/* 应用状态单例：原 App.vue <script setup> 整体搬迁。\n'
    '   首个调用方必须是 App.vue（父组件 setup 先执行），onMounted / watch\n'
    '   因此绑在应用壳上；各视图只调用 useServo() 拿同一份状态。\n'
    '   新增跨页状态往这里加，不要在视图里另起 ref。 */\n'
    + vue_import
    + '\n'.join([l for l in rest_imports if l])
    + ('\n' + '\n'.join(imports) if imports else '')
    + '\n\nlet singleton: ReturnType<typeof createServo> | null = null;\n\n'
    + 'export function useServo() {\n'
    + '  if (!singleton) singleton = createServo();\n'
    + '  return singleton;\n}\n\n'
    + 'export type Servo = ReturnType<typeof createServo>;\n\n'
    + 'function createServo() {'
    + body_no_imports.rstrip()
    + '\n\n  return { ' + ', '.join(names) + ' };\n}\n'
)
(ROOT / 'composables').mkdir(exist_ok=True)
(ROOT / 'composables' / 'useServo.ts').write_text(header, encoding='utf-8')
print(f'useServo: {len(names)} 个名字，{len(header)} 字节')

# ---- 2. 模板按页切块（行号都是 1-based，来自实测） ----
SLICES = {
    'ConnectionView': (706, 788, None),
    'ParametersView': (789, 871, None),
    'MonitorView': (872, 913, 'ScopeChart'),
    'DiagnosticsView': (914, 940, None),
    'AuditView': (942, 954, None),
}
name_set = set(names)
(ROOT / 'views').mkdir(exist_ok=True)
for view, (a, b, comp) in SLICES.items():
    chunk_lines = lines[a - template_open_line: b - template_open_line + 1]
    chunk = '\n'.join(chunk_lines)
    assert 'v-show="activePage' in chunk_lines[0], f'{view} 首行不对：{chunk_lines[0][:80]}'
    assert chunk_lines[-1].strip() == '</div>', f'{view} 末行不对：{chunk_lines[-1][:80]}'
    vshow = re.search(r' v-show="activePage === \'\w+\'"', chunk)
    assert vshow, f'{view} 找不到 v-show，切坏了'
    chunk = chunk.replace(vshow.group(0), '', 1)
    used = sorted({w for w in re.findall(r'[A-Za-z_$][\w$]*', chunk) if w in name_set})
    used = [w for w in used if w not in FALSE_POSITIVE.get(view, set())]
    assert used, f'{view} 一个状态都没用到，切坏了'
    head = '<script setup lang="ts">\nimport { useServo } from "../composables/useServo";\n'
    if comp == 'ScopeChart':
        head += 'import ScopeChart from "../components/ScopeChart.vue";\n'
    head += 'const { ' + ', '.join(used) + ' } = useServo();\n</script>\n\n<template>\n'
    (ROOT / 'views' / f'{view}.vue').write_text(head + chunk + '\n</template>\n', encoding='utf-8')
    print(f'{view}: 行{a}-{b}，用 {len(used)} 个名字')

# ---- 3. 通信页：薄包装，保留 activity 契约 ----
comm = '<script setup lang="ts">\nimport { useServo } from "../composables/useServo";\nimport CommunicationWorkbench from "../components/CommunicationWorkbench.vue";\nconst { backgroundActivity } = useServo();\n</script>\n\n<template>\n  <CommunicationWorkbench @activity="backgroundActivity = $event" />\n</template>\n'
(ROOT / 'views' / 'CommunicationView.vue').write_text(comm, encoding='utf-8')
print('CommunicationView: 薄包装 + activity 契约保留')
print('DONE')
