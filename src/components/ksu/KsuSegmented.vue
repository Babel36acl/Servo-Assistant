<script lang="ts">
export interface KsuSegmentOption {
  value: string;
  label: string;
}
</script>

<script setup lang="ts">
/** 分段控件：蓝底胶囊（FlClash 风格），用于主题模式/字号等互斥选项。 */
const props = defineProps<{
  modelValue: string;
  options: readonly KsuSegmentOption[];
  label?: string;
  disabled?: boolean;
}>();

const emit = defineEmits<{ 'update:modelValue': [value: string] }>();

function select(value: string): void {
  if (props.disabled || value === props.modelValue) return;
  emit('update:modelValue', value);
}
</script>

<template>
  <div class="ksu-segmented" role="group" :aria-label="props.label">
    <button
      v-for="option in props.options"
      :key="option.value"
      type="button"
      class="ksu-segmented-item"
      :class="{ active: option.value === props.modelValue }"
      :aria-pressed="option.value === props.modelValue"
      :disabled="props.disabled"
      @click="select(option.value)"
    >
      {{ option.label }}
    </button>
  </div>
</template>
