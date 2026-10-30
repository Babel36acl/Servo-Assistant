<script setup lang="ts">
/** 壳 UI 输入框（44 高 / 12 圆角 / field 底色）。 */
const props = defineProps<{
  modelValue: string | number;
  type?: 'text' | 'number' | 'search' | 'password' | 'url';
  placeholder?: string;
  label?: string;
  disabled?: boolean;
  readonly?: boolean;
}>();

const emit = defineEmits<{ 'update:modelValue': [value: string] }>();

function onInput(event: Event): void {
  const target = event.target as HTMLInputElement | null;
  emit('update:modelValue', target?.value ?? '');
}
</script>

<template>
  <input
    class="ksu-input"
    :type="props.type ?? 'text'"
    :value="props.modelValue"
    :placeholder="props.placeholder"
    :aria-label="props.label"
    :disabled="props.disabled"
    :readonly="props.readonly"
    autocomplete="off"
    spellcheck="false"
    @input="onInput"
  />
</template>
