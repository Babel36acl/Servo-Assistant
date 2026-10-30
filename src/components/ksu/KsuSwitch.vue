<script setup lang="ts">
/** 壳 UI 开关：52×32（选中带勾），尺寸/颜色全部由 token 决定。 */
const props = defineProps<{
  modelValue: boolean;
  disabled?: boolean;
  label?: string;
}>();

const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>();

function toggle(): void {
  if (props.disabled) return;
  emit('update:modelValue', !props.modelValue);
}
</script>

<template>
  <button
    type="button"
    class="ksu-switch"
    role="switch"
    :aria-checked="props.modelValue"
    :aria-label="props.label"
    :disabled="props.disabled"
    @click="toggle"
  >
    <span class="ksu-switch-knob">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="m5 12.5 4.5 4.5L19 7" />
      </svg>
    </span>
  </button>
</template>
