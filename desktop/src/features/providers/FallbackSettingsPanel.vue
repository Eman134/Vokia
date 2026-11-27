<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiSelect from '../../ui/UiSelect.vue'
import UiSkeleton from '../../ui/UiSkeleton.vue'
import UiSwitch from '../../ui/UiSwitch.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import type { OverrideReason } from '../team/events'
import type { ModelTier, ReasoningEffort } from '../team/teamApi'
import { EFFORT_LABELS, REASONING_EFFORT_OPTIONS, normalizeReasoningEffort } from './providerModelCatalog'
import { useActiveLlm, type FallbackChainItem } from './useActiveLlm'
import { useProviders } from './useProviders'

type TargetMode = 'tier' | 'model'

const PANEL_CLASSES = 'flex flex-col gap-5 rounded-md border border-line bg-surface p-5'
const CARD_CLASSES = 'rounded-md border border-line bg-surface-2/50 p-3.5'
const STEP_CARD_CLASSES = 'rounded-md border border-line bg-surface p-3 transition-colors'
const CHIP_BASE_CLASSES = `cursor-pointer rounded-md border px-2.5 py-1 text-small font-medium transition-colors ${INK_FOCUS_CLASSES}`
const CHIP_ACTIVE_CLASSES = 'border-accent bg-accent-soft text-accent'
const CHIP_IDLE_CLASSES = 'border-line bg-surface text-muted hover:bg-hover hover:text-ink'
const INPUT_CLASSES = `h-8 rounded-md border border-line bg-surface px-2.5 text-control text-ink ${INK_FOCUS_CLASSES}`

const TIER_OPTIONS: readonly { label: string; value: ModelTier }[] = [
  { label: 'Básico', value: 'basic' },
  { label: 'Intermediário', value: 'intermediate' },
  { label: 'Avançado', value: 'advanced' },
]

const TIER_LABELS: Record<ModelTier, string> = {
  basic: 'Básico',
  intermediate: 'Intermediário',
  advanced: 'Avançado',
}

const MODE_OPTIONS: readonly { label: string; value: TargetMode }[] = [
  { label: 'Por nível', value: 'tier' },
  { label: 'Modelo específico', value: 'model' },
]

const EFFORT_SELECT_OPTIONS: readonly { label: string; value: string }[] = [
  { label: 'Padrão do provedor', value: '' },
  ...REASONING_EFFORT_OPTIONS.map((opt) => ({ label: opt.label, value: opt.value })),
]

const COOLDOWN_PRESETS: readonly { label: string; secs: number }[] = [
  { label: '1 min', secs: 60 },
  { label: '2 min', secs: 120 },
  { label: '5 min', secs: 300 },
  { label: '15 min', secs: 900 },
]

const REASON_LABELS: Record<OverrideReason, string> = {
  'usage-limit': 'Limite de uso',
  'model-unavailable': 'Modelo indisponível',
  'manual-temp': 'Manual temporário',
}

const providersStore = useProviders()
const activeLlm = useActiveLlm()

const enabled = ref(true)
const cooldownSecs = ref(300)
const chain = ref<FallbackChainItem[]>([])
const expandedIndex = ref<number | null>(null)
const dragFromIndex = ref<number | null>(null)
const dragOverIndex = ref<number | null>(null)

const newCli = ref('')
const newMode = ref<TargetMode>('tier')
const newTier = ref<ModelTier>('intermediate')
const newModel = ref('')
const newEffort = ref('')

const initialLoading = ref(true)
const saving = ref(false)
const feedbackMsg = ref<string | null>(null)
const feedbackError = ref<string | null>(null)

function syncFromStore(): void {
  enabled.value = activeLlm.fallback.enabled
  cooldownSecs.value = activeLlm.fallback.cooldownSecs
  chain.value = activeLlm.fallback.chain.map((item) => ({ ...item }))
}

onMounted(async () => {
  try {
    await Promise.all([
      providersStore.load().catch(() => {}),
      activeLlm.load().catch(() => {}),
    ])
    syncFromStore()
    if (!newCli.value && providersStore.providers.length > 0) {
      newCli.value = providersStore.providers[0].id
    }
  } finally {
    initialLoading.value = false
  }
})

watch(
  () => activeLlm.fallback,
  () => {
    if (!hasUnsavedChanges.value) {
      syncFromStore()
    }
  },
  { deep: true },
)

const providerOptions = computed(() =>
  providersStore.providers.map((provider) => ({
    label: provider.installed ? provider.label : `${provider.label} (não instalado)`,
    value: provider.id,
  })),
)

const hasUnsavedChanges = computed(() => {
  const current = JSON.stringify({
    enabled: enabled.value,
    cooldownSecs: Math.max(0, Number(cooldownSecs.value) || 0),
    chain: chain.value,
  })
  const saved = JSON.stringify({
    enabled: activeLlm.fallback.enabled,
    cooldownSecs: activeLlm.fallback.cooldownSecs,
    chain: activeLlm.fallback.chain,
  })
  return current !== saved
})

function providerOf(cli: string) {
  return providersStore.providers.find((p) => p.id === cli)
}

function providerLabel(cli: string): string {
  return providerOf(cli)?.label ?? (cli || 'Não definido')
}

function isProviderInstalled(cli: string): boolean {
  return providerOf(cli)?.installed ?? false
}

function modelsForCli(cli: string): string[] {
  const provider = providerOf(cli)
  if (!provider) return []
  const list = [...provider.models]
  if (provider.defaultModel && !list.includes(provider.defaultModel)) {
    list.unshift(provider.defaultModel)
  }
  return list
}

function modelOptionsFor(cli: string): { label: string; value: string }[] {
  return modelsForCli(cli).map((model) => ({ label: model, value: model }))
}

function targetModeOf(item: FallbackChainItem): TargetMode {
  return item.model ? 'model' : 'tier'
}

function describeTarget(item: Pick<FallbackChainItem, 'cli' | 'modelTier' | 'model' | 'reasoningEffort'>): string {
  const parts: string[] = []
  if (item.model) {
    parts.push(item.model)
  } else if (item.modelTier) {
    parts.push(`Nível ${TIER_LABELS[item.modelTier]}`)
  } else {
    parts.push('Modelo padrão')
  }
  const effort = normalizeReasoningEffort(item.reasoningEffort)
  if (effort) {
    parts.push(`Raciocínio ${EFFORT_LABELS[effort]}`)
  }
  return parts.join(' · ')
}

function stepWarning(item: FallbackChainItem, index: number): string | null {
  if (!isProviderInstalled(item.cli)) {
    return 'Provedor não instalado ou desconectado.'
  }
  if (
    activeLlm.active.cli === item.cli &&
    (activeLlm.active.model ?? null) === (item.model ?? null) &&
    (activeLlm.active.modelTier ?? null) === (item.modelTier ?? null)
  ) {
    return 'Repete exatamente o modelo principal.'
  }
  const duplicateIndex = chain.value.findIndex(
    (other, idx) =>
      idx !== index &&
      other.cli === item.cli &&
      (other.model ?? null) === (item.model ?? null) &&
      (other.modelTier ?? null) === (item.modelTier ?? null),
  )
  if (duplicateIndex !== -1 && duplicateIndex < index) {
    return `Duplica o passo ${duplicateIndex + 1}.`
  }
  return null
}

function updateStepCli(index: number, cli: string): void {
  const next = [...chain.value]
  const current = next[index]
  if (!current) return
  const models = modelsForCli(cli)
  next[index] = {
    ...current,
    cli,
    model: current.model ? models[0] ?? null : null,
    modelTier: current.model ? null : current.modelTier ?? 'intermediate',
  }
  chain.value = next
}

function updateStepMode(index: number, mode: TargetMode): void {
  const next = [...chain.value]
  const current = next[index]
  if (!current) return
  const models = modelsForCli(current.cli)
  next[index] =
    mode === 'model'
      ? { ...current, model: current.model ?? models[0] ?? providerOf(current.cli)?.defaultModel ?? '', modelTier: null }
      : { ...current, model: null, modelTier: current.modelTier ?? 'intermediate' }
  chain.value = next
}

function updateStepTier(index: number, tier: ModelTier): void {
  const next = [...chain.value]
  const current = next[index]
  if (!current) return
  next[index] = { ...current, modelTier: tier, model: null }
  chain.value = next
}

function updateStepModel(index: number, model: string): void {
  const next = [...chain.value]
  const current = next[index]
  if (!current) return
  next[index] = { ...current, model: model.trim() || null, modelTier: null }
  chain.value = next
}

function updateStepEffort(index: number, effort: string): void {
  const next = [...chain.value]
  const current = next[index]
  if (!current) return
  next[index] = { ...current, reasoningEffort: (effort || null) as ReasoningEffort | null }
  chain.value = next
}

function moveStep(from: number, to: number): void {
  if (from === to || from < 0 || to < 0 || from >= chain.value.length || to >= chain.value.length) return
  const next = [...chain.value]
  const [item] = next.splice(from, 1)
  next.splice(to, 0, item)
  chain.value = next
  if (expandedIndex.value === from) {
    expandedIndex.value = to
  } else if (expandedIndex.value === to) {
    expandedIndex.value = from
  }
}

function removeStep(index: number): void {
  chain.value = chain.value.filter((_, idx) => idx !== index)
  if (expandedIndex.value === index) {
    expandedIndex.value = null
  } else if (expandedIndex.value !== null && expandedIndex.value > index) {
    expandedIndex.value -= 1
  }
}

function addStep(): void {
  if (!newCli.value) return
  const effort = (newEffort.value || null) as ReasoningEffort | null
  const models = modelsForCli(newCli.value)
  const item: FallbackChainItem =
    newMode.value === 'model'
      ? {
          cli: newCli.value,
          model: newModel.value.trim() || models[0] || providerOf(newCli.value)?.defaultModel || null,
          modelTier: null,
          reasoningEffort: effort,
        }
      : {
          cli: newCli.value,
          modelTier: newTier.value,
          model: null,
          reasoningEffort: effort,
        }
  chain.value = [...chain.value, item]
  expandedIndex.value = chain.value.length - 1
  feedbackMsg.value = null
  feedbackError.value = null
}

function onNewCliChange(cli: string | undefined): void {
  const next = cli ?? ''
  newCli.value = next
  if (newMode.value === 'model') {
    newModel.value = modelsForCli(next)[0] ?? ''
  }
}

function onNewModeChange(mode: TargetMode): void {
  newMode.value = mode
  if (mode === 'model' && !newModel.value) {
    newModel.value = modelsForCli(newCli.value)[0] ?? ''
  }
}

function onDragStart(index: number): void {
  dragFromIndex.value = index
}

function onDragOver(event: DragEvent, index: number): void {
  event.preventDefault()
  dragOverIndex.value = index
}

function onDrop(index: number): void {
  if (dragFromIndex.value !== null && dragFromIndex.value !== index) {
    moveStep(dragFromIndex.value, index)
  }
  dragFromIndex.value = null
  dragOverIndex.value = null
}

function onDragEnd(): void {
  dragFromIndex.value = null
  dragOverIndex.value = null
}

function discardChanges(): void {
  syncFromStore()
  expandedIndex.value = null
  feedbackMsg.value = null
  feedbackError.value = null
}

async function saveConfig(): Promise<void> {
  saving.value = true
  feedbackMsg.value = null
  feedbackError.value = null
  try {
    await activeLlm.saveFallback({
      enabled: enabled.value,
      cooldownSecs: Math.max(0, Number(cooldownSecs.value) || 0),
      chain: chain.value,
    })
    feedbackMsg.value = 'Cadeia de fallback salva.'
  } catch (error) {
    feedbackError.value = `Não foi possível salvar: ${String(error)}`
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div v-if="initialLoading" class="flex flex-col gap-4">
    <section :class="PANEL_CLASSES">
      <UiSkeleton type="heading, subtitle, list-item, card" />
    </section>
    <section :class="PANEL_CLASSES">
      <UiSkeleton type="heading, text" />
    </section>
  </div>
  <div v-else class="flex flex-col gap-4">
    <section data-testid="fallback-chain-editor" aria-label="Fallback e roteamento" :class="PANEL_CLASSES">
      <div class="flex flex-wrap items-start justify-between gap-3 border-b border-line pb-4">
        <div class="flex flex-col gap-1">
          <div class="flex items-center gap-2">
            <h3 class="text-control font-semibold text-ink">Cadeia de fallback automático</h3>
            <span
              class="rounded-sm px-2 py-0.5 text-tiny font-semibold"
              :class="enabled ? 'bg-ok-soft text-ok' : 'bg-surface-2 text-muted'"
            >
              {{ enabled ? `Ativo · ${chain.length} ${chain.length === 1 ? 'passo' : 'passos'}` : 'Desativado' }}
            </span>
          </div>
          <p class="text-caption text-muted">
            Quando o provedor principal atingir limite de uso ou recusar o modelo, os turnos seguem a ordem abaixo.
          </p>
        </div>
        <UiSwitch v-model="enabled" label="Ativar fallback automático" />
      </div>

      <!-- Cooldown -->
      <div class="flex flex-wrap items-center justify-between gap-3" :class="!enabled && 'opacity-50 pointer-events-none'">
        <div class="flex flex-col gap-0.5">
          <span class="text-label font-medium text-ink">Cooldown após falha ou limite</span>
          <span class="text-small text-muted">Tempo de espera antes de tentar o provedor original novamente.</span>
        </div>
        <div class="flex flex-wrap items-center gap-1.5">
          <button
            v-for="preset in COOLDOWN_PRESETS"
            :key="preset.secs"
            type="button"
            :class="[CHIP_BASE_CLASSES, cooldownSecs === preset.secs ? CHIP_ACTIVE_CLASSES : CHIP_IDLE_CLASSES]"
            @click="cooldownSecs = preset.secs"
          >
            {{ preset.label }}
          </button>
          <label class="ml-1 flex items-center gap-1.5 text-small text-muted">
            <input
              v-model.number="cooldownSecs"
              type="number"
              min="0"
              aria-label="Cooldown em segundos"
              class="w-20"
              :class="INPUT_CLASSES"
            />
            <span>s</span>
          </label>
        </div>
      </div>

      <!-- Priority Stack: Principal -> Passos -->
      <div class="flex flex-col gap-2" :class="!enabled && 'opacity-55'">
        <div :class="CARD_CLASSES" class="flex flex-wrap items-center justify-between gap-2">
          <div class="flex items-center gap-2.5">
            <span class="grid h-6 min-w-14 place-items-center rounded-sm bg-accent-soft px-2 text-tiny font-semibold text-accent">
              Principal
            </span>
            <span
              class="size-2 shrink-0 rounded-full"
              :class="isProviderInstalled(activeLlm.active.cli) ? 'bg-ok' : 'bg-warn'"
              aria-hidden="true"
            />
            <div class="flex flex-col">
              <span class="text-control font-semibold text-ink">{{ providerLabel(activeLlm.active.cli) }}</span>
              <span class="text-small text-muted">{{ describeTarget(activeLlm.active) }}</span>
            </div>
          </div>
          <UiButton variant="quiet" size="sm" @press="providersStore.openSetup()">
            Trocar principal
          </UiButton>
        </div>

        <div class="flex items-center gap-2 pl-4 text-tiny text-muted">
          <span class="h-4 w-px bg-line" aria-hidden="true" />
          <span>Se atingir limite ou falhar (cooldown {{ cooldownSecs }}s)</span>
        </div>

        <ul v-if="chain.length > 0" class="flex flex-col gap-2">
          <li
            v-for="(item, index) in chain"
            :key="`${item.cli}-${index}`"
            draggable="true"
            :class="[
              STEP_CARD_CLASSES,
              dragOverIndex === index ? 'border-accent bg-accent-soft/30' : undefined,
            ]"
            @dragstart="onDragStart(index)"
            @dragover="onDragOver($event, index)"
            @drop="onDrop(index)"
            @dragend="onDragEnd"
          >
            <div class="flex flex-wrap items-center justify-between gap-2">
              <div class="flex min-w-0 items-center gap-2.5">
                <span
                  class="cursor-grab text-muted select-none active:cursor-grabbing"
                  title="Arraste para reordenar"
                  aria-hidden="true"
                >
                  ⋮⋮
                </span>
                <span class="grid size-6 shrink-0 place-items-center rounded-sm bg-surface-2 font-mono text-tiny font-semibold text-ink">
                  {{ index + 1 }}
                </span>
                <span
                  class="size-2 shrink-0 rounded-full"
                  :class="isProviderInstalled(item.cli) ? 'bg-ok' : 'bg-warn'"
                  aria-hidden="true"
                />
                <div class="flex min-w-0 flex-col">
                  <span class="truncate text-control font-medium text-ink">{{ providerLabel(item.cli) }}</span>
                  <span class="truncate text-small text-muted">{{ describeTarget(item) }}</span>
                </div>
              </div>

              <div class="flex items-center gap-1">
                <UiButton
                  variant="quiet"
                  size="sm"
                  :disabled="index === 0"
                  aria-label="Mover passo para cima"
                  @press="moveStep(index, index - 1)"
                >
                  ↑
                </UiButton>
                <UiButton
                  variant="quiet"
                  size="sm"
                  :disabled="index === chain.length - 1"
                  aria-label="Mover passo para baixo"
                  @press="moveStep(index, index + 1)"
                >
                  ↓
                </UiButton>
                <UiButton
                  variant="quiet"
                  size="sm"
                  @press="expandedIndex = expandedIndex === index ? null : index"
                >
                  {{ expandedIndex === index ? 'Fechar' : 'Editar' }}
                </UiButton>
                <UiButton variant="danger" size="sm" @press="removeStep(index)">
                  Remover
                </UiButton>
              </div>
            </div>

            <p v-if="stepWarning(item, index)" class="mt-1.5 text-tiny font-medium text-warn-text">
              {{ stepWarning(item, index) }}
            </p>

            <!-- Edição inline do passo -->
            <div
              v-if="expandedIndex === index"
              class="mt-3 grid grid-cols-1 gap-2.5 border-t border-line pt-3 sm:grid-cols-2 lg:grid-cols-4"
            >
              <label class="flex flex-col gap-1 text-small text-muted">
                <span>Provedor</span>
                <UiSelect
                  :model-value="item.cli"
                  :options="providerOptions"
                  label="Provedor"
                  @update:model-value="updateStepCli(index, $event ?? '')"
                />
              </label>

              <label class="flex flex-col gap-1 text-small text-muted">
                <span>Seleção</span>
                <UiSelect
                  :model-value="targetModeOf(item)"
                  :options="MODE_OPTIONS"
                  label="Seleção"
                  @update:model-value="updateStepMode(index, $event as TargetMode)"
                />
              </label>

              <label v-if="targetModeOf(item) === 'tier'" class="flex flex-col gap-1 text-small text-muted">
                <span>Nível</span>
                <UiSelect
                  :model-value="item.modelTier ?? 'intermediate'"
                  :options="TIER_OPTIONS"
                  label="Nível"
                  @update:model-value="updateStepTier(index, $event as ModelTier)"
                />
              </label>

              <label v-else class="flex flex-col gap-1 text-small text-muted">
                <span>Modelo</span>
                <UiSelect
                  v-if="modelOptionsFor(item.cli).length > 0"
                  :model-value="item.model ?? ''"
                  :options="modelOptionsFor(item.cli)"
                  label="Modelo"
                  @update:model-value="updateStepModel(index, $event ?? '')"
                />
                <input
                  v-else
                  :value="item.model ?? ''"
                  type="text"
                  placeholder="ID do modelo"
                  :class="INPUT_CLASSES"
                  @input="updateStepModel(index, ($event.target as HTMLInputElement).value)"
                />
              </label>

              <label class="flex flex-col gap-1 text-small text-muted">
                <span>Raciocínio</span>
                <UiSelect
                  :model-value="item.reasoningEffort ?? ''"
                  :options="EFFORT_SELECT_OPTIONS"
                  label="Raciocínio"
                  @update:model-value="updateStepEffort(index, $event ?? '')"
                />
              </label>
            </div>
          </li>
        </ul>
        <div v-else class="rounded-md border border-dashed border-line px-4 py-5 text-center text-caption text-muted">
          Nenhum passo na cadeia de fallback. Adicione um provedor abaixo para garantir continuidade automática.
        </div>
      </div>

      <!-- Construtor rápido -->
      <div
        data-testid="fallback-add-row"
        class="flex flex-wrap items-end gap-2 rounded-md border border-line bg-surface-2/40 p-3"
      >
        <label class="flex flex-col gap-1 text-small text-muted">
          <span>Provedor</span>
          <UiSelect :model-value="newCli" :options="providerOptions" label="Provedor de fallback" @update:model-value="onNewCliChange" />
        </label>
        <label class="flex flex-col gap-1 text-small text-muted">
          <span>Modo</span>
          <UiSelect
            :model-value="newMode"
            :options="MODE_OPTIONS"
            label="Modo de seleção"
            @update:model-value="onNewModeChange($event as TargetMode)"
          />
        </label>
        <label v-if="newMode === 'tier'" class="flex flex-col gap-1 text-small text-muted">
          <span>Nível</span>
          <UiSelect v-model="newTier" :options="TIER_OPTIONS" label="Nível de fallback" />
        </label>
        <label v-else class="flex flex-col gap-1 text-small text-muted">
          <span>Modelo</span>
          <UiSelect
            v-if="modelOptionsFor(newCli).length > 0"
            v-model="newModel"
            :options="modelOptionsFor(newCli)"
            label="Modelo de fallback"
          />
          <input
            v-else
            v-model="newModel"
            type="text"
            placeholder="ID do modelo"
            :class="INPUT_CLASSES"
          />
        </label>
        <label class="flex flex-col gap-1 text-small text-muted">
          <span>Raciocínio</span>
          <UiSelect v-model="newEffort" :options="EFFORT_SELECT_OPTIONS" label="Raciocínio" />
        </label>
        <UiButton variant="quiet" size="sm" :disabled="!newCli" @press="addStep()">
          + Adicionar passo
        </UiButton>
      </div>

      <!-- Preview da rota + Ações de salvamento -->
      <div class="flex flex-wrap items-center justify-between gap-3 border-t border-line pt-3">
        <div class="flex min-w-0 flex-wrap items-center gap-1.5 text-small text-muted">
          <span class="font-medium text-ink">Rota:</span>
          <span>{{ providerLabel(activeLlm.active.cli) }}</span>
          <template v-for="(item, idx) in chain" :key="`route-${idx}`">
            <span aria-hidden="true">→</span>
            <span>{{ providerLabel(item.cli) }}</span>
          </template>
        </div>

        <div class="flex items-center gap-2">
          <span v-if="hasUnsavedChanges" class="text-small font-medium text-warn-text">
            Alterações não salvas
          </span>
          <UiButton
            v-if="hasUnsavedChanges"
            variant="quiet"
            size="sm"
            :disabled="saving"
            @press="discardChanges()"
          >
            Descartar
          </UiButton>
          <UiButton
            variant="primary"
            size="sm"
            :disabled="saving || !hasUnsavedChanges"
            @press="saveConfig()"
          >
            {{ saving ? 'Salvando...' : 'Salvar cadeia' }}
          </UiButton>
        </div>
      </div>

      <p v-if="feedbackMsg" class="text-small text-ok">{{ feedbackMsg }}</p>
      <p v-if="feedbackError" role="alert" class="text-small text-danger">{{ feedbackError }}</p>
    </section>

    <!-- Desvios temporários ativos -->
    <section
      aria-label="Desvios temporários ativos"
      class="flex flex-col gap-3 rounded-md border border-line bg-surface p-5"
    >
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 class="text-control font-semibold text-ink">Desvios temporários ativos</h3>
          <p class="text-caption text-muted">
            Agentes que estão usando temporariamente outro provedor/modelo nesta sessão.
          </p>
        </div>
        <UiButton
          v-if="activeLlm.overrides.length > 0"
          variant="quiet"
          size="sm"
          @press="activeLlm.clearAllOverrides()"
        >
          Restaurar todos
        </UiButton>
      </div>

      <ul v-if="activeLlm.overrides.length > 0" class="flex flex-col divide-y divide-line rounded-md border border-line">
        <li
          v-for="ov in activeLlm.overrides"
          :key="ov.agent"
          class="flex flex-wrap items-center justify-between gap-2 px-3 py-2.5 text-caption"
        >
          <div class="flex flex-col gap-0.5">
            <div class="flex items-center gap-2">
              <span class="font-semibold text-ink">{{ ov.agent }}</span>
              <span class="rounded-sm bg-warn-soft px-1.5 py-0.5 text-tiny font-medium text-warn-text">
                {{ REASON_LABELS[ov.reason] }}
              </span>
            </div>
            <span class="text-small text-muted">
              {{ providerLabel(ov.originalCli) }} ({{ ov.originalModel }}) → {{ providerLabel(ov.cli) }} ({{ ov.model }})
            </span>
          </div>
          <UiButton variant="quiet" size="sm" @press="activeLlm.clearOverride(ov.agent)">
            Restaurar
          </UiButton>
        </li>
      </ul>
      <p v-else class="text-caption text-muted">Nenhum agente em fallback temporário no momento.</p>
    </section>
  </div>
</template>
