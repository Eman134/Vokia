<script lang="ts">
export const PAGE_TITLE_CLASSES = 'text-page text-ink'
</script>

<script setup lang="ts">
import UiHandLine from './UiHandLine.vue'

defineProps<{
  title: string
  subtitle?: string
}>()

const HEADER_CLASSES = 'flex min-h-10 flex-wrap items-center gap-x-4 gap-y-2'
const TITLE_CLASSES = PAGE_TITLE_CLASSES
const SUBTITLE_CLASSES = 'mt-1 max-w-[64ch] text-body text-muted'
const ACTIONS_CLASSES = 'ml-auto flex flex-wrap items-center gap-2'
</script>

<template>
  <header :class="HEADER_CLASSES">
    <div class="min-w-0">
      <div class="w-fit max-w-full">
        <h1 :class="TITLE_CLASSES">{{ title }}</h1>
        <UiHandLine class="h-1.5 w-full text-graphite" />
      </div>
      <p v-if="subtitle" :class="SUBTITLE_CLASSES">{{ subtitle }}</p>
    </div>
    <div v-if="$slots.actions" :class="ACTIONS_CLASSES">
      <slot name="actions" />
    </div>
  </header>
</template>
