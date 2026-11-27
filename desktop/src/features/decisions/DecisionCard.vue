<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiChip from '../../ui/UiChip.vue'
import UiField from '../../ui/UiField.vue'
import { useNavigation } from '../../shell/useNavigation'
import { extractAttachments } from '../attachments/attachmentBlock'
import { COLUMN_LABEL } from '../backlog/backlogTypes'
import { useBacklog } from '../backlog/useBacklog'
import { agentName } from '../team/agents'
import AgentPortrait from '../team/AgentPortrait.vue'
import MarkdownText from '../team/MarkdownText.vue'
import { PORTRAIT_SIZES } from '../team/portraitArt'
import {
  acceptsTypedAnswer,
  actionsFor,
  ANSWER_LABEL,
  CHOOSE_HINT,
  CHOOSE_LABEL,
  chosenAnswer as pickAnswer,
  KIND_LABELS,
  OTHER_FIELD_LABEL,
  optionMessage,
  PERMISSION_ALWAYS,
  PERMISSION_DENY,
  type CardAction,
} from './decisionCopy'
import { useDecisions, type PanelCard } from './useDecisions'

const props = defineProps<{ card: PanelCard }>()

const decisions = useDecisions()
const backlog = useBacklog()
const navigation = useNavigation()
const selected = ref<string | null>(null)
const otherText = ref('')

const name = computed(() => agentName(props.card.agent))
const actions = computed(() => actionsFor(props.card.kind, name.value))
const isChoice = computed(() => acceptsTypedAnswer(props.card.kind, props.card.options.length))
const subject = computed(() => (props.card.kind === 'permissao' ? props.card.detail : props.card.title))
const chosenAnswer = computed(() => pickAnswer(selected.value, otherText.value))
const isReviewCard = computed(() => props.card.kind === 'validacao' || props.card.kind === 'revisao')
const validationTask = computed(() =>
  isReviewCard.value && props.card.itemId
    ? backlog.items.find((item) => item.id === props.card.itemId) ?? null
    : null,
)
const validationDescription = computed(() => {
  const raw = validationTask.value?.description
  if (!raw) return ''
  return extractAttachments(raw).text.trim()
})
const validationCriteria = computed(() => {
  const criteria = validationTask.value?.criteria ?? []
  if (criteria.length === 0) return null
  const done = criteria.filter((c) => c.done).length
  return `${done}/${criteria.length} critérios`
})

watch(otherText, (text) => {
  if (text.trim() !== '') selected.value = null
})

function choose(option: string): void {
  selected.value = option
  otherText.value = ''
}

function run(action: CardAction): void {
  const message = action.message(name.value, subject.value)
  if (props.card.permissionId !== null) {
    decisions.answerPermission(
      props.card.permissionId,
      action.answer !== PERMISSION_DENY,
      message,
      action.answer === PERMISSION_ALWAYS,
    )
  } else if (props.card.decisionId !== null) {
    decisions.answer(props.card.decisionId, action.answer, message)
  }
}

function viewValidationCard(): void {
  const itemId = props.card.itemId
  if (props.card.decisionId !== null) {
    decisions.dismiss(props.card.decisionId, `Validação de ${props.card.title} aberta no card`)
  }
  decisions.open = false
  if (itemId) {
    navigation.openCard(itemId)
  } else {
    navigation.goTo('backlog')
  }
}

function submitChoice(): void {
  const text = chosenAnswer.value
  if (text === '' || props.card.decisionId === null || props.card.kind === 'permissao') {
    return
  }
  decisions.answer(props.card.decisionId, text, optionMessage(props.card.kind, name.value, text))
}

function dismissChoice(): void {
  if (props.card.decisionId === null) return
  decisions.dismiss(props.card.decisionId, `Decisão de ${name.value} ignorada`)
}
</script>

<template>
  <li
    class="min-w-0 rounded-md border border-line bg-surface px-3 py-2.5"
    :data-testid="`decision-card-${card.key}`"
    :data-kind="card.kind"
  >
    <p class="text-tiny text-muted">{{ KIND_LABELS[card.kind] }}</p>
    <p class="flex items-start gap-1.5 wrap-break-word text-label font-semibold text-ink">
      <AgentPortrait :who="card.agent" :size="PORTRAIT_SIZES.stack" class="mt-0.5 shrink-0" />
      <span class="min-w-0">{{ name }} · {{ card.title }}</span>
    </p>
    <div
      v-if="isReviewCard"
      class="mt-2 flex flex-col gap-1.5 rounded-sm border border-line bg-surface-2 p-2.5"
      data-testid="validation-mini-card"
    >
      <div class="flex items-center gap-1.5 text-tiny">
        <span class="size-2 shrink-0 rounded-full bg-accent" aria-hidden="true" />
        <span class="rounded-sm bg-surface px-1.5 py-0.5 font-mono font-bold text-ink">
          {{ validationTask?.publicId ?? validationTask?.id ?? card.itemId ?? 'task' }}
        </span>
        <span v-if="validationTask?.column" class="rounded-sm bg-surface px-1.5 py-0.5 text-muted">
          {{ COLUMN_LABEL[validationTask.column] }}
        </span>
        <span v-if="validationCriteria" class="ml-auto text-muted tabular-nums">{{ validationCriteria }}</span>
      </div>
      <p class="text-caption font-semibold text-ink">{{ validationTask?.title ?? card.title }}</p>
      <p v-if="card.kind === 'revisao' && card.detail" class="text-meta leading-[1.35] text-ink">
        {{ card.detail }}
      </p>
      <p v-else-if="validationDescription" class="line-clamp-3 text-meta leading-[1.35] text-muted">
        {{ validationDescription }}
      </p>
    </div>
    <MarkdownText v-else-if="card.detail" :text="card.detail" tasks class="text-caption! text-muted" />
    <template v-if="isChoice">
      <div class="mt-2 flex flex-wrap gap-1.5">
        <UiChip
          v-for="option in card.options"
          :key="option"
          :label="option"
          :selected="selected === option"
          @press="choose(option)"
        />
      </div>
      <UiField v-model="otherText" class="mt-2" :label="OTHER_FIELD_LABEL" @keydown.enter.prevent="submitChoice" />
      <div class="mt-2 flex items-center justify-end gap-2">
        <span v-if="card.kind === 'escolha' && chosenAnswer === ''" class="mr-auto text-caption text-muted">{{ CHOOSE_HINT }}</span>
        <UiButton variant="quiet" size="sm" data-testid="decision-dismiss" @press="dismissChoice">
          Ignorar
        </UiButton>
        <UiButton variant="approve" size="sm" :disabled="chosenAnswer === ''" @press="submitChoice">
          {{ card.kind === 'escolha' ? CHOOSE_LABEL : ANSWER_LABEL }}
        </UiButton>
      </div>
    </template>
    <div v-else class="mt-2 flex flex-wrap gap-2">
      <UiButton
        v-if="isReviewCard"
        variant="primary"
        size="sm"
        data-testid="validation-view-card"
        @press="viewValidationCard"
      >
        Visualizar
      </UiButton>
      <UiButton
        v-for="action in actions"
        :key="action.id"
        :variant="action.variant"
        size="sm"
        :disabled="action.disabled"
        @press="run(action)"
      >
        {{ action.label }}
      </UiButton>
      <UiButton v-if="card.decisionId !== null" variant="quiet" size="sm" data-testid="decision-dismiss" @press="dismissChoice">
        Ignorar
      </UiButton>
    </div>
  </li>
</template>
