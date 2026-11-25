<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import UiCombobox from '../../ui/UiCombobox.vue'
import UiCommandBlock from '../../ui/UiCommandBlock.vue'
import UiField from '../../ui/UiField.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { trapFocus } from '../backlog/dialogFocus'
import type {
  InspectCliResult,
  ProviderConfig,
  ProviderProtocol,
  ProviderWithStatus,
  TestProviderResult,
  ToolTransport,
} from './providersApi'
import { SETUP_ADD_PROVIDER_TOKEN, useProviders } from './useProviders'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
const PROTOCOL_OPTIONS: readonly { label: string; value: ProviderProtocol }[] = [
  { label: 'Generic CLI', value: 'generic-cli' },
  { label: 'Claude Live', value: 'claude-live' },
  { label: 'Codex App Server', value: 'codex-app-server' },
  { label: 'Codex JSONL', value: 'codex-jsonl' },
  { label: 'OpenCode JSON', value: 'open-code-json' },
  { label: 'Antigravity Stream', value: 'antigravity-stream' },
]

const TRANSPORT_OPTIONS: readonly { label: string; value: ToolTransport }[] = [
  { label: 'MCP nativo', value: 'mcp-native' },
  { label: 'Vokia CLI', value: 'vokia-cli' },
  { label: 'Só texto', value: 'none' },
]

const TRANSPORT_LABELS: Record<ToolTransport, string> = {
  'mcp-native': 'MCP nativo',
  'vokia-cli': 'Vokia CLI',
  none: 'Só texto',
}

function effectiveTransport(provider: ProviderConfig): ToolTransport {
  if (provider.toolTransport) return provider.toolTransport
  if (provider.supportsMcp) return 'mcp-native'
  if (provider.protocol === 'open-code-json' || provider.protocol === 'generic-cli') return 'vokia-cli'
  return 'none'
}

const providersStore = useProviders()
const titleId = useId()
const editTitleId = useId()
const addTitleId = useId()
const removeTitleId = useId()

const dialog = ref<HTMLElement>()
const editDialog = ref<HTMLElement>()
const addDialog = ref<HTMLElement>()
const removeDialog = ref<HTMLElement>()
const previousFocus = ref<HTMLElement | null>(null)

const actionError = ref<string | null>(null)
const listTestResults = ref<Record<string, TestProviderResult | null>>({})
const testingId = ref<string | null>(null)

// Modal dedicado: Editar provedor existente
const editingDraft = ref<ProviderConfig | null>(null)
const editDiscoverWarning = ref<string | null>(null)
const editTestResult = ref<TestProviderResult | null>(null)
const testingEdit = ref(false)
const savingEdit = ref(false)

// Modal dedicado: Remover provedor
const removingProvider = ref<ProviderWithStatus | null>(null)
const removing = ref(false)

// Modal dedicado: Novo provedor / Auto-setup
const addOpen = ref(false)
const cliName = ref('')
const cliCommand = ref('')
const inspectConfirmed = ref(false)
const inspecting = ref(false)
const inspectResult = ref<InspectCliResult | null>(null)
const generatingPrompt = ref(false)
const setupPrompt = ref('')
const copiedPrompt = ref(false)
const autoConfiguring = ref(false)
const jsonInput = ref('')
const jsonError = ref<string | null>(null)
const addDraft = ref<ProviderConfig>(createEmptyConfig())
const addTestResult = ref<TestProviderResult | null>(null)
const testingAdd = ref(false)
const savingAdd = ref(false)

function createEmptyConfig(): ProviderConfig {
  return {
    id: '',
    label: '',
    protocol: 'generic-cli',
    defaultCommand: '',
    defaultModel: '',
    models: [],
    tiers: { basic: '', intermediate: '', advanced: '' },
    supportsMcp: false,
    toolTransport: 'vokia-cli',
    supportsSandbox: false,
    reservedFlags: [],
    rateLimitPatterns: [],
    modelUnavailablePatterns: [],
    discovery: null,
    generic: null,
    builtinSeed: false,
  }
}

function cloneConfig(provider: ProviderWithStatus): ProviderConfig {
  return {
    ...provider,
    models: [...provider.models],
    tiers: { ...provider.tiers },
    reservedFlags: [...provider.reservedFlags],
    rateLimitPatterns: [...provider.rateLimitPatterns],
    modelUnavailablePatterns: [...provider.modelUnavailablePatterns],
    discovery: provider.discovery ? { ...provider.discovery, args: [...provider.discovery.args] } : null,
    generic: provider.generic
      ? {
          ...provider.generic,
          promptArgs: [...provider.generic.promptArgs],
          jsonOutputArgs: [...provider.generic.jsonOutputArgs],
          textJsonPaths: [...provider.generic.textJsonPaths],
          errorJsonPaths: [...provider.generic.errorJsonPaths],
          sandboxArgs: { ...provider.generic.sandboxArgs },
        }
      : null,
  }
}

watch(
  () => providersStore.setupOpen,
  async (open) => {
    if (!open) return
    previousFocus.value = document.activeElement instanceof HTMLElement ? document.activeElement : null
    actionError.value = null
    editingDraft.value = null
    removingProvider.value = null
    addOpen.value = false

    try {
      await providersStore.load()
    } catch {
      // erro já registrado em providersStore.error
    }

    const initial = providersStore.setupInitialProvider.trim()
    if (initial === SETUP_ADD_PROVIDER_TOKEN) {
      openAddModal('')
      return
    }
    if (initial) {
      const normalized = initial.toLowerCase()
      const existing = providersStore.providers.find(
        (p) => p.id.toLowerCase() === normalized || p.label.toLowerCase() === normalized,
      )
      if (existing) {
        openEditModal(existing)
        return
      }
      openAddModal(initial)
      return
    }

    await nextTick()
    dialog.value?.focus()
  },
)

function closeMain(): void {
  if (savingEdit.value || savingAdd.value || removing.value) return
  editingDraft.value = null
  removingProvider.value = null
  addOpen.value = false
  providersStore.closeSetup()
  previousFocus.value?.focus()
}

function formatTestError(result: TestProviderResult): string {
  if (result.errorKind === 'model-unavailable') {
    return `Modelo indisponível na sua conta (${result.errorMessage ?? 'sem detalhes'}). Escolha outro modelo descoberto.`
  }
  if (result.errorKind === 'limit') {
    return `Limite de uso atingido (${result.errorMessage ?? 'tente outro provedor'}).`
  }
  return result.errorMessage ?? 'Falha ao testar provedor.'
}

async function testFromList(provider: ProviderWithStatus): Promise<void> {
  testingId.value = provider.id
  actionError.value = null
  try {
    const model = provider.tiers.intermediate || provider.defaultModel || provider.models[0] || undefined
    const result = await providersStore.testConfig(provider, model)
    listTestResults.value = { ...listTestResults.value, [provider.id]: result }
  } catch (err) {
    actionError.value = String(err)
  } finally {
    testingId.value = null
  }
}

async function openEditModal(provider: ProviderWithStatus): Promise<void> {
  const cloned = cloneConfig(provider)
  cloned.toolTransport = effectiveTransport(provider)
  editingDraft.value = cloned
  editDiscoverWarning.value = null
  editTestResult.value = null
  actionError.value = null
  await nextTick()
  editDialog.value?.focus()
}

function closeEditModal(): void {
  if (savingEdit.value) return
  editingDraft.value = null
}

async function refreshEditModels(): Promise<void> {
  if (!editingDraft.value) return
  editDiscoverWarning.value = null
  try {
    const result = await providersStore.discover(editingDraft.value.id)
    editingDraft.value.models = [...result.models]
    if (result.warning) {
      editDiscoverWarning.value = result.warning
    }
  } catch (err) {
    actionError.value = String(err)
  }
}

async function testEditDraft(): Promise<void> {
  if (!editingDraft.value) return
  testingEdit.value = true
  editTestResult.value = null
  try {
    const model =
      editingDraft.value.tiers.intermediate ||
      editingDraft.value.defaultModel ||
      editingDraft.value.models[0] ||
      undefined
    editTestResult.value = await providersStore.testConfig(editingDraft.value, model)
  } catch (err) {
    actionError.value = String(err)
  } finally {
    testingEdit.value = false
  }
}

async function saveEditDraft(): Promise<void> {
  if (!editingDraft.value) return
  savingEdit.value = true
  actionError.value = null
  try {
    await providersStore.save(editingDraft.value)
    editingDraft.value = null
  } catch (err) {
    actionError.value = String(err)
  } finally {
    savingEdit.value = false
  }
}

async function openRemoveModal(provider: ProviderWithStatus): Promise<void> {
  removingProvider.value = provider
  actionError.value = null
  await nextTick()
  removeDialog.value?.focus()
}

function closeRemoveModal(): void {
  if (removing.value) return
  removingProvider.value = null
}

async function confirmRemove(): Promise<void> {
  if (!removingProvider.value) return
  removing.value = true
  actionError.value = null
  try {
    await providersStore.remove(removingProvider.value.id)
    removingProvider.value = null
  } catch (err) {
    actionError.value = String(err)
  } finally {
    removing.value = false
  }
}

async function openAddModal(initial = ''): Promise<void> {
  cliName.value = initial
  cliCommand.value = initial
  inspectConfirmed.value = false
  inspectResult.value = null
  setupPrompt.value = ''
  copiedPrompt.value = false
  jsonInput.value = ''
  jsonError.value = null
  addDraft.value = createEmptyConfig()
  addTestResult.value = null
  addOpen.value = true
  await nextTick()
  addDialog.value?.focus()
}

function closeAddModal(): void {
  if (savingAdd.value) return
  addOpen.value = false
}

const canInspect = computed(() => cliCommand.value.trim() !== '' && inspectConfirmed.value && !inspecting.value)

async function runInspect(): Promise<void> {
  if (!canInspect.value) return
  inspecting.value = true
  actionError.value = null
  try {
    inspectResult.value = await providersStore.inspect(cliCommand.value.trim())
  } catch (err) {
    actionError.value = String(err)
  } finally {
    inspecting.value = false
  }
}

async function runBuildPrompt(): Promise<void> {
  if (!cliName.value.trim() || !cliCommand.value.trim()) return
  generatingPrompt.value = true
  actionError.value = null
  copiedPrompt.value = false
  try {
    setupPrompt.value = await providersStore.buildPrompt(
      cliName.value.trim(),
      cliCommand.value.trim(),
      inspectResult.value?.helpText ?? '',
    )
  } catch (err) {
    actionError.value = String(err)
  } finally {
    generatingPrompt.value = false
  }
}

async function copyPrompt(): Promise<void> {
  if (!setupPrompt.value) return
  try {
    await navigator.clipboard.writeText(setupPrompt.value)
    copiedPrompt.value = true
  } catch (err) {
    actionError.value = `Não foi possível copiar: ${String(err)}`
  }
}

function applyConfigToAddDraft(config: ProviderConfig): void {
  addDraft.value = {
    ...createEmptyConfig(),
    ...config,
    tiers: { ...createEmptyConfig().tiers, ...config.tiers },
  }
  jsonInput.value = JSON.stringify(addDraft.value, null, 2)
  jsonError.value = null
}

async function runAutoConfigure(): Promise<void> {
  if (!cliName.value.trim() || !cliCommand.value.trim()) return
  autoConfiguring.value = true
  actionError.value = null
  try {
    const config = await providersStore.autoConfigure(
      cliName.value.trim(),
      cliCommand.value.trim(),
      inspectResult.value?.helpText ?? '',
    )
    applyConfigToAddDraft(config)
  } catch (err) {
    actionError.value = String(err)
  } finally {
    autoConfiguring.value = false
  }
}

function parseAndApplyJson(): boolean {
  jsonError.value = null
  const raw = jsonInput.value.trim()
  if (!raw) return true
  try {
    const parsed = JSON.parse(raw) as Partial<ProviderConfig>
    if (!parsed || typeof parsed !== 'object' || !parsed.id || !parsed.defaultCommand) {
      jsonError.value = 'JSON inválido: informe ao menos "id" e "defaultCommand".'
      return false
    }
    applyConfigToAddDraft(parsed as ProviderConfig)
    return true
  } catch (err) {
    jsonError.value = `JSON inválido: ${String(err)}`
    return false
  }
}

const canTestAndSaveAdd = computed(
  () =>
    addDraft.value.id.trim() !== '' &&
    addDraft.value.defaultCommand.trim() !== '' &&
    !testingAdd.value &&
    !savingAdd.value,
)

async function testAndSaveAdd(): Promise<void> {
  if (jsonInput.value.trim() !== '' && !parseAndApplyJson()) return
  if (!canTestAndSaveAdd.value) return

  testingAdd.value = true
  addTestResult.value = null
  actionError.value = null
  try {
    const modelToTest = addDraft.value.tiers.intermediate || addDraft.value.defaultModel || undefined
    const result = await providersStore.testConfig(addDraft.value, modelToTest)
    addTestResult.value = result
    if (!result.ok) return

    savingAdd.value = true
    await providersStore.save(addDraft.value)
    addOpen.value = false
  } catch (err) {
    actionError.value = String(err)
  } finally {
    testingAdd.value = false
    savingAdd.value = false
  }
}

async function saveAddWithoutTest(): Promise<void> {
  if (jsonInput.value.trim() !== '' && !parseAndApplyJson()) return
  if (!addDraft.value.id.trim() || !addDraft.value.defaultCommand.trim()) return
  savingAdd.value = true
  actionError.value = null
  try {
    await providersStore.save(addDraft.value)
    addOpen.value = false
  } catch (err) {
    actionError.value = String(err)
  } finally {
    savingAdd.value = false
  }
}
</script>

<template>
  <div
    v-if="providersStore.setupOpen"
    class="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4"
    @click.self="closeMain()"
  >
    <section
      ref="dialog"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      data-testid="provider-setup-dialog"
      tabindex="-1"
      class="flex max-h-[88vh] w-145 max-w-full flex-col gap-4 overflow-y-auto rounded-md border border-line bg-surface p-5 shadow-paper"
      @keydown.esc.stop="closeMain()"
      @keydown.tab="trapFocus($event, dialog)"
    >
      <header class="flex items-center justify-between gap-3">
        <div>
          <h2 :id="titleId" class="text-control font-bold text-ink">Provedores de LLM</h2>
          <p class="text-caption text-muted">Gerencie os modelos disponíveis ou adicione novas CLIs.</p>
        </div>
        <div class="flex items-center gap-2">
          <UiCreateButton @press="openAddModal()">Novo provedor</UiCreateButton>
          <UiCloseButton @press="closeMain()" />
        </div>
      </header>

      <UiNotice
        v-if="actionError || providersStore.error"
        :text="(actionError ?? providersStore.error)!"
        @close="actionError = null"
      />

      <p v-if="providersStore.loading" class="py-4 text-center text-caption text-muted">
        Carregando provedores...
      </p>

      <ul v-else class="flex flex-col divide-y divide-line rounded-md border border-line bg-surface">
        <li
          v-for="provider in providersStore.providers"
          :key="provider.id"
          class="flex flex-col gap-2 px-4 py-3"
        >
          <div class="flex items-center justify-between gap-3">
            <div class="flex min-w-0 items-center gap-2.5">
              <span
                class="size-2 shrink-0 rounded-full"
                :class="provider.installed ? 'bg-ok' : 'bg-faint'"
                aria-hidden="true"
              />
              <div class="min-w-0">
                <div class="flex items-center gap-2">
                  <span class="truncate text-control font-semibold text-ink">{{ provider.label }}</span>
                  <span
                    class="inline-flex h-5 items-center rounded-sm px-2 text-small font-medium"
                    :class="provider.installed ? 'bg-ok-soft text-ok' : 'bg-surface-2 text-muted'"
                  >
                    {{ provider.installed ? 'Instalado' : 'Ausente' }}
                  </span>
                  <span
                    data-testid="provider-tool-transport"
                    class="inline-flex h-5 items-center rounded-sm bg-surface-2 px-2 text-small font-medium text-muted"
                  >
                    {{ TRANSPORT_LABELS[effectiveTransport(provider)] }}
                  </span>
                </div>
                <p class="truncate text-small text-muted">
                  <code class="font-mono">{{ provider.defaultCommand }}</code>
                  · {{ provider.tiers.intermediate || provider.defaultModel }}
                </p>
              </div>
            </div>

            <div class="flex shrink-0 items-center gap-1.5">
              <UiButton
                variant="quiet"
                size="sm"
                :disabled="testingId === provider.id"
                @press="testFromList(provider)"
              >
                {{ testingId === provider.id ? 'Testando...' : 'Testar' }}
              </UiButton>
              <UiButton variant="quiet" size="sm" @press="openEditModal(provider)">Editar</UiButton>
              <UiButton
                v-if="!provider.builtinSeed && provider.id !== 'claude'"
                variant="danger"
                size="sm"
                @press="openRemoveModal(provider)"
              >
                Remover
              </UiButton>
            </div>
          </div>

          <p
            v-if="listTestResults[provider.id]"
            class="text-small"
            :class="listTestResults[provider.id]!.ok ? 'text-ok' : 'text-danger'"
          >
            {{
              listTestResults[provider.id]!.ok
                ? `Conexão OK (${listTestResults[provider.id]!.latencyMs} ms)`
                : formatTestError(listTestResults[provider.id]!)
            }}
          </p>
        </li>
      </ul>
    </section>

    <!-- Modal dedicado: Editar provedor -->
    <div
      v-if="editingDraft"
      class="fixed inset-0 z-60 flex items-center justify-center bg-overlay p-4"
      @click.self="closeEditModal()"
    >
      <section
        ref="editDialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="editTitleId"
        tabindex="-1"
        class="flex max-h-[85vh] w-130 max-w-full flex-col gap-4 overflow-y-auto rounded-md border border-line bg-surface p-5 shadow-paper"
        @keydown.esc.stop="closeEditModal()"
        @keydown.tab="trapFocus($event, editDialog)"
      >
        <div class="flex items-center justify-between gap-3">
          <div>
            <h3 :id="editTitleId" class="text-control font-bold text-ink">Editar {{ editingDraft.label }}</h3>
            <p class="font-mono text-small text-muted">{{ editingDraft.defaultCommand }}</p>
          </div>
          <UiCloseButton @press="closeEditModal()" />
        </div>

        <div class="flex flex-col gap-3">
          <label class="flex flex-col gap-1.5">
            <span class="text-label font-medium text-muted">Básico</span>
            <UiCombobox
              v-model="editingDraft.tiers.basic"
              :options="editingDraft.models"
              :loading="Boolean(providersStore.discovering[editingDraft.id])"
              show-refresh
              refresh-label="Atualizar"
              placeholder="Modelo básico"
              @refresh="refreshEditModels()"
            />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-label font-medium text-muted">Intermediário</span>
            <UiCombobox
              v-model="editingDraft.tiers.intermediate"
              :options="editingDraft.models"
              placeholder="Modelo intermediário"
            />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-label font-medium text-muted">Avançado</span>
            <UiCombobox
              v-model="editingDraft.tiers.advanced"
              :options="editingDraft.models"
              placeholder="Modelo avançado"
            />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-label font-medium text-muted">Protocolo</span>
            <UiSelect v-model="editingDraft.protocol" :options="PROTOCOL_OPTIONS" label="Protocolo" />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-label font-medium text-muted">Transporte de ferramentas</span>
            <UiSelect
              :model-value="editingDraft.toolTransport ?? 'none'"
              :options="TRANSPORT_OPTIONS"
              label="Transporte de ferramentas"
              @update:model-value="editingDraft.toolTransport = $event as ToolTransport"
            />
          </label>
        </div>

        <p v-if="editDiscoverWarning" class="text-small text-warn-text">{{ editDiscoverWarning }}</p>

        <p
          v-if="editTestResult"
          class="text-small"
          :class="editTestResult.ok ? 'text-ok' : 'text-danger'"
        >
          {{ editTestResult.ok ? `Conexão OK (${editTestResult.latencyMs} ms)` : formatTestError(editTestResult) }}
        </p>

        <div class="flex items-center justify-between gap-2 border-t border-line pt-3">
          <UiButton variant="quiet" size="lg" :disabled="testingEdit" @press="testEditDraft()">
            {{ testingEdit ? 'Testando...' : 'Testar' }}
          </UiButton>
          <div class="flex items-center gap-2">
            <UiButton variant="quiet" size="lg" @press="closeEditModal()">Cancelar</UiButton>
            <UiButton variant="primary" size="lg" :disabled="savingEdit" @press="saveEditDraft()">
              {{ savingEdit ? 'Salvando...' : 'Salvar' }}
            </UiButton>
          </div>
        </div>
      </section>
    </div>

    <!-- Modal dedicado: Confirmar remoção -->
    <div
      v-if="removingProvider"
      class="fixed inset-0 z-60 flex items-center justify-center bg-overlay p-4"
      @click.self="closeRemoveModal()"
    >
      <section
        ref="removeDialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="removeTitleId"
        tabindex="-1"
        class="flex w-100 max-w-full flex-col gap-4 rounded-md border border-line bg-surface p-5 shadow-paper"
        @keydown.esc.stop="closeRemoveModal()"
        @keydown.tab="trapFocus($event, removeDialog)"
      >
        <h3 :id="removeTitleId" class="text-control font-bold text-ink">
          Remover {{ removingProvider.label }}?
        </h3>
        <p class="text-caption text-muted">
          O provedor será removido da lista de configurações. Você poderá adicioná-lo novamente quando quiser.
        </p>
        <div class="flex justify-end gap-2">
          <UiButton variant="quiet" size="lg" :disabled="removing" @press="closeRemoveModal()">Cancelar</UiButton>
          <UiButton variant="danger" size="lg" :disabled="removing" @press="confirmRemove()">
            {{ removing ? 'Removendo...' : 'Remover' }}
          </UiButton>
        </div>
      </section>
    </div>

    <!-- Modal dedicado: Novo provedor / Auto-setup -->
    <div
      v-if="addOpen"
      class="fixed inset-0 z-60 flex items-center justify-center bg-overlay p-4"
      @click.self="closeAddModal()"
    >
      <section
        ref="addDialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="addTitleId"
        tabindex="-1"
        class="flex max-h-[88vh] w-145 max-w-full flex-col gap-4 overflow-y-auto rounded-md border border-line bg-surface p-5 shadow-paper"
        @keydown.esc.stop="closeAddModal()"
        @keydown.tab="trapFocus($event, addDialog)"
      >
        <div class="flex items-center justify-between gap-3">
          <h3 :id="addTitleId" class="text-control font-bold text-ink">Adicionar provedor de LLM</h3>
          <UiCloseButton @press="closeAddModal()" />
        </div>

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <UiField v-model="cliName" label="Nome do provedor" hint="ex.: gemini" />
          <UiField v-model="cliCommand" label="Comando base" hint="ex.: gemini" />
        </div>

        <label class="flex items-center gap-2 text-caption text-ink">
          <input v-model="inspectConfirmed" type="checkbox" class="accent-accent" />
          <span>Confirmar execução de <code class="font-mono">--help</code> (5s, sem shell)</span>
        </label>

        <div class="flex flex-wrap items-center gap-2">
          <UiButton
            data-testid="setup-inspect-btn"
            variant="quiet"
            size="sm"
            :disabled="!canInspect"
            @press="runInspect()"
          >
            {{ inspecting ? 'Inspecionando...' : 'Inspecionar CLI (--help)' }}
          </UiButton>
          <UiButton
            variant="quiet"
            size="sm"
            :disabled="generatingPrompt || !cliName.trim() || !cliCommand.trim()"
            @press="runBuildPrompt()"
          >
            {{ generatingPrompt ? 'Gerando...' : 'Gerar prompt' }}
          </UiButton>
          <UiButton
            data-testid="setup-auto-btn"
            variant="approve"
            size="sm"
            :disabled="autoConfiguring || !cliName.trim() || !cliCommand.trim()"
            @press="runAutoConfigure()"
          >
            {{ autoConfiguring ? 'Configurando...' : 'Auto-configurar com agente' }}
          </UiButton>
        </div>

        <div v-if="inspectResult" class="flex flex-col gap-1.5 rounded-md border border-line bg-surface-2 p-3">
          <span class="text-caption font-medium" :class="inspectResult.found ? 'text-ok' : 'text-warn-text'">
            {{
              inspectResult.found
                ? `Encontrado: ${inspectResult.programPath ?? cliCommand}`
                : 'Executável não encontrado no PATH'
            }}
          </span>
          <UiCommandBlock v-if="inspectResult.helpText" class="max-h-32 overflow-y-auto" :command="inspectResult.helpText" />
        </div>

        <div v-if="setupPrompt" class="flex flex-col gap-2">
          <div class="flex items-center justify-between">
            <span class="text-label font-medium text-muted">Prompt de auto-configuração</span>
            <UiButton data-testid="setup-copy-prompt-btn" variant="quiet" size="sm" @press="copyPrompt()">
              {{ copiedPrompt ? 'Copiado!' : 'Copiar prompt' }}
            </UiButton>
          </div>
          <UiCommandBlock class="max-h-36 overflow-y-auto" :command="setupPrompt" />
        </div>

        <label class="flex flex-col gap-1.5">
          <span class="flex items-center justify-between text-label font-medium text-muted">
            <span>Colar JSON de configuração</span>
            <button type="button" class="cursor-pointer text-accent hover:underline" @click="parseAndApplyJson()">
              Aplicar JSON
            </button>
          </span>
          <textarea
            v-model="jsonInput"
            rows="3"
            placeholder='{"id": "gemini", "label": "Gemini CLI", "protocol": "generic-cli", "defaultCommand": "gemini", ...}'
            class="w-full rounded-md border border-line bg-surface p-2.5 font-mono text-small text-ink placeholder:text-faint" :class="INK_FOCUS_CLASSES"
          />
          <span v-if="jsonError" role="alert" class="text-small text-danger">{{ jsonError }}</span>
        </label>

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <UiField v-model="addDraft.id" label="ID" />
          <UiField v-model="addDraft.label" label="Nome" />
          <label class="flex flex-col gap-1.5">
            <span class="text-label font-medium text-muted">Protocolo</span>
            <UiSelect v-model="addDraft.protocol" :options="PROTOCOL_OPTIONS" label="Protocolo" />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-label font-medium text-muted">Transporte de ferramentas</span>
            <UiSelect
              :model-value="addDraft.toolTransport ?? 'vokia-cli'"
              :options="TRANSPORT_OPTIONS"
              label="Transporte de ferramentas"
              @update:model-value="addDraft.toolTransport = $event as ToolTransport"
            />
          </label>
          <UiField v-model="addDraft.defaultCommand" label="Comando" />
        </div>

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <UiField v-model="addDraft.tiers.basic" label="Básico" />
          <UiField v-model="addDraft.tiers.intermediate" label="Intermediário" />
          <UiField v-model="addDraft.tiers.advanced" label="Avançado" />
        </div>

        <p
          v-if="addTestResult"
          class="text-caption"
          :class="addTestResult.ok ? 'text-ok' : 'text-danger'"
        >
          {{ addTestResult.ok ? `Teste OK (${addTestResult.latencyMs} ms)` : formatTestError(addTestResult) }}
        </p>

        <div class="flex flex-wrap justify-end gap-2 border-t border-line pt-3">
          <UiButton variant="quiet" size="lg" @press="closeAddModal()">Cancelar</UiButton>
          <UiButton
            v-if="addTestResult && !addTestResult.ok"
            variant="quiet"
            size="lg"
            :disabled="savingAdd"
            @press="saveAddWithoutTest()"
          >
            Salvar mesmo assim
          </UiButton>
          <UiButton
            data-testid="setup-test-save-btn"
            variant="primary"
            size="lg"
            :disabled="!canTestAndSaveAdd"
            @press="testAndSaveAdd()"
          >
            {{ testingAdd ? 'Testando...' : savingAdd ? 'Salvando...' : 'Testar e salvar' }}
          </UiButton>
        </div>
      </section>
    </div>
  </div>
</template>
