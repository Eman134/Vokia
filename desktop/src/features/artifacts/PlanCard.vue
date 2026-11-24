<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiProgress from '../../ui/UiProgress.vue'
import { DEFAULT_JITTER, sketchRect } from '../../ui/sketch/sketchPath'
import type { BacklogColumn, BacklogItem, BacklogState } from '../backlog/backlogTypes'
import type { ParsedProposal } from '../backlog/planProposal'
import { CARD_CLASSES, CARD_FOOTER_CLASSES, SUMMARY_LINES_CLASSES, WRAP_CLASSES, sheetTilt } from './artifactCardClasses'
import { plainSummary } from './artifactsCopy'
import { planChip, planProgress } from './planProgress'

const DONE_COLUMN: BacklogColumn = 'Done'
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
const chip = computed(() => planChip(props.state, props.item.column))
const summary = computed(() => plainSummary(props.proposal.summary))
const finished = computed(() => props.item.column === DONE_COLUMN)
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
    <span class="flex min-w-0 flex-wrap items-center gap-2.5 pt-1">
      <span :class="['min-w-0 flex-1 text-lead', finished ? 'text-muted' : 'text-ink', WRAP_CLASSES]">{{ item.title }}</span>
      <UiBadge :status="chip.status" :label="chip.label" />
    </span>
    <span v-if="summary" :class="['text-small text-muted', SUMMARY_LINES_CLASSES, WRAP_CLASSES]">{{ summary }}</span>
    <span :class="CARD_FOOTER_CLASSES">
      <span v-if="progress" class="flex flex-wrap justify-between gap-x-3">
        <span>{{ progress.done }} de {{ progress.total }} etapas feitas</span>
        <span>{{ progress.percent }}%</span>
      </span>
      <UiProgress v-if="progress" :value="progress.percent" pencil />
      <span class="flex flex-wrap justify-between gap-x-3">
        <span class="font-mono text-faint">{{ item.id }}</span>
        <span v-if="!progress && stageCount > 0">{{ stageLabel }}</span>
      </span>
    </span>
  </button>
</template>
