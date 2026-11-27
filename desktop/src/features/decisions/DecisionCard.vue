<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiChip from '../../ui/UiChip.vue'
import UiField from '../../ui/UiField.vue'
import { agentName } from '../team/agents'
import AgentPortrait from '../team/AgentPortrait.vue'
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
  RETURN_MOTIVE_PLACEHOLDER,
  VALIDATION_RETURN_LABEL,
  VALIDATION_RETURN_MESSAGE,
  VALIDATION_RETURN_PREFIX,
  type CardAction,
} from './decisionCopy'
import { useDecisions, type PanelCard } from './useDecisions'

const props = defineProps<{ card: PanelCard }>()

const decisions = useDecisions()
const selected = ref<string | null>(null)
const otherText = ref('')
const returning = ref(false)
const returnMotive = ref('')

const name = computed(() => agentName(props.card.agent))
const actions = computed(() => actionsFor(props.card.kind, name.value))
const isChoice = computed(() => acceptsTypedAnswer(props.card.kind, props.card.options.length))
const subject = computed(() => (props.card.kind === 'permissao' ? props.card.detail : props.card.title))
const chosenAnswer = computed(() => pickAnswer(selected.value, otherText.value))

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

// Devolver na validação final exige o motivo; o backend recebe "Devolver: <motivo>".
function submitReturn(): void {
  const motive = returnMotive.value.trim()
  if (motive === '' || props.card.decisionId === null) return
  decisions.answer(props.card.decisionId, `${VALIDATION_RETURN_PREFIX}${motive}`, VALIDATION_RETURN_MESSAGE(name.value))
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
    <p v-if="card.detail" class="wrap-break-word whitespace-pre-wrap text-caption text-muted">{{ card.detail }}</p>
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
        v-for="action in actions"
        :key="action.id"
        :variant="action.variant"
        size="sm"
        :disabled="action.disabled"
        @press="run(action)"
      >
        {{ action.label }}
      </UiButton>
      <UiButton v-if="card.kind === 'validacao'" variant="quiet" size="sm" @press="returning = !returning">
        {{ VALIDATION_RETURN_LABEL }}
      </UiButton>
      <UiButton v-if="card.decisionId !== null" variant="quiet" size="sm" data-testid="decision-dismiss" @press="dismissChoice">
        Ignorar
      </UiButton>
    </div>
    <div v-if="card.kind === 'validacao' && returning" class="mt-2 flex items-end gap-2">
      <UiField v-model="returnMotive" class="min-w-0 flex-1" :label="RETURN_MOTIVE_PLACEHOLDER" />
      <UiButton variant="danger" size="sm" :disabled="returnMotive.trim() === ''" @press="submitReturn">
        {{ VALIDATION_RETURN_LABEL }}
      </UiButton>
    </div>
  </li>
</template>
