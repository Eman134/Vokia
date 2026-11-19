<script setup lang="ts">
import { computed, ref } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiProgress from '../../ui/UiProgress.vue'
import type { BacklogItem } from '../backlog/backlogTypes'
import { useBacklog } from '../backlog/useBacklog'
import { progressOf, stageChip, stagesOf } from './planProgress'

const WRAP_CLASSES = 'wrap-break-word'
const ROW_CLASSES = 'flex min-w-0 flex-col gap-3 rounded-control-lg border border-line p-3.5'
const CRITERION_CLASSES = 'flex cursor-pointer items-start gap-2.5 text-control text-ink'
const EMPTY_PROGRESS = { done: 0, total: 0, percent: 0 }

const props = defineProps<{ stage: BacklogItem; parentTitle?: string }>()

// Marcação só visual: alterar o critério ainda não grava no item.
const checked = ref<Record<string, boolean>>(
  Object.fromEntries((props.stage.criteria ?? []).map((criterion) => [criterion.id, criterion.done])),
)

const backlog = useBacklog()
const criteria = computed(() => props.stage.criteria ?? [])
const chip = computed(() => stageChip(props.stage))
const children = computed(() => stagesOf(backlog.items, props.stage.id))
const childCriteria = computed(() => children.value.flatMap((child) => child.criteria ?? []))
const childProgress = computed(
  () =>
    progressOf(childCriteria.value.filter((criterion) => criterion.done).length, childCriteria.value.length) ??
    EMPTY_PROGRESS,
)
</script>

<template>
  <article :class="ROW_CLASSES" :aria-label="`Etapa ${stage.title}`">
    <header class="flex min-w-0 flex-col items-start gap-2 sm:flex-row sm:gap-2.5">
      <div class="flex min-w-0 flex-1 flex-col gap-0.5">
        <p v-if="parentTitle" class="truncate text-meta text-muted">↳ {{ parentTitle }}</p>
        <span class="font-mono text-caption text-muted">{{ stage.id }}</span>
        <h4 :class="['text-control font-semibold text-ink', WRAP_CLASSES]">{{ stage.title }}</h4>
      </div>
      <UiBadge :status="chip.status" :label="chip.label" />
    </header>

    <div v-if="children.length" class="flex flex-col gap-1.5">
      <div class="flex flex-wrap justify-between gap-x-3 text-meta text-muted">
        <span>{{ childProgress.done }} de {{ childProgress.total }} critérios dos filhos marcados</span>
        <span>{{ childProgress.percent }}%</span>
      </div>
      <UiProgress :value="childProgress.percent" />
    </div>

    <div v-if="criteria.length" class="flex flex-col gap-2">
      <label v-for="criterion in criteria" :key="criterion.id" :class="CRITERION_CLASSES">
        <input v-model="checked[criterion.id]" type="checkbox" class="mt-1 size-4 flex-none" />
        <span :class="['min-w-0 flex-1', WRAP_CLASSES]">{{ criterion.text }}</span>
      </label>
    </div>
  </article>
</template>
