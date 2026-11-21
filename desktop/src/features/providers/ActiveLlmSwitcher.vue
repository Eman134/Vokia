<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { useRoster } from '../team/useRoster'
import { useActiveLlm } from './useActiveLlm'
import { useProviders } from './useProviders'

const open = defineModel<boolean>('open')

const activeLlm = useActiveLlm()
const providersStore = useProviders()
const roster = useRoster()
const busy = ref(false)

const providerOptions = computed(() => {
  if (providersStore.providers.length > 0) {
    return providersStore.providers.map((p) => ({ label: p.label, value: p.id }))
  }
  return roster.presets.map((p) => ({ label: p.label, value: p.id }))
})

const activeProviderLabel = computed(() => {
  const id = activeLlm.active.cli
  return (
    providersStore.providers.find((p) => p.id === id)?.label ??
    roster.presets.find((p) => p.id === id)?.label ??
    id
  )
})

const activeTooltip = computed(() => `Provedor ativo: ${activeProviderLabel.value}`)

watch(open, async (isOpen) => {
  if (!isOpen) return
  if (!activeLlm.loaded) {
    await activeLlm.load()
  } else {
    await activeLlm.loadOverrides()
  }
  if (providersStore.providers.length === 0) {
    void providersStore.load().catch(() => {})
  }
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

async function selectProvider(cli: string | undefined): Promise<void> {
  if (!cli || cli === activeLlm.active.cli || busy.value) return
  busy.value = true
  try {
    await activeLlm.saveActive({
      cli,
      modelTier: activeLlm.active.modelTier ?? 'intermediate',
      model: null,
    })
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
      class="w-7 px-0"
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
      <div class="flex w-64 flex-col gap-2.5 rounded-popover border border-line bg-surface p-3 shadow-float">
        <div class="flex items-center justify-between gap-2">
          <div class="min-w-0">
            <span class="block text-small text-muted">Provedor atual</span>
            <span class="block truncate text-caption font-semibold text-ink">
              {{ activeProviderLabel }}
            </span>
          </div>
          <button
            type="button"
            title="Configurar provedores"
            aria-label="Configurar provedores"
            class="inline-flex shrink-0 cursor-pointer items-center gap-1 rounded-control px-2 py-1 text-small font-medium text-muted hover:bg-hover hover:text-ink focus:outline-2 focus:outline-solid focus:outline-accent"
            @click="openSetup()"
          >
            <span>Setup</span>
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
              <polyline points="15 3 21 3 21 9" />
              <line x1="10" y1="14" x2="21" y2="3" />
            </svg>
          </button>
        </div>

        <UiSelect
          :model-value="activeLlm.active.cli"
          :options="providerOptions"
          :disabled="busy"
          label="Alterar provedor"
          @update:model-value="selectProvider($event)"
        />

        <div
          v-if="activeLlm.overrides.length > 0"
          class="flex items-center justify-between border-t border-line pt-2 text-small"
        >
          <span class="text-warn-text">{{ activeLlm.overrides.length }} temporário(s)</span>
          <button
            type="button"
            class="cursor-pointer text-ink hover:underline"
            @click="activeLlm.clearAllOverrides()"
          >
            Restaurar
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
