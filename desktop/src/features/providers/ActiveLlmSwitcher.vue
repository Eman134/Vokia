<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useNavigation } from '../../shell/useNavigation'
import UiButton from '../../ui/UiButton.vue'
import { AGENTS_SECTION_TITLE } from '../settings/configuracoesSample'
import { useRoster } from '../team/useRoster'
import ProviderModelPickerModal, { type ModelSelectionPayload } from './ProviderModelPickerModal.vue'
import { EFFORT_LABELS } from './providerModelCatalog'
import { useActiveLlm } from './useActiveLlm'
import { useProviders } from './useProviders'

const open = defineModel<boolean>('open')

const activeLlm = useActiveLlm()
const providersStore = useProviders()
const roster = useRoster()
const navigation = useNavigation()

const busy = ref(false)

const effectiveProviderId = computed(() => {
  if (activeLlm.active.cli) return activeLlm.active.cli
  const installed = providersStore.providers.find((p) => p.installed)
  return installed?.id ?? providersStore.providers[0]?.id ?? roster.presets[0]?.id ?? ''
})

const activeProvider = computed(() => {
  const id = effectiveProviderId.value
  if (!id) return null
  const fromStore = providersStore.providers.find((p) => p.id === id)
  if (fromStore) {
    return {
      id: fromStore.id,
      label: fromStore.label,
      defaultModel: fromStore.defaultModel,
      tiers: fromStore.tiers,
    }
  }
  const fromPreset = roster.presets.find((p) => p.id === id)
  if (fromPreset) {
    return {
      id: fromPreset.id,
      label: fromPreset.label,
      defaultModel: fromPreset.defaultModel,
      tiers: fromPreset.tiers,
    }
  }
  return null
})

const activeProviderLabel = computed(() => activeProvider.value?.label ?? effectiveProviderId.value)

const activeModelLabel = computed(() => {
  const provider = activeProvider.value
  if (!provider) {
    return activeLlm.active.model ?? activeLlm.active.modelTier ?? ''
  }
  if (activeLlm.active.modelTier) {
    const fromTier = provider.tiers[activeLlm.active.modelTier]?.trim()
    if (fromTier) return fromTier
  }
  const explicit = activeLlm.active.model?.trim()
  if (explicit) return explicit
  return provider.defaultModel.trim()
})

const activeTooltip = computed(() => {
  if (!activeProviderLabel.value) return 'Selecionar provedor, modelo e esforço'
  const parts = [`Provedor ativo: ${activeProviderLabel.value}`]
  if (activeModelLabel.value) parts.push(activeModelLabel.value)
  if (activeLlm.active.reasoningEffort) {
    parts.push(`Esforço: ${EFFORT_LABELS[activeLlm.active.reasoningEffort]}`)
  }
  return parts.join(' · ')
})

watch(open, async (isOpen) => {
  if (!isOpen) return
  if (!activeLlm.loaded) {
    await activeLlm.load()
  } else {
    void activeLlm.loadOverrides()
  }
  if (providersStore.providers.length === 0) {
    void providersStore.load().catch(() => {})
  }
})

onMounted(() => {
  if (!activeLlm.loaded) {
    void activeLlm.load()
  }
})

function togglePicker(): void {
  open.value = !open.value
}

function closePicker(): void {
  if (busy.value) return
  open.value = false
}

async function handleSelect(payload: ModelSelectionPayload): Promise<void> {
  if (busy.value) return
  if (
    activeLlm.active.cli === payload.cli &&
    activeLlm.active.modelTier == null &&
    activeLlm.active.model === payload.model &&
    (activeLlm.active.reasoningEffort ?? null) === (payload.reasoningEffort ?? null)
  ) {
    open.value = false
    return
  }
  busy.value = true
  try {
    await activeLlm.saveActive({
      cli: payload.cli,
      modelTier: null,
      model: payload.model,
      reasoningEffort: payload.reasoningEffort,
    })
    open.value = false
  } finally {
    busy.value = false
  }
}

function handleOpenBackup(): void {
  open.value = false
  navigation.openSettingsSection(AGENTS_SECTION_TITLE)
}
</script>

<template>
  <div class="relative inline-flex items-center">
    <UiButton
      variant="quiet"
      size="sm"
      class="gap-1.5 px-2 max-sm:w-7 max-sm:px-0"
      aria-label="Selecionar provedor, modelo e esforço"
      :title="activeTooltip"
      aria-haspopup="dialog"
      :aria-expanded="Boolean(open)"
      data-testid="active-llm-switcher"
      @press="togglePicker()"
    >
      <span class="relative inline-flex items-center gap-1.5">
        <svg
          width="15"
          height="15"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="shrink-0"
          aria-hidden="true"
        >
          <rect x="4" y="4" width="16" height="16" rx="2" />
          <rect x="9" y="9" width="6" height="6" />
          <path d="M9 1v3M15 1v3M9 20v3M15 20v3M20 9h3M20 14h3M1 9h3M1 14h3" />
        </svg>
        <span
          v-if="activeModelLabel || activeProviderLabel"
          class="max-w-28 truncate text-caption font-medium max-sm:hidden"
        >{{ activeModelLabel || activeProviderLabel }}</span>
        <span
          v-if="activeLlm.overrides.length > 0"
          class="absolute -top-0.5 -right-0.5 size-2 rounded-full bg-warn-text"
          aria-hidden="true"
        />
      </span>
    </UiButton>

    <ProviderModelPickerModal
      :open="Boolean(open)"
      :selected-provider-id="effectiveProviderId"
      :selected-model="activeLlm.active.model ?? null"
      :selected-model-tier="activeLlm.active.modelTier ?? null"
      :selected-effort="activeLlm.active.reasoningEffort ?? null"
      :show-backup-shortcut="true"
      :show-overrides="true"
      :busy="busy"
      @close="closePicker()"
      @select="handleSelect"
      @open-backup="handleOpenBackup()"
    />
  </div>
</template>
