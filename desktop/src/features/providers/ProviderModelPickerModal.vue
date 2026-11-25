<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import { trapFocus } from '../backlog/dialogFocus'
import type { ModelTier, ReasoningEffort } from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import { useUsage } from '../usage/useUsage'
import {
  EFFORT_LABELS,
  buildCollapsedModelSections,
  buildProviderRailItems,
  normalizeReasoningEffort,
  providerSourceFromStatus,
  resolveModelSelectionPayload,
  stripModelEffortSuffix,
  type CatalogModelSection,
  type CollapsedModelItem,
  type ProviderCatalogSource,
  type ProviderRailItem,
  type ProviderRailStatus,
} from './providerModelCatalog'
import { useActiveLlm } from './useActiveLlm'
import { useProviders } from './useProviders'

export interface ModelSelectionPayload {
  cli: string
  model: string
  modelTier: null
  reasoningEffort: ReasoningEffort | null
}

const props = withDefaults(
  defineProps<{
    open: boolean
    title?: string
    selectedProviderId?: string | null
    selectedModel?: string | null
    selectedModelTier?: ModelTier | null
    selectedEffort?: ReasoningEffort | null
    allowedProviderIds?: readonly string[] | null
    showBackupShortcut?: boolean
    showOverrides?: boolean
    midTurnHint?: boolean
    busy?: boolean
  }>(),
  {
    title: 'Selecionar modelo',
    selectedProviderId: null,
    selectedModel: null,
    selectedModelTier: null,
    selectedEffort: null,
    allowedProviderIds: null,
    showBackupShortcut: true,
    showOverrides: false,
    midTurnHint: false,
    busy: false,
  },
)

const emit = defineEmits<{
  close: []
  select: [payload: ModelSelectionPayload]
  openBackup: []
}>()

const SEARCH_PLACEHOLDER = 'Buscar modelos ou serviços'
const EMPTY_SEARCH_TEXT = 'Nenhum modelo encontrado para esse filtro.'
const MANAGE_MODELS_LABEL = 'Gerenciar modelos'
const BACKUP_MODELS_LABEL = 'Modelos de backup'
const CONNECT_PROVIDER_LABEL = 'Conectar provedor'
const REFRESH_LABEL = 'Atualizar'
const REFRESHING_LABEL = 'Atualizando...'
const NEXT_TURN_HINT = 'A troca entra em vigor no próximo turno.'

const STATUS_DOT_CLASSES: Record<ProviderRailStatus, string> = {
  connected: 'bg-ok',
  checking: 'bg-accent animate-pulse',
  'signed-out': 'bg-warn',
  missing: 'bg-faint',
  'limit-reached': 'bg-danger',
  error: 'bg-danger',
}

const STATUS_TEXT_CLASSES: Record<ProviderRailStatus, string> = {
  connected: 'text-muted',
  checking: 'text-muted',
  'signed-out': 'text-warn-text',
  missing: 'text-muted',
  'limit-reached': 'text-danger',
  error: 'text-danger',
}

const QUOTA_BAR_CLASSES: Record<'ink' | 'warn' | 'danger' | 'accent', string> = {
  ink: 'bg-ok',
  warn: 'bg-warn',
  danger: 'bg-danger',
  accent: 'bg-accent',
}

const providersStore = useProviders()
const roster = useRoster()
const usageStore = useUsage()
const activeLlm = useActiveLlm()

const titleId = useId()
const dialogRef = ref<HTMLElement>()
const searchInputRef = ref<HTMLInputElement>()
const previousFocus = ref<HTMLElement | null>(null)

const activeTabProviderId = ref('')
const searchQuery = ref('')
const highlightedModelKey = ref<string | null>(null)
const pendingEffortByKey = ref<Record<string, ReasoningEffort>>({})
const recheckingProvider = ref<string | null>(null)

const catalogSources = computed<ProviderCatalogSource[]>(() => {
  const allowed = props.allowedProviderIds ? new Set(props.allowedProviderIds) : null
  const list: ProviderCatalogSource[] =
    providersStore.providers.length > 0
      ? providersStore.providers.map(providerSourceFromStatus)
      : roster.presets.map((preset) => ({
          id: preset.id,
          label: preset.label,
          installed: true,
          connectionStatus: 'connected' as const,
          statusDetail: null,
          models: preset.models,
          freeModels: preset.freeModels ?? [],
          defaultModel: preset.defaultModel,
          tiers: preset.tiers,
          modelGrouping: preset.modelGrouping,
          effort: preset.effort ?? null,
          supportsFreeWithoutLogin: false,
          installHint: null,
          connectHint: null,
          badgeCode: preset.badgeCode ?? null,
          displayOrder: preset.displayOrder ?? null,
        }))

  return allowed ? list.filter((p) => allowed.has(p.id)) : list
})

const railItems = computed<ProviderRailItem[]>(() =>
  buildProviderRailItems({
    providers: catalogSources.value,
    usageByProvider: usageStore.providers,
    activeLimits: usageStore.activeLimits,
    discovering: providersStore.discovering,
    checkingStatus: providersStore.checkingStatus,
    activeProviderId: props.selectedProviderId,
    nowMs: usageStore.now,
  }),
)

const selectedProviderSource = computed<ProviderCatalogSource | null>(
  () => catalogSources.value.find((p) => p.id === activeTabProviderId.value) ?? catalogSources.value[0] ?? null,
)

const selectedRailItem = computed<ProviderRailItem | null>(
  () => railItems.value.find((item) => item.id === selectedProviderSource.value?.id) ?? railItems.value[0] ?? null,
)

const resolvedCurrentModel = computed<string | null>(() => {
  const provider = selectedProviderSource.value
  if (!provider || props.selectedProviderId !== provider.id) return null
  if (props.selectedModelTier) {
    const tierModel = provider.tiers[props.selectedModelTier]?.trim()
    if (tierModel) return tierModel
  }
  const explicit = props.selectedModel?.trim()
  if (explicit) return explicit
  return provider.defaultModel.trim() || null
})

const modelSections = computed<CatalogModelSection[]>(() => {
  const provider = selectedProviderSource.value
  if (!provider) return []
  return buildCollapsedModelSections({
    provider,
    usage: usageStore.providers[provider.id],
    activeLimits: usageStore.activeLimits,
    activeProviderId: props.selectedProviderId,
    activeModelId: resolvedCurrentModel.value,
    searchQuery: searchQuery.value,
    nowMs: usageStore.now,
  })
})

const flatVisibleModels = computed<CollapsedModelItem[]>(() =>
  modelSections.value.flatMap((section) => section.models),
)

const selectableModels = computed<CollapsedModelItem[]>(() =>
  flatVisibleModels.value.filter((item) => item.selectable),
)

const showInlineActionableState = computed(() => {
  const rail = selectedRailItem.value
  if (!rail) return false
  if (rail.status === 'missing' || rail.status === 'error') return true
  if (rail.status === 'signed-out' && !rail.supportsFreeWithoutLogin) return true
  return false
})

const showFreeUpgradeBanner = computed(() => {
  const source = selectedProviderSource.value
  const rail = selectedRailItem.value
  if (!source || !rail) return false
  return (
    rail.installed &&
    rail.supportsFreeWithoutLogin &&
    source.connectionStatus === 'signed-out' &&
    !showInlineActionableState.value
  )
})

const anyRefreshing = computed(
  () =>
    Object.values(providersStore.discovering).some(Boolean) ||
    Object.values(providersStore.checkingStatus).some(Boolean) ||
    usageStore.refreshing,
)

const highlightedModel = computed<CollapsedModelItem | null>(() => {
  const key = highlightedModelKey.value
  if (!key) return null
  return selectableModels.value.find((item) => modelKey(item) === key) ?? null
})

function modelKey(item: Pick<CollapsedModelItem, 'providerId' | 'baseModelId'>): string {
  return `${item.providerId}::${item.baseModelId}`
}

function effortForItem(item: CollapsedModelItem): ReasoningEffort | null {
  if (item.supportedEfforts.length === 0) return null
  const key = modelKey(item)
  const pending = pendingEffortByKey.value[key]
  if (pending && item.supportedEfforts.includes(pending)) return pending

  if (item.isActive) {
    const explicit = normalizeReasoningEffort(props.selectedEffort)
    if (explicit && item.supportedEfforts.includes(explicit)) return explicit
    if (resolvedCurrentModel.value) {
      const fromSuffix = stripModelEffortSuffix(
        resolvedCurrentModel.value,
        selectedProviderSource.value?.effort,
      ).effort
      if (fromSuffix && item.supportedEfforts.includes(fromSuffix)) return fromSuffix
    }
  }
  return item.defaultEffort
}

function setPendingEffort(item: CollapsedModelItem, effort: ReasoningEffort): void {
  pendingEffortByKey.value = {
    ...pendingEffortByKey.value,
    [modelKey(item)]: effort,
  }
}

function pickInitialTab(): string {
  const items = railItems.value
  if (items.length === 0) return ''
  if (props.selectedProviderId && items.some((p) => p.id === props.selectedProviderId)) {
    return props.selectedProviderId
  }
  const connected = items.find((p) => p.status === 'connected')
  if (connected) return connected.id
  return items[0].id
}

function syncHighlight(): void {
  const list = selectableModels.value
  if (list.length === 0) {
    highlightedModelKey.value = null
    return
  }
  if (highlightedModelKey.value && list.some((item) => modelKey(item) === highlightedModelKey.value)) {
    return
  }
  const active = list.find((item) => item.isActive) ?? list[0]
  highlightedModelKey.value = active ? modelKey(active) : null
}

watch(selectableModels, syncHighlight)

watch(
  () => props.open,
  async (isOpen) => {
    if (!isOpen) {
      searchQuery.value = ''
      previousFocus.value?.focus()
      previousFocus.value = null
      return
    }
    previousFocus.value =
      document.activeElement instanceof HTMLElement ? document.activeElement : null
    searchQuery.value = ''
    pendingEffortByKey.value = {}

    if (providersStore.providers.length === 0) {
      try {
        await providersStore.load()
      } catch {
        // mantém fallback de presets se a carga falhar
      }
    }

    activeTabProviderId.value = pickInitialTab()
    syncHighlight()
    await nextTick()
    if (!showInlineActionableState.value) {
      searchInputRef.value?.focus()
    } else {
      dialogRef.value?.focus()
    }

    void providersStore.checkStatus().catch(() => {})
    void providersStore.discoverAllInstalled(false)
    void usageStore.refresh().catch(() => {})
  },
)

function selectRailProvider(providerId: string): void {
  activeTabProviderId.value = providerId
  searchQuery.value = ''
  syncHighlight()
  void nextTick(() => {
    if (!showInlineActionableState.value) {
      searchInputRef.value?.focus()
    }
  })
}

function closeModal(): void {
  if (props.busy) return
  emit('close')
}

function confirmModel(item: CollapsedModelItem, explicitEffort?: ReasoningEffort): void {
  if (!item.selectable || props.busy) return
  const provider = selectedProviderSource.value
  if (!provider) return
  const chosenEffort = explicitEffort ?? effortForItem(item)
  if (chosenEffort) {
    setPendingEffort(item, chosenEffort)
  }
  const payload = resolveModelSelectionPayload(provider, item, chosenEffort)
  emit('select', payload)
}

function moveHighlight(delta: -1 | 1): void {
  const list = selectableModels.value
  if (list.length === 0) return
  const currentIdx = list.findIndex((item) => modelKey(item) === highlightedModelKey.value)
  const nextIdx = currentIdx < 0 ? 0 : (currentIdx + delta + list.length) % list.length
  const target = list[nextIdx]
  if (target) {
    highlightedModelKey.value = modelKey(target)
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
    const target = highlightedModel.value
    if (target) {
      event.preventDefault()
      confirmModel(target)
    }
  }
}

function openConnectProvider(): void {
  emit('close')
  providersStore.openAddProvider()
}

function openManageModels(): void {
  emit('close')
  providersStore.openSetup()
}

function openProviderSetup(providerId: string): void {
  emit('close')
  providersStore.openSetup(providerId)
}

function openBackupModels(): void {
  emit('openBackup')
}

async function recheckSelectedProvider(): Promise<void> {
  const id = selectedRailItem.value?.id
  if (!id || recheckingProvider.value) return
  recheckingProvider.value = id
  try {
    await providersStore.checkStatus(id)
    const updated = providersStore.providers.find((p) => p.id === id)
    if (updated?.installed && updated.discovery) {
      await providersStore.discover(id).catch(() => {})
    }
    await usageStore.refresh(id).catch(() => {})
  } finally {
    recheckingProvider.value = null
  }
}

async function refreshAll(): Promise<void> {
  await Promise.allSettled([
    providersStore.checkStatus(),
    providersStore.discoverAllInstalled(true),
    usageStore.refresh(),
  ])
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
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
        class="flex h-[min(560px,82vh)] w-[680px] max-w-full flex-col overflow-hidden rounded-md border border-line bg-surface shadow-paper focus:outline-none"
        @keydown.esc.stop="closeModal()"
        @keydown.tab="trapFocus($event, dialogRef)"
      >
        <!-- Cabeçalho -->
        <header class="flex items-center justify-between gap-3 border-b border-line px-4 py-3">
          <div class="flex min-w-0 items-center gap-2.5">
            <h2 :id="titleId" class="truncate text-control font-bold text-ink">{{ title }}</h2>
            <span
              v-if="midTurnHint"
              data-testid="model-selector-next-turn-hint"
              class="hidden rounded-sm bg-surface-2 px-2 py-0.5 text-tiny text-muted sm:inline-flex"
            >
              {{ NEXT_TURN_HINT }}
            </span>
          </div>
          <div class="flex shrink-0 items-center gap-2">
            <UiCreateButton data-testid="model-selector-connect-provider" @press="openConnectProvider()">
              {{ CONNECT_PROVIDER_LABEL }}
            </UiCreateButton>
            <UiCloseButton data-testid="model-selector-close" @press="closeModal()" />
          </div>
        </header>

        <!-- Corpo em duas colunas: trilho de provedores à esquerda, modelos à direita -->
        <div class="flex min-h-0 flex-1 overflow-hidden">
          <!-- Coluna esquerda: Trilho de provedores -->
          <aside
            data-testid="provider-rail"
            role="tablist"
            aria-orientation="vertical"
            aria-label="Provedores"
            class="flex w-48 shrink-0 flex-col gap-1 overflow-y-auto border-r border-line bg-surface-2/50 p-2 sm:w-52"
          >
            <button
              v-for="rail in railItems"
              :key="rail.id"
              type="button"
              role="tab"
              :data-testid="`provider-rail-item-${rail.id}`"
              :aria-selected="selectedRailItem?.id === rail.id"
              :class="[
                `flex w-full cursor-pointer flex-col gap-1.5 rounded-md px-2.5 py-2 text-left transition-colors ${INK_FOCUS_CLASSES}`,
                selectedRailItem?.id === rail.id
                  ? 'border border-line bg-surface text-ink shadow-seg'
                  : 'border border-transparent text-muted hover:bg-hover hover:text-ink',
              ]"
              @click="selectRailProvider(rail.id)"
            >
              <div class="flex items-center gap-2">
                <span
                  :class="[
                    'inline-flex size-6 shrink-0 items-center justify-center rounded-sm font-mono text-tiny font-bold',
                    selectedRailItem?.id === rail.id
                      ? 'bg-accent-soft text-accent'
                      : 'bg-surface-2 text-ink',
                  ]"
                  aria-hidden="true"
                >
                  {{ rail.badgeCode }}
                </span>
                <div class="flex min-w-0 flex-1 flex-col">
                  <span class="flex items-center gap-1.5">
                    <span class="truncate text-caption font-semibold text-ink">{{ rail.label }}</span>
                    <span
                      v-if="rail.isActive"
                      class="size-1.5 shrink-0 rounded-full bg-accent"
                      title="Provedor selecionado"
                      aria-hidden="true"
                    />
                  </span>
                  <span class="flex items-center gap-1 text-tiny" :class="STATUS_TEXT_CLASSES[rail.status]">
                    <span
                      class="size-1.5 shrink-0 rounded-full"
                      :class="STATUS_DOT_CLASSES[rail.status]"
                      aria-hidden="true"
                    />
                    <span class="truncate">{{ rail.statusLabel }}</span>
                  </span>
                </div>
              </div>

              <!-- Mini-barra de cota restante (degradação graciosa se não houver snapshot) -->
              <div
                v-if="rail.quota"
                :data-testid="`provider-rail-quota-${rail.id}`"
                class="flex flex-col gap-0.5 pl-8"
              >
                <div class="flex items-center justify-between gap-1 text-tiny">
                  <span v-if="rail.quota.usingCredits" class="font-medium text-accent">
                    Usando créditos
                  </span>
                  <span
                    v-else-if="rail.quota.limitedUntilLabel"
                    class="truncate font-medium text-danger"
                  >
                    {{ rail.quota.limitedUntilLabel }}
                  </span>
                  <span v-else class="text-muted">Cota restante</span>
                  <span
                    v-if="!rail.quota.usingCredits"
                    class="shrink-0 font-mono tabular-nums text-muted"
                  >
                    {{ rail.quota.remainingPercent }}%
                  </span>
                </div>
                <div class="h-1 w-full overflow-hidden rounded-full bg-surface-2">
                  <div
                    class="h-full rounded-full transition-all"
                    :class="QUOTA_BAR_CLASSES[rail.quota.tone]"
                    :style="{
                      width: `${rail.quota.usingCredits ? 100 : rail.quota.remainingPercent}%`,
                    }"
                  />
                </div>
              </div>
            </button>
          </aside>

          <!-- Coluna direita: Busca + Lista de modelos ou Estado acionável -->
          <div class="flex min-w-0 flex-1 flex-col overflow-hidden">
            <!-- Busca instantânea -->
            <div v-if="!showInlineActionableState" class="border-b border-line px-3.5 py-2.5">
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
                  :aria-label="SEARCH_PLACEHOLDER"
                  :placeholder="SEARCH_PLACEHOLDER"
                  autocomplete="off"
                  class="h-8.5 w-full rounded-md border border-line bg-surface-2 pr-3 pl-9 text-control text-ink placeholder:text-faint focus:border-accent focus:bg-surface focus:outline-none"
                  @keydown="onSearchKeydown"
                />
              </div>
            </div>

            <!-- Estado acionável inline quando provedor ausente / sem login / erro -->
            <div
              v-if="showInlineActionableState && selectedRailItem"
              data-testid="provider-actionable-state"
              class="flex flex-1 flex-col items-center justify-center gap-4 px-6 py-8 text-center"
            >
              <span
                class="inline-flex size-11 items-center justify-center rounded-md border border-line bg-surface-2 font-mono text-lead font-bold text-ink"
                aria-hidden="true"
              >
                {{ selectedRailItem.badgeCode }}
              </span>

              <div class="flex max-w-sm flex-col gap-1">
                <h3 class="text-control font-semibold text-ink">
                  {{ selectedRailItem.label }} · {{ selectedRailItem.statusLabel }}
                </h3>
                <p class="text-caption text-muted">
                  {{
                    selectedRailItem.statusDetail ??
                    (selectedRailItem.status === 'missing'
                      ? 'Instale a CLI deste provedor ou aponte o executável nas configurações.'
                      : 'Faça login na CLI para listar e usar os modelos deste provedor.')
                  }}
                </p>
              </div>

              <code
                v-if="
                  (selectedRailItem.status === 'missing' && selectedRailItem.installHint) ||
                  (selectedRailItem.status !== 'missing' && selectedRailItem.connectHint)
                "
                class="max-w-full rounded-md border border-line bg-surface-2 px-3 py-1.5 font-mono text-small text-ink"
              >
                {{
                  selectedRailItem.status === 'missing'
                    ? selectedRailItem.installHint
                    : selectedRailItem.connectHint
                }}
              </code>

              <div class="flex flex-wrap items-center justify-center gap-2">
                <UiButton
                  v-if="selectedRailItem.status === 'missing'"
                  data-testid="provider-action-install"
                  variant="primary"
                  size="sm"
                  @press="openProviderSetup(selectedRailItem.id)"
                >
                  Instalar CLI
                </UiButton>
                <UiButton
                  v-else
                  data-testid="provider-action-connect"
                  variant="primary"
                  size="sm"
                  @press="openProviderSetup(selectedRailItem.id)"
                >
                  Conectar
                </UiButton>
                <UiButton
                  data-testid="provider-action-recheck"
                  variant="quiet"
                  size="sm"
                  :disabled="recheckingProvider === selectedRailItem.id"
                  @press="recheckSelectedProvider()"
                >
                  {{ recheckingProvider === selectedRailItem.id ? 'Verificando...' : 'Verificar novamente' }}
                </UiButton>
              </div>
            </div>

            <!-- Lista de modelos agrupados e colapsados -->
            <template v-else>
              <div
                v-if="showFreeUpgradeBanner && selectedRailItem"
                class="flex items-center justify-between gap-2 border-b border-line bg-surface-2/60 px-3.5 py-2 text-small"
              >
                <span class="text-muted">Modo gratuito ativo. Conecte para modelos pagos.</span>
                <button
                  type="button"
                  class="shrink-0 cursor-pointer font-medium text-accent hover:underline"
                  @click="openProviderSetup(selectedRailItem.id)"
                >
                  Conectar
                </button>
              </div>

              <div
                data-testid="model-selector-list"
                class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-3 py-2.5"
              >
                <p
                  v-if="modelSections.length === 0"
                  data-testid="model-selector-empty"
                  class="py-8 text-center text-caption text-muted"
                >
                  {{ EMPTY_SEARCH_TEXT }}
                </p>

                <div
                  v-for="section in modelSections"
                  :key="section.id"
                  :data-testid="`model-section-${section.id}`"
                  class="flex flex-col gap-0.5"
                >
                  <div
                    v-if="section.label"
                    class="flex items-center justify-between gap-2 px-2 py-1"
                  >
                    <span class="text-tiny font-semibold tracking-wide text-muted uppercase">
                      {{ section.label }}
                    </span>
                    <span
                      v-if="section.isFreeSection"
                      class="rounded-sm bg-ok-soft px-1.5 py-0.5 text-tiny font-semibold text-ok"
                    >
                      Grátis
                    </span>
                  </div>

                  <div
                    v-for="item in section.models"
                    :key="modelKey(item)"
                    role="option"
                    data-testid="model-selector-option"
                    :data-provider-id="item.providerId"
                    :data-model-id="item.baseModelId"
                    :aria-selected="item.isActive"
                    :class="[
                      'group flex w-full items-center justify-between gap-2 rounded-md px-2.5 py-2 text-left text-control transition-colors',
                      !item.selectable || busy
                        ? 'cursor-not-allowed opacity-45'
                        : modelKey(item) === highlightedModelKey
                          ? 'cursor-pointer bg-hover text-ink'
                          : item.isActive
                            ? 'cursor-pointer bg-accent-soft/60 text-ink'
                            : 'cursor-pointer text-ink hover:bg-hover',
                    ]"
                    @mouseenter="item.selectable && (highlightedModelKey = modelKey(item))"
                    @click="confirmModel(item)"
                  >
                    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                      <div class="flex min-w-0 items-center gap-1.5">
                        <span class="truncate font-medium text-ink" :title="item.baseModelId">
                          {{ item.displayName }}
                        </span>
                        <span
                          v-for="tierLabel in item.tierLabels"
                          :key="tierLabel"
                          data-testid="model-tier-badge"
                          class="inline-flex h-4.5 shrink-0 items-center rounded-sm bg-surface-2 px-1.5 text-tiny font-medium text-muted"
                        >
                          {{ tierLabel }}
                        </span>
                        <span
                          v-if="item.isFree && !section.isFreeSection"
                          data-testid="model-free-badge"
                          class="inline-flex h-4.5 shrink-0 items-center rounded-sm bg-ok-soft px-1.5 text-tiny font-medium text-ok"
                        >
                          Grátis
                        </span>
                      </div>

                      <!-- Badge inline de cota / créditos -->
                      <div v-if="item.limitBadge" class="flex items-center gap-1.5">
                        <span
                          data-testid="model-limit-badge"
                          :class="[
                            'inline-flex items-center rounded-sm px-1.5 py-0.5 text-tiny font-medium',
                            item.limitBadge.kind === 'credits'
                              ? 'bg-accent-soft text-accent'
                              : 'bg-warn-soft text-warn-text',
                          ]"
                        >
                          {{ item.limitBadge.label }}
                        </span>
                      </div>
                    </div>

                    <!-- Seletor segmentado inline de Esforço de raciocínio + Check ativo -->
                    <div class="flex shrink-0 items-center gap-2">
                      <div
                        v-if="item.supportedEfforts.length > 0"
                        data-testid="model-effort-selector"
                        role="group"
                        aria-label="Esforço de raciocínio"
                        class="inline-flex items-center rounded-sm border border-line bg-surface-2 p-0.5"
                        @click.stop
                      >
                        <button
                          v-for="level in item.supportedEfforts"
                          :key="level"
                          type="button"
                          :disabled="!item.selectable || busy"
                          :aria-pressed="effortForItem(item) === level"
                          :data-effort="level"
                          :class="[
                            'cursor-pointer rounded-sm px-1.5 py-0.5 text-tiny font-medium transition-colors disabled:cursor-not-allowed',
                            effortForItem(item) === level
                              ? 'bg-surface text-ink shadow-seg'
                              : 'text-muted hover:text-ink',
                          ]"
                          @click="confirmModel(item, level)"
                        >
                          {{ EFFORT_LABELS[level] }}
                        </button>
                      </div>

                      <span
                        class="inline-flex size-4 shrink-0 items-center justify-center text-accent"
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
                    </div>
                  </div>
                </div>
              </div>

              <!-- Barra inferior de esforço para operação via teclado no item destacado -->
              <div
                v-if="highlightedModel && highlightedModel.supportedEfforts.length > 0"
                data-testid="model-effort-footer-bar"
                class="flex items-center justify-between gap-2 border-t border-line bg-surface-2/40 px-3.5 py-2"
              >
                <span class="truncate text-tiny text-muted">
                  Esforço para <strong class="font-medium text-ink">{{ highlightedModel.displayName }}</strong>
                </span>
                <div class="inline-flex shrink-0 items-center rounded-sm border border-line bg-surface-2 p-0.5">
                  <button
                    v-for="level in highlightedModel.supportedEfforts"
                    :key="level"
                    type="button"
                    :disabled="busy"
                    :aria-pressed="effortForItem(highlightedModel) === level"
                    :class="[
                      'cursor-pointer rounded-sm px-2 py-0.5 text-tiny font-medium transition-colors disabled:cursor-not-allowed',
                      effortForItem(highlightedModel) === level
                        ? 'bg-surface text-ink shadow-seg'
                        : 'text-muted hover:text-ink',
                    ]"
                    @click="setPendingEffort(highlightedModel, level)"
                  >
                    {{ EFFORT_LABELS[level] }}
                  </button>
                </div>
              </div>
            </template>
          </div>
        </div>

        <!-- Rodapé: Modelos de backup | Gerenciar modelos | Overrides | Atualizar -->
        <footer class="flex flex-wrap items-center justify-between gap-2 border-t border-line bg-surface px-4 py-2.5">
          <div class="flex items-center gap-1.5">
            <button
              v-if="showBackupShortcut"
              type="button"
              data-testid="model-selector-backup"
              :class="`inline-flex cursor-pointer items-center gap-1.5 rounded-md px-2 py-1 text-caption font-medium text-muted transition-colors hover:bg-hover hover:text-ink ${INK_FOCUS_CLASSES}`"
              @click="openBackupModels()"
            >
              <svg
                width="13"
                height="13"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <polyline points="17 1 21 5 17 9" />
                <path d="M3 11V9a4 4 0 0 1 4-4h14" />
                <polyline points="7 23 3 19 7 15" />
                <path d="M21 13v2a4 4 0 0 1-4 4H3" />
              </svg>
              <span>{{ BACKUP_MODELS_LABEL }}</span>
            </button>

            <button
              type="button"
              data-testid="model-selector-manage-models"
              :class="`inline-flex cursor-pointer items-center gap-1.5 rounded-md px-2 py-1 text-caption font-medium text-muted transition-colors hover:bg-hover hover:text-ink ${INK_FOCUS_CLASSES}`"
              @click="openManageModels()"
            >
              <svg
                width="13"
                height="13"
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
              <span>{{ MANAGE_MODELS_LABEL }}</span>
            </button>
          </div>

          <div class="flex items-center gap-3 text-small">
            <div
              v-if="showOverrides && activeLlm.overrides.length > 0"
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
              :disabled="anyRefreshing"
              :class="`cursor-pointer rounded-md px-2 py-1 text-small text-muted hover:bg-hover hover:text-ink disabled:cursor-not-allowed disabled:opacity-40 ${INK_FOCUS_CLASSES}`"
              @click="refreshAll()"
            >
              {{ anyRefreshing ? REFRESHING_LABEL : REFRESH_LABEL }}
            </button>
          </div>
        </footer>
      </section>
    </div>
  </Teleport>
</template>
