<script setup lang="ts">
/** 筛选与检索卡：查询 / 方向 / 时间 / 来源 / 协议 / 站号 / 功能码 / 地址 / 结果。
 *  只做表单，不持有筛选状态——全部 v-model 回父视图，分页语义也在父视图。 */
const query = defineModel<string>("query", { required: true });
const direction = defineModel<string>("direction", { required: true });
const fromTime = defineModel<string>("fromTime", { required: true });
const toTime = defineModel<string>("toTime", { required: true });
const sourceFilter = defineModel<string>("sourceFilter", { required: true });
const protocolFilter = defineModel<string>("protocolFilter", { required: true });
const slaveFilter = defineModel<string>("slaveFilter", { required: true });
const functionFilter = defineModel<string>("functionFilter", { required: true });
const addressFilter = defineModel<string>("addressFilter", { required: true });
const outcomeFilter = defineModel<string>("outcomeFilter", { required: true });

const props = defineProps<{
  fileMode: boolean;
  busy: boolean;
  pageBusy: boolean;
  follow: boolean;
  offset: number;
  rowCount: number;
  total: number | null;
  nextOffset: number | null;
}>();

const emit = defineEmits<{ clear: []; page: [offset: number] }>();
</script>

<template>
  <section class="ksu-card ksu-span-2">
    <div class="ksu-card-head">
      <div>
        <h2 class="ksu-card-title">筛选与检索</h2>
        <p class="ksu-card-sub">全部筛选作用于整个会话；站号、地址和结果关联完整事务</p>
      </div>
      <div class="ksu-card-actions">
        <button class="ksu-btn ksu-btn--sm" @click="emit('clear')">清除筛选</button>
      </div>
    </div>
    <div class="ksu-card-body">
      <div class="ksu-form-grid">
        <label class="ksu-field">
          <span class="ksu-field-label">报文筛选</span>
          <input class="ksu-input" v-model="query" :placeholder="fileMode ? '全部 PCAP 分卷搜索：来源、协议、事件、字节' : '全会话搜索：来源、协议、事务、事件、十六进制字节'" aria-label="报文筛选" />
        </label>
        <label class="ksu-field">
          <span class="ksu-field-label">方向</span>
          <select class="ksu-select" v-model="direction" aria-label="方向筛选">
            <option value="">所有方向</option>
            <option>tx</option>
            <option>rx</option>
            <option>event</option>
            <option>unknown</option>
          </select>
        </label>
      </div>
      <div class="ksu-actions">
        <button class="ksu-btn ksu-btn--sm" :disabled="props.busy || props.pageBusy || props.offset === 0" @click="emit('page', Math.max(0, props.offset - 100))">上一页</button>
        <button class="ksu-btn ksu-btn--sm" :disabled="props.busy || props.pageBusy || props.nextOffset === null" @click="emit('page', props.nextOffset ?? 0)">下一页</button>
        <span class="ksu-hint">{{ props.total === null ? '本页 ' + props.rowCount + ' 条' : '共 ' + props.total + ' 条匹配 · ' + (props.rowCount ? props.offset + 1 : 0) + '–' + (props.offset + props.rowCount) }}{{ props.follow ? ' · 自动跟随中' : '' }}</span>
      </div>

      <details class="ksu-field" open>
        <summary class="ksu-field-label">时间与协议精确筛选</summary>
        <div class="ksu-form-grid">
          <label class="ksu-field">
            <span class="ksu-field-label">开始</span>
            <input class="ksu-input" v-model="fromTime" type="datetime-local" step="0.001" />
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">结束</span>
            <input class="ksu-input" v-model="toTime" type="datetime-local" step="0.001" />
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">来源精确匹配</span>
            <input class="ksu-input" v-model="sourceFilter" placeholder="来源精确匹配（可从报文复制）" aria-label="来源筛选" />
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">协议</span>
            <input class="ksu-input" v-model="protocolFilter" placeholder="协议，如 modbus-rtu / ethernet" aria-label="协议筛选" />
          </label>
        </div>
        <div v-if="!fileMode" class="ksu-form-grid">
          <label class="ksu-field">
            <span class="ksu-field-label">站号</span>
            <input class="ksu-input" v-model="slaveFilter" placeholder="站号" aria-label="站号筛选" />
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">功能码</span>
            <input class="ksu-input" v-model="functionFilter" placeholder="功能码，如 3 / 0x06" aria-label="功能码筛选" />
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">寄存器地址</span>
            <input class="ksu-input" v-model="addressFilter" placeholder="包含寄存器地址，如 0x100" aria-label="地址筛选" />
          </label>
          <label class="ksu-field">
            <span class="ksu-field-label">事务结果</span>
            <select class="ksu-select" v-model="outcomeFilter" aria-label="事务结果筛选">
              <option value="">所有事务结果</option>
              <option value="success">成功</option>
              <option value="failure">失败 / 设备异常</option>
              <option value="incomplete">缺少事务结束</option>
            </select>
          </label>
        </div>
      </details>
      <p class="ksu-hint">{{ props.fileMode ? '搜索与时间筛选覆盖所选文件或目录中的全部 PCAP 分卷。首次建立缓存可能较慢；源文件变化后重建，请优先打开已停止的捕获。' : '全部筛选作用于整个会话；站号、地址和结果关联完整事务。跟随显示最新 100 条已落盘匹配记录，查看详情时录制继续。' }}</p>
    </div>
  </section>
</template>
