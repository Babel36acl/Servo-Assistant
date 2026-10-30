<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';

export interface PaletteCommand {
  id: string;
  title: string;
  hint?: string;
  run: () => void | Promise<void>;
}

const props = defineProps<{ open: boolean; commands: PaletteCommand[] }>();
const emit = defineEmits<{ close: [] }>();

const query = ref('');
const cursor = ref(0);
const inputEl = ref<HTMLInputElement | null>(null);

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  const list = q
    ? props.commands.filter((c) => (c.title + ' ' + (c.hint ?? '')).toLowerCase().includes(q))
    : props.commands;
  return list.slice(0, 12);
});

watch(
  () => props.open,
  async (o) => {
    query.value = '';
    cursor.value = 0;
    if (o) {
      await nextTick();
      inputEl.value?.focus();
    }
  },
);

function onInput(): void {
  cursor.value = 0;
}

function move(delta: number): void {
  cursor.value = Math.max(0, Math.min(cursor.value + delta, filtered.value.length - 1));
}

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape') emit('close');
  else if (e.key === 'ArrowDown') { e.preventDefault(); move(1); }
  else if (e.key === 'ArrowUp') { e.preventDefault(); move(-1); }
  else if (e.key === 'Enter') {
    const c = filtered.value[cursor.value];
    if (c) {
      void c.run();
      emit('close');
    }
  }
}
</script>

<template>
  <div v-if="props.open" class="sa-palette-overlay" @pointerdown.self="emit('close')">
    <div class="sa-palette" role="dialog" aria-modal="true" aria-label="命令面板" @keydown="onKey">
      <input
        ref="inputEl"
        v-model="query"
        type="search"
        class="sa-palette-input"
        placeholder="输入命令：连接、读取、录制、前往…"
        aria-label="搜索命令"
        autocomplete="off"
        spellcheck="false"
        @input="onInput"
      />
      <ul v-if="filtered.length" class="sa-palette-list" role="listbox" aria-label="匹配的命令">
        <li v-for="(c, i) in filtered" :key="c.id" role="option" :aria-selected="i === cursor">
          <button type="button" :class="{ active: i === cursor }" @click="void c.run(); emit('close')" @mousemove="cursor = i">
            <span>{{ c.title }}</span><small v-if="c.hint">{{ c.hint }}</small>
          </button>
        </li>
      </ul>
      <p v-else class="sa-palette-empty">没有匹配的命令，换个词试试</p>
    </div>
  </div>
</template>
