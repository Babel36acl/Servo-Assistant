<script setup lang="ts">
import { useServo } from "../composables/useServo";
import ConnectionParamsCard from "../features/connection/ConnectionParamsCard.vue";
import DeviceStatusCard from "../features/connection/DeviceStatusCard.vue";
import DiscoveryCard from "../features/connection/DiscoveryCard.vue";
import ProfileCard from "../features/connection/ProfileCard.vue";
import RegisterScanner from "../features/modbus/RegisterScanner.vue";

/** 设备与连接：页头 + 设备状态大卡 + 双列（配置 / 查找 / 寄存器扫描 | 连接参数）。
 *  四张卡片各自读 useServo() 单例，这里只负责栅格与顺序。 */
const { connected } = useServo();
</script>

<template>
  <div class="ksu-page">
    <header class="ksu-page-head">
      <div>
        <h1 class="ksu-page-title">设备与连接</h1>
        <p class="ksu-page-sub">导入 Profile，选择模式并建立会话</p>
      </div>
      <div class="ksu-page-actions">
        <span class="ksu-pill" :class="connected ? 'ksu-pill--ok' : 'ksu-pill--muted'">{{ connected ? '运行中' : '未连接' }}</span>
      </div>
    </header>

    <div class="ksu-page-body">
      <DeviceStatusCard />

      <div class="ksu-columns">
        <div class="ksu-col ksu-col--wide">
          <ProfileCard />
          <DiscoveryCard />
          <RegisterScanner />
        </div>
        <div class="ksu-col ksu-col--narrow">
          <ConnectionParamsCard />
        </div>
      </div>
    </div>

    <div class="ksu-page-bottom" aria-hidden="true"></div>
  </div>
</template>
