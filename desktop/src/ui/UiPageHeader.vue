<script setup lang="ts">
defineProps<{
  title: string
  projectName?: string
  subtitle?: string
}>()

const HEADER_CLASSES = 'flex flex-wrap items-end gap-4'
const TITLE_CLASSES = 'mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink'
const PROJECT_NAME_CLASSES = 'mt-1 text-caption text-muted'
const SUBTITLE_CLASSES = 'mt-1 max-w-[64ch] text-body text-muted'
const ACTIONS_CLASSES = 'ml-auto flex flex-wrap items-center gap-2'
</script>

<template>
  <header :class="HEADER_CLASSES">
    <div class="min-w-0">
      <h1 :class="TITLE_CLASSES">{{ title }}</h1>
      <p v-if="projectName" :class="PROJECT_NAME_CLASSES">{{ projectName }}</p>
      <p v-if="subtitle" :class="SUBTITLE_CLASSES">{{ subtitle }}</p>
    </div>
    <div v-if="$slots.actions" :class="ACTIONS_CLASSES">
      <slot name="actions" />
    </div>
  </header>
</template>
