<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiChip from '../../ui/UiChip.vue'
import { colorVar, DEFAULT_FEATURE_COLOR, FEATURE_COLOR, MICRO_STATUS_BADGE, STATE_BADGE } from './backlogLabels'
import { SUGGESTED_TAGS, TAG_MAX_LENGTH, tagColor, tagLabel, tagTextColor } from './tags'
import {
  AUTO_COLUMNS_HINT,
  CONVERSATIONS_LABEL,
  DEFAULT_COLUMN,
  DELEGATE_LABEL,
  MAIN_ACTION,
  MICRO_EXPLANATION,
  MICRO_SUMMARY,
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
  type BacklogTag,
  type TriageQuestion,
} from './backlogTypes'
import MicroTaskScript, { isScriptValid } from './MicroTaskScript.vue'
import PlanPanel from './PlanPanel.vue'
import { useBacklog } from './useBacklog'
import { useItemQuestions } from './useItemQuestions'
import { useItemPlan } from './useItemPlan'
import { useRunItem } from './useRunItem'
import { trapFocus } from './dialogFocus'
import AgentPortrait from '../team/AgentPortrait.vue'
import { useRoster } from '../team/useRoster'
import { agentName, DEFAULT_RECIPIENT } from '../team/useTeam'
import { useNavigation } from '../../shell/useNavigation'

const TAG_TINT_PERCENT = 13
const MICRO_PORTRAIT_SIZE = 20
const OTHER_INPUT_CLASSES = 'h-9 w-full rounded-control border border-line bg-surface px-3 text-control text-ink placeholder:text-faint'
const TRIAGE_PLAN_TEXT = 'Plano sai da triagem'

const backlog = useBacklog()
const itemQuestions = useItemQuestions()
const itemPlan = useItemPlan()
const runItem = useRunItem()
const roster = useRoster()
const navigation = useNavigation()
const leaderName = computed(() => agentName(DEFAULT_RECIPIENT))
const titleId = useId()
const dialog = ref<HTMLElement>()
const addingTag = ref(false)
const newTag = ref('')
const expandedMicro = ref(new Set<string>())
const answerDrafts = ref<Record<string, string>>({})
let previousFocus: HTMLElement | null = null

const item = computed(() => backlog.openItem)
const column = computed(() => item.value?.column ?? DEFAULT_COLUMN)
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
const visibleMicro = computed(() => item.value?.micro?.filter((task) => isScriptValid(task.script)) ?? [])
const microDone = computed(() => visibleMicro.value.filter((task) => task.status === 'feito').length)
const planState = computed(() => (item.value ? itemPlan.stateFor(item.value) : 'vazio'))
const planText = computed(() => (planState.value === 'triagem' ? TRIAGE_PLAN_TEXT : item.value?.plan))
const details = computed(() => {
  const current = item.value
  if (!current) return []
  const rows: [string, string | undefined][] = [
    ['Tipo', current.type === 'feature' ? 'Feature' : 'Tarefa'],
    ['Aberto por', current.by],
    ['Feature', current.feature],
    ['Prioridade', current.priority],
    ['Estimativa', current.estimate],
  ]
  return rows.filter((row): row is [string, string] => Boolean(row[1]))
})

function tagPillStyle(tag: BacklogTag) {
  const token = colorVar(tagColor(tag))
  return {
    color: colorVar(tagTextColor(tag)),
    backgroundColor: `color-mix(in srgb, ${token} ${TAG_TINT_PERCENT}%, transparent)`,
  }
}

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

function toggleMicro(id: string): void {
  const next = new Set(expandedMicro.value)
  if (!next.delete(id)) next.add(id)
  expandedMicro.value = next
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

watch(
  () => backlog.openId,
  async (id) => {
    addingTag.value = false
    newTag.value = ''
    if (id === null) {
      previousFocus?.focus()
      previousFocus = null
      return
    }
    const firstMicro = visibleMicro.value[0]
    expandedMicro.value = new Set(firstMicro ? [firstMicro.id] : [])
    answerDrafts.value = {}
    previousFocus ??= document.activeElement instanceof HTMLElement ? document.activeElement : null
    await nextTick()
    dialog.value?.focus()
  },
  { immediate: true },
)
</script>

<template>
  <div
    v-if="item"
    class="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4"
    @click.self="closeModal()"
  >
    <section
      ref="dialog"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      tabindex="-1"
      class="flex max-h-full w-215 max-w-full flex-col overflow-hidden rounded-dialog border border-line bg-surface shadow-float"
      @keydown.esc.stop="closeModal()"
      @keydown.tab="trapFocus($event, dialog)"
    >
      <div class="flex flex-col gap-2 border-b border-line px-6 pt-6 pb-4">
        <header class="flex flex-col gap-2">
          <div class="flex items-center gap-2 text-caption text-muted">
            <span>{{ item.id }}</span>
            <UiBadge v-if="item.type === 'feature'" status="backlog" label="feature" />
            <span v-if="item.feature" class="inline-flex items-center gap-1.5">
              <span class="size-2 rounded-full" :style="{ backgroundColor: colorVar(FEATURE_COLOR[item.feature] ?? DEFAULT_FEATURE_COLOR) }" />
              {{ item.feature }}
            </span>
            <UiBadge class="ml-auto" :status="STATE_BADGE[planState]" :label="MODAL_STATE_CHIP[planState]" />
            <UiButton variant="quiet" size="sm" @press="closeModal()">Fechar</UiButton>
          </div>
          <h2 :id="titleId" class="wrap-break-word text-section text-ink">{{ item.title }}</h2>
        </header>

        <section class="flex flex-wrap items-center gap-1.5" aria-label="Etiquetas">
          <span
            v-for="tag in item.tags"
            :key="tag"
            class="inline-flex h-5.5 items-center gap-1 rounded-chip px-2 text-meta font-medium whitespace-nowrap"
            :style="tagPillStyle(tag)"
          >
            {{ tagLabel(tag) }}
            <button type="button" :aria-label="`Remover ${tagLabel(tag)}`" @click="removeTag(tag)">×</button>
          </span>
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
            class="h-7 rounded-control-sm border border-line bg-surface px-2.5 text-label text-ink placeholder:text-faint"
            @keydown.enter.prevent="submitNewTag"
          />
        </div>
      </div>

      <div class="grid min-h-0 flex-1 grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] overflow-y-auto max-md:grid-cols-1">
        <div class="flex flex-col gap-5 border-r border-line px-6 py-5 max-md:border-r-0">
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

          <section v-if="visibleMicro.length" class="flex flex-col gap-2">
            <div class="flex items-baseline justify-between">
              <h3 class="text-label font-semibold text-ink">Desdobramento</h3>
              <span class="text-caption text-muted">{{ MICRO_SUMMARY(visibleMicro.length, microDone) }}</span>
            </div>
            <div v-for="task in visibleMicro" :key="task.id" class="flex flex-col gap-2 rounded-control-lg border border-line p-3">
              <button
                type="button"
                class="flex items-center gap-2 text-left text-control text-ink"
                :aria-expanded="expandedMicro.has(task.id)"
                @click="toggleMicro(task.id)"
              >
                <span aria-hidden="true">{{ expandedMicro.has(task.id) ? '▾' : '▸' }}</span>
                <span class="font-mono text-meta text-muted">{{ task.id }}</span>
                <span class="font-medium">{{ task.title }}</span>
                <span class="ml-auto flex items-center gap-2">
                  <AgentPortrait :who="task.who" :size="MICRO_PORTRAIT_SIZE" />
                  <span class="text-caption text-muted">{{ task.model }}</span>
                  <UiBadge :status="MICRO_STATUS_BADGE[task.status]" :label="task.status" />
                </span>
              </button>
              <MicroTaskScript
                v-if="expandedMicro.has(task.id)"
                :script="task.script"
                :model="task.model"
                :leader="leaderName"
                :returned="task.returned"
                :learned="task.learned"
              />
            </div>
            <p class="text-caption text-muted">{{ MICRO_EXPLANATION(leaderName) }}</p>
          </section>

          <section v-if="item.criteria?.length">
            <h3 class="mb-1 text-label font-semibold text-ink">Critérios de aceite</h3>
            <ul class="flex flex-col gap-1">
              <li v-for="criterion in item.criteria" :key="criterion.id">
                <label class="flex items-center gap-2 text-control text-ink">
                  <input type="checkbox" :checked="criterion.done" @change="backlog.toggleCriterion(item.id, criterion.id)" />
                  <span :class="criterion.done && 'text-muted line-through'">{{ criterion.text }}</span>
                </label>
              </li>
            </ul>
          </section>

          <section v-if="item.activity?.length">
            <h3 class="mb-1 text-label font-semibold text-ink">Atividade</h3>
            <ul class="flex flex-col gap-1 text-caption text-muted">
              <li v-for="entry in item.activity" :key="entry.id">{{ entry.at }} · {{ entry.text }}</li>
            </ul>
          </section>
        </div>

        <aside class="flex flex-col gap-4 bg-bg px-5.5 py-5">
          <section>
            <h3 class="mb-1 text-label font-semibold text-ink">Detalhes</h3>
            <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-caption">
              <template v-for="[label, value] in details" :key="label">
                <dt class="text-muted">{{ label }}</dt>
                <dd class="text-ink">{{ value }}</dd>
              </template>
            </dl>
          </section>

          <section class="flex flex-col gap-1">
            <h3 class="text-label font-semibold text-ink">Plano</h3>
            <p v-if="planText" class="whitespace-pre-wrap text-control text-ink">{{ planText }}</p>
            <p class="text-caption text-muted">{{ PLAN_HINT[planState] }}</p>
            <PlanPanel :item="item" />
          </section>

          <section v-if="item.links?.length">
            <h3 class="mb-1 text-label font-semibold text-ink">Vínculos</h3>
            <ul class="flex flex-col gap-1 text-caption text-muted">
              <li v-for="link in item.links" :key="link.id">{{ link.label }}</li>
            </ul>
          </section>
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
        <div class="ml-auto flex gap-2">
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
          <UiButton variant="quiet" size="lg" @press="openConversations">{{ CONVERSATIONS_LABEL }}</UiButton>
          <UiButton
            variant="primary"
            size="lg"
            data-testid="run-item"
            :disabled="!runItem.canRun(item.id)"
            @press="onMainAction"
          >
            {{ MAIN_ACTION[planState] }}
          </UiButton>
        </div>
      </footer>
    </section>
  </div>
</template>
