<script setup lang="ts">
import { computed, ref, watch, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { HISTORY_COLORS as colors } from '../../styles/chart-palette';
type Channel = { source: string; protocol: string; slave: number; firstAddress: number; lastAddress: number };
type Overview = { firstUs: number | null; lastUs: number | null; count: number; channels: Channel[]; warning: string | null };
type Point = { sequence: number; timestampUs: number; source: string; protocol: string; transaction: number; raw: number; value: number; name: string; unit: string };
type History = { points: Point[]; total: number; offset: number; warning: string | null };
const props = defineProps<{ path: string; fileMode: boolean; fromUs: number | null; toUs: number | null }>();
const emit = defineEmits<{ select: [point: Point] }>();
const overview = ref<Overview | null>(null), history = ref<History | null>(null), channel = ref(0), address = ref('0'), error = ref(''), busy = ref(false), cursor = ref(0), playing = ref(false);
let version = 0, timer: ReturnType<typeof setTimeout> | undefined;
function pause() { playing.value = false; clearTimeout(timer); }
function reset() { version++; busy.value = false; history.value = null; cursor.value = 0; pause(); }
watch(() => [props.path,props.fileMode], () => { reset(); overview.value = null; error.value = ''; });
watch(() => [props.fromUs,props.toUs,channel.value,address.value], reset);
onUnmounted(() => { version++; pause(); });
async function inspect() {
  reset(); const token = ++version; busy.value = true; error.value = '';
  try { const value = await invoke<Overview>('recording_overview',{ path: props.path }); if(token !== version) return; overview.value = value; channel.value = 0; address.value = String(value.channels[0]?.firstAddress ?? 0); }
  catch(e) { if(token === version) error.value = String(e); } finally { if(token === version) busy.value = false; }
}
async function load(offset = 0) {
  pause(); const selected = overview.value?.channels[channel.value]; if(!selected) return;
  const numeric = Number(address.value);
  if(!address.value.trim() || !Number.isInteger(numeric) || numeric<0 || numeric>65535) { error.value = '地址须为 0～65535，支持 0x 十六进制'; return; }
  const token = ++version; busy.value = true; error.value = '';
  try { const value = await invoke<History>('recording_history',{ path:props.path, offset, filter:{ source:selected.source,protocol:selected.protocol,slave:selected.slave,address:numeric,fromUs:props.fromUs,toUs:props.toUs } }); if(token === version) { history.value = value; cursor.value = 0; } }
  catch(e) { if(token === version) error.value = String(e); } finally { if(token === version) busy.value = false; }
}
const graph = computed(() => {
  const points = history.value?.points ?? []; if(!points.length) return [];
  const first = points[0].timestampUs, last = points[points.length-1].timestampUs;
  const groups = new Map<string,Point[]>();
  for(const p of points) { const key = p.name+' / '+p.unit; const group = groups.get(key); if(group) group.push(p); else groups.set(key,[p]); }
  return [...groups].map(([name,values],i) => {
    const min = Math.min(...values.map(p=>p.value)), max = Math.max(...values.map(p=>p.value));
    const coordinates = values.map(p=>({ p, x:30+(p.timestampUs-first)/Math.max(1,last-first)*940, y:190-(p.value-min)/Math.max(1e-9,max-min)*160 }));
    return { name,min,max,color:colors[i%colors.length],coordinates,points:coordinates.map(p=>`${p.x},${p.y}`).join(' ') };
  });
});
function select(index: number) { cursor.value = index; const p = history.value?.points[index]; if(p) emit('select',p); }
function play() { if(playing.value) { pause(); return; } playing.value = true; advance(); }
function advance() { if(!playing.value) return; select(cursor.value); if(cursor.value >= (history.value?.points.length ?? 0)-1) { pause(); return; } timer = setTimeout(()=>{ cursor.value++; advance(); },500); }
</script>
<template>
  <section class="ksu-card ksu-span-full">
    <div class="ksu-card-head">
      <div>
        <h2 class="ksu-card-title">时间概览与历史曲线</h2>
        <p class="ksu-card-sub">使用上方时间范围，按录制当时的 Profile 缩放；连线只连接已观测样本，不补齐丢失数据</p>
      </div>
      <div class="ksu-card-actions">
        <button class="ksu-btn ksu-btn--sm" :disabled="!path || busy" @click="inspect">{{ busy ? '读取中…' : '读取时间与历史通道' }}</button>
      </div>
    </div>
    <div class="ksu-card-body">
      <div v-if="overview" class="ksu-list">
        <div class="ksu-row ksu-row--static">
          <div class="ksu-row-body">
            <span class="ksu-row-title">{{ overview.count }} 条记录</span>
            <span class="ksu-row-sub">{{ overview.firstUs ? new Date(overview.firstUs/1000).toLocaleString() : '时间未知' }} → {{ overview.lastUs ? new Date(overview.lastUs/1000).toLocaleString() : '时间未知' }}</span>
          </div>
          <div class="ksu-row-control">
            <span class="ksu-pill ksu-pill--muted">{{ overview.firstUs && overview.lastUs ? ((overview.lastUs-overview.firstUs)/1000000).toFixed(3) + ' 秒' : '时长未知' }}</span>
          </div>
        </div>
      </div>
      <div v-if="overview?.warning" class="ksu-banner ksu-banner--info"><div class="ksu-banner-body"><span>{{ overview.warning }}</span></div></div>
      <div v-if="error" class="ksu-banner ksu-banner--error" role="alert"><div class="ksu-banner-body"><span>{{ error }}</span></div></div>

      <template v-if="!fileMode && overview?.channels.length">
        <div class="ksu-form-grid">
          <label class="ksu-field">
            <span class="ksu-field-label">历史曲线来源</span>
            <select class="ksu-select" v-model.number="channel" aria-label="历史曲线来源">
              <option v-for="(c,i) in overview.channels" :key="i" :value="i">{{ c.source }} · {{ c.protocol }} · 站 {{ c.slave }}</option>
            </select>
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">寄存器地址</span>
            <input class="ksu-input" v-model="address" aria-label="历史寄存器地址" placeholder="十进制或 0x" />
          </label>
          <div class="ksu-field">
            <span class="ksu-field-label">加载</span>
            <button class="ksu-btn ksu-btn--tonal" :disabled="busy" @click="load(0)">加载曲线</button>
          </div>
        </div>

        <template v-if="history">
          <div class="button-row">
            <button class="ksu-btn ksu-btn--sm" :disabled="busy || !history.offset" @click="load(Math.max(0,history.offset-2000))">更早</button>
            <button class="ksu-btn ksu-btn--sm" :disabled="busy || history.offset+history.points.length>=history.total" @click="load(history.offset+2000)">更晚</button>
            <span class="ksu-hint">{{ history.total }} 点 · {{ history.offset+1 }}–{{ history.offset+history.points.length }}</span>
          </div>
          <div v-if="history.warning" class="ksu-banner ksu-banner--info"><div class="ksu-banner-body"><span>{{ history.warning }}</span></div></div>
          <div v-if="history.points.length" class="ksu-chart">
            <svg viewBox="0 0 1000 230" aria-label="历史寄存器曲线">
              <g v-for="g in graph" :key="g.name">
                <polyline :points="g.points" :stroke="g.color" fill="none" />
                <circle
                  v-for="c in g.coordinates" :key="c.p.sequence" :cx="c.x" :cy="c.y" r="4" :fill="g.color" tabindex="0"
                  @click="pause(); select(history!.points.indexOf(c.p))" @keydown.enter="pause(); select(history!.points.indexOf(c.p))"
                ><title>{{ new Date(c.p.timestampUs/1000).toLocaleString() }} · {{ c.p.name }} {{ c.p.value }} {{ c.p.unit }} · 事务 {{ c.p.transaction }}</title></circle>
              </g>
            </svg>
          </div>
          <p v-for="g in graph" :key="g.name" class="ksu-hint" :style="{ color:g.color }">{{ g.name }} · {{ g.min }}…{{ g.max }}（各定义独立缩放）</p>
          <div v-if="history.points.length" class="button-row">
            <button class="ksu-btn ksu-btn--sm" @click="play">{{ playing ? '暂停浏览' : '逐点浏览（0.5 秒/点）' }}</button>
            <input v-model.number="cursor" type="range" class="ksu-slider" min="0" :max="history.points.length-1" aria-label="历史采样点" @input="pause(); select(cursor)" />
            <button class="ksu-btn ksu-btn--sm" @click="select(cursor)">定位对应事务</button>
            <span class="ksu-hint">{{ history.points[cursor]?.value }} {{ history.points[cursor]?.unit }}</span>
          </div>
          <p v-else class="ksu-empty">所选时间、来源和地址没有可重建的采样点。</p>
        </template>
      </template>
      <p v-else-if="overview && !fileMode" class="ksu-empty">没有完整的 Modbus FC03 事务或模拟器状态采样。原始报文仍可浏览。</p>
      <p v-if="fileMode" class="ksu-hint">PCAP 多卷共用时间范围和关键词检索；没有已绑定的过程数据定义时，不推断历史曲线。</p>
    </div>
  </section>
</template>
