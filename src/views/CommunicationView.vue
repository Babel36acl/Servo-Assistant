<script setup lang="ts">
import { ref } from 'vue';
import { useServo } from '../composables/useServo';
import RecordingSessionView from '../features/recording/RecordingSessionView.vue';
import EthercatMasterView from '../features/ethercat/EthercatMasterView.vue';

type MailboxMap = { mailboxes: Array<{ station: number; offset: number }> };

const { backgroundActivity, communicationTab: tab } = useServo();
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
  <div class="ksu-page">
    <header class="ksu-page-head">
      <div>
        <h1 class="ksu-page-title">通信工作台</h1>
        <p class="ksu-page-sub">统一录制 · 事务解析 · 主站配置</p>
      </div>
    </header>

    <div class="ksu-page-body">
      <nav class="ksu-tabs" role="tablist" aria-label="通信工作台">
        <button
          type="button" role="tab" class="ksu-tab" :class="{ active: tab === 'recording' }"
          :aria-selected="tab === 'recording'" @click="tab = 'recording'"
        >录制与解析</button>
        <button
          type="button" role="tab" class="ksu-tab" :class="{ active: tab === 'ethercat' }"
          :aria-selected="tab === 'ethercat'" @click="tab = 'ethercat'"
        >EtherCAT 主站</button>
      </nav>
      <RecordingSessionView v-show="tab === 'recording'" ref="recRef" @activity="onRecActivity" />
      <EthercatMasterView v-show="tab === 'ethercat'" @activity="onEthActivity" @mapping="forwardMapping" />
    </div>
    <div class="ksu-page-bottom" aria-hidden="true"></div>
  </div>
</template>
