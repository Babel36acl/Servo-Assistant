<script setup lang="ts">
import { computed } from 'vue';

/** KSU 设置行：左（图标）+ 标题/副标题 + 右侧控件 + 可选箭头。
 *  clickable 时渲染成 <button>；否则静态 div（列表里用 ksu-row--static 语义）。 */
const props = defineProps<{
  title?: string;
  sub?: string;
  clickable?: boolean;
  danger?: boolean;
  chevron?: boolean;
  disabled?: boolean;
}>();

const emit = defineEmits<{ click: [event: MouseEvent] }>();

const tag = computed(() => (props.clickable ? 'button' : 'div'));

function onClick(event: MouseEvent): void {
  if (!props.clickable || props.disabled) return;
  emit('click', event);
}
</script>

<template>
  <component
    :is="tag"
    :type="props.clickable ? 'button' : undefined"
    :disabled="props.clickable ? props.disabled : undefined"
    class="ksu-row"
    :class="{
      'ksu-row--static': !props.clickable,
      'ksu-row--clickable': props.clickable,
      'ksu-row--danger': props.danger,
    }"
    @click="onClick"
  >
    <slot name="icon" />
    <div class="ksu-row-body">
      <slot>
        <span v-if="props.title" class="ksu-row-title">{{ props.title }}</span>
        <span v-if="props.sub" class="ksu-row-sub">{{ props.sub }}</span>
      </slot>
    </div>
    <slot name="control" />
    <span v-if="props.chevron" class="ksu-row-chevron" aria-hidden="true">›</span>
  </component>
</template>
