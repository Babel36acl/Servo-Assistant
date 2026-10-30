<script setup lang="ts">
/** KSU 状态标签：default / ok / warn / bad / muted。 */
const props = defineProps<{ tone?: 'default' | 'ok' | 'warn' | 'bad' | 'muted' }>();
</script>

<template>
  <span class="ksu-pill" :class="props.tone && props.tone !== 'default' ? `ksu-pill--${props.tone}` : ''">
    <slot />
  </span>
</template>
