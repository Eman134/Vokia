<script lang="ts">
export const PAGE_TITLE_CLASSES = 'text-page text-ink'
</script>

<script setup lang="ts">
import UiHandLine from './UiHandLine.vue'

defineProps<{
  title: string
  projectName?: string
  subtitle?: string
}>()

const HEADER_CLASSES = 'flex flex-wrap items-end gap-4'
const TITLE_CLASSES = `mt-1 ${PAGE_TITLE_CLASSES}`
const PROJECT_NAME_CLASSES = 'mt-1 font-hand text-annotation text-muted'
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
      <p v-if="projectName" :class="PROJECT_NAME_CLASSES">{{ projectName }}</p>
      <p v-if="subtitle" :class="SUBTITLE_CLASSES">{{ subtitle }}</p>
    </div>
    <div v-if="$slots.actions" :class="ACTIONS_CLASSES">
      <slot name="actions" />
    </div>
  </header>
</template>
