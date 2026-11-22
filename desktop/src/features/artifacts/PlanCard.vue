<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiProgress from '../../ui/UiProgress.vue'
import { DEFAULT_JITTER, sketchRect } from '../../ui/sketch/sketchPath'
import { STATE_BADGE, STATE_CHIP } from '../backlog/backlogLabels'
import type { BacklogItem, BacklogState } from '../backlog/backlogTypes'
import type { ParsedProposal } from '../backlog/planProposal'
import { CARD_CLASSES, SUMMARY_LINES_CLASSES, WRAP_CLASSES, sheetTilt } from './artifactCardClasses'
import { planProgress } from './planProgress'

const TAB_WIDTH = 44
const TAB_HEIGHT = 18
const TAB_SEED = 9
const TAB_PATH = sketchRect(TAB_WIDTH, TAB_HEIGHT, TAB_SEED, DEFAULT_JITTER)
const TAB_CLASSES = 'absolute -top-2.5 left-4 flex items-center justify-center bg-surface font-hand text-annotation leading-none text-muted'

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
  <button type="button" :class="CARD_CLASSES" :style="sheetTilt(item.id)" :aria-label="`Abrir plano ${item.title}`" @click="$emit('open')">
    <!-- O plano se distingue pela aba desenhada à mão, não pela cor do fundo. -->
    <span aria-hidden="true" :class="TAB_CLASSES" :style="{ width: `${TAB_WIDTH}px`, height: `${TAB_HEIGHT}px` }">
      <svg class="absolute inset-0 overflow-visible text-graphite" :viewBox="`0 0 ${TAB_WIDTH} ${TAB_HEIGHT}`" fill="none" stroke="currentColor">
        <path :d="TAB_PATH" stroke-width="1.2" stroke-linecap="round" vector-effect="non-scaling-stroke" />
      </svg>
      plano
    </span>
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
      <UiProgress :value="progress.percent" pencil />
    </span>
  </button>
</template>
