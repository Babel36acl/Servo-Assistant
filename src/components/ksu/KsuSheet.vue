<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';

/** KSU 弹层：对话框（默认，windowDimming 遮罩）或底部弹层（variant='bottom'）。
 *  自绘结构，替代旧的 .modal-overlay/.modal；遮罩点按与 Esc 都会 emit close。 */
const props = defineProps<{
  open: boolean;
  title?: string;
  sub?: string;
  variant?: 'dialog' | 'bottom';
  ariaLabel?: string;
}>();

const emit = defineEmits<{ close: [] }>();

function close(): void {
  emit('close');
}

function onKeydown(event: KeyboardEvent): void {
  if (props.open && event.key === 'Escape') close();
}

onMounted(() => document.addEventListener('keydown', onKeydown));
onUnmounted(() => document.removeEventListener('keydown', onKeydown));
</script>

<template>
  <Teleport to="body">
    <div
      v-if="props.open"
      class="ksu-sheet-overlay"
      :class="{ 'ksu-sheet-overlay--bottom': props.variant === 'bottom' }"
      @pointerdown.self="close"
    >
      <div
        class="ksu-sheet"
        :class="{ 'ksu-sheet--bottom': props.variant === 'bottom' }"
        role="dialog"
        aria-modal="true"
        :aria-label="props.ariaLabel ?? props.title"
      >
        <div class="ksu-sheet-head">
          <div>
            <h2 v-if="props.title" class="ksu-sheet-title">{{ props.title }}</h2>
            <p v-if="props.sub" class="ksu-sheet-sub">{{ props.sub }}</p>
          </div>
          <button type="button" class="ksu-iconbtn" aria-label="关闭" @click="close">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M6 6l12 12M18 6 6 18" />
            </svg>
          </button>
        </div>

        <div class="ksu-sheet-body">
          <slot />
        </div>

        <div v-if="$slots.actions" class="ksu-sheet-actions">
          <slot name="actions" />
        </div>
      </div>
    </div>
  </Teleport>
</template>
