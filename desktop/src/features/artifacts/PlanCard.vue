<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiProgress from '../../ui/UiProgress.vue'
import { STATE_BADGE, STATE_CHIP } from '../backlog/backlogLabels'
import type { BacklogItem, BacklogState } from '../backlog/backlogTypes'
import type { ParsedProposal } from '../backlog/planProposal'
import { planProgress } from './planProgress'

const CARD_CLASSES =
  'flex min-w-0 flex-col gap-3 rounded-dialog border border-line bg-surface p-4 text-left transition-colors hover:border-accent focus-visible:outline-2 focus-visible:outline-accent'
const WRAP_CLASSES = 'wrap-break-word'
const SUMMARY_LINES_CLASSES = 'line-clamp-3'

const props = defineProps<{
  item: BacklogItem
  proposal: ParsedProposal
  state: BacklogState
  stages: BacklogItem[]
}>()

defineEmits<{ open: [] }>()

const progress = computed(() => planProgress(props.stages))
const stageCount = computed(() => (props.stages.length > 0 ? props.stages.length : props.proposal.lots.length))
const stageLabel = computed(() => `${stageCount.value} ${stageCount.value === 1 ? 'etapa' : 'etapas'}`)
</script>

<template>
  <button type="button" :class="CARD_CLASSES" :aria-label="`Abrir plano ${item.title}`" @click="$emit('open')">
    <span class="text-caption text-muted">Artefatos › Plano · <span class="font-mono">{{ item.id }}</span></span>
    <span class="flex min-w-0 flex-wrap items-center gap-2.5">
      <span :class="['min-w-0 flex-1 text-lead text-ink', WRAP_CLASSES]">{{ item.title }}</span>
      <UiBadge :status="STATE_BADGE[state]" :label="STATE_CHIP[state]" />
    </span>
    <span v-if="proposal.summary" :class="['text-small text-muted', SUMMARY_LINES_CLASSES, WRAP_CLASSES]">{{ proposal.summary }}</span>
    <span class="text-meta text-muted">{{ stageLabel }}</span>
    <span v-if="progress" class="flex flex-col gap-1.5">
      <span class="flex flex-wrap justify-between gap-x-3 text-meta text-muted">
        <span>{{ progress.done }} de {{ progress.total }} etapas feitas</span>
        <span>{{ progress.percent }}%</span>
      </span>
      <UiProgress :value="progress.percent" />
    </span>
  </button>
</template>
