<script setup lang="ts">
import UiMarker from '../../ui/UiMarker.vue'
import UiProgress from '../../ui/UiProgress.vue'
import { tagLabel, tagMarkerTone } from '../backlog/tags'
import type { ArtifactSummary } from './artifactApi'
import type { ChecklistProgress } from './artifactChecks'
import { CARD_CLASSES, SUMMARY_LINES_CLASSES, WRAP_CLASSES, sheetTilt } from './artifactCardClasses'
import { updatedAgo } from './artifactsCopy'

defineProps<{
  summary: ArtifactSummary
  kindLabel: string
  progress?: ChecklistProgress
}>()

defineEmits<{ open: [] }>()
</script>

<template>
  <button type="button" :class="CARD_CLASSES" :style="sheetTilt(summary.id)" :aria-label="`Abrir artefato ${summary.title}`" @click="$emit('open')">
    <span class="text-caption text-muted">Artefatos › {{ kindLabel }} · <span class="font-mono">{{ summary.id }}</span></span>
    <span :class="['min-w-0 text-lead text-ink', WRAP_CLASSES]">{{ summary.title }}</span>
    <span v-if="summary.summary" :class="['text-small text-muted', SUMMARY_LINES_CLASSES, WRAP_CLASSES]">{{ summary.summary }}</span>
    <span v-if="summary.tags.length" class="flex flex-wrap gap-1.5">
      <UiMarker v-for="tag in summary.tags" :key="tag" :tone="tagMarkerTone(tag)" class="text-meta">{{ tagLabel(tag) }}</UiMarker>
    </span>
    <span v-if="progress && progress.total > 0" class="flex flex-col gap-1.5">
      <span class="flex flex-wrap justify-between gap-x-3 text-meta text-muted">
        <span>{{ progress.done }} de {{ progress.total }} itens marcados</span>
        <span>{{ progress.percent }}%</span>
      </span>
      <UiProgress :value="progress.percent" pencil />
    </span>
    <span class="text-meta text-muted">{{ updatedAgo(summary.updatedAt) }}</span>
  </button>
</template>
