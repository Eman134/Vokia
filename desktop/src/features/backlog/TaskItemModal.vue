<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiMarker from '../../ui/UiMarker.vue'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import UiChip from '../../ui/UiChip.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import {
  AUTO_CLOSED_LABEL,
  CLOSED_BY_AUTOMATIC,
  colorVar,
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
import { SUGGESTED_TAGS, TAG_MAX_LENGTH, tagLabel, tagMarkerTone } from './tags'
import {
  AUTO_COLUMNS_HINT,
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
import { agentName, DEFAULT_RECIPIENT } from '../team/agents'
import { useNavigation } from '../../shell/useNavigation'

const OVERLAY_CLASSES = 'fixed inset-0 z-50 flex items-center justify-center bg-paper/70 p-4'
const FRAME_CLASSES = 'w-215 max-w-full rounded-md border border-line bg-surface shadow-paper'
// O backdrop tem p-4 (2rem no total): o diálogo rola por dentro em vez de passar da janela.
const DIALOG_CLASSES = 'flex max-h-[calc(100dvh-2rem)] flex-col overflow-hidden'
const OTHER_INPUT_CLASSES = 'h-9 w-full rounded-md border border-line bg-surface px-3 text-control text-ink placeholder:text-faint'
const TRIAGE_PLAN_TEXT = 'Plano sai da triagem'
const ARCHIVE_LABEL = 'Arquivar'
const ARCHIVE_MESSAGE = 'Tira o card do quadro. Dá para encontrá-lo arquivado depois.'
const ARCHIVE_CONFIRM_LABEL = 'Arquivar card'
const DELETE_LABEL = 'Remover'
const DELETE_MESSAGE = 'Remove o card de vez. Não dá para desfazer.'
const DELETE_CONFIRM_LABEL = 'Remover card'
const NO_LEADER_TEXT = 'Sem responsável · nenhum líder configurado'

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
const dialog = ref<HTMLElement>()
const addingTag = ref(false)
const newTag = ref('')
const expandedMicro = ref(new Set<string>())
const parentStack = ref<string[]>([])
const answerDrafts = ref<Record<string, string>>({})
let previousFocus: HTMLElement | null = null

const item = computed(() => backlog.openItem)

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
const column = computed(() => item.value?.column ?? DEFAULT_COLUMN)
// Só o card concluído pela validação automática pode ser reaberto por aqui; o manual o usuário mesmo moveu.
const autoClosed = computed(() => column.value === DONE_COLUMN_NAME && item.value?.closedBy === CLOSED_BY_AUTOMATIC)
const moveTargets = computed(() => MOVABLE_COLUMNS.filter((target) => target !== column.value))
const readyBlocked = computed(() => !item.value || !itemPlan.isReviewed(item.value.id))
const delegateTargets = computed(() =>
  roster.agents.filter((profile) => profile.id !== (itemPlan.planOf(item.value?.id ?? '').agent ?? DEFAULT_RECIPIENT)),
)
const showAutoHint = computed(() => column.value === 'In progress' || column.value === 'In review')
const suggestedTags = computed(() =>
  [...new Set([...SUGGESTED_TAGS, ...backlog.items.flatMap((current) => current.tags)])].filter(
    (tag) => !item.value?.tags.includes(tag),
  ),
)
const itemStages = computed(() => (item.value ? stagesOf(backlog.items, `plano-${item.value.id}`) : []))
const proposal = computed(() => (item.value ? itemPlan.proposalFor(item.value) : undefined))
const breakdown = computed(() =>
  item.value
    ? breakdownOf(item.value, proposal.value, itemStages.value, (agentId) => roster.profileOf(agentId)?.model)
    : undefined,
)
const planState = computed(() => (item.value ? itemPlan.stateFor(item.value) : 'vazio'))
// Sem plano ou com a proposta esperando o usuário, o selo só repetiria o que o conteúdo do modal já mostra.
const showStateBadge = computed(() => planState.value !== 'vazio' && planState.value !== 'proposto')
const planText = computed(() => (planState.value === 'triagem' ? TRIAGE_PLAN_TEXT : undefined))
function responsibleLabel(current: BacklogItem): string {
  if (!isUnassigned(current)) return roster.nameOf(current.who)
  return triage.isWithoutLeader(current.id) ? NO_LEADER_TEXT : UNASSIGNED_TEXT
}

const details = computed(() => {
  const current = item.value
  if (!current) return []
  const rows: [string, string | undefined][] = [
    ['Responsável', responsibleLabel(current)],
    ['Tipo', current.type === 'feature' ? 'Feature' : 'Tarefa'],
    ['Aberto por', current.by],
    ['Origem', ORIGIN_LABEL[current.origin]],
    ['Feature', current.feature],
    ['Prioridade', current.priority],
    ['Estimativa', current.estimate],
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
  if (target === READY_COLUMN && readyBlocked.value) return
  dialog.value?.focus()
  backlog.moveTo(item.value!.id, target)
}

function onMainAction(): void {
  if (item.value) void runItem.run(item.value.id)
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

// Salva antes de fechar: depois do close, item.value vira null e o rascunho não tem mais onde ser gravado.
function closeModal(): void {
  item.value?.questions?.filter(isOtherAnswer).forEach((question) => saveOther(question))
  backlog.close()
}

function openConversations(): void {
  closeModal()
  navigation.goTo('conversas')
}

function archiveItem(): void {
  const id = item.value!.id
  closeModal()
  backlog.archive(id)
}

function removeItem(): void {
  const id = item.value!.id
  closeModal()
  backlog.remove(id)
}

watch(
  () => backlog.openId,
  async (id) => {
    addingTag.value = false
    newTag.value = ''
    if (id === null) {
      parentStack.value = []
      previousFocus?.focus()
      previousFocus = null
      return
    }
    const firstStep = breakdown.value?.steps[0]
    expandedMicro.value = new Set(firstStep ? [firstStep.id] : [])
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
      @keydown.esc.stop="closeModal()"
      @keydown.tab="trapFocus($event, dialog)"
    >
      <div class="flex flex-col gap-2 border-b border-line px-6 pt-6 pb-4">
        <header class="flex flex-col gap-2">
          <div class="flex flex-wrap items-center gap-2 text-caption text-muted">
            <UiButton v-if="parentStack.length > 0" variant="quiet" size="sm" data-testid="modal-back-to-parent" @press="goBackToParent">
              ← Voltar ao item
            </UiButton>
            <span>{{ item.id }}</span>
            <UiBadge v-if="item.type === 'feature'" status="backlog" label="feature" />
            <span v-if="item.feature" class="inline-flex items-center gap-1.5">
              <span class="size-2 rounded-full" :style="{ backgroundColor: colorVar(FEATURE_COLOR[item.feature] ?? DEFAULT_FEATURE_COLOR) }" />
              {{ item.feature }}
            </span>
            <UiBadge v-if="autoClosed" status="trabalhando" :label="AUTO_CLOSED_LABEL" />
            <UiBadge v-if="showStateBadge" class="ml-auto" :status="STATE_BADGE[planState]" :label="MODAL_STATE_CHIP[planState]" />
            <UiCloseButton :class="!showStateBadge && 'ml-auto'" @press="closeModal()" />
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
            <p class="whitespace-pre-wrap text-control text-ink">{{ item.description }}</p>
          </section>

          <section v-if="item.questions?.length" class="flex flex-col gap-3">
            <h3 class="text-label font-semibold text-ink">{{ QUESTIONS_TITLE(leaderName) }}</h3>
            <div v-for="question in item.questions" :key="question.id" class="flex flex-col gap-1.5">
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
            v-if="item.criteria?.length"
            :criteria="item.criteria"
            @toggle="backlog.toggleCriterion(item.id, $event)"
          />

          <section v-if="item.activity?.length">
            <h3 class="mb-1 text-label font-semibold text-ink">Atividade</h3>
            <ul class="flex flex-col gap-1 text-caption text-muted">
              <li v-for="entry in item.activity" :key="entry.id">{{ entry.at }} · {{ entry.text }}</li>
            </ul>
          </section>
        </div>

        <aside class="flex min-w-0 flex-col gap-4 bg-bg px-5.5 py-5">
          <section>
            <h3 class="mb-1 text-label font-semibold text-ink">Detalhes</h3>
            <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-caption">
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

      <footer class="flex flex-wrap items-center gap-2 border-t border-line px-6 py-4">
        <div class="flex flex-wrap items-center gap-1.5">
          <h3 class="mr-1 text-label text-muted">Mover para</h3>
          <UiChip
            v-for="target in moveTargets"
            :key="target"
            kind="add"
            :label="target"
            :disabled="runItem.isRunning(item.id) || (target === READY_COLUMN && readyBlocked)"
            @press="moveTo(target)"
          />
          <span v-if="showAutoHint" class="text-caption text-muted">{{ AUTO_COLUMNS_HINT }}</span>
          <span v-if="readyBlocked && moveTargets.includes(READY_COLUMN)" class="text-caption text-muted">{{ READY_BLOCKED_HINT }}</span>
        </div>
        <div class="ml-auto flex min-w-0 flex-wrap justify-end gap-2">
          <template v-if="planState === 'vinculado' && column === READY_COLUMN">
            <span class="self-center text-label text-muted">{{ DELEGATE_LABEL }}</span>
            <UiChip
              v-for="profile in delegateTargets"
              :key="profile.id"
              :label="profile.name"
              :disabled="!runItem.canRun(item.id)"
              @press="void runItem.run(item.id, profile.id)"
            />
          </template>
          <template v-if="autoClosed">
            <input
              v-if="reopening"
              v-model="reopenMotive"
              :aria-label="REOPEN_MOTIVE_PLACEHOLDER"
              :placeholder="REOPEN_MOTIVE_PLACEHOLDER"
              :class="OTHER_INPUT_CLASSES"
              @keydown.enter.prevent="reopen"
            />
            <UiButton v-if="!reopening" variant="quiet" size="lg" @press="reopening = true">{{ REOPEN_LABEL }}</UiButton>
            <UiButton v-else variant="primary" size="lg" :disabled="reopenMotive.trim() === ''" @press="reopen">
              {{ REOPEN_LABEL }}
            </UiButton>
          </template>
          <UiConfirm
            :label="DELETE_LABEL"
            :message="DELETE_MESSAGE"
            :confirm-label="DELETE_CONFIRM_LABEL"
            :busy="runItem.isRunning(item.id)"
            @confirm="removeItem"
          />
          <UiConfirm :label="ARCHIVE_LABEL" :message="ARCHIVE_MESSAGE" :confirm-label="ARCHIVE_CONFIRM_LABEL" @confirm="archiveItem" />
          <UiButton variant="quiet" size="lg" @press="openConversations">{{ CONVERSATIONS_LABEL }}</UiButton>
          <UiButton
            variant="primary"
            size="lg"
            data-testid="run-item"
            :disabled="!runItem.canRun(item.id)"
            @press="onMainAction"
          >
            {{ MAIN_ACTION_LABEL }}
          </UiButton>
        </div>
      </footer>
    </section>
    </div>
  </div>
</template>
