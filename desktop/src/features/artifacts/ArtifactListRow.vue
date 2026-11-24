<script setup lang="ts">
import UiBadge from '../../ui/UiBadge.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import { updatedAgo } from './artifactsCopy'

type BadgeStatus = InstanceType<typeof UiBadge>['$props']['status']

const ROW_CLASSES = `grid w-full min-w-0 cursor-pointer grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 gap-y-0.5 border-b border-line px-3 py-1.5 text-left hover:bg-hover sm:grid-cols-[5.5rem_minmax(0,1fr)_auto_7rem_5rem] ${INK_FOCUS_CLASSES}`

defineProps<{
  id: string
  title: string
  kindLabel: string
  chip?: { label: string; status: BadgeStatus }
  progress?: string
  updatedAt?: number
}>()

defineEmits<{ open: [] }>()
</script>

<template>
  <button type="button" :class="ROW_CLASSES" :aria-label="`Abrir ${kindLabel.toLocaleLowerCase('pt-BR')} ${title}`" data-testid="artifact-row" @click="$emit('open')">
    <span class="hidden text-caption text-muted sm:block">{{ kindLabel }}</span>
    <span class="min-w-0 truncate text-control text-ink" :title="title">
      {{ title }} <span class="font-mono text-caption text-faint">{{ id }}</span>
    </span>
    <span class="flex justify-end"><UiBadge v-if="chip" :status="chip.status" :label="chip.label" /></span>
    <span class="hidden truncate text-caption text-muted sm:block">{{ progress }}</span>
    <span class="hidden text-right text-caption text-muted sm:block">{{ updatedAt === undefined ? '' : updatedAgo(updatedAt) }}</span>
  </button>
</template>
