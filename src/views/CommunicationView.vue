<script setup lang="ts">
import { ref } from 'vue';
import { useServo } from '../composables/useServo';
import RecordingSessionView from '../features/recording/RecordingSessionView.vue';
import EthercatMasterView from '../features/ethercat/EthercatMasterView.vue';

type MailboxMap = { mailboxes: Array<{ station: number; offset: number }> };

const { backgroundActivity } = useServo();
const tab = ref<'recording' | 'ethercat'>('recording');
const recRef = ref<{ applyMailboxes: (value: MailboxMap) => void } | null>(null);
const recActivity = ref('');
const ethActivity = ref('');

function join(): void {
  backgroundActivity.value = [recActivity.value, ethActivity.value].filter(Boolean).join(' ｜ ');
}

function onRecActivity(label: string): void {
  recActivity.value = label;
  join();
}

function onEthActivity(label: string): void {
  ethActivity.value = label;
  join();
}

function forwardMapping(value: MailboxMap): void {
  recRef.value?.applyMailboxes(value);
}
</script>

<template>
  <div class="comm-tabs" role="tablist" aria-label="通信工作台">
    <button
      type="button" role="tab" :aria-selected="tab === 'recording'"
      :class="{ active: tab === 'recording' }" @click="tab = 'recording'"
    >录制与解析</button>
    <button
      type="button" role="tab" :aria-selected="tab === 'ethercat'"
      :class="{ active: tab === 'ethercat' }" @click="tab = 'ethercat'"
    >EtherCAT 主站</button>
  </div>
  <RecordingSessionView v-show="tab === 'recording'" ref="recRef" @activity="onRecActivity" />
  <EthercatMasterView v-show="tab === 'ethercat'" @activity="onEthActivity" @mapping="forwardMapping" />
</template>

<style scoped>
.comm-tabs {
  display: flex;
  gap: var(--sa-space-2);
  margin-bottom: var(--sa-space-3);
}
.comm-tabs button {
  min-height: var(--sa-touch-min);
  padding: 0 var(--sa-space-4);
  border: 1px solid var(--sa-line);
  border-radius: var(--sa-radius-pill);
  background: transparent;
  color: var(--sa-muted);
  font-size: var(--sa-text-13);
}
.comm-tabs button.active {
  color: var(--sa-accent-ink);
  background: var(--sa-accent);
  border-color: transparent;
}
</style>
