<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'

const SECTION_CLASSES = 'flex flex-col rounded-dialog border border-line bg-surface px-6 py-5.5'
const TITLE_CLASSES = 'text-[20px] font-semibold tracking-[-0.02em] text-ink'

defineProps<{
  title: string
  description: string
  action?: string
}>()
</script>

<template>
  <section :aria-label="title" :class="SECTION_CLASSES">
    <header class="flex flex-wrap items-baseline gap-2.5 pb-3">
      <h2 :class="TITLE_CLASSES">{{ title }}</h2>
      <span class="text-label text-muted">{{ description }}</span>
      <span v-if="action" class="ml-auto">
        <UiButton variant="quiet" size="sm" disabled>{{ action }}</UiButton>
      </span>
    </header>
    <slot />
  </section>
</template>
