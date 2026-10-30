"""P3：CommunicationWorkbench -> features/recording/RecordingSessionView.vue。

只做减法：摘掉 EtherCAT 主站 Tab（搬去 features/ethercat，归 CommunicationView 编排），
录制 + 浏览原样保留；样式里的散写色值/字号同步换成 var(--sa-*)。
"""
import pathlib
import re

ROOT = pathlib.Path('/vol4/1000/Backup/code/Servo-Assistant/src')
CW = ROOT / 'components/CommunicationWorkbench.vue'
OUT = ROOT / 'features/recording/RecordingSessionView.vue'
text = CW.read_text(encoding='utf-8')

script = text.split('<script setup lang="ts">', 1)[1].split('</script>', 1)[0]
template = text.split('<template>', 1)[1].rsplit('</template>', 1)[0]
style = text.split('<style scoped>', 1)[1].rsplit('</style>', 1)[0]

# ---- 脚本：删主站接线 ----
script = re.sub(r"^import EthercatMaster from '\./EthercatMaster\.vue';\r?\n", '', script, flags=re.M)
assert 'EthercatMaster' not in script, '主站 import 没摘干净'
script = script.replace("const tab = ref('recording'), error = ", 'const error = ')
assert "ref('recording')" not in script, 'tab 没摘干净'
m = re.search(r'^const masterConnected.*$', script, flags=re.M)
assert m, '找不到 masterConnected 声明'
script = script.replace(m.group(0) + '\n', '')
old_watch = "watch(() => [status.value?.active, status.value?.error, capture.value?.active, capture.value?.error, masterConnected.value], () => emit('activity', [status.value?.error ? '录制异常：' + status.value.error : status.value?.active ? '通信录制中' : '', capture.value?.error ? '捕获异常：' + capture.value.error : capture.value?.active ? '网卡捕获中' : '', masterConnected.value ? 'EtherCAT 已连接' : ''].filter(Boolean).join(' · ')), { immediate: true });"
assert old_watch in script, 'activity watcher 对不上原件'
new_watch = "watch(() => [status.value?.active, status.value?.error, capture.value?.active, capture.value?.error], () => emit('activity', [status.value?.error ? '录制异常：' + status.value.error : status.value?.active ? '通信录制中' : '', capture.value?.error ? '捕获异常：' + capture.value.error : capture.value?.active ? '网卡捕获中' : ''].filter(Boolean).join(' · ')), { immediate: true });"
script = script.replace(old_watch, new_watch)
old_mb = "function mailboxes(value: { mailboxes: Array<{ station: number; offset: number }> }) { try { const config = JSON.parse(mapping.value); config.mailboxes = value.mailboxes; mapping.value = JSON.stringify(config, null, 2); notice.value = '已使用当前主站发现的邮箱地址，PDO 映射保持不变。'; } catch (e) { error.value = String(e); } }"
assert old_mb in script, 'mailboxes 对不上原件'
script = script.replace(old_mb + '\n', '')
script = script.replace(
    'async function tick()',
    'function applyMailboxes(value: { mailboxes: Array<{ station: number; offset: number }> }) { try { const config = JSON.parse(mapping.value); config.mailboxes = value.mailboxes; mapping.value = JSON.stringify(config, null, 2); notice.value = \'已使用当前主站发现的邮箱地址，PDO 映射保持不变。\'; } catch (e) { error.value = String(e); } }\ndefineExpose({ applyMailboxes });\nasync function tick()',
)
assert 'masterConnected' not in script and '@mapping' not in script
assert 'mailbox' in script.lower(), 'mailbox 逻辑不应全删'

# ---- 模板：删 tabs + 主站区，录制区去 v-show ----
old_tabs = '<div class="controls workbench-tabs" aria-label="通信工作台视图"><h2>通信工具</h2><button :aria-pressed="tab === \'recording\'" :class="{ active: tab === \'recording\' }" @click="tab = \'recording\'">录制与解析</button><button :aria-pressed="tab === \'master\'" :class="{ active: tab === \'master\' }" @click="tab = \'master\'">EtherCAT 主站</button></div>'
assert old_tabs in template, 'tabs 对不上原件'
template = template.replace(old_tabs + '\n', '')
old_master = '<EthercatMaster v-show="tab === \'master\'" @mapping="mailboxes" @connection="masterConnected = $event" />'
assert old_master in template, '主站区对不上原件'
template = template.replace('    ' + old_master + '\n', '')
template = template.replace(
    '<div v-show="tab === \'recording\'" class="recording-grid">',
    '<div class="recording-grid">',
)
assert "tab ===" not in template, '模板里还有 tab'

# ---- 样式：散写 -> tokens ----
old_active = '.active{border-color:var(--info);color:var(--info)}'
assert old_active in style
style = style.replace(old_active, '')
style = style.replace('background:#101015', 'background:var(--sa-bg-sunken)')
style = style.replace('tbody tr:hover,.chosen{background:rgba(235,169,55,.14)}',
                      'tbody tr:hover,.chosen{background:var(--sa-accent-soft)}')
style = style.replace('h3{font-size:14px', 'h3{font-size:var(--sa-text-14)')
style = style.replace('p{font-size:12px', 'p{font-size:var(--sa-text-12)')
style = style.replace('padding:8px;border-bottom:1px solid var(--line);font-size:12px',
                      'padding:8px;border-bottom:1px solid var(--line);font-size:var(--sa-text-12)')
style = style.replace('overflow:auto;background:var(--sa-bg-sunken);padding:12px;font-size:12px}',
                      'overflow:auto;background:var(--sa-bg-sunken);padding:12px;font-size:var(--sa-text-12)}')
assert not re.search(r'#[0-9a-fA-F]{3,8}\b', style), '样式里还有 hex'
assert 'font-size:12px' not in style and 'font-size:14px' not in style, '还有裸字号'

OUT.parent.mkdir(parents=True, exist_ok=True)
OUT.write_text('<script setup lang="ts">\n' + script + '</script>\n\n<template>' + template + '</template>\n\n<style scoped>\n' + style + '</style>\n', encoding='utf-8')
print(f'RecordingSessionView: {len(script)} script / {len(template)} template / {len(style)} style')
print('DONE')
