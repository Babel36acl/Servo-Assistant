<script setup lang="ts">
import { computed } from 'vue';
import { useServo } from '../composables/useServo';
import type { PageId } from './activities';

const emit = defineEmits<{ go: [id: PageId]; palette: [] }>();

const { connected, connectionMode, communicationError, summary, backgroundActivity, sampleInterval } = useServo();

const pillText = computed(() =>
  connected.value
    ? (communicationError.value ? '通讯异常' : connectionMode.value === 'simulator' ? '模拟器已连接' : '串口已连接')
    : '未连接',
);
</script>

<template>
  <footer class="sa-statusbar" aria-label="状态栏">
    <div class="sa-statusbar-left">
      <span class="sa-statusbar-pill" :class="{ online: connected && !communicationError, fault: connected && !!communicationError }">
        <span class="sa-statusbar-dot" aria-hidden="true"></span>{{ pillText }}
      </span>
      <span v-if="summary" class="sa-statusbar-item sa-statusbar-keep">{{ summary.deviceName }}</span>
      <button
        v-if="backgroundActivity"
        type="button"
        class="sa-statusbar-item sa-statusbar-task"
        title="返回通信工作台"
        @click="emit('go', 'communication')"
      >{{ backgroundActivity }}</button>
      <button
        v-if="connected && communicationError"
        type="button"
        class="sa-statusbar-item sa-statusbar-fault"
        title="查看诊断"
        @click="emit('go', 'diagnostics')"
      >{{ communicationError }}</button>
    </div>
    <div class="sa-statusbar-right">
      <span v-if="summary" class="sa-statusbar-item sa-statusbar-hide-narrow">{{ summary.parameterCount }} 参数 · {{ sampleInterval }} ms</span>
      <button type="button" class="sa-statusbar-item sa-statusbar-palette" title="打开命令面板" @click="emit('palette')">Ctrl+K 命令</button>
    </div>
  </footer>
</template>
