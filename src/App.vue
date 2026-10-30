<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import AppShell from './shell/AppShell.vue';
import CommandPalette, { type PaletteCommand } from './shell/CommandPalette.vue';
import { pageById } from './shell/activities';
import { useServo } from './composables/useServo';
import ConnectionView from './views/ConnectionView.vue';
import ParametersView from './views/ParametersView.vue';
import MonitorView from './views/MonitorView.vue';
import DiagnosticsView from './views/DiagnosticsView.vue';
import CommunicationView from './views/CommunicationView.vue';
import AuditView from './views/AuditView.vue';

const {
  activePage, currentPage, errorMessage, notice, communicationError,
  readActive, testActive, discoveryActive, discovery, readProgress, testProgress,
  cancelRequested, backgroundActivity, confirmReq, confirmInput, confirmError,
  confirmField, closeConfirm, busy, cancelDiscovery, connected, disconnect,
  refreshPorts, readAll,
} = useServo();

const paletteOpen = ref(false);

const paletteCommands = computed<PaletteCommand[]>(() => [
  ...(['connection', 'parameters', 'monitor', 'diagnostics', 'communication', 'audit'] as const).map((id) => ({
    id: `goto-${id}`,
    title: `前往：${pageById(id).label}`,
    hint: pageById(id).caption,
    run: () => { activePage.value = id; },
  })),
  {
    id: 'disconnect', title: '断开当前连接', hint: connected.value ? '结束当前会话' : '当前未连接',
    run: () => { void disconnect(); },
  },
  { id: 'refresh-ports', title: '刷新串口列表', run: () => { void refreshPorts(); } },
  {
    id: 'read-all', title: '读取全部参数', hint: connected.value ? '当前已连接' : '需要先连接',
    run: () => { void readAll(); },
  },
  {
    id: 'cancel-op', title: '取消当前操作',
    run: () => { if (discoveryActive.value) { void cancelDiscovery(); } else { cancelRequested.value = true; } },
  },
]);

function onGlobalKey(e: KeyboardEvent): void {
  const mod = e.ctrlKey || e.metaKey;
  if ((mod && e.key.toLowerCase() === 'k') || (mod && e.shiftKey && e.key.toLowerCase() === 'p')) {
    e.preventDefault();
    paletteOpen.value = true;
  }
}

onMounted(() => window.addEventListener('keydown', onGlobalKey));
onUnmounted(() => window.removeEventListener('keydown', onGlobalKey));
</script>

<template>
  <AppShell :active="activePage" title="伺服参数调试器" :subtitle="currentPage.caption" @select="activePage = $event" @palette="paletteOpen = true">
    <div v-if="errorMessage" class="banner error" role="alert"><strong>操作失败</strong><span>{{ errorMessage }}</span><button @click="errorMessage = ''">关闭</button></div>
    <div class="banner info" role="status"><strong>当前状态</strong><span>{{ notice }}</span></div>
    <div v-if="communicationError" class="banner error" role="alert"><strong>通讯异常</strong><span>{{ communicationError }}</span><button @click="activePage = 'diagnostics'">查看诊断</button></div>
    <div v-if="readActive || testActive || discoveryActive" class="operation-progress" role="status"><span>{{ discoveryActive ? '正在查找设备 · ' + (discovery?.completed ?? 0) + '/' + (discovery?.total ?? '—') : readActive ? readProgress : testProgress }}</span><button v-if="discoveryActive" @click="cancelDiscovery">取消查找</button><button v-else :disabled="cancelRequested" @click="cancelRequested = true">{{ cancelRequested ? '等待当前事务结束' : '取消当前操作' }}</button></div>
    <div v-if="backgroundActivity && activePage !== 'communication'" class="operation-progress" role="status"><span>{{ backgroundActivity }}</span><button @click="activePage = 'communication'">返回通信工作台</button></div>

    <ConnectionView v-if="activePage === 'connection'" />
    <ParametersView v-else-if="activePage === 'parameters'" />
    <MonitorView v-else-if="activePage === 'monitor'" />
    <DiagnosticsView v-else-if="activePage === 'diagnostics'" />
    <CommunicationView v-else-if="activePage === 'communication'" />
    <AuditView v-else-if="activePage === 'audit'" />

    <div v-if="confirmReq" class="modal-overlay" @pointerdown.self="closeConfirm(null)">
      <div class="modal" role="alertdialog" aria-modal="true" :aria-label="confirmReq.title" @keydown.escape="closeConfirm(null)">
        <h2>{{ confirmReq.title }}</h2>
        <p class="modal-body">{{ confirmReq.body }}</p>
        <label v-if="confirmReq.phrase">请输入“{{ confirmReq.phrase }}”以继续<input ref="confirmField" v-model="confirmInput" autocomplete="off" spellcheck="false" :placeholder="confirmReq.phrase" @keydown.enter="closeConfirm('submit')" /></label>
        <p v-if="confirmError" class="modal-error" role="alert">{{ confirmError }}</p>
        <div class="modal-actions">
          <button class="secondary" @click="closeConfirm(null)">取消</button>
          <button class="primary" :disabled="busy" @click="closeConfirm('submit')">确认</button>
        </div>
      </div>
    </div>
  </AppShell>
  <CommandPalette :open="paletteOpen" :commands="paletteCommands" @close="paletteOpen = false" />
</template>
