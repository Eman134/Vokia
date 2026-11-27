<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiMarker from '../../ui/UiMarker.vue'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import UiChip from '../../ui/UiChip.vue'
import {
  AUTO_CLOSED_LABEL,
  BADGE_DOT,
  CLOSED_BY_AUTOMATIC,
  colorVar,
  COLUMN_CHIP,
  DEFAULT_FEATURE_COLOR,
  DONE_COLUMN_NAME,
  FEATURE_COLOR,
  ORIGIN_LABEL,
  REOPEN_LABEL,
  REOPEN_MOTIVE_PLACEHOLDER,
  STATE_BADGE,
} from './backlogLabels'
import { returnTask } from './backlogApi'
import { useFeed } from '../team/useFeed'
import { useProjects } from '../projects/useProjects'
import { fetchPlanArtifact, setArtifactCriterion, type PlanArtifactDetail } from '../artifacts/artifactApi'
import ContextGraphModal from '../memory/ContextGraphModal.vue'
import { SUGGESTED_TAGS, TAG_MAX_LENGTH, tagLabel, tagMarkerTone } from './tags'
import {
  AUTO_COLUMNS_HINT,
  COLUMN_LABEL,
  CONVERSATIONS_LABEL,
  DEFAULT_COLUMN,
  DELEGATE_LABEL,
  isUnassigned,
  MAIN_ACTION_LABEL,
  UNASSIGNED_TEXT,
  MODAL_STATE_CHIP,
  MOVABLE_COLUMNS,
  OTHER_INPUT_LABEL,
  OTHER_OPTION,
  OTHER_PLACEHOLDER,
  PLAN_HINT,
  QUESTIONS_TITLE,
  READY_BLOCKED_HINT,
  READY_COLUMN,
  type BacklogColumn,
  type BacklogItem,
  type BacklogTag,
  type TriageQuestion,
} from './backlogTypes'
import AcceptanceCriteria from './AcceptanceCriteria.vue'
import LinkedTo from './LinkedTo.vue'
import PlanBreakdown from './PlanBreakdown.vue'
import { firstPendingStepId } from './breakdownAccordion'
import PlanPanel from './PlanPanel.vue'
import PlanSummaryCard from './PlanSummaryCard.vue'
import { stagesOf } from '../artifacts/planProgress'
import { breakdownOf } from './planBreakdown'
import { useBacklog } from './useBacklog'
import { useItemQuestions } from './useItemQuestions'
import { useItemPlan } from './useItemPlan'
import { useRunItem } from './useRunItem'
import { useTriage } from './useTriage'
import { trapFocus } from './dialogFocus'
import { useRoster } from '../team/useRoster'
import { useItemUsage } from '../team/useItemUsage'
import { formatCostUsd, formatThinkingDuration } from '../team/messageUsageFormat'
import { agentName, DEFAULT_RECIPIENT } from '../team/agents'
import { useNavigation } from '../../shell/useNavigation'
import { extractAttachments } from '../attachments/attachmentBlock'
import AttachmentPreviewList from '../attachments/AttachmentPreviewList.vue'
import MarkdownText from '../team/MarkdownText.vue'
import AgentPicker from '../agents/AgentPicker.vue'
import type { Agent } from '../team/events'
import { activityTime } from './activityTime'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'

type PendingConfirm = 'archive' | 'delete' | 'discard-plan' | null

const OVERLAY_CLASSES = 'fixed inset-0 z-50 flex items-center justify-center bg-paper/70 p-4'
const FRAME_CLASSES = 'w-215 max-w-full rounded-md border border-line bg-surface shadow-paper'
// O backdrop tem p-4 (2rem no total): o diálogo rola por dentro em vez de passar da janela.
const DIALOG_CLASSES = 'flex max-h-[calc(100dvh-2rem)] flex-col overflow-hidden'
const OTHER_INPUT_CLASSES = 'h-9 w-full rounded-md border border-line bg-surface px-3 text-control text-ink placeholder:text-faint'
const HEADER_PILL_CLASSES = `inline-flex cursor-pointer items-center gap-1 rounded-sm border border-line bg-surface-2 px-2 py-0.5 text-tiny font-medium text-muted hover:text-ink ${INK_FOCUS_CLASSES}`
const POPOVER_CLASSES = 'absolute z-30 mt-1.5 rounded-md border border-line bg-surface p-1 shadow-paper'
const MENU_ITEM_CLASSES = `flex w-full cursor-pointer items-center gap-2 rounded-sm px-2.5 py-1.5 text-left text-caption text-ink hover:bg-hover disabled:cursor-not-allowed disabled:opacity-45 ${INK_FOCUS_CLASSES}`
const CONFIRM_OVERLAY_CLASSES = 'fixed inset-0 z-60 flex items-center justify-center bg-paper/70 p-4'
const CONFIRM_FRAME_CLASSES = 'relative w-100 max-w-full rounded-md border border-line bg-surface p-5 shadow-paper'
const TRIAGE_PLAN_TEXT = 'Plano sai da triagem'
const ARCHIVE_LABEL = 'Arquivar card'
const ARCHIVE_MESSAGE = 'Tira o card do quadro sem apagar o histórico. Você pode restaurá-lo depois.'
const ARCHIVE_CONFIRM_LABEL = 'Arquivar'
const DELETE_LABEL = 'Remover card'
const DELETE_MESSAGE = 'Remove o card definitivamente. Não dá para desfazer.'
const DELETE_CASCADE_MESSAGE = 'Este item tem um plano vinculado. Remover mesmo assim apagará o item, o plano e as etapas filhas.'
const DELETE_CONFIRM_LABEL = 'Remover'
const DELETE_CASCADE_CONFIRM_LABEL = 'Remover mesmo assim'
const DISCARD_PLAN_LABEL = 'Descartar plano'
const DISCARD_PLAN_MESSAGE = 'Apaga o plano proposto e volta o card para sem plano.'
const NO_LEADER_TEXT = 'Sem responsável · nenhum líder configurado'
const OPEN_THREAD_LABEL = 'Abrir thread'

const backlog = useBacklog()
const itemQuestions = useItemQuestions()
const itemPlan = useItemPlan()
const runItem = useRunItem()
const triage = useTriage()
const roster = useRoster()
const navigation = useNavigation()
const feed = useFeed()
const leaderName = computed(() => agentName(DEFAULT_RECIPIENT))
const reopening = ref(false)
const reopenMotive = ref('')
const titleId = useId()
const confirmTitleId = useId()
const dialog = ref<HTMLElement>()
const confirmDialog = ref<HTMLElement>()
const confirmCancelBtn = ref<{ $el: HTMLElement } | null>(null)
const statusMenuRef = ref<HTMLElement>()
const moreMenuRef = ref<HTMLElement>()
const statusMenuOpen = ref(false)
const moreMenuOpen = ref(false)
const pendingConfirm = ref<PendingConfirm>(null)
const stopping = ref(false)
const addingTag = ref(false)
const newTag = ref('')
const expandedMicro = ref(new Set<string>())
const parentStack = ref<string[]>([])
const answerDrafts = ref<Record<string, string>>({})
let previousFocus: HTMLElement | null = null
let confirmOrigin: HTMLElement | null = null

const item = computed(() => backlog.openItem)
const pendingQuestions = computed(() =>
  (item.value?.questions ?? []).filter((question) => !question.selected?.trim()),
)
const usage = useItemUsage(() => item.value?.id)
const descriptionContent = computed(() => extractAttachments(item.value?.description ?? ''))

// O motivo é obrigatório; o líder recebe a task de volta e o card sai de Done.
async function reopen(): Promise<void> {
  const projectId = backlog.loadedProjectId
  const current = item.value
  const motive = reopenMotive.value.trim()
  if (projectId === null || current === null || motive === '') return
  reopenMotive.value = ''
  reopening.value = false
  try {
    await returnTask(projectId, current.id, motive)
  } catch (error) {
    feed.feed.push({ type: 'error', message: error instanceof Error ? error.message : String(error) })
  }
}

function reopenManual(): void {
  if (!item.value) return
  backlog.moveTo(item.value.id, DEFAULT_COLUMN)
}

const column = computed(() => item.value?.column ?? DEFAULT_COLUMN)
const statusChip = computed(() => COLUMN_CHIP[column.value])
// Só o card concluído pela validação automática pode ser reaberto por aqui; o manual o usuário mesmo moveu.
const autoClosed = computed(() => column.value === DONE_COLUMN_NAME && item.value?.closedBy === CLOSED_BY_AUTOMATIC)
const readyBlocked = computed(() => !item.value || !itemPlan.isReviewed(item.value.id))
const delegateTargets = computed(() =>
  roster.agents.filter((profile) => profile.id !== (itemPlan.planOf(item.value?.id ?? '').agent ?? DEFAULT_RECIPIENT)),
)
const isActiveRun = computed(() =>
  item.value !== null && (runItem.isRunning(item.value.id) || column.value === 'In progress'),
)
const isPlanBusy = computed(() => planState.value === 'triagem' || planState.value === 'gerando')
const canDiscardPlan = computed(() =>
  item.value !== null && (Boolean(item.value.plan) || planState.value === 'proposto' || planState.value === 'vinculado'),
)
const suggestedTags = computed(() =>
  [...new Set([...SUGGESTED_TAGS, ...backlog.items.flatMap((current) => current.tags)])].filter(
    (tag) => !item.value?.tags.includes(tag),
  ),
)
const itemStages = computed(() => (item.value ? stagesOf(backlog.items, `plano-${item.value.id}`) : []))
const proposal = computed(() => (item.value ? itemPlan.proposalFor(item.value) : undefined))
const breakdown = computed(() =>
  item.value
    ? breakdownOf(item.value, proposal.value, itemStages.value)
    : undefined,
)
const planState = computed(() => (item.value ? itemPlan.stateFor(item.value) : 'vazio'))
const planArtifact = ref<PlanArtifactDetail | null>(null)
let planArtifactSeq = 0

async function loadPlanArtifact(): Promise<void> {
  const current = item.value
  const projectId = backlog.loadedProjectId
  if (!current || projectId === null || (planState.value !== 'proposto' && planState.value !== 'vinculado')) {
    planArtifact.value = null
    return
  }
  const seq = (planArtifactSeq += 1)
  try {
    const fresh = await fetchPlanArtifact(projectId, current.id)
    if (seq === planArtifactSeq && item.value?.id === current.id) {
      planArtifact.value = fresh
    }
  } catch {
    if (seq === planArtifactSeq) planArtifact.value = null
  }
}

watch(
  [() => item.value?.id, planState, () => backlog.items],
  () => {
    void loadPlanArtifact()
  },
  { immediate: true },
)

async function togglePlanCriterion(criterionId: string): Promise<void> {
  const projectId = backlog.loadedProjectId
  const current = planArtifact.value
  const target = current?.criteria.find((c) => c.id === criterionId)
  if (projectId === null || !current || !target) return
  const done = !target.done
  target.done = done
  try {
    await setArtifactCriterion(projectId, current.id, criterionId, done)
  } catch {
    target.done = !done
  }
}

// Sem plano ou com a proposta esperando o usuário, o selo só repetiria o que o conteúdo do modal já mostra.
const showStateBadge = computed(() => planState.value !== 'vazio' && planState.value !== 'proposto')
const planText = computed(() => (planState.value === 'triagem' ? TRIAGE_PLAN_TEXT : undefined))

function responsibleLabel(current: BacklogItem): string {
  if (!isUnassigned(current)) return roster.nameOf(current.who)
  return triage.isWithoutLeader(current.id) ? NO_LEADER_TEXT : UNASSIGNED_TEXT
}

const openTasksCountByAgent = computed<Record<string, number>>(() => {
  const counts: Record<string, number> = {}
  for (const current of backlog.items) {
    if ((current.column ?? DEFAULT_COLUMN) === DONE_COLUMN_NAME) continue
    if (!isUnassigned(current)) {
      counts[current.who] = (counts[current.who] ?? 0) + 1
    }
  }
  return counts
})

function openTasksLabel(count: number): string {
  return `${count} ${count === 1 ? 'task aberta' : 'tasks abertas'}`
}

function onAssignOwner(agentId: Agent | null): void {
  if (!item.value || !agentId) return
  backlog.assignOwner(item.value.id, agentId)
}

function onDelegateTo(agentId: Agent | null): void {
  if (!item.value || !agentId) return
  void runItem.run(item.value.id, agentId)
}

const details = computed(() => {
  const current = item.value
  if (!current) return []
  const rows: [string, string | undefined][] = [
    ['Tipo', current.type === 'feature' ? 'Feature' : 'Tarefa'],
    ['Aberto por', current.by],
    ['Origem', ORIGIN_LABEL[current.origin]],
    ['Feature', current.feature],
    ['Prioridade', current.priority],
    ['Estimativa', current.estimate],
    ['Custo', usage.value.turns > 0 ? (formatCostUsd(usage.value.costUsd) ?? undefined) : undefined],
    ['Tempo', usage.value.workMs > 0 ? (formatThinkingDuration(usage.value.workMs) ?? undefined) : undefined],
  ]
  return rows.filter((row): row is [string, string] => Boolean(row[1]))
})

// O botão acionado sai do DOM com a ação; sem isso o foco cai no body e Esc/Tab deixam de valer.
function addTag(raw: string): void {
  dialog.value?.focus()
  backlog.addTags(item.value!.id, [raw])
}

function removeTag(tag: BacklogTag): void {
  dialog.value?.focus()
  backlog.removeTag(item.value!.id, tag)
}

function submitNewTag(): void {
  addTag(newTag.value)
  newTag.value = ''
}

function moveTo(target: BacklogColumn): void {
  statusMenuOpen.value = false
  if (target === READY_COLUMN && readyBlocked.value) return
  dialog.value?.focus()
  backlog.moveTo(item.value!.id, target)
}

function onMainAction(): void {
  if (!item.value) return
  if (planState.value === 'proposto') {
    itemPlan.accept(item.value.id)
  }
  void runItem.run(item.value.id)
}

async function stopActiveWork(): Promise<void> {
  if (!item.value || stopping.value) return
  stopping.value = true
  try {
    if (isPlanBusy.value) {
      itemPlan.cancel(item.value.id)
    }
    if (isActiveRun.value) {
      await runItem.stop(item.value.id)
    }
  } finally {
    stopping.value = false
  }
}

function openPlanArtifact(): void {
  const id = item.value?.id
  closeModal()
  if (id) navigation.openPlanArtifact(id)
}

function openStage(stageId: string): void {
  const currentId = item.value?.id
  if (!currentId) return
  parentStack.value = [...parentStack.value, currentId]
  backlog.open(stageId)
}

function goBackToParent(): void {
  const parentId = parentStack.value[parentStack.value.length - 1]
  if (!parentId) return
  parentStack.value = parentStack.value.slice(0, -1)
  backlog.open(parentId)
}

function isOtherAnswer(question: TriageQuestion): boolean {
  return question.selected !== undefined && !question.options.includes(question.selected)
}

function otherText(question: TriageQuestion): string {
  return answerDrafts.value[question.id] ?? (question.selected === OTHER_OPTION ? '' : (question.selected ?? ''))
}

// O change pode chegar depois do fechamento, quando item.value já é null; closeModal já salvou o texto.
function saveOther(question: TriageQuestion): void {
  const current = item.value
  const text = otherText(question).trim()
  if (current && text !== '') itemQuestions.answer(current.id, question.id, text)
}

// Com texto livre já salvo, "Outro…" só leva o foco ao campo; regravar o marcador apagaria a resposta.
function pickOption(question: TriageQuestion, option: string): void {
  if (option === OTHER_OPTION && isOtherAnswer(question)) {
    dialog.value?.querySelector<HTMLInputElement>(`input[data-other-for="${CSS.escape(question.id)}"]`)?.focus()
    return
  }
  itemQuestions.answer(item.value!.id, question.id, option)
}

const graphModalOpen = ref(false)
const projects = useProjects()

const taskGraphContext = computed(() => {
  const current = item.value
  if (!current) return ''
  return [
    `${current.publicId ?? current.id} — ${current.title}`,
    current.description ? `\nDescrição:\n${current.description}` : '',
    current.planNote ? `\nNota do plano:\n${current.planNote}` : '',
  ]
    .filter(Boolean)
    .join('\n')
})

function closeMenus(): void {
  statusMenuOpen.value = false
  moreMenuOpen.value = false
}

function onEscape(): void {
  if (statusMenuOpen.value || moreMenuOpen.value) {
    closeMenus()
    dialog.value?.focus()
    return
  }
  closeModal()
}

function closeModal(): void {
  closeMenus()
  item.value?.questions?.filter(isOtherAnswer).forEach((question) => saveOther(question))
  backlog.close()
}

function openThread(): void {
  if (!item.value) return
  feed.showScreen({ kind: 'thread', itemId: item.value.id })
  backlog.close()
  navigation.goTo('conversas')
}

function openConversations(): void {
  closeModal()
  navigation.goTo('conversas')
}

function openInTasks(): void {
  const id = item.value?.id
  if (!id) return
  navigation.openCard(id)
}

async function openConfirm(kind: Exclude<PendingConfirm, null>): Promise<void> {
  closeMenus()
  confirmOrigin = document.activeElement instanceof HTMLElement ? document.activeElement : null
  pendingConfirm.value = kind
  await nextTick()
  confirmCancelBtn.value?.$el.focus()
}

function closeConfirm(): void {
  pendingConfirm.value = null
  confirmOrigin?.focus()
  confirmOrigin = null
}

async function acceptConfirm(): Promise<void> {
  const kind = pendingConfirm.value
  const current = item.value
  closeConfirm()
  if (!kind || !current) return
  if (kind === 'discard-plan') {
    itemPlan.discard(current.id)
    return
  }
  if (isPlanBusy.value) {
    itemPlan.cancel(current.id)
  }
  if (isActiveRun.value) {
    await runItem.stop(current.id)
  }
  if (kind === 'archive') {
    closeModal()
    backlog.archive(current.id)
  } else if (kind === 'delete') {
    const cascade = Boolean(current.plan)
    closeModal()
    backlog.remove(current.id, cascade)
  }
}

const confirmMeta = computed(() => {
  const current = item.value
  switch (pendingConfirm.value) {
    case 'archive':
      return {
        title: ARCHIVE_LABEL,
        message: ARCHIVE_MESSAGE,
        confirmLabel: ARCHIVE_CONFIRM_LABEL,
        variant: 'primary' as const,
      }
    case 'delete':
      return {
        title: DELETE_LABEL,
        message: current?.plan ? DELETE_CASCADE_MESSAGE : DELETE_MESSAGE,
        confirmLabel: current?.plan ? DELETE_CASCADE_CONFIRM_LABEL : DELETE_CONFIRM_LABEL,
        variant: 'danger' as const,
      }
    case 'discard-plan':
      return {
        title: DISCARD_PLAN_LABEL,
        message: DISCARD_PLAN_MESSAGE,
        confirmLabel: DISCARD_PLAN_LABEL,
        variant: 'danger' as const,
      }
    default:
      return null
  }
})

function onDocumentPointerDown(event: PointerEvent): void {
  const target = event.target as Node | null
  if (statusMenuOpen.value && statusMenuRef.value && !statusMenuRef.value.contains(target)) {
    statusMenuOpen.value = false
  }
  if (moreMenuOpen.value && moreMenuRef.value && !moreMenuRef.value.contains(target)) {
    moreMenuOpen.value = false
  }
}

watch(
  () => statusMenuOpen.value || moreMenuOpen.value,
  (open) => {
    if (open) {
      document.addEventListener('pointerdown', onDocumentPointerDown)
    } else {
      document.removeEventListener('pointerdown', onDocumentPointerDown)
    }
  },
)

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onDocumentPointerDown)
})

watch(
  () => backlog.openId,
  async (id) => {
    addingTag.value = false
    newTag.value = ''
    closeMenus()
    pendingConfirm.value = null
    if (id === null) {
      parentStack.value = []
      previousFocus?.focus()
      previousFocus = null
      return
    }
    const firstPending = breakdown.value ? firstPendingStepId(breakdown.value.steps) : undefined
    expandedMicro.value = new Set(firstPending ? [firstPending] : [])
    answerDrafts.value = {}
    previousFocus ??= document.activeElement instanceof HTMLElement ? document.activeElement : null
    await nextTick()
    dialog.value?.focus()
  },
  { immediate: true },
)
</script>

<template>
  <div v-if="item" :class="OVERLAY_CLASSES" @click.self="closeModal()">
    <div :class="FRAME_CLASSES">
      <section
        ref="dialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="titleId"
        tabindex="-1"
        :class="DIALOG_CLASSES"
        @keydown.esc.stop="onEscape()"
        @keydown.tab="trapFocus($event, dialog)"
      >
        <div class="flex flex-col gap-2 border-b border-line px-6 pt-5 pb-4">
          <header class="flex flex-col gap-2">
            <div class="flex flex-wrap items-center gap-2 text-caption text-muted">
              <UiButton v-if="parentStack.length > 0" variant="quiet" size="sm" data-testid="modal-back-to-parent" @press="goBackToParent">
                ← Voltar ao item
              </UiButton>
              <span class="font-mono font-semibold text-ink">{{ item.publicId ?? item.id }}</span>
              <span v-if="item.publicId && item.publicId !== item.id" class="font-mono text-tiny text-muted">({{ item.id }})</span>

              <div ref="statusMenuRef" class="relative">
                <button
                  type="button"
                  :disabled="isActiveRun"
                  :aria-expanded="statusMenuOpen"
                  aria-haspopup="menu"
                  data-testid="task-status-picker"
                  :title="isActiveRun ? 'Em execução — use Parar execução para alterar a coluna' : 'Alterar status do card'"
                  class="inline-flex cursor-pointer items-center gap-1.5 rounded-sm border border-line bg-surface-2 px-2 py-0.5 text-tiny font-medium text-ink transition-colors hover:bg-hover disabled:cursor-not-allowed disabled:opacity-70"
                  @click="statusMenuOpen = !statusMenuOpen; moreMenuOpen = false"
                >
                  <span class="size-2 shrink-0 rounded-full" :class="BADGE_DOT[statusChip?.status ?? 'backlog']" aria-hidden="true" />
                  <span>{{ COLUMN_LABEL[column] }}</span>
                  <svg v-if="!isActiveRun" viewBox="0 0 16 16" class="size-3 text-muted" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="m4.5 6.5 3.5 3.5 3.5-3.5" />
                  </svg>
                </button>
                <div v-if="statusMenuOpen" role="menu" class="w-60" :class="POPOVER_CLASSES">
                  <button
                    v-for="target in MOVABLE_COLUMNS"
                    :key="target"
                    type="button"
                    role="menuitem"
                    :disabled="target === column || (target === READY_COLUMN && readyBlocked)"
                    :class="MENU_ITEM_CLASSES"
                    :title="target === READY_COLUMN && readyBlocked ? READY_BLOCKED_HINT : undefined"
                    @click="moveTo(target)"
                  >
                    <span class="size-2 shrink-0 rounded-full" :class="BADGE_DOT[COLUMN_CHIP[target]?.status ?? 'backlog']" aria-hidden="true" />
                    <span class="flex-1">{{ COLUMN_LABEL[target] }}</span>
                    <span v-if="target === READY_COLUMN && readyBlocked" class="text-tiny text-muted">Requer plano</span>
                    <span v-else-if="target === column" class="text-tiny font-semibold text-accent">Atual</span>
                  </button>
                  <p class="mt-1 border-t border-line px-2.5 pt-1.5 pb-1 text-tiny text-muted">
                    {{ AUTO_COLUMNS_HINT }}
                  </p>
                </div>
              </div>

              <UiBadge v-if="item.type === 'feature'" status="backlog" label="feature" />
              <span v-if="item.feature" class="inline-flex items-center gap-1.5">
                <span class="size-2 rounded-full" :style="{ backgroundColor: colorVar(FEATURE_COLOR[item.feature] ?? DEFAULT_FEATURE_COLOR) }" />
                {{ item.feature }}
              </span>
              <UiBadge v-if="autoClosed" status="trabalhando" :label="AUTO_CLOSED_LABEL" />

              <div class="ml-auto flex items-center gap-1.5">
                <UiBadge v-if="showStateBadge" :status="STATE_BADGE[planState]" :label="MODAL_STATE_CHIP[planState]" />
                <button
                  type="button"
                  :class="HEADER_PILL_CLASSES"
                  title="Abrir grafo e contexto da task"
                  data-testid="modal-open-task-graph"
                  @click="graphModalOpen = true"
                >
                  <svg viewBox="0 0 16 16" class="size-3 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <circle cx="3.5" cy="4" r="1.5" />
                    <circle cx="12.5" cy="4" r="1.5" />
                    <circle cx="8" cy="12" r="1.5" />
                    <path d="M4.8 4.8 7 10.7M11.2 4.8 9 10.7M5 4h6" />
                  </svg>
                  <span>Grafo</span>
                </button>
                <button
                  v-if="navigation.page !== 'backlog'"
                  type="button"
                  :class="HEADER_PILL_CLASSES"
                  title="Abrir na tela de Tasks"
                  data-testid="modal-open-external-tasks"
                  @click="openInTasks()"
                >
                  <svg viewBox="0 0 16 16" class="size-3 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="M6 3H3.5A1.5 1.5 0 0 0 2 4.5v8A1.5 1.5 0 0 0 3.5 14h8a1.5 1.5 0 0 0 1.5-1.5V10" />
                    <path d="M9.5 2H14v4.5" />
                    <path d="M14 2L7.5 8.5" />
                  </svg>
                  <span>Tasks</span>
                </button>

                <div ref="moreMenuRef" class="relative">
                  <button
                    type="button"
                    :aria-expanded="moreMenuOpen"
                    aria-haspopup="menu"
                    aria-label="Mais ações do card"
                    title="Mais ações"
                    data-testid="task-more-actions"
                    :class="HEADER_PILL_CLASSES"
                    @click="moreMenuOpen = !moreMenuOpen; statusMenuOpen = false"
                  >
                    <svg viewBox="0 0 16 16" class="size-3.5" fill="currentColor" aria-hidden="true">
                      <circle cx="3.5" cy="8" r="1.2" />
                      <circle cx="8" cy="8" r="1.2" />
                      <circle cx="12.5" cy="8" r="1.2" />
                    </svg>
                  </button>
                  <div v-if="moreMenuOpen" role="menu" class="right-0 w-52" :class="POPOVER_CLASSES">
                    <button type="button" role="menuitem" :class="MENU_ITEM_CLASSES" @click="openConversations()">
                      {{ CONVERSATIONS_LABEL }}
                    </button>
                    <button
                      v-if="canDiscardPlan"
                      type="button"
                      role="menuitem"
                      :class="MENU_ITEM_CLASSES"
                      @click="openConfirm('discard-plan')"
                    >
                      {{ DISCARD_PLAN_LABEL }}
                    </button>
                    <div class="my-1 border-t border-line" />
                    <button type="button" role="menuitem" :class="MENU_ITEM_CLASSES" @click="openConfirm('archive')">
                      {{ ARCHIVE_LABEL }}
                    </button>
                    <button
                      type="button"
                      role="menuitem"
                      class="text-danger"
                      :class="MENU_ITEM_CLASSES"
                      @click="openConfirm('delete')"
                    >
                      {{ DELETE_LABEL }}
                    </button>
                  </div>
                </div>

                <UiCloseButton @press="closeModal()" />
              </div>
            </div>
            <h2 :id="titleId" class="wrap-break-word text-section text-ink">{{ item.title }}</h2>
          </header>

          <section class="flex flex-wrap items-center gap-1.5" aria-label="Etiquetas">
            <UiMarker
              v-for="tag in item.tags"
              :key="tag"
              :tone="tagMarkerTone(tag)"
              class="inline-flex items-center gap-1 text-meta whitespace-nowrap"
            >
              {{ tagLabel(tag) }}
              <button type="button" :aria-label="`Remover ${tagLabel(tag)}`" @click="removeTag(tag)">×</button>
            </UiMarker>
            <UiChip kind="add" label="+ etiqueta" @press="addingTag = !addingTag" />
          </section>
          <div v-if="addingTag" class="flex flex-wrap items-center gap-1.5">
            <UiChip
              v-for="tag in suggestedTags"
              :key="tag"
              kind="add"
              :label="`+ ${tagLabel(tag)}`"
              @press="addTag(tag)"
            />
            <input
              v-model="newTag"
              :maxlength="TAG_MAX_LENGTH"
              aria-label="Nova etiqueta"
              placeholder="Nova etiqueta"
              class="h-7 rounded-sm border border-line bg-surface px-2.5 text-label text-ink placeholder:text-faint"
              @keydown.enter.prevent="submitNewTag"
            />
          </div>
        </div>

        <div class="grid min-h-0 flex-1 grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] overflow-y-auto max-md:grid-cols-[minmax(0,1fr)]">
          <div class="flex min-w-0 flex-col gap-5 border-r border-line px-6 py-5 max-md:border-r-0">
            <section v-if="item.description">
              <h3 class="mb-1 text-label font-semibold text-ink">Descrição</h3>
              <MarkdownText v-if="descriptionContent.text" :text="descriptionContent.text" />
              <AttachmentPreviewList
                v-if="descriptionContent.attachments.length > 0"
                :attachments="descriptionContent.attachments"
                :class="descriptionContent.text ? 'mt-2' : ''"
                data-testid="modal-attachments"
              />
            </section>

            <section v-if="pendingQuestions.length" class="flex flex-col gap-3">
              <h3 class="text-label font-semibold text-ink">{{ QUESTIONS_TITLE(leaderName) }}</h3>
              <div v-for="question in pendingQuestions" :key="question.id" class="flex flex-col gap-1.5">
                <p class="text-control text-ink">{{ question.text }}</p>
                <div class="flex flex-wrap gap-1.5">
                  <UiChip
                    v-for="option in [...question.options, OTHER_OPTION]"
                    :key="option"
                    :label="option"
                    :selected="question.selected === option || (option === OTHER_OPTION && isOtherAnswer(question))"
                    @press="pickOption(question, option)"
                  />
                </div>
                <input
                  v-if="isOtherAnswer(question)"
                  :data-other-for="question.id"
                  :value="otherText(question)"
                  :aria-label="OTHER_INPUT_LABEL"
                  :placeholder="OTHER_PLACEHOLDER(leaderName)"
                  :class="OTHER_INPUT_CLASSES"
                  @input="answerDrafts[question.id] = ($event.target as HTMLInputElement).value"
                  @change="saveOther(question)"
                />
              </div>
            </section>

            <PlanBreakdown
              v-if="breakdown && breakdown.steps.length > 0"
              v-model:expanded="expandedMicro"
              :breakdown="breakdown"
              :leader="leaderName"
              @open-stage="openStage"
            />

            <AcceptanceCriteria
              v-if="planArtifact?.criteria.length"
              title="Critérios do plano"
              :criteria="planArtifact.criteria"
              data-testid="plan-criteria-in-card"
              @toggle="togglePlanCriterion"
            />

            <AcceptanceCriteria
              v-if="item.criteria?.length"
              :criteria="item.criteria"
              @toggle="backlog.toggleCriterion(item.id, $event)"
            />

            <section v-if="item.activity?.length">
              <h3 class="mb-1 text-label font-semibold text-ink">Atividade</h3>
              <ul class="flex flex-col gap-1 text-caption text-muted">
                <li v-for="entry in item.activity" :key="entry.id">
                  <time :title="entry.at">{{ activityTime(entry.at) }}</time> · {{ entry.text }}
                </li>
              </ul>
            </section>
          </div>

          <aside class="flex min-w-0 flex-col gap-4 bg-bg px-5.5 py-5">
            <section>
              <h3 class="mb-1.5 text-label font-semibold text-ink">Detalhes</h3>
              <dl class="grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-1.5 text-caption">
                <dt class="text-muted">Responsável</dt>
                <dd class="min-w-0">
                  <AgentPicker
                    :model-value="isUnassigned(item) ? null : item.who"
                    :placeholder="responsibleLabel(item)"
                    :counts="openTasksCountByAgent"
                    :count-label="openTasksLabel"
                    :disabled="isActiveRun || roster.agents.length === 0"
                    busy-first
                    floating="fixed"
                    trigger-variant="compact"
                    trigger-class="w-full"
                    label="Alterar responsável do card"
                    test-id="task-owner-picker"
                    @select="onAssignOwner"
                  />
                </dd>
                <template v-for="[label, value] in details" :key="label">
                  <dt class="text-muted">{{ label }}</dt>
                  <dd class="wrap-break-word text-ink">{{ value }}</dd>
                </template>
              </dl>
            </section>

            <PlanSummaryCard
              v-if="breakdown && (planState === 'proposto' || planState === 'vinculado')"
              :item="item"
              :state="planState"
              :breakdown="breakdown"
            />
            <section v-else class="flex flex-col gap-1">
              <h3 class="text-label font-semibold text-ink">Plano</h3>
              <p v-if="planText" class="whitespace-pre-wrap text-control text-ink">{{ planText }}</p>
              <p class="text-caption text-muted">{{ PLAN_HINT[planState] }}</p>
              <PlanPanel :item="item" simplified />
            </section>

            <LinkedTo
              :item-id="item.id"
              :has-plan="planState === 'vinculado'"
              :links="item.links"
              @open-plan="openPlanArtifact()"
              @open-link="openPlanArtifact()"
            />
          </aside>
        </div>

        <footer class="flex flex-wrap items-center gap-3 border-t border-line px-6 py-3.5">
          <div class="flex min-w-0 items-center gap-3">
            <UiButton variant="quiet" size="sm" @press="openThread">
              {{ OPEN_THREAD_LABEL }}
            </UiButton>
            <span v-if="isActiveRun" class="inline-flex items-center gap-1.5 text-caption text-ink">
              <span class="size-2 animate-pulse rounded-full bg-accent" aria-hidden="true" />
              Em execução…
            </span>
            <span v-else-if="isPlanBusy" class="inline-flex items-center gap-1.5 text-caption text-ink">
              <span class="size-2 animate-pulse rounded-full bg-accent" aria-hidden="true" />
              {{ planState === 'triagem' ? 'Triagem em andamento…' : 'Escrevendo plano…' }}
            </span>
            <span v-else-if="column === 'In review'" class="inline-flex items-center gap-1.5 text-caption text-muted">
              <span class="size-2 rounded-full bg-warn" aria-hidden="true" />
              Aguardando sua revisão
            </span>
          </div>

          <div class="ml-auto flex min-w-0 flex-wrap items-center justify-end gap-2">
            <template v-if="isActiveRun">
              <UiButton
                variant="danger"
                size="lg"
                :disabled="stopping"
                data-testid="stop-item"
                @press="stopActiveWork"
              >
                {{ stopping ? 'Parando…' : 'Parar execução' }}
              </UiButton>
            </template>

            <template v-else-if="isPlanBusy">
              <UiButton
                variant="danger"
                size="lg"
                :disabled="stopping"
                data-testid="cancel-plan"
                @press="stopActiveWork"
              >
                {{ planState === 'triagem' ? 'Parar triagem' : 'Cancelar plano' }}
              </UiButton>
            </template>

            <template v-else-if="column === 'In review'">
              <UiButton
                variant="quiet"
                size="lg"
                data-testid="return-to-ready"
                @press="moveTo(READY_COLUMN)"
              >
                Devolver
              </UiButton>
              <UiButton
                variant="approve"
                size="lg"
                data-testid="complete-item"
                @press="moveTo(DONE_COLUMN_NAME)"
              >
                Concluir
              </UiButton>
            </template>

            <template v-else-if="column === DONE_COLUMN_NAME">
              <template v-if="autoClosed">
                <input
                  v-if="reopening"
                  v-model="reopenMotive"
                  :aria-label="REOPEN_MOTIVE_PLACEHOLDER"
                  :placeholder="REOPEN_MOTIVE_PLACEHOLDER"
                  :class="OTHER_INPUT_CLASSES"
                  @keydown.enter.prevent="reopen"
                />
                <UiButton v-if="!reopening" variant="quiet" size="lg" @press="reopening = true">
                  {{ REOPEN_LABEL }}
                </UiButton>
                <UiButton v-else variant="primary" size="lg" :disabled="reopenMotive.trim() === ''" @press="reopen">
                  {{ REOPEN_LABEL }}
                </UiButton>
              </template>
              <UiButton v-else variant="quiet" size="lg" @press="reopenManual">
                {{ REOPEN_LABEL }}
              </UiButton>
            </template>

            <template v-else>
              <UiButton
                v-if="planState === 'proposto'"
                variant="quiet"
                size="lg"
                @press="openConfirm('discard-plan')"
              >
                Descartar proposta
              </UiButton>
              <AgentPicker
                v-if="planState === 'vinculado' && column === READY_COLUMN && delegateTargets.length > 0"
                :agents="delegateTargets"
                :counts="openTasksCountByAgent"
                :count-label="openTasksLabel"
                :disabled="!runItem.canRun(item.id)"
                busy-first
                placement="top"
                align="end"
                floating="fixed"
                trigger-variant="compact"
                :placeholder="`${DELEGATE_LABEL}…`"
                :label="DELEGATE_LABEL"
                :show-check="false"
                test-id="task-delegate-picker"
                @select="onDelegateTo"
              />
              <UiButton
                variant="primary"
                size="lg"
                data-testid="run-item"
                :disabled="!runItem.canRun(item.id)"
                @press="onMainAction"
              >
                {{ planState === 'proposto' ? 'Aprovar e iniciar' : MAIN_ACTION_LABEL }}
              </UiButton>
            </template>
          </div>
        </footer>
      </section>
    </div>

    <Teleport to="body">
      <div
        v-if="confirmMeta"
        :class="CONFIRM_OVERLAY_CLASSES"
        @click.stop="($event.target === $event.currentTarget) && closeConfirm()"
        @pointerdown.stop
        @mousedown.stop
      >
        <div :class="CONFIRM_FRAME_CLASSES">
          <UiCloseButton class="absolute right-3 top-3 z-10" @press="closeConfirm()" />
          <section
            ref="confirmDialog"
            role="dialog"
            aria-modal="true"
            :aria-labelledby="confirmTitleId"
            tabindex="-1"
            class="flex flex-col gap-4"
            @keydown.esc.stop="closeConfirm()"
            @keydown.tab="trapFocus($event, confirmDialog)"
          >
            <h3 :id="confirmTitleId" class="text-control font-bold text-ink">{{ confirmMeta.title }}</h3>
            <p class="text-caption text-muted">{{ confirmMeta.message }}</p>
            <div class="flex justify-end gap-2">
              <UiButton ref="confirmCancelBtn" variant="quiet" size="lg" @press="closeConfirm()">
                Cancelar
              </UiButton>
              <UiButton :variant="confirmMeta.variant" size="lg" @press="acceptConfirm()">
                {{ confirmMeta.confirmLabel }}
              </UiButton>
            </div>
          </section>
        </div>
      </div>
    </Teleport>

    <ContextGraphModal
      v-if="graphModalOpen"
      :open="graphModalOpen"
      :project-id="projects.selected?.id"
      :title="`Grafo da task — ${item.publicId ?? item.id}`"
      :query="item.id"
      :context-text="taskGraphContext"
      :card-id="item.id"
      :agent="item.who"
      :meta-summary="item.title"
      @close="graphModalOpen = false"
    />
  </div>
</template>
