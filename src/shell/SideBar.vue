<script setup lang="ts">
import { ref } from 'vue';
import { useServo } from '../composables/useServo';
import { pageById, type PageId } from './activities';

const props = defineProps<{ active: PageId }>();

const {
  activePage, summary, connected, communicationError, connectionMode,
  groups, selectedGroup, scopeChannels, pollingEnabled, sampleInterval,
  audit, retries, maxRegisters, backgroundActivity, profile,
} = useServo();

const drawerOpen = ref(false);

function goto(id: PageId): void {
  activePage.value = id;
  drawerOpen.value = false;
}

function pickGroup(group: string): void {
  selectedGroup.value = group;
  activePage.value = 'parameters';
  drawerOpen.value = false;
}
</script>

<template>
  <button type="button" class="sa-drawer-fab" :aria-expanded="drawerOpen" aria-controls="sa-sidebar" @click="drawerOpen = !drawerOpen">
    {{ drawerOpen ? '收起侧栏' : '展开侧栏' }}
  </button>
  <aside id="sa-sidebar" class="sa-sidebar" :class="{ open: drawerOpen }" :aria-label="pageById(props.active).label + '侧栏'">
    <p class="sa-sidebar-eyebrow">{{ pageById(props.active).label }}</p>
    <p class="sa-sidebar-caption">{{ pageById(props.active).caption }}</p>

    <div class="sa-sidebar-block">
      <p class="sa-sidebar-heading">当前设备</p>
      <strong class="sa-sidebar-device">{{ summary?.deviceName ?? '未导入 Profile' }}</strong>
      <small v-if="summary">{{ summary.parameterCount }} 个参数 · Profile {{ summary.profileVersion }}</small>
      <small v-else>设备定义由 Profile 提供</small>
      <p class="sa-sidebar-status" :class="{ online: connected && !communicationError, fault: connected && !!communicationError }">
        {{ connected ? (communicationError ? '通讯异常' : connectionMode === 'simulator' ? '模拟器已连接' : '串口已连接') : '未连接' }}
      </p>
    </div>

    <div v-if="props.active === 'parameters'" class="sa-sidebar-block">
      <p class="sa-sidebar-heading">参数分组</p>
      <div class="sa-sidebar-groups" role="listbox" aria-label="参数分组">
        <button
          v-for="group in groups"
          :key="group"
          type="button"
          role="option"
          :aria-selected="selectedGroup === group"
          :class="{ active: selectedGroup === group }"
          @click="pickGroup(group)"
        >{{ group }}</button>
      </div>
      <small v-if="profile">共 {{ profile.parameters.length }} 个参数</small>
    </div>

    <div v-if="props.active === 'monitor'" class="sa-sidebar-block">
      <p class="sa-sidebar-heading">监控通道</p>
      <small v-if="!scopeChannels.length">尚未选择通道，请在主区勾选</small>
      <ul v-else class="sa-sidebar-list">
        <li v-for="channel in scopeChannels" :key="channel">{{ channel }}</li>
      </ul>
      <label class="sa-sidebar-check"><input v-model="pollingEnabled" type="checkbox" />状态轮询{{ pollingEnabled ? '开' : '关' }}</label>
      <small>采样间隔 {{ sampleInterval }} ms</small>
    </div>

    <div v-if="props.active === 'diagnostics'" class="sa-sidebar-block">
      <p class="sa-sidebar-heading">采集设置</p>
      <small>重试 {{ retries }} 次 · 每组 {{ maxRegisters }} 寄存器</small>
      <button type="button" class="sa-sidebar-btn" @click="goto('diagnostics')">调整采集与测试 →</button>
    </div>

    <div v-if="props.active === 'communication'" class="sa-sidebar-block">
      <p class="sa-sidebar-heading">后台任务</p>
      <small v-if="backgroundActivity">{{ backgroundActivity }}</small>
      <small v-else>录制与捕获空闲中</small>
      <button type="button" class="sa-sidebar-btn" @click="goto('communication')">返回通信工作台 →</button>
    </div>

    <div v-if="props.active === 'audit'" class="sa-sidebar-block">
      <p class="sa-sidebar-heading">证据统计</p>
      <small>已记录 {{ audit.length }} 条操作</small>
      <button type="button" class="sa-sidebar-btn" @click="goto('audit')">查看操作证据 →</button>
    </div>

    <div v-if="props.active === 'connection'" class="sa-sidebar-block">
      <p class="sa-sidebar-heading">三步上手</p>
      <small>1 导入设备配置 · 2 选择模式连接 · 3 读取后再修改</small>
      <button type="button" class="sa-sidebar-btn" @click="goto('parameters')">进入参数工作区 →</button>
    </div>
  </aside>
</template>
