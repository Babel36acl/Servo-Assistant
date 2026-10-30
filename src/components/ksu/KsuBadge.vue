<script setup lang="ts">
/** KSU 角标：数字（primary 底）或纯圆点（6dp，Badge.kt:182）。 */
const props = defineProps<{ count?: string | number; dot?: boolean; label?: string }>();
</script>

<template>
  <span
    class="ksu-badge"
    :class="{ 'ksu-badge--dot': props.dot }"
    :aria-label="props.label"
    role="status"
  >
    <template v-if="!props.dot">{{ props.count ?? '' }}</template>
  </span>
</template>
