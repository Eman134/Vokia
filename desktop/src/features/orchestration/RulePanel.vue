<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import UiSelect from '../../ui/UiSelect.vue'
import CountStepper from './CountStepper.vue'
import { saveRule } from './orchestrationApi'
import {
  EXEC_MODE_LABELS,
  THEN_CHOICE_LABELS,
  VIEW_COPY,
  WHEN_EVENT_LABELS,
} from './orchestrationLabels'
import { useOrchestration } from './useOrchestration'
import { actionAndKindOf, rulePhrase, thenChoiceOf, thenChoicesFor } from './ruleText'
import {
  EXEC_MODE_VALUES,
  VOCE_NODE,
  WHEN_EVENT_VALUES,
  type ExecMode,
  type LayerKind,
  type OrchLayer,
  type OrchRule,
  type OrchRuleDraft,
  type ThenChoice,
  type WhenEvent,
} from './orchestrationTypes'

const RETURN_LIMIT_MIN = 1
const RETURN_LIMIT_MAX = 5
const DEFAULT_RETURN_LIMIT = 2
const INITIAL_WHEN_EVENT: WhenEvent = 'tarefa_concluida'
const INITIAL_EXEC_MODE: ExecMode = 'sequencia'

const props = defineProps<{ rule: OrchRule | null; layers: OrchLayer[] }>()
const emit = defineEmits<{ close: []; created: [rule: OrchRule] }>()

const orchestration = useOrchestration()

const whenOptions = WHEN_EVENT_VALUES.map((value) => ({ label: WHEN_EVENT_LABELS[value], value }))
const execOptions = EXEC_MODE_VALUES.map((value) => ({ label: EXEC_MODE_LABELS[value], value }))

// Muda a chave do formulário quando um salvamento falha, para o <select> voltar ao valor do snapshot.
const revision = ref(0)
const failure = ref<string | null>(null)
const submitting = ref(false)

const layerKindById = computed(() => new Map(props.layers.map((layer) => [layer.id, layer.kind])))
const layerOptions = computed(() => props.layers.map((layer) => ({ label: layer.name, value: layer.id })))

function draftOf(rule: OrchRule): OrchRuleDraft {
  return {
    id: rule.id,
    fromLayer: rule.fromLayer,
    toLayer: rule.toLayer,
    whenEvent: rule.whenEvent,
    action: rule.action,
    kind: rule.kind,
    execMode: rule.execMode,
    returnLimit: rule.returnLimit,
  }
}

// Cada campo grava a seta inteira; se o banco recusar, o formulário volta ao snapshot.
async function saveField(patch: Partial<OrchRuleDraft>): Promise<void> {
  if (!props.rule) return
  const saved = await orchestration.saveRule({ ...draftOf(props.rule), ...patch })
  if (!saved) revision.value += 1
}

const thenChoice = computed<ThenChoice | null>(() => (props.rule ? thenChoiceOf(props.rule) : null))
const thenOptions = computed(() =>
  props.rule
    ? thenChoicesFor(props.rule.toLayer).map((choice) => ({ label: THEN_CHOICE_LABELS[choice], value: choice }))
    : [],
)

function onThenChange(value: string | undefined): void {
  if (!props.rule) return
  const choice = value as ThenChoice
  const targetKind = layerKindById.value.get(props.rule.toLayer) as LayerKind | undefined
  const toLayer = choice === 'chamar_voce' ? VOCE_NODE : props.rule.toLayer
  void saveField({ ...actionAndKindOf(choice, targetKind), toLayer })
}

function onWhenChange(value: string | undefined): void {
  void saveField({ whenEvent: value as WhenEvent })
}

function onExecChange(value: ExecMode): void {
  void saveField({ execMode: value })
}

const returnLimit = computed({
  get: () => props.rule?.returnLimit ?? DEFAULT_RETURN_LIMIT,
  set: (value: number) => void saveField({ returnLimit: value }),
})

const phrase = computed(() => (props.rule ? rulePhrase(props.rule, props.layers) : ''))
const isDevolver = computed(() => props.rule?.action === 'devolver')

async function removeRule(): Promise<void> {
  if (!props.rule) return
  await orchestration.deleteRule(props.rule.id)
}

// Formulário de nova seta.
const fromId = ref('')
const toId = ref('')
const whenEvent = ref<string>(INITIAL_WHEN_EVENT)
const thenNew = ref<string>('delegar')
const execMode = ref<ExecMode>(INITIAL_EXEC_MODE)
const newReturnLimit = ref(DEFAULT_RETURN_LIMIT)

const fromOptions = computed(() => layerOptions.value)
const toOptions = computed(() => [
  ...props.layers.filter((layer) => layer.id !== fromId.value).map((layer) => ({ label: layer.name, value: layer.id })),
  { label: VIEW_COPY.you, value: VOCE_NODE },
])
const newThenOptions = computed(() =>
  thenChoicesFor(toId.value).map((choice) => ({ label: THEN_CHOICE_LABELS[choice], value: choice })),
)
const newThenChoice = computed(() => thenNew.value as ThenChoice)
const canCreate = computed(() => fromId.value !== '' && toId.value !== '' && fromId.value !== toId.value)

function initNewForm(): void {
  const [first, second] = props.layers
  fromId.value = first?.id ?? ''
  toId.value = second?.id ?? (first ? VOCE_NODE : '')
  whenEvent.value = INITIAL_WHEN_EVENT
  thenNew.value = toId.value === VOCE_NODE ? 'chamar_voce' : 'delegar'
  execMode.value = INITIAL_EXEC_MODE
  newReturnLimit.value = DEFAULT_RETURN_LIMIT
  failure.value = null
}
initNewForm()

function onFromChange(value: string | undefined): void {
  const next = value ?? ''
  fromId.value = next
  if (toId.value === next) toId.value = ''
}

function onToChange(value: string | undefined): void {
  const next = value ?? ''
  const previous = toId.value
  toId.value = next
  if (next === VOCE_NODE) thenNew.value = 'chamar_voce'
  else if (previous === VOCE_NODE || thenNew.value === 'chamar_voce') thenNew.value = 'delegar'
}

function onNewThenChange(value: string | undefined): void {
  const choice = (value ?? '') as ThenChoice
  thenNew.value = choice
  if (choice === 'chamar_voce') toId.value = VOCE_NODE
}

function onNewWhenChange(value: string | undefined): void {
  whenEvent.value = value ?? ''
}

async function createRule(): Promise<void> {
  if (!canCreate.value || submitting.value) return
  const projectId = orchestration.loadedProjectId
  if (projectId === null) return
  submitting.value = true
  failure.value = null
  try {
    const targetKind = layerKindById.value.get(toId.value) as LayerKind | undefined
    const created = await saveRule(projectId, {
      id: null,
      fromLayer: fromId.value,
      toLayer: toId.value,
      whenEvent: whenEvent.value as WhenEvent,
      execMode: execMode.value,
      returnLimit: newReturnLimit.value,
      ...actionAndKindOf(newThenChoice.value, targetKind),
    })
    emit('created', created)
  } catch (error) {
    failure.value = String(error)
  } finally {
    submitting.value = false
  }
}

const newIsDevolver = computed(() => thenNew.value === 'devolver')
</script>

<template>
  <section class="flex min-w-0 flex-col gap-4 rounded-[24px] border border-line bg-surface p-4.5">
    <header class="flex items-center justify-between gap-3">
      <h3 class="text-control font-bold text-ink">{{ rule ? VIEW_COPY.ruleTitle : VIEW_COPY.newArrowTitle }}</h3>
      <UiButton variant="quiet" size="sm" @press="emit('close')">{{ VIEW_COPY.close }}</UiButton>
    </header>

    <div v-if="rule" :key="revision" class="flex flex-col gap-4">
      <label class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.when }}</span>
        <UiSelect :model-value="rule.whenEvent" :options="whenOptions" :label="VIEW_COPY.when" @update:model-value="onWhenChange" />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.then }}</span>
        <UiSelect :model-value="thenChoice ?? ''" :options="thenOptions" :label="VIEW_COPY.then" @update:model-value="onThenChange" />
      </label>
      <div class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.exec }}</span>
        <UiSegmented :model-value="rule.execMode" :options="execOptions" :label="VIEW_COPY.exec" wrap @update:model-value="onExecChange" />
      </div>
      <div v-if="isDevolver" class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.returns }}</span>
        <CountStepper v-model="returnLimit" :label="VIEW_COPY.returns" :min="RETURN_LIMIT_MIN" :max="RETURN_LIMIT_MAX" />
      </div>
      <p class="text-body text-ink">{{ phrase }}</p>
      <UiConfirm
        :label="VIEW_COPY.removeArrow"
        :message="VIEW_COPY.removeArrowConfirm"
        :confirm-label="VIEW_COPY.removeArrowAction"
        @confirm="removeRule()"
      />
    </div>

    <form v-else class="flex flex-col gap-4" @submit.prevent="createRule()">
      <label class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.from }}</span>
        <UiSelect :model-value="fromId" :options="fromOptions" :label="VIEW_COPY.from" @update:model-value="onFromChange" />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.to }}</span>
        <UiSelect :model-value="toId" :options="toOptions" :label="VIEW_COPY.to" @update:model-value="onToChange" />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.when }}</span>
        <UiSelect :model-value="whenEvent" :options="whenOptions" :label="VIEW_COPY.when" @update:model-value="onNewWhenChange" />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.then }}</span>
        <UiSelect :model-value="thenNew" :options="newThenOptions" :label="VIEW_COPY.then" @update:model-value="onNewThenChange" />
      </label>
      <div class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.exec }}</span>
        <UiSegmented v-model="execMode" :options="execOptions" :label="VIEW_COPY.exec" wrap />
      </div>
      <div v-if="newIsDevolver" class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.returns }}</span>
        <CountStepper v-model="newReturnLimit" :label="VIEW_COPY.returns" :min="RETURN_LIMIT_MIN" :max="RETURN_LIMIT_MAX" />
      </div>
      <UiNotice v-if="failure !== null" :text="failure" @close="failure = null" />
      <div class="flex justify-end gap-2">
        <UiButton variant="quiet" size="lg" @press="emit('close')">{{ VIEW_COPY.cancel }}</UiButton>
        <UiButton variant="primary" size="lg" :disabled="!canCreate || submitting" @press="createRule()">
          {{ VIEW_COPY.createArrow }}
        </UiButton>
      </div>
    </form>
  </section>
</template>
