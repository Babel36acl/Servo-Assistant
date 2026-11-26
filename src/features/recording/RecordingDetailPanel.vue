<script setup lang="ts">
import { hexBytes, timeLabel } from "./format";
import type { ModbusTransaction, RecordItem } from "./types";

/** 报文详情：Modbus 完整事务（请求 / 响应 / 事件）或原始字节 + 结构化解析。
 *  三种状态（加载中 / 事务详情 / 原始内容）互斥，由父视图的 detailBusy 驱动。 */
const props = defineProps<{
  detailBusy: boolean;
  modbusDetail: ModbusTransaction | null;
  selected: RecordItem | null;
  decoded: unknown;
}>();
</script>

<template>
  <section v-if="props.detailBusy" class="ksu-card ksu-span-full">
    <div class="ksu-card-body"><p class="ksu-hint" role="status">正在关联完整事务…</p></div>
  </section>

  <section v-if="props.modbusDetail" class="ksu-card ksu-span-full">
    <div class="ksu-card-head">
      <div>
        <h3 class="ksu-card-title">Modbus 事务 #{{ props.modbusDetail.transaction }} · {{ props.modbusDetail.outcome }}</h3>
        <p class="ksu-card-sub">{{ props.modbusDetail.source }} · {{ props.modbusDetail.protocol }} · 应用观测耗时 {{ props.modbusDetail.durationUs === null ? '未知' : props.modbusDetail.durationUs + ' μs' }}（不是线缆传输时间）</p>
      </div>
    </div>
    <div class="ksu-card-body">
      <div v-if="props.modbusDetail.warning" class="ksu-banner ksu-banner--info"><div class="ksu-banner-body"><span>{{ props.modbusDetail.warning }}</span></div></div>
      <div class="ksu-form-grid">
        <div v-for="(frame, i) in [props.modbusDetail.request, props.modbusDetail.response]" :key="i" class="ksu-field">
          <span class="ksu-field-label">{{ i ? '响应' : '请求' }} · {{ frame.checksum }}</span>
          <p class="ksu-hint">站号 {{ frame.slave ?? '—' }} · 功能码 {{ frame.function === null ? '—' : '0x' + frame.function.toString(16).toUpperCase() }} · 起始地址 {{ frame.address === null ? '—' : '0x' + frame.address.toString(16).toUpperCase() }} · 数量 {{ frame.count ?? '—' }}</p>
          <p v-if="frame.error" class="ksu-hint" role="alert">{{ frame.error }}</p>
          <p v-if="frame.exception" class="ksu-hint" role="alert">{{ frame.exception }}</p>
          <pre class="mono">{{ frame.hex || '未记录字节' }}</pre>
          <p v-if="frame.values.length" class="ksu-hint">原始寄存器值（未按设备 Profile 缩放）</p>
          <pre v-if="frame.values.length" class="mono">{{ frame.values.join(', ') }}</pre>
        </div>
      </div>
      <details class="ksu-field">
        <summary class="ksu-field-label">事务事件（包含部分收发、清空与结束结果）</summary>
        <pre class="mono">{{ props.modbusDetail.events.join('\n') || '无事件' }}</pre>
      </details>
    </div>
  </section>

  <section v-if="props.selected && !props.modbusDetail && !props.detailBusy" class="ksu-card ksu-span-full">
    <div class="ksu-card-head">
      <div>
        <h3 class="ksu-card-title">记录 #{{ props.selected.sequence }} 原始内容</h3>
        <p class="ksu-card-sub">{{ props.selected.source }} · {{ props.selected.protocol }} · {{ timeLabel(props.selected.timestampUs) }}</p>
      </div>
    </div>
    <div class="ksu-card-body">
      <div class="ksu-form-grid">
        <div class="ksu-field">
          <span class="ksu-field-label">原始字节</span>
          <pre class="mono">{{ hexBytes(props.selected.bytes) || '无字节，只有事件' }}</pre>
        </div>
        <div class="ksu-field">
          <span class="ksu-field-label">标准字段解析</span>
          <pre class="mono">{{ props.decoded ? JSON.stringify(props.decoded, null, 2) : '该记录保留原始内容；没有可用的结构化协议解析。' }}</pre>
        </div>
      </div>
    </div>
  </section>
</template>
