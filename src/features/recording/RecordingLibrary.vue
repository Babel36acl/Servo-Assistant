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
watch(() => props.sessions.join('\\n'), refreshCatalog, { immediate: true });
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
  <div class="ksu-field">
    <span class="ksu-field-label">历史录制会话</span>
    <div class="inline-field">
      <select class="ksu-select" aria-label="历史录制会话" :value="fileMode ? '' : path" @change="choose(($event.target as HTMLSelectElement).value,false)">
        <option value="">选择历史会话</option>
        <option v-for="s in catalog" :key="s.path" :value="s.path">{{ s.annotation.name || s.path.split(/[\\\\/]/).pop() }} · {{ (s.bytes / 1048576).toFixed(1) }} MiB{{ s.warning ? ' · 待检查' : '' }}</option>
      </select>
      <button class="ksu-btn ksu-btn--sm" @click="browse()">浏览文件与会话目录</button>
      <button class="ksu-btn ksu-btn--sm" @click="refreshInfo">刷新会话信息</button>
    </div>
  </div>

  <div v-if="error" class="ksu-banner ksu-banner--error" role="alert">
    <div class="ksu-banner-body"><strong>会话读取失败</strong><span>{{ error }}</span></div>
  </div>

  <div class="ksu-divider" aria-hidden="true"></div>

  <div v-if="chooser" class="ksu-list" aria-label="录制文件选择器">
    <div class="ksu-row ksu-row--static">
      <div class="ksu-row-body">
        <span class="ksu-row-title">文件与会话目录</span>
        <span class="ksu-row-sub">{{ directory?.path || '正在读取目录…' }}</span>
      </div>
      <div class="ksu-row-control">
        <button class="ksu-btn ksu-btn--sm" @click="chooser = false">关闭</button>
      </div>
    </div>
    <div class="ksu-row ksu-row--static">
      <div class="ksu-row-body">
        <div class="inline-field">
          <input class="ksu-input" v-model="location" aria-label="浏览目录路径" @keydown.enter="browse(location)" />
          <button class="ksu-btn ksu-btn--sm" :disabled="loading" @click="browse(location)">前往</button>
          <button class="ksu-btn ksu-btn--sm" :disabled="loading || !directory?.parent" @click="browse(directory!.parent!)">上一级</button>
        </div>
        <div class="button-row">
          <button class="ksu-btn ksu-btn--sm ksu-btn--tonal" :disabled="loading || !directory" @click="choose(directory!.path,false)">打开此录制会话</button>
          <button class="ksu-btn ksu-btn--sm" :disabled="loading || !directory" @click="choose(directory!.path,true)">检索此目录全部 PCAP 分卷</button>
        </div>
      </div>
    </div>
    <div v-if="loading" class="ksu-empty">正在读取目录…</div>
    <template v-else>
      <button
        v-for="entry in directory?.entries" :key="entry.path" type="button" class="ksu-row"
        @click="entry.directory ? browse(entry.path) : choose(entry.path,true)"
      >
        <span class="ksu-row-icon" aria-hidden="true">{{ entry.directory ? '▸' : '▤' }}</span>
        <div class="ksu-row-body">
          <span class="ksu-row-title">{{ entry.name }}</span>
          <span class="ksu-row-sub">{{ entry.directory ? '目录' : 'PCAP / 录制文件' }}</span>
        </div>
        <span class="ksu-row-chevron" aria-hidden="true">›</span>
      </button>
    </template>
  </div>

  <div class="ksu-divider" aria-hidden="true"></div>

  <div v-if="info" class="ksu-list">
    <div class="ksu-row ksu-row--static">
      <div class="ksu-row-body">
        <span class="ksu-row-title">会话信息与备注</span>
        <span class="ksu-row-sub">{{ (info.bytes / 1048576).toFixed(2) }} MiB · 用于现场现象与本轮操作条件回溯</span>
      </div>
    </div>
  </div>
  <div v-if="info" class="ksu-card-body">
    <div class="ksu-form-grid">
      <label class="ksu-field">
        <span class="ksu-field-label">会话名称</span>
        <input class="ksu-input" v-model="info.annotation.name" maxlength="100" placeholder="会话名称，如：2 号轴间歇超时" aria-label="会话名称" />
      </label>
      <div class="ksu-field">
        <span class="ksu-field-label">保存</span>
        <button class="ksu-btn ksu-btn--tonal" :disabled="saving" @click="save">{{ saving ? '保存中…' : '保存名称与备注' }}</button>
      </div>
    </div>
    <label class="ksu-field">
      <span class="ksu-field-label">会话备注</span>
      <textarea class="ksu-textarea" v-model="info.annotation.notes" maxlength="4000" rows="3" placeholder="现场现象、操作条件、关键时间点" aria-label="会话备注" />
    </label>
    <div v-if="info.warning" class="ksu-banner ksu-banner--info"><div class="ksu-banner-body"><span>{{ info.warning }}</span></div></div>
    <details class="ksu-field">
      <summary class="ksu-field-label">录制开始时的设备配置与触发设置</summary>
      <pre class="mono">{{ JSON.stringify(info.metadata,null,2) }}</pre>
    </details>
  </div>
</template>
