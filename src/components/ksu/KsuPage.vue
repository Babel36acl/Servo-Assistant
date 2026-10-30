<script setup lang="ts">
/** KSU 页面骨架：页头（标题 + 副标题 + 右侧操作）+ 分组卡容器 + 底部留白。 */
const props = defineProps<{ title: string; sub?: string }>();
</script>

<template>
  <div class="ksu-page">
    <header class="ksu-page-head">
      <div>
        <h1 class="ksu-page-title">{{ props.title }}</h1>
        <p v-if="props.sub" class="ksu-page-sub">{{ props.sub }}</p>
      </div>
      <div v-if="$slots.actions" class="ksu-page-actions">
        <slot name="actions" />
      </div>
    </header>

    <div class="ksu-page-body">
      <slot />
    </div>

    <div class="ksu-page-bottom" aria-hidden="true"></div>
  </div>
</template>
