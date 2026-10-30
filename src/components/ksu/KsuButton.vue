<script setup lang="ts">
/** KSU 按钮：default（描边）/ primary / tonal / ghost / danger，支持 sm / block。 */
const props = defineProps<{
  variant?: 'default' | 'primary' | 'tonal' | 'ghost' | 'danger';
  sm?: boolean;
  block?: boolean;
  disabled?: boolean;
  type?: 'button' | 'submit';
}>();

const emit = defineEmits<{ click: [event: MouseEvent] }>();

function onClick(event: MouseEvent): void {
  if (props.disabled) return;
  emit('click', event);
}
</script>

<template>
  <button
    :type="props.type ?? 'button'"
    class="ksu-btn"
    :class="[
      props.variant && props.variant !== 'default' ? `ksu-btn--${props.variant}` : '',
      { 'ksu-btn--sm': props.sm, 'ksu-btn--block': props.block },
    ]"
    :disabled="props.disabled"
    @click="onClick"
  >
    <slot />
  </button>
</template>
