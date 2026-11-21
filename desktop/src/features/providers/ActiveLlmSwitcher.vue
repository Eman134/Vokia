<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiCombobox from '../../ui/UiCombobox.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { agentName } from '../team/agents'
import type { ModelTier } from '../team/teamApi'
import { useFeed } from '../team/useFeed'
import { useRoster } from '../team/useRoster'
import { useActiveLlm } from './useActiveLlm'
import { useProviders } from './useProviders'

type Mode = 'tier' | 'model'

const EFFORT_OPTIONS: readonly { label: string; value: ModelTier }[] = [
  { label: 'Básico', value: 'basic' },
  { label: 'Intermediário', value: 'intermediate' },
  { label: 'Avançado', value: 'advanced' },
]

const TIER_LABELS: Record<ModelTier, string> = {
  basic: 'Básico',
  intermediate: 'Intermediário',
  advanced: 'Avançado',
}

const MODE_OPTIONS: readonly { label: string; value: Mode }[] = [
  { label: 'Nível', value: 'tier' },
  { label: 'Modelo', value: 'model' },
]

const open = defineModel<boolean>('open')

const activeLlm = useActiveLlm()
const providersStore = useProviders()
const roster = useRoster()
const feed = useFeed()

const selectedCli = ref(activeLlm.active.cli)
const mode = ref<Mode>(activeLlm.active.model ? 'model' : 'tier')
const selectedTier = ref<ModelTier>(activeLlm.active.modelTier ?? 'intermediate')
const freeModel = ref(activeLlm.active.model ?? '')
const busy = ref(false)
const feedback = ref<string | null>(null)

const currentProvider = computed(
  () =>
    providersStore.providers.find((p) => p.id === selectedCli.value) ??
    roster.presets.find((p) => p.id === selectedCli.value),
)

const providerOptions = computed(() => {
  if (providersStore.providers.length > 0) {
    return providersStore.providers.map((p) => ({ label: p.label, value: p.id }))
  }
  return roster.presets.map((p) => ({ label: p.label, value: p.id }))
})

const modelSuggestions = computed(() => currentProvider.value?.models ?? [])

const activeProviderLabel = computed(() => {
  const id = activeLlm.active.cli
  return (
    providersStore.providers.find((p) => p.id === id)?.label ??
    roster.presets.find((p) => p.id === id)?.label ??
    id
  )
})

const activeDetailLabel = computed(() => {
  if (activeLlm.active.model) return activeLlm.active.model
  const tier = activeLlm.active.modelTier ?? 'intermediate'
  return TIER_LABELS[tier]
})

const activeTooltip = computed(
  () => `Provedor ativo: ${activeProviderLabel.value} · ${activeDetailLabel.value}`,
)

const targetAgent = computed(() => {
  if (feed.screen.kind === 'agent') {
    return feed.screen.agent
  }
  return roster.agents.find((a) => !a.builtin)?.id ?? roster.agents[0]?.id ?? 'ember'
})

const emberRequiresMcpNotice = computed(
  () => currentProvider.value !== undefined && !currentProvider.value.supportsMcp,
)

watch(open, async (isOpen) => {
  if (!isOpen) return
  feedback.value = null
  if (!activeLlm.loaded) {
    await activeLlm.load()
  } else {
    await activeLlm.loadOverrides()
  }
  if (providersStore.providers.length === 0) {
    void providersStore.load().catch(() => {})
  }
  selectedCli.value = activeLlm.active.cli
  mode.value = activeLlm.active.model ? 'model' : 'tier'
  selectedTier.value = activeLlm.active.modelTier ?? 'intermediate'
  freeModel.value = activeLlm.active.model ?? currentProvider.value?.defaultModel ?? ''
})

function onKeydown(event: KeyboardEvent): void {
  if (open.value && event.key === 'Escape' && !event.defaultPrevented) {
    open.value = false
  }
}

onMounted(() => {
  if (!activeLlm.loaded) {
    void activeLlm.load()
  }
  window.addEventListener('keydown', onKeydown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
})

async function applyGlobal(): Promise<void> {
  busy.value = true
  feedback.value = null
  try {
    await activeLlm.saveActive({
      cli: selectedCli.value,
      modelTier: mode.value === 'tier' ? selectedTier.value : null,
      model: mode.value === 'model' ? freeModel.value.trim() || null : null,
    })
    feedback.value = 'Padrão global atualizado.'
  } catch (err) {
    feedback.value = String(err)
  } finally {
    busy.value = false
  }
}

async function applyTempAgent(): Promise<void> {
  busy.value = true
  feedback.value = null
  try {
    await activeLlm.applyOverride({
      agent: targetAgent.value,
      cli: selectedCli.value,
      modelTier: mode.value === 'tier' ? selectedTier.value : null,
      model: mode.value === 'model' ? freeModel.value.trim() || null : null,
      reason: 'manual-temp',
    })
    feedback.value = `Temporário em ${agentName(targetAgent.value)}.`
  } catch (err) {
    feedback.value = String(err)
  } finally {
    busy.value = false
  }
}

function openSetup(): void {
  open.value = false
  providersStore.openSetup()
}
</script>

<template>
  <div class="relative inline-flex items-center">
    <UiButton
      variant="quiet"
      size="sm"
      aria-label="Selecionar provedor ativo"
      :title="activeTooltip"
      aria-haspopup="dialog"
      :aria-expanded="open"
      data-testid="active-llm-switcher"
      @press="open = !open"
    >
      <span class="relative inline-flex items-center">
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.4"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <rect x="4" y="4" width="16" height="16" rx="2" />
          <rect x="9" y="9" width="6" height="6" />
          <path d="M9 1v3M15 1v3M9 20v3M15 20v3M20 9h3M20 14h3M1 9h3M1 14h3" />
        </svg>
        <span
          v-if="activeLlm.overrides.length > 0"
          class="absolute -top-0.5 -right-0.5 size-2 rounded-full bg-warn-text"
          aria-hidden="true"
        />
      </span>
    </UiButton>

    <div
      v-if="open"
      data-testid="active-llm-popover"
      role="dialog"
      aria-label="Selecionar provedor ativo"
      class="absolute left-0 bottom-full z-40 pb-2"
    >
      <div class="flex w-76 flex-col gap-3 rounded-popover border border-line bg-surface p-3.5 shadow-float">
        <div class="flex items-center justify-between gap-2">
          <div class="min-w-0">
            <span class="block text-small text-muted">Ativo agora</span>
            <span class="block truncate text-caption font-semibold text-ink">
              {{ activeProviderLabel }} · {{ activeDetailLabel }}
            </span>
          </div>
          <button
            type="button"
            class="shrink-0 cursor-pointer text-small text-accent hover:underline"
            @click="openSetup()"
          >
            /setup
          </button>
        </div>

        <UiSelect v-model="selectedCli" :options="providerOptions" label="Provedor Padrão" />

        <UiSegmented v-model="mode" :options="MODE_OPTIONS" label="Modo de seleção" />

        <UiSegmented
          v-if="mode === 'tier'"
          v-model="selectedTier"
          :options="EFFORT_OPTIONS"
          label="Nível de Esforço/Inteligência"
        />

        <UiCombobox
          v-else
          v-model="freeModel"
          :options="modelSuggestions"
          placeholder="Digite ou escolha um modelo"
        />

        <div class="flex items-center justify-between gap-2">
          <UiButton variant="quiet" size="sm" :disabled="busy" @press="applyTempAgent()">
            Só este agente
          </UiButton>
          <UiButton variant="primary" size="sm" :disabled="busy" @press="applyGlobal()">
            Aplicar global
          </UiButton>
        </div>

        <p v-if="emberRequiresMcpNotice" class="text-small text-muted">
          Ember exige MCP e continua no Claude; agentes no padrão global usam {{ currentProvider?.label }}.
        </p>

        <p v-if="feedback" class="text-small text-muted">{{ feedback }}</p>

        <div v-if="activeLlm.overrides.length > 0" class="flex flex-col gap-1.5 border-t border-line pt-2.5">
          <div class="flex items-center justify-between">
            <span class="text-small font-semibold text-warn-text">
              Temporários ({{ activeLlm.overrides.length }})
            </span>
            <button
              type="button"
              class="cursor-pointer text-small text-accent hover:underline"
              @click="activeLlm.clearAllOverrides()"
            >
              Restaurar todos
            </button>
          </div>
          <ul class="flex flex-col gap-1 text-small text-muted">
            <li v-for="ov in activeLlm.overrides" :key="ov.agent" class="flex items-center justify-between gap-2">
              <span class="truncate">{{ agentName(ov.agent) }} · {{ ov.cli }}</span>
              <button
                type="button"
                class="shrink-0 cursor-pointer text-ink hover:underline"
                @click="activeLlm.clearOverride(ov.agent)"
              >
                Restaurar
              </button>
            </li>
          </ul>
        </div>
      </div>
    </div>
  </div>
</template>
