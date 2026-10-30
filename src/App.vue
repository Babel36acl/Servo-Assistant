<script setup lang="ts">
import { computed } from 'vue';
import KsuButton from './components/ksu/KsuButton.vue';
import KsuSheet from './components/ksu/KsuSheet.vue';
import AppShell from './shell/AppShell.vue';
import { pages, type PageId } from './shell/activities';
import { useServo } from './composables/useServo';
import ConnectionView from './views/ConnectionView.vue';
import ParametersView from './views/ParametersView.vue';
import MonitorView from './views/MonitorView.vue';
import DiagnosticsView from './views/DiagnosticsView.vue';
import CommunicationView from './views/CommunicationView.vue';
import AuditView from './views/AuditView.vue';
import SettingsView from './views/SettingsView.vue';

const {
  activePage, currentPage, errorMessage, communicationError,
  readActive, testActive, discoveryActive, discovery, readProgress, testProgress,
  cancelRequested, backgroundActivity, confirmReq, confirmInput, confirmError,
  confirmField, closeConfirm, busy, cancelDiscovery, failedGroups, staleIds,
} = useServo();
// confirmField 只在模板里以 ref="confirmField" 形式绑定（确认框输入聚焦），这里显式读一次，免得被判未使用。
void confirmField;
void currentPage;

// 截图/深链用：?page=monitor 指定初始页（仅首次进入读取，不写回 URL）。
const initialPage = new URLSearchParams(window.location.search).get('page');
if (initialPage && pages.some((page) => page.id === initialPage)) {
  activePage.value = initialPage as PageId;
}

const showProgress = computed(() => readActive.value || testActive.value || discoveryActive.value);const progressText = computed(() =>
  discoveryActive.value
    ? `正在查找设备 · ${discovery.value?.completed ?? 0}/${discovery.value?.total ?? '—'}`
    : readActive.value
      ? readProgress.value
      : testProgress.value,
);
const progressTitle = computed(() =>
  discoveryActive.value ? '查找设备' : readActive.value ? '读取参数' : '稳定性测试',
);
/** 从进度文案（如 “3/12 地址组 …”）解析百分比；解析不到就退化为不定量动画。 */
const progressPercent = computed<number | null>(() => {
  const match = /(\d+)\s*\/\s*(\d+)/.exec(progressText.value);
  if (!match) return null;
  const done = Number(match[1]);
  const total = Number(match[2]);
  return total > 0 ? Math.min(100, Math.round((done / total) * 100)) : null;
});

const navBadges = computed<Partial<Record<PageId, string | number>>>(() => {
  const badges: Partial<Record<PageId, string | number>> = {};
  const parameterBadge = failedGroups.value.length || staleIds.value.size;
  if (parameterBadge) badges.parameters = parameterBadge;
  if (communicationError.value) badges.monitor = '!';
  return badges;
});
</script>

<template>
  <AppShell :active="activePage" :badges="navBadges" @select="activePage = $event">
    <div class="ksu-status-stack">
      <div v-if="errorMessage" class="ksu-banner ksu-banner--error" role="alert">
        <div class="ksu-banner-body">
          <strong>操作失败</strong>
          <span>{{ errorMessage }}</span>
        </div>
        <div class="ksu-banner-actions">
          <KsuButton sm variant="ghost" @click="errorMessage = ''">关闭</KsuButton>
        </div>
      </div>

      <div v-if="communicationError" class="ksu-banner ksu-banner--error" role="alert">
        <div class="ksu-banner-body">
          <strong>通讯异常</strong>
          <span>{{ communicationError }}</span>
        </div>
        <div class="ksu-banner-actions">
          <KsuButton sm @click="activePage = 'diagnostics'">查看诊断</KsuButton>
        </div>
      </div>

      <div v-if="showProgress" class="ksu-banner ksu-banner--info" role="status">
        <div class="ksu-banner-body">
          <strong>{{ progressTitle }}</strong>
          <span>{{ progressText }}</span>
          <div
            class="ksu-progress"
            role="progressbar"
            aria-valuemin="0"
            aria-valuemax="100"
            :aria-valuenow="progressPercent ?? undefined"
            :aria-valuetext="progressText"
          >
            <div
              class="ksu-progress-bar"
              :data-indeterminate="progressPercent === null"
              :style="progressPercent === null ? undefined : { width: `${progressPercent}%` }"
            ></div>
          </div>
        </div>
        <div class="ksu-banner-actions">
          <KsuButton v-if="discoveryActive" sm @click="cancelDiscovery">取消查找</KsuButton>
          <KsuButton v-else sm :disabled="cancelRequested" @click="cancelRequested = true">
            {{ cancelRequested ? '等待当前事务结束' : '取消当前操作' }}
          </KsuButton>
        </div>
      </div>

      <div
        v-if="backgroundActivity && activePage !== 'communication'"
        class="ksu-banner ksu-banner--info"
        role="status"
      >
        <div class="ksu-banner-body">
          <span>{{ backgroundActivity }}</span>
        </div>
        <div class="ksu-banner-actions">
          <KsuButton sm @click="activePage = 'communication'">返回通信工作台</KsuButton>
        </div>
      </div>
    </div>

    <ConnectionView v-if="activePage === 'connection'" />
    <ParametersView v-else-if="activePage === 'parameters'" />
    <MonitorView v-else-if="activePage === 'monitor'" />
    <DiagnosticsView v-else-if="activePage === 'diagnostics'" />
    <CommunicationView v-else-if="activePage === 'communication'" />
    <AuditView v-else-if="activePage === 'audit'" />
    <SettingsView v-else-if="activePage === 'settings'" />

    <KsuSheet
      :open="!!confirmReq"
      :title="confirmReq?.title"
      aria-label="确认操作"
      @close="closeConfirm(null)"
    >
      <p v-if="confirmReq">{{ confirmReq.body }}</p>
      <label v-if="confirmReq?.phrase">
        请输入“{{ confirmReq.phrase }}”以继续
        <input
          ref="confirmField"
          v-model="confirmInput"
          class="ksu-input"
          autocomplete="off"
          spellcheck="false"
          :placeholder="confirmReq.phrase"
          @keydown.enter="closeConfirm('submit')"
        />
      </label>
      <p v-if="confirmError" class="ksu-banner ksu-banner--error" role="alert">{{ confirmError }}</p>
      <template #actions>
        <KsuButton @click="closeConfirm(null)">取消</KsuButton>
        <KsuButton variant="primary" :disabled="busy" @click="closeConfirm('submit')">确认</KsuButton>
      </template>
    </KsuSheet>
  </AppShell>
</template>
