<script setup lang="ts">
import { ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
const props = defineProps<{ path: string; fileMode: boolean; sessions: string[] }>();
const emit = defineEmits<{ open: [path: string, fileMode: boolean] }>();
type Info = { path: string; annotation: { name: string; notes: string }; bytes: number; metadata: unknown; warning: string | null };
type Directory = { path: string; parent: string | null; entries: { path: string; name: string; directory: boolean }[] };
const catalog = ref<Info[]>([]), info = ref<Info | null>(null), error = ref(''), saving = ref(false);
const chooser = ref(false), directory = ref<Directory | null>(null), location = ref(''), loading = ref(false);
let version = 0;
async function refreshCatalog() {
  try { catalog.value = await Promise.all(props.sessions.map(path => invoke<Info>('recording_info', { path }).catch(e => ({ path, annotation: { name: '', notes: '' }, bytes: 0, metadata: null, warning: String(e) })))); }
  catch(e) { error.value = String(e); }
}
watch(() => props.sessions.join('\n'), refreshCatalog, { immediate: true });
async function refreshInfo() {
  await refreshCatalog();
  if(info.value) { const latest=catalog.value.find(s=>s.path===info.value?.path); if(latest) info.value={...info.value,bytes:latest.bytes,warning:latest.warning}; }
}
watch(() => [props.path, props.fileMode], async () => {
  const token = ++version; info.value = null; error.value = '';
  if (!props.path || props.fileMode) return;
  try { const value = await invoke<Info>('recording_info', { path: props.path }); if (token === version) info.value = value; }
  catch(e) { if (token === version) error.value = String(e); }
}, { immediate: true });
async function browse(path?: string) {
  loading.value = true; error.value = ''; chooser.value = true;
  try { directory.value = await invoke<Directory>('recording_browse', { path: path || null }); location.value = directory.value.path; }
  catch(e) { error.value = String(e); }
  finally { loading.value = false; }
}
function choose(path: string, fileMode: boolean) { chooser.value = false; emit('open',path,fileMode); }
async function save() {
  if (!info.value || saving.value) return; saving.value = true; error.value = '';
  const value = info.value;
  try { await invoke('recording_annotate', { path: value.path, annotation: value.annotation }); const item = catalog.value.find(i => i.path === value.path); if(item) item.annotation = { ...value.annotation }; }
  catch(e) { error.value = String(e); } finally { saving.value = false; }
}
</script>
<template>
  <div class="library">
    <div class="controls"><select aria-label="历史录制会话" :value="fileMode ? '' : path" @change="choose(($event.target as HTMLSelectElement).value,false)"><option value="">选择历史会话</option><option v-for="s in catalog" :key="s.path" :value="s.path">{{ s.annotation.name || s.path.split(/[\\/]/).pop() }} · {{ (s.bytes / 1048576).toFixed(1) }} MiB{{ s.warning ? ' · 待检查' : '' }}</option></select><button @click="browse()">浏览文件与会话目录</button><button @click="refreshInfo">刷新会话信息</button></div>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <section v-if="chooser" class="file-browser" aria-label="录制文件选择器">
      <div class="controls"><input v-model="location" aria-label="浏览目录路径" @keydown.enter="browse(location)" /><button :disabled="loading" @click="browse(location)">前往</button><button :disabled="loading || !directory?.parent" @click="browse(directory!.parent!)">上一级</button><button @click="chooser = false">关闭</button></div>
      <div class="controls"><button :disabled="loading || !directory" @click="choose(directory!.path,false)">打开此录制会话</button><button :disabled="loading || !directory" @click="choose(directory!.path,true)">检索此目录全部 PCAP 分卷</button></div>
      <p v-if="loading">正在读取目录…</p><ul v-else><li v-for="entry in directory?.entries" :key="entry.path"><button @click="entry.directory ? browse(entry.path) : choose(entry.path,true)">{{ entry.directory ? '▸' : '▤' }} {{ entry.name }}</button></li></ul>
    </section>
    <details v-if="info" class="session-info"><summary>会话信息与备注 · {{ (info.bytes / 1048576).toFixed(2) }} MiB</summary>
      <div class="controls"><input v-model="info.annotation.name" maxlength="100" placeholder="会话名称，如：2 号轴间歇超时" aria-label="会话名称" /><button :disabled="saving" @click="save">{{ saving ? '保存中…' : '保存名称与备注' }}</button></div>
      <textarea v-model="info.annotation.notes" maxlength="4000" rows="3" placeholder="现场现象、操作条件、关键时间点" aria-label="会话备注" />
      <p v-if="info.warning" class="warning">{{ info.warning }}</p>
      <details><summary>录制开始时的设备配置与触发设置</summary><pre>{{ JSON.stringify(info.metadata,null,2) }}</pre></details>
    </details>
  </div>
</template>
<style scoped>
.library,.file-browser{display:grid;gap:10px;min-width:0}.controls{display:flex;gap:8px;flex-wrap:wrap}.controls>input,.controls>select{flex:1 1 250px;min-width:0}.file-browser,.session-info{border:1px solid var(--line);padding:12px}.file-browser ul{list-style:none;padding:0;max-height:300px;overflow:auto}.file-browser li>button{width:100%;text-align:left;overflow-wrap:anywhere}textarea{width:100%;margin:10px 0}pre{white-space:pre-wrap;overflow-wrap:anywhere;max-height:300px;overflow:auto}.error{color:var(--red)}.warning{color:var(--amber)}summary{cursor:pointer;margin-bottom:10px}
</style>
