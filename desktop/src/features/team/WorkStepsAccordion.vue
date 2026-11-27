<script setup lang="ts">
import { computed, onMounted, ref, useId } from 'vue'
import type { Agent, TurnActivity } from './events'
import { listTurnWorkSteps, type HistoryWorkStepEntry } from './teamApi'
import { parseWorkStepsBlock, pickClosestWorkStepEntry, workStepsFromActivity } from './workStepsFormat'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiSkeleton from '../../ui/UiSkeleton.vue'

const props = defineProps<{
  projectId: number | null
  agent: Agent
  cardId?: string
  activity?: TurnActivity
  provider?: string
  isFallback?: boolean
  messageTime?: number
}>()

const TOGGLE_CLASSES = `flex w-fit cursor-pointer items-center gap-1.5 rounded-sm text-left text-caption text-muted transition-colors hover:text-ink ${INK_FOCUS_CLASSES}`
const PANEL_CLASSES =
  'mt-1 flex min-w-0 flex-col gap-1.5 rounded-md border border-line bg-surface-2/60 px-2.5 py-2 text-meta text-ink'
const BADGE_HANDOFF_CLASSES = 'rounded-sm bg-accent-soft px-1.5 py-0.5 text-tiny font-medium text-accent'
const BADGE_OMITTED_CLASSES = 'rounded-sm bg-warn-soft px-1.5 py-0.5 text-tiny font-medium text-warn-text'
const STEP_ITEM_CLASSES = 'whitespace-pre-wrap break-words font-mono text-meta leading-note text-ink'

const open = ref(false)
const loading = ref(false)
const loaded = ref(false)
const persistedEntry = ref<HistoryWorkStepEntry | null>(null)
const panelId = useId()

const localParsed = computed(() => workStepsFromActivity(props.activity))
const remoteParsed = computed(() =>
  persistedEntry.value ? parseWorkStepsBlock(persistedEntry.value) : null,
)

const activeSteps = computed(() => {
  if (remoteParsed.value && remoteParsed.value.steps.length > 0) {
    return remoteParsed.value.steps
  }
  return localParsed.value.steps
})

const omittedCount = computed(
  () => remoteParsed.value?.omittedCount ?? localParsed.value.omittedCount ?? 0,
)

const totalCount = computed(() => {
  if (persistedEntry.value) {
    return Math.max(persistedEntry.value.stepCount, activeSteps.value.length + omittedCount.value)
  }
  return activeSteps.value.length
})

const effectiveProvider = computed(
  () => remoteParsed.value?.provider ?? props.provider,
)

const visible = computed(() => totalCount.value > 0)

const toggleLabel = computed(() => {
  const count = totalCount.value
  if (open.value) {
    return `Etapas executadas (${count})`
  }
  return count === 1 ? 'Ver 1 etapa executada' : `Ver ${count} etapas executadas`
})

const omittedLabel = computed(() => {
  const count = omittedCount.value
  return count === 1 ? '1 passo anterior omitido' : `${count} passos anteriores omitidos`
})

async function fetchPersistedSteps(): Promise<void> {
  if (loaded.value || loading.value || props.projectId === null) return
  loading.value = true
  try {
    const response = await listTurnWorkSteps({
      projectId: props.projectId,
      agent: props.agent,
      cardId: props.cardId,
      limit: 5,
    })
    persistedEntry.value = pickClosestWorkStepEntry(response.workSteps, props.messageTime) ?? null
    loaded.value = true
  } catch {
    // Falha silenciosa: mantém os passos locais derivados de TurnActivity se existirem.
  } finally {
    loading.value = false
  }
}

async function toggle(): Promise<void> {
  open.value = !open.value
  if (open.value && !loaded.value) {
    await fetchPersistedSteps()
  }
}

onMounted(() => {
  if (props.isFallback && localParsed.value.steps.length === 0 && props.projectId !== null) {
    void fetchPersistedSteps()
  }
})
</script>

<template>
  <div v-if="visible" class="mt-0.5 flex min-w-0 flex-col items-start" data-testid="work-steps-accordion">
    <button
      type="button"
      :class="TOGGLE_CLASSES"
      :aria-expanded="open"
      :aria-controls="panelId"
      data-testid="work-steps-toggle"
      @click="toggle"
    >
      <span>{{ toggleLabel }}</span>
      <span v-if="isFallback && effectiveProvider" :class="BADGE_HANDOFF_CLASSES" data-testid="work-steps-handoff">
        Handoff · {{ effectiveProvider }}
      </span>
      <span aria-hidden="true">{{ open ? '▾' : '▸' }}</span>
    </button>
    <div v-if="open" :id="panelId" :class="PANEL_CLASSES" data-testid="work-steps-panel">
      <div v-if="omittedCount > 0 || (isFallback && effectiveProvider)" class="flex flex-wrap items-center gap-1.5">
        <span v-if="omittedCount > 0" :class="BADGE_OMITTED_CLASSES" data-testid="work-steps-omitted">
          {{ omittedLabel }}
        </span>
        <span v-if="isFallback && effectiveProvider" class="text-tiny text-muted">
          Provedor: {{ effectiveProvider }}
        </span>
      </div>
      <UiSkeleton v-if="loading && activeSteps.length === 0" type="list-item" :count="3" />
      <ul v-else class="flex min-w-0 flex-col gap-1">
        <li
          v-for="(step, idx) in activeSteps"
          :key="idx"
          :class="STEP_ITEM_CLASSES"
          data-testid="work-steps-item"
        >
          {{ step }}
        </li>
      </ul>
    </div>
  </div>
</template>
