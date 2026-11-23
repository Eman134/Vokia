<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSelect from '../../ui/UiSelect.vue'
import type { BacklogItem } from '../backlog/backlogTypes'
import { trapFocus } from '../backlog/dialogFocus'
import { joinTasks, parseProposal, serializeProposal, splitTasks, type ParsedProposal, type ProposalLot } from '../backlog/planProposal'
import { normalizeTag } from '../backlog/tags'
import { useItemPlan } from '../backlog/useItemPlan'
import { useProjects } from '../projects/useProjects'
import { useRoster } from '../team/useRoster'
import {
  fetchPlanArtifact,
  isArtifactConflict,
  startedStageOf,
  type PlanArtifactDetail,
} from './artifactApi'
import {
  EDITOR_CANCEL_LABEL,
  EDITOR_DISCARD_LABEL,
  EDITOR_KEEP_LABEL,
  EDITOR_RELOAD_LABEL,
  EDITOR_SAVE_LABEL,
  EDITOR_SAVING_LABEL,
  EDITOR_TAGS_LABEL,
  EDITOR_TAGS_PLACEHOLDER,
  PLAN_EDITOR_ADD_STAGE_LABEL,
  PLAN_EDITOR_CONFLICT_MESSAGE,
  PLAN_EDITOR_CONTEXT_LABEL,
  PLAN_EDITOR_DISCARD_MESSAGE,
  PLAN_EDITOR_DONE_LABEL,
  PLAN_EDITOR_EMPTY_STAGE_ERROR,
  PLAN_EDITOR_ESTIMATE_LABEL,
  PLAN_EDITOR_ESTIMATE_PLACEHOLDER,
  PLAN_EDITOR_FILES_LABEL,
  PLAN_EDITOR_LIMITS_LABEL,
  PLAN_EDITOR_MOVE_DOWN_LABEL,
  PLAN_EDITOR_MOVE_UP_LABEL,
  PLAN_EDITOR_NO_STAGES_ERROR,
  PLAN_EDITOR_REMOVE_STAGE_LABEL,
  PLAN_EDITOR_RISK_LABEL,
  PLAN_EDITOR_SCRIPT_LABEL,
  PLAN_EDITOR_STAGE_GOAL_LABEL,
  PLAN_EDITOR_STAGE_TASKS_LABEL,
  PLAN_EDITOR_STAGE_TITLE_LABEL,
  PLAN_EDITOR_STAGES_LABEL,
  PLAN_EDITOR_STARTED_HINT,
  PLAN_EDITOR_STEPS_LABEL,
  PLAN_EDITOR_SUMMARY_LABEL,
  PLAN_EDITOR_TITLE,
  PLAN_EDITOR_WHO_LABEL,
  PLAN_EDITOR_WHO_NONE,
} from './artifactsCopy'
import { isStageStarted, matchStages } from './planProgress'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiCloseButton from '../../ui/UiCloseButton.vue'

const OVERLAY_CLASSES = 'fixed inset-0 z-50 flex items-center justify-center bg-paper/70 p-4'
const FRAME_CLASSES = 'relative border border-line w-200 max-w-full rounded-md bg-surface shadow-paper'
// O backdrop tem p-4 (2rem no total): o diálogo rola por dentro em vez de passar da janela.
const DIALOG_CLASSES = 'flex max-h-[calc(100dvh-2rem)] flex-col gap-3 overflow-y-auto p-5'
interface StageDraft {
  key: string
  // Card da etapa no plano salvo, casado como o backend faz: título primeiro, posição depois.
  card?: BacklogItem
  title: string
  goal: string
  tasks: string
  scriptOpen: boolean
  who: string
  context: string
  files: string
  steps: string
  done: string
  limits: string
}

const LIST_SEPARATOR = ','
const LIST_JOINER = ', '
const LINE_SEPARATOR = '\n'
const FIRST_FIELD_SELECTOR = 'textarea, input'
const TEXTAREA_ROWS = 3
const SUMMARY_ROWS = 4
const LABEL_CLASSES = 'text-label font-medium text-muted'
const CONTROL_CLASSES =
  `w-full min-w-0 rounded-md border border-line bg-surface px-3 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`
const INPUT_CLASSES = `${CONTROL_CLASSES} h-9`
const TEXTAREA_CLASSES = `${CONTROL_CLASSES} py-2`
const STAGE_CLASSES = 'flex min-w-0 flex-col gap-2.5 rounded-md border p-3'
const STAGE_MARK_CLASSES =
  'grid size-5.5 flex-none place-items-center rounded-full border-[1.5px] border-line text-tiny font-bold text-muted'

const props = defineProps<{ item: BacklogItem; artifact: PlanArtifactDetail; stages: BacklogItem[] }>()
const emit = defineEmits<{ close: []; saved: [artifact: PlanArtifactDetail] }>()

const itemPlan = useItemPlan()
const projects = useProjects()
const roster = useRoster()
const titleId = useId()
const dialog = ref<HTMLElement>()
let previousFocus: HTMLElement | null = null

function lines(text: string): string[] {
  return text
    .split(LINE_SEPARATOR)
    .map((line) => line.trim())
    .filter((line) => line !== '')
}

function listOf(text: string): string[] {
  return text
    .split(LIST_SEPARATOR)
    .map((value) => value.trim())
    .filter((value) => value !== '')
}

function textToTags(text: string): string[] {
  return [...new Set(text.split(LIST_SEPARATOR).flatMap((raw) => normalizeTag(raw) ?? []))]
}

function draftOf(lot: ProposalLot, card: BacklogItem | undefined): StageDraft {
  const block = lot.script?.block
  return {
    key: crypto.randomUUID(),
    card,
    title: lot.title,
    goal: lot.meta,
    tasks: splitTasks(lot.tasks).join(LINE_SEPARATOR),
    scriptOpen: false,
    who: lot.script?.who ?? '',
    context: block?.context ?? '',
    files: block?.files.join(LIST_JOINER) ?? '',
    steps: block?.steps.join(LINE_SEPARATOR) ?? '',
    done: block?.done.join(LINE_SEPARATOR) ?? '',
    limits: block?.limits.join(LINE_SEPARATOR) ?? '',
  }
}

function emptyDraft(): StageDraft {
  return {
    key: crypto.randomUUID(),
    title: '',
    goal: '',
    tasks: '',
    scriptOpen: false,
    who: '',
    context: '',
    files: '',
    steps: '',
    done: '',
    limits: '',
  }
}

function lotOf(stage: StageDraft, index: number): ProposalLot {
  const block = {
    context: stage.context.trim(),
    files: listOf(stage.files),
    steps: lines(stage.steps),
    done: lines(stage.done),
    limits: lines(stage.limits),
  }
  const who = stage.who.trim()
  const hasScript =
    who !== '' || block.context !== '' || [block.files, block.steps, block.done, block.limits].some((list) => list.length > 0)
  return {
    n: index + 1,
    title: stage.title.trim(),
    meta: stage.goal.trim(),
    tasks: joinTasks(lines(stage.tasks)),
    ...(hasScript ? { script: { ...(who ? { who } : {}), block } } : {}),
  }
}

const summary = ref('')
const estimate = ref('')
const risk = ref('')
const tagsText = ref('')
const drafts = ref<StageDraft[]>([])
const baseline = ref({ updatedAt: props.artifact.updatedAt, snapshot: '' })
const saving = ref(false)
const error = ref<string | null>(null)
const conflict = ref(false)
const startedId = ref<string | null>(null)
const askingDiscard = ref(false)

function proposalOf(): ParsedProposal {
  return {
    estimate: estimate.value.trim(),
    lots: drafts.value.map(lotOf),
    risk: risk.value.trim(),
    tags: textToTags(tagsText.value),
    summary: summary.value.trim(),
  }
}

function fill(detail: string, updatedAt: number): void {
  const parsed = parseProposal(detail)
  summary.value = parsed.summary
  estimate.value = parsed.estimate
  risk.value = parsed.risk
  tagsText.value = parsed.tags.join(LIST_JOINER)
  const cards = matchStages(props.item.id, props.stages, parsed.lots.map((lot) => lot.title))
  drafts.value = parsed.lots.map((lot, index) => draftOf(lot, cards[index]))
  baseline.value = { updatedAt, snapshot: JSON.stringify(proposalOf()) }
}

fill(props.artifact.detail, props.artifact.updatedAt)

const dirty = computed(() => JSON.stringify(proposalOf()) !== baseline.value.snapshot)

// O card vem da lista viva do backlog: a coluna reflete o que andou depois de o editor abrir.
function cardOf(stage: StageDraft): BacklogItem | undefined {
  return stage.card && props.stages.find((candidate) => candidate.id === stage.card!.id)
}

function isStarted(stage: StageDraft): boolean {
  const card = cardOf(stage)
  return card !== undefined && isStageStarted(card)
}

function isRefused(stage: StageDraft): boolean {
  return stage.card !== undefined && startedId.value === stage.card.id
}

function whoOptions(stage: StageDraft): { value: string; label: string }[] {
  const options = [{ value: '', label: PLAN_EDITOR_WHO_NONE }, ...roster.agents.map((agent) => ({ value: agent.id, label: agent.name }))]
  const who = stage.who.trim()
  return who === '' || options.some((option) => option.value === who) ? options : [...options, { value: who, label: who }]
}

function addStage(): void {
  drafts.value = [...drafts.value, emptyDraft()]
}

function removeStage(index: number): void {
  drafts.value = drafts.value.filter((_, current) => current !== index)
}

function moveStage(index: number, offset: number): void {
  const target = index + offset
  if (target < 0 || target >= drafts.value.length) return
  const next = [...drafts.value]
  ;[next[index], next[target]] = [next[target], next[index]]
  drafts.value = next
}

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

function requestClose(): void {
  if (saving.value) return
  if (dirty.value) {
    askingDiscard.value = true
    return
  }
  emit('close')
}

function validationError(): string | null {
  if (drafts.value.length === 0) return PLAN_EDITOR_NO_STAGES_ERROR
  if (drafts.value.some((stage) => stage.title.trim() === '')) return PLAN_EDITOR_EMPTY_STAGE_ERROR
  return null
}

async function submit(): Promise<void> {
  if (saving.value) return
  const invalid = validationError()
  error.value = invalid
  conflict.value = false
  startedId.value = null
  if (invalid) return
  saving.value = true
  try {
    const fresh = await itemPlan.edit(props.item.id, serializeProposal(proposalOf()), baseline.value.updatedAt)
    emit('saved', fresh)
  } catch (cause) {
    const message = cause instanceof Error ? cause.message : String(cause)
    if (isArtifactConflict(message)) {
      conflict.value = true
      error.value = null
    } else {
      startedId.value = startedStageOf(message) ?? null
      error.value = message
    }
  } finally {
    saving.value = false
  }
}

async function reload(): Promise<void> {
  const projectId = projects.selected?.id
  if (projectId === undefined) return
  try {
    const fresh = await fetchPlanArtifact(projectId, props.item.id)
    fill(fresh.detail, fresh.updatedAt)
    conflict.value = false
    error.value = null
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause)
  }
}
</script>

<template>
  <div :class="OVERLAY_CLASSES" @click.self="requestClose">
    <div :class="FRAME_CLASSES">
      <UiCloseButton class="absolute right-3 top-3 z-10" @press="requestClose" />
    <section
      ref="dialog"
      data-testid="plan-editor"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      tabindex="-1"
      :class="DIALOG_CLASSES"
      @keydown.esc.stop="requestClose"
      @keydown.tab="trapFocus($event, dialog)"
      @keydown.enter.ctrl.exact.prevent="submit"
      @keydown.enter.meta.exact.prevent="submit"
    >
      <b :id="titleId" class="text-control text-ink">{{ PLAN_EDITOR_TITLE }} · {{ item.title }}</b>

      <label class="flex flex-col gap-1.5">
        <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_SUMMARY_LABEL }}</span>
        <textarea v-model="summary" data-testid="plan-editor-summary" :rows="SUMMARY_ROWS" :class="TEXTAREA_CLASSES" />
      </label>

      <div class="grid gap-3 sm:grid-cols-2">
        <label class="flex flex-col gap-1.5">
          <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_ESTIMATE_LABEL }}</span>
          <input v-model="estimate" :placeholder="PLAN_EDITOR_ESTIMATE_PLACEHOLDER" :class="INPUT_CLASSES" />
        </label>
        <label class="flex flex-col gap-1.5">
          <span :class="LABEL_CLASSES">{{ EDITOR_TAGS_LABEL }}</span>
          <input v-model="tagsText" :placeholder="EDITOR_TAGS_PLACEHOLDER" :class="INPUT_CLASSES" />
        </label>
      </div>

      <label class="flex flex-col gap-1.5">
        <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_RISK_LABEL }}</span>
        <input v-model="risk" :class="INPUT_CLASSES" />
      </label>

      <div class="flex min-w-0 flex-col gap-2">
        <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_STAGES_LABEL }}</span>
        <ol class="flex flex-col gap-2.5">
          <li
            v-for="(stage, index) in drafts"
            :key="stage.key"
            data-testid="plan-editor-stage"
            :class="[STAGE_CLASSES, isRefused(stage) ? 'border-danger' : 'border-line']"
          >
            <div class="flex min-w-0 items-center gap-2">
              <span :class="STAGE_MARK_CLASSES">{{ index + 1 }}</span>
              <input
                v-model="stage.title"
                data-testid="plan-editor-stage-title"
                :aria-label="`${PLAN_EDITOR_STAGE_TITLE_LABEL} da etapa ${index + 1}`"
                :placeholder="PLAN_EDITOR_STAGE_TITLE_LABEL"
                :class="INPUT_CLASSES"
              />
              <UiButton
                variant="quiet"
                size="sm"
                :aria-label="PLAN_EDITOR_MOVE_UP_LABEL"
                :disabled="index === 0"
                @press="moveStage(index, -1)"
              >↑</UiButton>
              <UiButton
                variant="quiet"
                size="sm"
                :aria-label="PLAN_EDITOR_MOVE_DOWN_LABEL"
                :disabled="index === drafts.length - 1"
                @press="moveStage(index, 1)"
              >↓</UiButton>
              <UiButton
                variant="danger"
                size="sm"
                data-testid="plan-editor-remove"
                :disabled="isStarted(stage)"
                :title="isStarted(stage) ? PLAN_EDITOR_STARTED_HINT : undefined"
                @press="removeStage(index)"
              >{{ PLAN_EDITOR_REMOVE_STAGE_LABEL }}</UiButton>
            </div>

            <label class="flex flex-col gap-1.5">
              <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_STAGE_GOAL_LABEL }}</span>
              <input v-model="stage.goal" :class="INPUT_CLASSES" />
            </label>
            <label class="flex flex-col gap-1.5">
              <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_STAGE_TASKS_LABEL }}</span>
              <textarea v-model="stage.tasks" :rows="TEXTAREA_ROWS" :class="TEXTAREA_CLASSES" />
            </label>

            <button
              type="button"
              class="flex cursor-pointer items-center gap-1.5 self-start text-label font-medium text-muted hover:text-ink"
              :class="INK_FOCUS_CLASSES"
              :aria-expanded="stage.scriptOpen"
              @click="stage.scriptOpen = !stage.scriptOpen"
            >
              <span aria-hidden="true">{{ stage.scriptOpen ? '▾' : '▸' }}</span>{{ PLAN_EDITOR_SCRIPT_LABEL }}
            </button>
            <div v-if="stage.scriptOpen" class="flex flex-col gap-2.5 rounded-md bg-surface-2 p-3">
              <div class="flex flex-col gap-1.5">
                <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_WHO_LABEL }}</span>
                <UiSelect v-model="stage.who" :options="whoOptions(stage)" :label="PLAN_EDITOR_WHO_LABEL" />
              </div>
              <label class="flex flex-col gap-1.5">
                <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_CONTEXT_LABEL }}</span>
                <textarea v-model="stage.context" :rows="TEXTAREA_ROWS" :class="TEXTAREA_CLASSES" />
              </label>
              <label class="flex flex-col gap-1.5">
                <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_FILES_LABEL }}</span>
                <input v-model="stage.files" :class="INPUT_CLASSES" />
              </label>
              <label class="flex flex-col gap-1.5">
                <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_STEPS_LABEL }}</span>
                <textarea v-model="stage.steps" :rows="TEXTAREA_ROWS" :class="TEXTAREA_CLASSES" />
              </label>
              <label class="flex flex-col gap-1.5">
                <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_DONE_LABEL }}</span>
                <textarea v-model="stage.done" :rows="TEXTAREA_ROWS" :class="TEXTAREA_CLASSES" />
              </label>
              <label class="flex flex-col gap-1.5">
                <span :class="LABEL_CLASSES">{{ PLAN_EDITOR_LIMITS_LABEL }}</span>
                <textarea v-model="stage.limits" :rows="TEXTAREA_ROWS" :class="TEXTAREA_CLASSES" />
              </label>
            </div>
          </li>
        </ol>
        <div>
          <UiButton variant="quiet" size="sm" data-testid="plan-editor-add" @press="addStage">
            + {{ PLAN_EDITOR_ADD_STAGE_LABEL }}
          </UiButton>
        </div>
      </div>

      <div
        v-if="conflict"
        class="flex min-w-0 flex-wrap items-center gap-3 rounded-md bg-surface-2 py-2 pl-3.5 pr-2 text-caption text-ink"
        role="alert"
      >
        <p class="min-w-0 flex-1">{{ PLAN_EDITOR_CONFLICT_MESSAGE }}</p>
        <UiButton variant="quiet" size="sm" data-testid="plan-editor-reload" @press="reload">{{ EDITOR_RELOAD_LABEL }}</UiButton>
      </div>
      <UiNotice v-else-if="error" :text="error" role="alert" @close="error = null" />

      <div v-if="askingDiscard" class="flex min-w-0 flex-wrap items-center gap-2" role="alert">
        <p class="min-w-0 flex-1 text-caption text-muted">{{ PLAN_EDITOR_DISCARD_MESSAGE }}</p>
        <UiButton variant="quiet" size="sm" @press="askingDiscard = false">{{ EDITOR_KEEP_LABEL }}</UiButton>
        <UiButton variant="danger" size="sm" @press="emit('close')">{{ EDITOR_DISCARD_LABEL }}</UiButton>
      </div>
      <div v-else class="flex justify-end gap-2">
        <UiButton variant="quiet" size="lg" :disabled="saving" @press="requestClose">{{ EDITOR_CANCEL_LABEL }}</UiButton>
        <UiButton variant="primary" size="lg" data-testid="plan-editor-save" :disabled="saving" @press="submit">
          {{ saving ? EDITOR_SAVING_LABEL : EDITOR_SAVE_LABEL }}
        </UiButton>
      </div>
    </section>
    </div>
  </div>
</template>
