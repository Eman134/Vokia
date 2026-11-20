<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiChip from '../../ui/UiChip.vue'
import UiField from '../../ui/UiField.vue'
import { agentName } from '../team/agents'
import {
  actionsFor,
  ANSWER_LABEL,
  CHOOSE_HINT,
  CHOOSE_LABEL,
  KIND_LABELS,
  OTHER_OPTION,
  OTHER_PLACEHOLDER,
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
const isChoice = computed(() => props.card.kind === 'escolha' || props.card.kind === 'pergunta')
const allowsOther = computed(() => props.card.kind === 'pergunta')
const typingOther = computed(() => selected.value === OTHER_OPTION)
const subject = computed(() => (props.card.kind === 'permissao' ? props.card.detail : props.card.title))
const chosenAnswer = computed(() => (typingOther.value ? otherText.value.trim() : (selected.value ?? '')))

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
</script>

<template>
  <li
    class="rounded-card border border-line bg-surface px-3 py-2.5"
    :data-testid="`decision-card-${card.key}`"
    :data-kind="card.kind"
  >
    <p class="text-tiny text-muted">{{ KIND_LABELS[card.kind] }}</p>
    <p class="wrap-break-word text-label font-semibold text-ink">{{ name }} · {{ card.title }}</p>
    <p v-if="card.detail" class="wrap-break-word whitespace-pre-wrap text-caption text-muted">{{ card.detail }}</p>
    <template v-if="isChoice">
      <div class="mt-2 flex flex-wrap gap-1.5">
        <UiChip
          v-for="option in card.options"
          :key="option"
          :label="option"
          :selected="selected === option"
          @press="selected = option"
        />
        <UiChip
          v-if="allowsOther"
          :label="OTHER_OPTION"
          :selected="typingOther"
          @press="selected = OTHER_OPTION"
        />
      </div>
      <UiField v-if="typingOther" v-model="otherText" class="mt-2" :label="OTHER_PLACEHOLDER" />
      <div class="mt-2 flex items-center justify-end gap-2">
        <span v-if="card.kind === 'escolha' && selected === null" class="text-caption text-muted">{{ CHOOSE_HINT }}</span>
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
    </div>
    <div v-if="card.kind === 'validacao' && returning" class="mt-2 flex items-end gap-2">
      <UiField v-model="returnMotive" class="min-w-0 flex-1" :label="RETURN_MOTIVE_PLACEHOLDER" />
      <UiButton variant="danger" size="sm" :disabled="returnMotive.trim() === ''" @press="submitReturn">
        {{ VALIDATION_RETURN_LABEL }}
      </UiButton>
    </div>
  </li>
</template>
