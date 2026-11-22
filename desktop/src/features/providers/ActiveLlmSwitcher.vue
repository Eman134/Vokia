<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { trapFocus } from '../backlog/dialogFocus'
import type { ModelTier, ModelTiers } from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import { useActiveLlm } from './useActiveLlm'
import { useProviders } from './useProviders'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
export interface CatalogModelItem {
  providerId: string
  providerLabel: string
  installed: boolean
  modelId: string
  isFree: boolean
  tierLabels: string[]
  isActive: boolean
}

export interface CatalogProviderGroup {
  id: string
  label: string
  installed: boolean
  discovering: boolean
  models: CatalogModelItem[]
}

const TIER_BADGE_LABELS: Record<ModelTier, string> = {
  basic: 'Básico',
  intermediate: 'Intermediário',
  advanced: 'Avançado',
}

const open = defineModel<boolean>('open')

const activeLlm = useActiveLlm()
const providersStore = useProviders()
const roster = useRoster()
const titleId = useId()

const modelModalOpen = ref(false)
const rootRef = ref<HTMLElement>()
const dialogRef = ref<HTMLElement>()
const searchInputRef = ref<HTMLInputElement>()
const previousFocus = ref<HTMLElement | null>(null)
const busy = ref(false)
const searchQuery = ref('')
const highlightedKey = ref<string | null>(null)

function isFreeModel(modelId: string, explicitFreeModels?: readonly string[]): boolean {
  if (explicitFreeModels?.includes(modelId)) return true
  const lower = modelId.toLowerCase()
  return (
    lower.endsWith(':free') ||
    lower.endsWith('-free') ||
    lower.endsWith('/free') ||
    /\bfree\b/.test(lower)
  )
}

function collectProviderModels(
  models: readonly string[],
  defaultModel: string,
  tiers: ModelTiers,
  currentActiveModel: string | null,
): string[] {
  const seen = new Set<string>()
  const push = (val: string | null | undefined) => {
    const trimmed = val?.trim()
    if (trimmed && !seen.has(trimmed)) {
      seen.add(trimmed)
    }
  }
  for (const m of models) push(m)
  push(tiers.advanced)
  push(tiers.intermediate)
  push(tiers.basic)
  push(defaultModel)
  push(currentActiveModel)
  return Array.from(seen)
}

function resolveActiveModelForProvider(
  providerId: string,
  defaultModel: string,
  tiers: ModelTiers,
): string | null {
  if (activeLlm.active.cli !== providerId) return null
  if (activeLlm.active.modelTier) {
    const fromTier = tiers[activeLlm.active.modelTier]?.trim()
    if (fromTier) return fromTier
  }
  const explicit = activeLlm.active.model?.trim()
  if (explicit) return explicit
  return defaultModel.trim() || null
}

function tierLabelsForModel(modelId: string, tiers: ModelTiers): string[] {
  const labels: string[] = []
  if (tiers.basic?.trim() === modelId) labels.push(TIER_BADGE_LABELS.basic)
  if (tiers.intermediate?.trim() === modelId) labels.push(TIER_BADGE_LABELS.intermediate)
  if (tiers.advanced?.trim() === modelId) labels.push(TIER_BADGE_LABELS.advanced)
  return labels
}

const rawProviders = computed(() => {
  if (providersStore.providers.length > 0) {
    return providersStore.providers.map((p) => ({
      id: p.id,
      label: p.label,
      installed: p.installed,
      models: p.models,
      freeModels: p.freeModels ?? [],
      defaultModel: p.defaultModel,
      tiers: p.tiers,
    }))
  }
  return roster.presets.map((p) => ({
    id: p.id,
    label: p.label,
    installed: true,
    models: p.models,
    freeModels: [] as string[],
    defaultModel: p.defaultModel,
    tiers: p.tiers,
  }))
})

const providerOptions = computed(() =>
  rawProviders.value.map((p) => ({ label: p.label, value: p.id })),
)

const activeProviderLabel = computed(() => {
  const id = activeLlm.active.cli
  return rawProviders.value.find((p) => p.id === id)?.label ?? id
})

const activeModelLabel = computed(() => {
  const provider = rawProviders.value.find((p) => p.id === activeLlm.active.cli)
  if (!provider) {
    return activeLlm.active.model ?? activeLlm.active.modelTier ?? ''
  }
  return (
    resolveActiveModelForProvider(provider.id, provider.defaultModel, provider.tiers) ??
    provider.defaultModel
  )
})

const activeTooltip = computed(() =>
  activeModelLabel.value
    ? `Provedor ativo: ${activeProviderLabel.value} · ${activeModelLabel.value}`
    : `Provedor ativo: ${activeProviderLabel.value}`,
)

const catalogGroups = computed<CatalogProviderGroup[]>(() => {
  const query = searchQuery.value.trim().toLowerCase()
  const sortedProviders = [...rawProviders.value].sort((a, b) => {
    if (a.installed !== b.installed) return a.installed ? -1 : 1
    return 0
  })

  const groups: CatalogProviderGroup[] = []
  for (const provider of sortedProviders) {
    const activeModel = resolveActiveModelForProvider(
      provider.id,
      provider.defaultModel,
      provider.tiers,
    )
    const allModels = collectProviderModels(
      provider.models,
      provider.defaultModel,
      provider.tiers,
      activeLlm.active.cli === provider.id ? activeLlm.active.model ?? null : null,
    )
    const providerMatches = query !== '' && provider.label.toLowerCase().includes(query)
    const filteredModels = allModels.filter((modelId) => {
      if (!query || providerMatches) return true
      return modelId.toLowerCase().includes(query)
    })
    if (filteredModels.length === 0) continue

    groups.push({
      id: provider.id,
      label: provider.label,
      installed: provider.installed,
      discovering: Boolean(providersStore.discovering[provider.id]),
      models: filteredModels.map((modelId) => ({
        providerId: provider.id,
        providerLabel: provider.label,
        installed: provider.installed,
        modelId,
        isFree: isFreeModel(modelId, provider.freeModels),
        tierLabels: tierLabelsForModel(modelId, provider.tiers),
        isActive: activeModel === modelId,
      })),
    })
  }
  return groups
})

const selectableItems = computed<CatalogModelItem[]>(() =>
  catalogGroups.value.flatMap((group) => group.models.filter((item) => item.installed)),
)

const anyDiscovering = computed(() =>
  Object.values(providersStore.discovering).some((flag) => flag),
)

function itemKey(providerId: string, modelId: string): string {
  return `${providerId}::${modelId}`
}

function syncDefaultHighlight(): void {
  const list = selectableItems.value
  if (list.length === 0) {
    highlightedKey.value = null
    return
  }
  if (
    highlightedKey.value &&
    list.some((item) => itemKey(item.providerId, item.modelId) === highlightedKey.value)
  ) {
    return
  }
  const activeItem = list.find((item) => item.isActive) ?? list[0]
  highlightedKey.value = activeItem ? itemKey(activeItem.providerId, activeItem.modelId) : null
}

watch(selectableItems, () => {
  syncDefaultHighlight()
})

watch(open, async (isOpen) => {
  if (!isOpen) {
    modelModalOpen.value = false
    return
  }
  if (!activeLlm.loaded) {
    await activeLlm.load()
  } else {
    void activeLlm.loadOverrides()
  }
  if (providersStore.providers.length === 0) {
    void providersStore.load().catch(() => {})
  }
})

watch(modelModalOpen, async (isOpen) => {
  if (!isOpen) {
    searchQuery.value = ''
    previousFocus.value?.focus()
    previousFocus.value = null
    return
  }
  previousFocus.value =
    document.activeElement instanceof HTMLElement ? document.activeElement : null
  searchQuery.value = ''

  if (!activeLlm.loaded) {
    await activeLlm.load()
  } else {
    void activeLlm.loadOverrides()
  }

  if (providersStore.providers.length === 0) {
    try {
      await providersStore.load()
    } catch {
      // mantém fallback de presets se falhar
    }
  }

  syncDefaultHighlight()
  await nextTick()
  searchInputRef.value?.focus()

  void providersStore.discoverAllInstalled(false)
})

function onKeydown(event: KeyboardEvent): void {
  if (open.value && !modelModalOpen.value && event.key === 'Escape' && !event.defaultPrevented) {
    open.value = false
  }
}

function onDocumentMouseDown(event: MouseEvent): void {
  if (!open.value || modelModalOpen.value) return
  if (event.target instanceof Node && !rootRef.value?.contains(event.target)) {
    open.value = false
  }
}

onMounted(() => {
  if (!activeLlm.loaded) {
    void activeLlm.load()
  }
  window.addEventListener('keydown', onKeydown)
  document.addEventListener('mousedown', onDocumentMouseDown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
  document.removeEventListener('mousedown', onDocumentMouseDown)
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
  modelModalOpen.value = false
  open.value = false
  providersStore.openSetup()
}

function openModelSelector(): void {
  // Mantém open = true para a ActionBar permanecer expandida e não desmontar o Teleport do modal.
  open.value = true
  modelModalOpen.value = true
}

function closeModal(): void {
  if (busy.value) return
  modelModalOpen.value = false
  open.value = false
}

async function selectModel(item: CatalogModelItem): Promise<void> {
  if (!item.installed || busy.value) return
  if (
    activeLlm.active.cli === item.providerId &&
    activeLlm.active.modelTier == null &&
    activeLlm.active.model === item.modelId
  ) {
    modelModalOpen.value = false
    open.value = false
    return
  }
  busy.value = true
  try {
    await activeLlm.saveActive({
      cli: item.providerId,
      modelTier: null,
      model: item.modelId,
    })
    modelModalOpen.value = false
    open.value = false
  } finally {
    busy.value = false
  }
}

function moveHighlight(delta: -1 | 1): void {
  const list = selectableItems.value
  if (list.length === 0) return
  const currentIdx = list.findIndex(
    (item) => itemKey(item.providerId, item.modelId) === highlightedKey.value,
  )
  const nextIdx =
    currentIdx < 0
      ? 0
      : (currentIdx + delta + list.length) % list.length
  const target = list[nextIdx]
  if (target) {
    highlightedKey.value = itemKey(target.providerId, target.modelId)
  }
}

function onSearchKeydown(event: KeyboardEvent): void {
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    moveHighlight(1)
    return
  }
  if (event.key === 'ArrowUp') {
    event.preventDefault()
    moveHighlight(-1)
    return
  }
  if (event.key === 'Enter') {
    const target = selectableItems.value.find(
      (item) => itemKey(item.providerId, item.modelId) === highlightedKey.value,
    )
    if (target) {
      event.preventDefault()
      void selectModel(target)
    }
  }
}

function openConnectProvider(): void {
  modelModalOpen.value = false
  open.value = false
  providersStore.openAddProvider()
}

function openManageModels(): void {
  modelModalOpen.value = false
  open.value = false
  providersStore.openSetup()
}

function openProviderSetup(providerId: string): void {
  modelModalOpen.value = false
  open.value = false
  providersStore.openSetup(providerId)
}

function refreshCatalog(): void {
  void providersStore.discoverAllInstalled(true)
}
</script>

<template>
  <div ref="rootRef" class="relative inline-flex items-center">
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

    <!-- Popover minimalista anterior com botão Selecionar modelo ao lado de Setup -->
    <div
      v-if="open && !modelModalOpen"
      data-testid="active-llm-popover"
      role="dialog"
      aria-label="Selecionar provedor ativo"
      class="absolute left-0 bottom-full z-40 pb-2"
    >
      <div class="flex w-72 flex-col gap-2.5 rounded-md border border-line bg-surface p-3 shadow-paper">
        <div class="flex flex-col gap-1">
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0">
              <span class="block text-small text-muted">Provedor atual</span>
              <span class="block truncate text-caption font-semibold text-ink">
                {{ activeProviderLabel }}
              </span>
            </div>
            <div class="flex shrink-0 items-center gap-1">
              <button
                type="button"
                data-testid="open-model-selector"
                title="Selecionar modelo"
                aria-label="Selecionar modelo"
                class="inline-flex shrink-0 cursor-pointer items-center gap-1 rounded-md px-2 py-1 text-small font-medium text-muted hover:bg-hover hover:text-ink" :class="INK_FOCUS_CLASSES"
                @click="openModelSelector()"
              >
                <span>Modelo</span>
                <svg
                  class="size-3"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.8"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  aria-hidden="true"
                >
                  <polygon points="12 2 2 7 12 12 22 7 12 2" />
                  <polyline points="2 17 12 22 22 17" />
                  <polyline points="2 12 12 17 22 12" />
                </svg>
              </button>
              <button
                type="button"
                title="Configurar provedores"
                aria-label="Configurar provedores"
                class="inline-flex shrink-0 cursor-pointer items-center gap-1 rounded-md px-2 py-1 text-small font-medium text-muted hover:bg-hover hover:text-ink" :class="INK_FOCUS_CLASSES"
                @click="openSetup()"
              >
                <span>Setup</span>
                <svg
                  class="size-3"
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
          </div>
          <div
            v-if="activeModelLabel"
            data-testid="active-llm-model-label"
            class="flex items-center justify-between gap-2 rounded-md bg-surface-2 px-2 py-1"
          >
            <span class="text-tiny text-muted">Modelo</span>
            <span
              :class="[
                'truncate font-mono text-tiny',
                activeLlm.active.model ? 'font-semibold text-ink' : 'text-muted',
              ]"
              :title="activeModelLabel"
            >
              {{ activeModelLabel }}
            </span>
          </div>
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

    <!-- Modal centralizado Selecionar modelo -->
    <Teleport to="body">
      <div
        v-if="modelModalOpen"
        class="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4"
        @click.self="closeModal()"
      >
        <section
          ref="dialogRef"
          data-testid="model-selector-dialog"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="titleId"
          tabindex="-1"
          class="flex max-h-[82vh] w-130 max-w-full flex-col overflow-hidden rounded-md border border-line bg-surface shadow-paper"
          @keydown.esc.stop="closeModal()"
          @keydown.tab="trapFocus($event, dialogRef)"
        >
          <!-- Cabeçalho: Selecionar modelo | Conectar provedor | Fechar -->
          <header class="flex items-center justify-between gap-3 px-5 pt-4 pb-3">
            <h2 :id="titleId" class="text-control font-bold text-ink">Selecionar modelo</h2>
            <div class="flex items-center gap-2">
              <UiCreateButton data-testid="model-selector-connect-provider" @press="openConnectProvider()">
                Conectar provedor
              </UiCreateButton>
              <UiCloseButton data-testid="model-selector-close" @press="closeModal()" />
            </div>
          </header>

          <!-- Campo Buscar modelos -->
          <div class="border-b border-line px-5 pb-3">
            <div class="relative flex items-center">
              <svg
                width="15"
                height="15"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="pointer-events-none absolute left-3 text-muted"
                aria-hidden="true"
              >
                <circle cx="11" cy="11" r="8" />
                <line x1="21" y1="21" x2="16.65" y2="16.65" />
              </svg>
              <input
                ref="searchInputRef"
                v-model="searchQuery"
                type="search"
                data-testid="model-selector-search"
                aria-label="Buscar modelos"
                placeholder="Buscar modelos"
                autocomplete="off"
                class="h-9 w-full rounded-md border border-line bg-surface-2 pr-3 pl-9 text-control text-ink placeholder:text-faint focus:border-accent focus:bg-surface focus:outline-none"
                @keydown="onSearchKeydown"
              />
            </div>
          </div>

          <!-- Lista agrupada por provedor -->
          <div
            data-testid="model-selector-list"
            class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-3 py-3"
          >
            <p
              v-if="catalogGroups.length === 0"
              data-testid="model-selector-empty"
              class="py-8 text-center text-caption text-muted"
            >
              Nenhum modelo encontrado.
            </p>

            <div
              v-for="group in catalogGroups"
              :key="group.id"
              :data-testid="`model-group-${group.id}`"
              class="flex flex-col gap-0.5"
            >
              <div class="flex items-center justify-between gap-2 px-2.5 py-1">
                <div class="flex items-center gap-2">
                  <span class="text-small font-semibold text-muted">{{ group.label }}</span>
                  <span
                    v-if="!group.installed"
                    data-testid="provider-missing-badge"
                    class="inline-flex h-4.5 items-center rounded-sm bg-surface-2 px-1.5 text-tiny font-medium text-muted"
                  >
                    Ausente
                  </span>
                  <span
                    v-if="group.discovering"
                    class="text-tiny text-faint"
                  >
                    Atualizando...
                  </span>
                </div>
                <button
                  v-if="!group.installed"
                  type="button"
                  class="cursor-pointer text-tiny font-medium text-accent hover:underline"
                  @click="openProviderSetup(group.id)"
                >
                  Configurar
                </button>
              </div>

              <button
                v-for="item in group.models"
                :key="itemKey(item.providerId, item.modelId)"
                type="button"
                data-testid="model-selector-option"
                :data-provider-id="item.providerId"
                :data-model-id="item.modelId"
                :disabled="!item.installed || busy"
                :aria-selected="item.isActive"
                :class="[
                  'flex w-full items-center justify-between gap-3 rounded-md px-2.5 py-2 text-left text-control transition-colors',
                  !item.installed
                    ? 'cursor-not-allowed opacity-45'
                    : itemKey(item.providerId, item.modelId) === highlightedKey
                      ? 'cursor-pointer bg-hover text-ink'
                      : item.isActive
                        ? 'cursor-pointer bg-accent-soft/60 text-ink'
                        : 'cursor-pointer text-ink hover:bg-hover',
                ]"
                @mouseenter="
                  item.installed && (highlightedKey = itemKey(item.providerId, item.modelId))
                "
                @click="selectModel(item)"
              >
                <span class="flex min-w-0 items-center gap-2">
                  <span class="truncate font-medium">{{ item.modelId }}</span>
                </span>

                <span class="flex shrink-0 items-center gap-1.5">
                  <span
                    v-for="tierLabel in item.tierLabels"
                    :key="tierLabel"
                    data-testid="model-tier-badge"
                    class="inline-flex h-5 items-center rounded-sm bg-surface-2 px-2 text-tiny font-medium text-muted"
                  >
                    {{ tierLabel }}
                  </span>
                  <span
                    v-if="item.isFree"
                    data-testid="model-free-badge"
                    class="inline-flex h-5 items-center rounded-sm bg-surface-2 px-2 text-tiny font-medium text-muted"
                  >
                    Grátis
                  </span>
                  <span
                    class="inline-flex size-4 items-center justify-center text-ink"
                    aria-hidden="true"
                  >
                    <svg
                      v-if="item.isActive"
                      data-testid="model-active-check"
                      width="14"
                      height="14"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2.2"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    >
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  </span>
                </span>
              </button>
            </div>
          </div>

          <!-- Rodapé: Gerenciar modelos | Atualizar | Overrides temporários -->
          <footer class="flex items-center justify-between gap-3 border-t border-line bg-surface px-5 py-3">
            <button
              type="button"
              data-testid="model-selector-manage-models"
              class="inline-flex cursor-pointer items-center gap-2 rounded-md px-2 py-1 text-caption font-medium text-muted transition-colors hover:bg-hover hover:text-ink" :class="INK_FOCUS_CLASSES"
              @click="openManageModels()"
            >
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
                <line x1="4" y1="21" x2="4" y2="14" />
                <line x1="4" y1="10" x2="4" y2="3" />
                <line x1="12" y1="21" x2="12" y2="12" />
                <line x1="12" y1="8" x2="12" y2="3" />
                <line x1="20" y1="21" x2="20" y2="16" />
                <line x1="20" y1="12" x2="20" y2="3" />
                <line x1="1" y1="14" x2="7" y2="14" />
                <line x1="9" y1="8" x2="15" y2="8" />
                <line x1="17" y1="16" x2="23" y2="16" />
              </svg>
              <span>Gerenciar modelos</span>
            </button>

            <div class="flex items-center gap-3 text-small">
              <div
                v-if="activeLlm.overrides.length > 0"
                class="flex items-center gap-2"
              >
                <span class="text-warn-text">{{ activeLlm.overrides.length }} temporário(s)</span>
                <button
                  type="button"
                  class="cursor-pointer font-medium text-ink hover:underline"
                  @click="activeLlm.clearAllOverrides()"
                >
                  Restaurar
                </button>
              </div>

              <button
                type="button"
                data-testid="model-selector-refresh"
                :disabled="anyDiscovering"
                class="cursor-pointer rounded-md px-2 py-1 text-small text-muted hover:bg-hover hover:text-ink disabled:cursor-default disabled:opacity-40"
                @click="refreshCatalog()"
              >
                {{ anyDiscovering ? 'Atualizando...' : 'Atualizar' }}
              </button>
            </div>
          </footer>
        </section>
      </div>
    </Teleport>
  </div>
</template>
