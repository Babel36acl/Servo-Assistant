<script setup lang="ts">
import { useServo } from "../composables/useServo";
const { audit, formatTimestamp } = useServo();
</script>

<template>
        <div class="page-stack">
        <section class="panel audit-panel">
          <div class="section-title"><span>LOG</span><h2>操作证据</h2></div>
          <div class="audit-list">
            <div v-for="entry in [...audit].reverse().slice(0, 20)" :key="`${entry.timestampMs}-${entry.action}`" :class="`audit-${entry.status}`">
              <time>{{ formatTimestamp(entry.timestampMs) }}</time>
              <code>{{ entry.action }}</code>
              <span>{{ entry.action === 'profile.import' && entry.status === 'success' ? '设备配置已导入' : entry.detail }}</span>
            </div>
            <p v-if="!audit.length">尚无操作记录。</p>
          </div>
        </section>
        </div>
</template>
