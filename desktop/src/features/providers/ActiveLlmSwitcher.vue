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
      class="w-7 px-0"
      aria-label="Selecionar provedor, modelo e esforço"
      :title="activeTooltip"
      aria-haspopup="dialog"
      :aria-expanded="Boolean(open)"
      data-testid="active-llm-switcher"
      @press="togglePicker()"
    >
      <span class="relative inline-flex items-center">
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.55"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="M5.2 4.4 Q12 3.8 18.8 4.5 Q19.5 12 18.9 19.3 Q12 19.9 4.9 19.2 Q4.3 12 5.2 4.4" />
          <path d="M9.4 9.2 Q12 8.8 14.7 9.3 Q15 12 14.6 14.8 Q12 15.1 9.2 14.7 Q8.9 12 9.4 9.2" />
          <path d="M9.2 1.6 Q9 2.9 9.3 4.2 M14.8 1.5 Q15.1 2.8 14.7 4.3 M9.1 19.6 Q9.4 21 9 22.3 M14.7 19.7 Q14.4 21 14.9 22.4 M19.4 9.1 Q20.9 8.9 22.3 9.3 M19.3 14.7 Q20.9 15 22.4 14.6 M1.6 9.3 Q3 8.9 4.5 9.2 M1.7 14.8 Q3.1 14.5 4.4 14.9" />
        </svg>
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
