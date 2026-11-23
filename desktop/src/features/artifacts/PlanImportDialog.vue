<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiChip from '../../ui/UiChip.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import type { AppPage } from '../../shell/railNav'
import { useNavigation } from '../../shell/useNavigation'
import { ORIGIN_USER, type BacklogItem } from '../backlog/backlogTypes'
import { trapFocus } from '../backlog/dialogFocus'
import PlanBreakdown from '../backlog/PlanBreakdown.vue'
import { breakdownOf } from '../backlog/planBreakdown'
import { parseProposal, type ParsedProposal } from '../backlog/planProposal'
import { useBacklog } from '../backlog/useBacklog'
import { PLAN_LINE_GENERATING, PLAN_STEPS, useItemPlan } from '../backlog/useItemPlan'
import { useProjects } from '../projects/useProjects'
import { agentName, DEFAULT_RECIPIENT } from '../team/agents'
import type { Agent } from '../team/events'
import { useConnection } from '../team/useConnection'
import { useRoster } from '../team/useRoster'
import { pickPlanFile } from './artifactApi'
import {
  IMPORT_ACCEPT_LABEL,
  IMPORT_AGENT_LABEL,
  IMPORT_ANSWER_LABEL,
  IMPORT_ANSWER_TEXT,
  IMPORT_BACK_LABEL,
  IMPORT_CANCEL_LABEL,
  IMPORT_CONTINUE_LABEL,
  IMPORT_DELETE_CARD_CONFIRM,
  IMPORT_DELETE_CARD_LABEL,
  IMPORT_DELETE_CARD_MESSAGE,
  IMPORT_DELETE_CARD_WARNING,
  IMPORT_FREE_TEXT,
  IMPORT_KEEP_CARD_LABEL,
  IMPORT_NO_FILE_TEXT,
  IMPORT_OPEN_PLAN_LABEL,
  IMPORT_PASTE_LABEL,
  IMPORT_PASTE_PLACEHOLDER,
  IMPORT_PICK_LABEL,
  IMPORT_RECOGNIZED_TEXT,
  IMPORT_SAVE_LABEL,
  IMPORT_SOURCE_FILE,
  IMPORT_SOURCE_LABEL,
  IMPORT_SOURCE_PASTE,
  IMPORT_TITLE,
  IMPORT_TITLE_LABEL,
  IMPORT_TRIAGE_LABEL,
  PLAN_RETRY_LABEL,
} from './artifactsCopy'
import { extractPlanBlock, titleFromText } from './planMarkdown'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiCloseButton from '../../ui/UiCloseButton.vue'

const OVERLAY_CLASSES = 'fixed inset-0 z-50 flex items-center justify-center bg-paper/70 p-4'
const FRAME_CLASSES = 'relative border border-line w-160 max-w-full rounded-md bg-surface shadow-paper'
// O backdrop tem p-4 (2rem no total): o diálogo rola por dentro em vez de passar da janela.
const DIALOG_CLASSES = 'flex max-h-[calc(100dvh-2rem)] flex-col gap-3 overflow-y-auto p-5'
const FILE_SOURCE = 'arquivo'
const PASTE_SOURCE = 'colar'
const ORIGIN_STEP = 'origem'
const TRIAGE_STEP = 'triagem'
const BACKLOG_PAGE: AppPage = 'backlog'
// O card de prévia só existe para breakdownOf montar os passos; nada dele é gravado.
const PREVIEW_ITEM_ID = 'importado'
const PASTE_ROWS = 10
const FIRST_FIELD_SELECTOR = 'button, textarea, input'
const LABEL_CLASSES = 'text-label font-medium text-muted'
const CONTROL_CLASSES =
  `w-full min-w-0 rounded-md border border-line bg-surface px-3 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`
const STATUS_CLASSES = 'text-small font-semibold text-ink'

type Source = typeof FILE_SOURCE | typeof PASTE_SOURCE
type Step = typeof ORIGIN_STEP | typeof TRIAGE_STEP

const emit = defineEmits<{ close: []; done: [itemId: string] }>()

const backlog = useBacklog()
const itemPlan = useItemPlan()
const projects = useProjects()
const roster = useRoster()
const connection = useConnection()
const navigation = useNavigation()
const titleId = useId()
const dialog = ref<HTMLElement>()
let previousFocus: HTMLElement | null = null

const sourceOptions = [
  { label: IMPORT_SOURCE_FILE, value: FILE_SOURCE },
  { label: IMPORT_SOURCE_PASTE, value: PASTE_SOURCE },
] as const

const source = ref<Source>(FILE_SOURCE)
const step = ref<Step>(ORIGIN_STEP)
const fileName = ref('')
const fileText = ref('')
const pasted = ref('')
const title = ref('')
const titleEdited = ref(false)
const agent = ref<Agent>(DEFAULT_RECIPIENT)
const createdId = ref<string | null>(null)
const busy = ref(false)
const error = ref<string | null>(null)
const askingDelete = ref(false)
const expandedSteps = ref(new Set<string>())

const text = computed(() => (source.value === FILE_SOURCE ? fileText.value : pasted.value))
const block = computed(() => extractPlanBlock(text.value))
const canContinue = computed(() => text.value.trim() !== '' && title.value.trim() !== '')
const plan = computed(() => (createdId.value === null ? null : itemPlan.planOf(createdId.value)))
const createdItem = computed(() => backlog.items.find((item) => item.id === createdId.value))
const triaging = computed(() => plan.value?.state === 'gerando')
const proposed = computed(() => plan.value?.state === 'proposto')
const triageBlocked = computed(() => busy.value || connection.checking)
const pendingQuestion = computed(() => createdId.value !== null && itemPlan.hasPendingQuestion(createdId.value))
const chosenAgentName = computed(() => roster.profileOf(agent.value)?.name ?? agentName(agent.value))

function previewItem(): BacklogItem {
  return {
    id: PREVIEW_ITEM_ID,
    title: title.value,
    type: 'task',
    state: 'proposto',
    tags: [],
    who: '',
    by: '',
    origin: ORIGIN_USER,
    branch: null,
    worktreePath: null,
  }
}

const exactBreakdown = computed(() =>
  block.value === undefined ? undefined : breakdownOf(previewItem(), parseProposal(block.value), []),
)
const proposedBreakdown = computed(() => {
  const item = createdItem.value
  const proposal: ParsedProposal | undefined = item ? itemPlan.proposalFor(item) : undefined
  return item && proposal ? breakdownOf(item, proposal, []) : undefined
})

watch(text, (next) => {
  if (!titleEdited.value) title.value = titleFromText(next, fileName.value)
})

watch(
  () => projects.selected?.id,
  () => emit('close'),
)

onMounted(async () => {
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
  await nextTick()
  dialog.value?.querySelector<HTMLElement>(FIRST_FIELD_SELECTOR)?.focus()
})

onBeforeUnmount(() => {
  previousFocus?.focus()
})

function messageOf(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause)
}

async function run(action: () => Promise<void>): Promise<void> {
  if (busy.value) return
  busy.value = true
  error.value = null
  try {
    await action()
  } catch (cause) {
    error.value = messageOf(cause)
  } finally {
    busy.value = false
  }
}

function pick(): Promise<void> {
  return run(async () => {
    const file = await pickPlanFile()
    if (!file) return
    fileName.value = file.name
    fileText.value = file.text
  })
}

function saveExact(): Promise<void> {
  const protocol = block.value
  if (protocol === undefined) return Promise.resolve()
  return run(async () => {
    const item = await itemPlan.importExact(title.value.trim(), protocol)
    emit('done', item.id)
  })
}

function triage(): Promise<void> {
  if (triageBlocked.value) return Promise.resolve()
  return run(async () => {
    const id = await itemPlan.importFree(title.value.trim(), text.value, agent.value, createdId.value ?? undefined)
    if (id !== undefined) createdId.value = id
  })
}

function answerInCard(): void {
  if (createdId.value === null) return
  backlog.open(createdId.value)
  emit('close')
  navigation.goTo(BACKLOG_PAGE)
}

function accept(): void {
  if (createdId.value === null) return
  itemPlan.accept(createdId.value)
  emit('done', createdId.value)
}

function cancelTriage(): void {
  if (createdId.value === null) return
  itemPlan.cancel(createdId.value)
  askingDelete.value = true
}

function deleteCard(): void {
  if (createdId.value !== null) backlog.remove(createdId.value)
  emit('close')
}

function requestClose(): void {
  if (busy.value) return
  if (triaging.value) {
    cancelTriage()
    return
  }
  emit('close')
}
</script>

<template>
  <div :class="OVERLAY_CLASSES" @click.self="requestClose">
    <div :class="FRAME_CLASSES">
      <UiCloseButton class="absolute right-3 top-3 z-10" @press="requestClose" />
    <section
      ref="dialog"
      data-testid="plan-import"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      tabindex="-1"
      :class="DIALOG_CLASSES"
      @keydown.esc.stop="requestClose"
      @keydown.tab="trapFocus($event, dialog)"
    >
      <b :id="titleId" class="text-control text-ink">{{ IMPORT_TITLE }}</b>

      <template v-if="step === ORIGIN_STEP">
        <UiSegmented v-model="source" :options="sourceOptions" :label="IMPORT_SOURCE_LABEL" />

        <div v-if="source === FILE_SOURCE" class="flex min-w-0 flex-wrap items-center gap-3">
          <UiButton variant="quiet" size="lg" data-testid="plan-import-pick" :disabled="busy" @press="pick">
            {{ IMPORT_PICK_LABEL }}
          </UiButton>
          <span class="min-w-0 flex-1 truncate text-caption text-muted">{{ fileName || IMPORT_NO_FILE_TEXT }}</span>
        </div>
        <label v-else class="flex flex-col gap-1.5">
          <span :class="LABEL_CLASSES">{{ IMPORT_PASTE_LABEL }}</span>
          <textarea
            v-model="pasted"
            data-testid="plan-import-text"
            :rows="PASTE_ROWS"
            spellcheck="false"
            :placeholder="IMPORT_PASTE_PLACEHOLDER"
            :class="[CONTROL_CLASSES, 'py-2 font-mono text-small']"
          />
        </label>

        <label class="flex flex-col gap-1.5">
          <span :class="LABEL_CLASSES">{{ IMPORT_TITLE_LABEL }}</span>
          <input
            v-model="title"
            data-testid="plan-import-title"
            :class="[CONTROL_CLASSES, 'h-9']"
            @input="titleEdited = true"
          />
        </label>
      </template>

      <template v-else-if="block !== undefined">
        <p :class="STATUS_CLASSES" data-testid="plan-import-recognized">{{ IMPORT_RECOGNIZED_TEXT }}</p>
        <PlanBreakdown
          v-if="exactBreakdown && exactBreakdown.steps.length"
          v-model:expanded="expandedSteps"
          :breakdown="exactBreakdown"
          :leader="agentName(DEFAULT_RECIPIENT)"
        />
      </template>

      <template v-else>
        <p :class="STATUS_CLASSES" data-testid="plan-import-free">{{ IMPORT_FREE_TEXT }}</p>

        <div v-if="createdId === null || (!triaging && !proposed && !askingDelete)" class="flex flex-col gap-1.5">
          <p class="text-label font-semibold text-ink">{{ IMPORT_AGENT_LABEL }}</p>
          <div class="flex flex-wrap gap-1.5">
            <UiChip
              v-for="profile in roster.agents"
              :key="profile.id"
              :label="profile.name"
              :selected="agent === profile.id"
              @press="agent = profile.id"
            />
          </div>
        </div>

        <div v-if="triaging" class="flex flex-col gap-2" data-testid="plan-import-triaging">
          <p class="text-small text-muted">{{ chosenAgentName }} · {{ PLAN_LINE_GENERATING }}</p>
          <ol class="flex flex-col gap-1 pl-1 text-caption text-muted">
            <li v-for="planStep in PLAN_STEPS" :key="planStep">○ {{ planStep }}</li>
          </ol>
          <div v-if="pendingQuestion" class="flex min-w-0 flex-wrap items-center gap-3 rounded-md bg-surface-2 py-2 pl-3.5 pr-2 text-caption text-ink">
            <p class="min-w-0 flex-1">{{ IMPORT_ANSWER_TEXT }}</p>
            <UiButton variant="quiet" size="sm" data-testid="plan-import-answer" @press="answerInCard">{{ IMPORT_ANSWER_LABEL }}</UiButton>
          </div>
        </div>

        <PlanBreakdown
          v-else-if="proposed && proposedBreakdown"
          v-model:expanded="expandedSteps"
          :breakdown="proposedBreakdown"
          :leader="agentName(DEFAULT_RECIPIENT)"
        />

        <div
          v-if="!triaging && plan?.error"
          class="flex min-w-0 flex-wrap items-center gap-3 rounded-md bg-surface-2 py-2 pl-3.5 pr-2 text-caption text-ink"
          role="alert"
        >
          <p class="min-w-0 flex-1">{{ plan.error }}</p>
          <UiButton variant="quiet" size="sm" data-testid="plan-import-retry" :disabled="triageBlocked" @press="triage">
            {{ PLAN_RETRY_LABEL }}
          </UiButton>
        </div>
      </template>

      <UiNotice v-if="error" :text="error" role="alert" @close="error = null" />

      <div v-if="askingDelete" class="flex min-w-0 flex-wrap items-center gap-2" role="alert">
        <p class="min-w-0 flex-1 text-caption text-muted">{{ IMPORT_DELETE_CARD_MESSAGE }}</p>
        <UiButton variant="quiet" size="sm" data-testid="plan-import-keep" @press="emit('close')">{{ IMPORT_KEEP_CARD_LABEL }}</UiButton>
        <UiConfirm
          :label="IMPORT_DELETE_CARD_LABEL"
          :message="IMPORT_DELETE_CARD_WARNING"
          :confirm-label="IMPORT_DELETE_CARD_CONFIRM"
          @confirm="deleteCard"
        />
      </div>

      <div v-else class="flex flex-wrap justify-end gap-2">
        <UiButton
          v-if="step === TRIAGE_STEP && createdId === null"
          variant="quiet"
          size="lg"
          class="mr-auto"
          :disabled="busy"
          @press="step = ORIGIN_STEP"
        >
          {{ IMPORT_BACK_LABEL }}
        </UiButton>
        <UiButton variant="quiet" size="lg" data-testid="plan-import-cancel" :disabled="busy" @press="requestClose">
          {{ IMPORT_CANCEL_LABEL }}
        </UiButton>

        <UiButton
          v-if="step === ORIGIN_STEP"
          variant="primary"
          size="lg"
          data-testid="plan-import-continue"
          :disabled="!canContinue || busy"
          @press="step = TRIAGE_STEP"
        >
          {{ IMPORT_CONTINUE_LABEL }}
        </UiButton>
        <UiButton
          v-else-if="block !== undefined"
          variant="primary"
          size="lg"
          data-testid="plan-import-save"
          :disabled="busy"
          @press="saveExact"
        >
          {{ IMPORT_SAVE_LABEL }}
        </UiButton>
        <template v-else-if="proposed && createdId !== null">
          <UiButton variant="quiet" size="lg" data-testid="plan-import-open" @press="emit('done', createdId)">
            {{ IMPORT_OPEN_PLAN_LABEL }}
          </UiButton>
          <UiButton variant="approve" size="lg" data-testid="plan-import-accept" @press="accept">{{ IMPORT_ACCEPT_LABEL }}</UiButton>
        </template>
        <UiButton
          v-else-if="createdId === null"
          variant="primary"
          size="lg"
          data-testid="plan-import-triage"
          :disabled="triageBlocked"
          @press="triage"
        >
          {{ IMPORT_TRIAGE_LABEL(chosenAgentName) }}
        </UiButton>
      </div>
    </section>
    </div>
  </div>
</template>
