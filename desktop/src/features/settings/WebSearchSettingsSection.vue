<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiSelect from '../../ui/UiSelect.vue'
import UiSwitch from '../../ui/UiSwitch.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import { useProviders } from '../providers/useProviders'
import type { ProviderWithStatus } from '../providers/providersApi'
import {
  MCP_SEARCH_PRESETS,
  WEB_SEARCH_SETTING_KEY,
  fetchSettings,
  saveSetting,
  type McpSearchConnectorConfig,
  type WebSearchMode,
  type WebSearchSettings,
} from './settingsApi'

const CARD_CLASSES = 'flex flex-col gap-4 rounded-md border border-line bg-surface p-5'
const INPUT_CLASSES = `h-9 w-full rounded-md border border-line bg-surface px-3 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`
const LABEL_TEXT_CLASSES = 'text-label font-medium text-muted'
const AUTO_PROVIDER_VALUE = ''
const CUSTOM_PRESET_VALUE = 'custom'

const MODE_OPTIONS: readonly { label: string; value: WebSearchMode }[] = [
  { label: 'Provedor auxiliar (chamada efêmera sem arquivos)', value: 'auxiliary_provider' },
  { label: 'Conector MCP / API de busca', value: 'mcp_connector' },
  { label: 'Desativado', value: 'disabled' },
]

const PRESET_OPTIONS = [
  ...MCP_SEARCH_PRESETS.map((preset) => ({ label: preset.label, value: preset.id })),
  { label: 'Personalizado', value: CUSTOM_PRESET_VALUE },
]

const providersStore = useProviders()

const mode = ref<WebSearchMode>('auxiliary_provider')
const auxiliaryProviderId = ref<string>(AUTO_PROVIDER_VALUE)
const selectedPreset = ref<string>(MCP_SEARCH_PRESETS[0].id)
const serverName = ref<string>(MCP_SEARCH_PRESETS[0].serverName)
const command = ref<string>(MCP_SEARCH_PRESETS[0].command)
const argsText = ref<string>(MCP_SEARCH_PRESETS[0].args.join(' '))
const url = ref<string>('')
const toolName = ref<string>(MCP_SEARCH_PRESETS[0].toolName)
const envKey = ref<string>('')
const envValue = ref<string>('')

const saving = ref(false)
const saveFeedback = ref<string | null>(null)
const saveError = ref<string | null>(null)
const togglingProviderId = ref<string | null>(null)

const eligibleAuxiliaryProviders = computed(() =>
  providersStore.providers.filter(
    (p) =>
      p.enabled !== false &&
      Boolean(p.hasNativeWebSearch) &&
      p.installed &&
      p.connectionStatus === 'connected',
  ),
)

const auxiliaryProviderOptions = computed(() => [
  { label: 'Automático (primeiro provedor conectado com busca nativa)', value: AUTO_PROVIDER_VALUE },
  ...eligibleAuxiliaryProviders.value.map((p) => ({ label: p.label, value: p.id })),
])

const activeEnvHint = computed(
  () => MCP_SEARCH_PRESETS.find((p) => p.id === selectedPreset.value)?.envHint ?? null,
)

onMounted(async () => {
  try {
    const [settings] = await Promise.all([
      fetchSettings(),
      providersStore.providers.length === 0 ? providersStore.load().catch(() => {}) : Promise.resolve(),
    ])
    const raw = settings[WEB_SEARCH_SETTING_KEY]
    if (!raw) return
    const parsed = JSON.parse(raw) as Partial<WebSearchSettings>
    if (parsed.mode === 'auxiliary_provider' || parsed.mode === 'mcp_connector' || parsed.mode === 'disabled') {
      mode.value = parsed.mode
    }
    auxiliaryProviderId.value = parsed.auxiliaryProviderId ?? AUTO_PROVIDER_VALUE
    if (parsed.mcpConnector) {
      const conn = parsed.mcpConnector
      selectedPreset.value = conn.preset ?? CUSTOM_PRESET_VALUE
      serverName.value = conn.serverName ?? ''
      command.value = conn.command ?? ''
      argsText.value = Array.isArray(conn.args) ? conn.args.join(' ') : ''
      url.value = conn.url ?? ''
      toolName.value = conn.toolName ?? ''
      const entries = Object.entries(conn.env ?? {})
      if (entries.length > 0) {
        envKey.value = entries[0][0]
        envValue.value = entries[0][1]
      }
    }
  } catch {
    // Mantém valores padrão se o JSON salvo estiver ausente ou inválido.
  }
})

function applyPreset(presetId: string): void {
  selectedPreset.value = presetId
  const preset = MCP_SEARCH_PRESETS.find((p) => p.id === presetId)
  if (!preset) return
  serverName.value = preset.serverName
  command.value = preset.command
  argsText.value = preset.args.join(' ')
  url.value = ''
  toolName.value = preset.toolName
  envKey.value = preset.envHint ?? ''
}

async function toggleProviderNativeSearch(provider: ProviderWithStatus, nextValue: boolean): Promise<void> {
  togglingProviderId.value = provider.id
  saveError.value = null
  try {
    const { installed: _installed, connectionStatus: _status, statusDetail: _detail, ...config } = provider
    await providersStore.save({
      ...config,
      hasNativeWebSearch: nextValue,
    })
  } catch (error) {
    saveError.value = `Não foi possível atualizar ${provider.label}: ${String(error)}`
  } finally {
    togglingProviderId.value = null
  }
}

async function saveSearchConfig(): Promise<void> {
  saving.value = true
  saveFeedback.value = null
  saveError.value = null
  try {
    const parsedArgs = argsText.value
      .split(/\s+/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0)
    const env: Record<string, string> = {}
    if (envKey.value.trim() && envValue.value.trim()) {
      env[envKey.value.trim()] = envValue.value.trim()
    }
    const mcpConnector: McpSearchConnectorConfig | null =
      mode.value === 'mcp_connector'
        ? {
            preset: selectedPreset.value === CUSTOM_PRESET_VALUE ? null : selectedPreset.value,
            serverName: serverName.value.trim(),
            command: command.value.trim(),
            args: parsedArgs,
            url: url.value.trim() || null,
            toolName: toolName.value.trim() || null,
            env,
          }
        : null

    const payload: WebSearchSettings = {
      mode: mode.value,
      auxiliaryProviderId: auxiliaryProviderId.value || null,
      mcpConnector,
    }
    await saveSetting(WEB_SEARCH_SETTING_KEY, JSON.stringify(payload))
    saveFeedback.value = 'Configuração de pesquisa web salva.'
  } catch (error) {
    saveError.value = `Erro ao salvar pesquisa: ${String(error)}`
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div class="flex flex-col gap-4" data-testid="web-search-settings">
    <section :class="CARD_CLASSES" aria-label="Pesquisa web unificada">
      <div>
        <h3 class="text-control font-semibold text-ink">Pesquisa web para provedores sem busca nativa</h3>
        <p class="text-caption text-muted">
          Provedores com busca nativa continuam usando sua busca inline (preservando prompt cache). Os demais usam o backend configurado abaixo.
        </p>
      </div>

      <label class="flex flex-col gap-1.5">
        <span :class="LABEL_TEXT_CLASSES">Modo de pesquisa</span>
        <UiSelect v-model="mode" :options="MODE_OPTIONS" label="Modo de pesquisa web" />
      </label>

      <div v-if="mode === 'auxiliary_provider'" class="flex flex-col gap-2">
        <label class="flex flex-col gap-1.5">
          <span :class="LABEL_TEXT_CLASSES">Provedor auxiliar</span>
          <UiSelect
            v-model="auxiliaryProviderId"
            :options="auxiliaryProviderOptions"
            label="Provedor auxiliar de pesquisa"
          />
        </label>
        <p v-if="eligibleAuxiliaryProviders.length === 0" class="text-small text-warn-text">
          Nenhum provedor conectado possui busca web nativa ativa no momento. Conecte um provedor compatível ou use um conector MCP.
        </p>
        <p v-else class="text-small text-muted">
          Executa turno efêmero sem ferramentas locais nem arquivos do projeto, retornando apenas resumo e URLs fonte.
        </p>
      </div>

      <div v-if="mode === 'mcp_connector'" class="flex flex-col gap-3 rounded-md border border-line bg-surface-2 p-4">
        <label class="flex flex-col gap-1.5">
          <span :class="LABEL_TEXT_CLASSES">Preset de conector MCP</span>
          <UiSelect
            :model-value="selectedPreset"
            :options="PRESET_OPTIONS"
            label="Preset de conector MCP"
            @update:model-value="applyPreset(String($event))"
          />
        </label>

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <label class="flex flex-col gap-1">
            <span :class="LABEL_TEXT_CLASSES">Nome do servidor</span>
            <input v-model="serverName" type="text" placeholder="brave-search" :class="INPUT_CLASSES" />
          </label>
          <label class="flex flex-col gap-1">
            <span :class="LABEL_TEXT_CLASSES">Ferramenta (opcional)</span>
            <input v-model="toolName" type="text" placeholder="web_search" :class="INPUT_CLASSES" />
          </label>
        </div>

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <label class="flex flex-col gap-1">
            <span :class="LABEL_TEXT_CLASSES">Comando (stdio)</span>
            <input v-model="command" type="text" placeholder="npx" :class="INPUT_CLASSES" />
          </label>
          <label class="flex flex-col gap-1">
            <span :class="LABEL_TEXT_CLASSES">Argumentos</span>
            <input v-model="argsText" type="text" placeholder="-y @upstash/context7-mcp" :class="INPUT_CLASSES" />
          </label>
        </div>

        <label class="flex flex-col gap-1">
          <span :class="LABEL_TEXT_CLASSES">URL HTTP MCP (opcional, substitui comando stdio)</span>
          <input v-model="url" type="url" placeholder="https://..." :class="INPUT_CLASSES" />
        </label>

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <label class="flex flex-col gap-1">
            <span :class="LABEL_TEXT_CLASSES">Variável de ambiente (chave)</span>
            <input
              v-model="envKey"
              type="text"
              :placeholder="activeEnvHint ?? 'API_KEY'"
              :class="INPUT_CLASSES"
            />
          </label>
          <label class="flex flex-col gap-1">
            <span :class="LABEL_TEXT_CLASSES">Valor / Token</span>
            <input v-model="envValue" type="password" placeholder="••••••" :class="INPUT_CLASSES" />
          </label>
        </div>
      </div>

      <div class="flex items-center justify-end gap-3 pt-1">
        <p v-if="saveFeedback" class="mr-auto text-small text-ok">{{ saveFeedback }}</p>
        <p v-if="saveError" class="mr-auto text-small text-danger">{{ saveError }}</p>
        <UiButton
          variant="primary"
          size="sm"
          :disabled="saving"
          data-testid="save-web-search"
          @press="saveSearchConfig()"
        >
          {{ saving ? 'Salvando...' : 'Salvar pesquisa' }}
        </UiButton>
      </div>
    </section>

    <section :class="CARD_CLASSES" aria-label="Busca web nativa por provedor">
      <div>
        <h3 class="text-control font-semibold text-ink">Busca web nativa por provedor</h3>
        <p class="text-caption text-muted">
          Quando ativado, o provedor usa sua busca embutida e não recebe a ferramenta <code>web_search</code> no prompt.
        </p>
      </div>

      <ul v-if="providersStore.providers.length > 0" class="flex flex-col divide-y divide-line rounded-md border border-line">
        <li
          v-for="provider in providersStore.providers"
          :key="provider.id"
          class="flex items-center justify-between gap-3 px-3.5 py-2.5"
        >
          <div class="flex min-w-0 flex-col">
            <span class="truncate text-control font-medium text-ink">{{ provider.label }}</span>
            <span class="text-small text-muted">
              {{ provider.hasNativeWebSearch ? 'Usa busca nativa inline' : 'Usa pesquisa unificada do Vokia' }}
            </span>
          </div>
          <UiSwitch
            :model-value="Boolean(provider.hasNativeWebSearch)"
            :disabled="togglingProviderId === provider.id"
            :label="`Busca web nativa em ${provider.label}`"
            @update:model-value="toggleProviderNativeSearch(provider, $event)"
          />
        </li>
      </ul>
      <p v-else class="text-caption text-muted">Nenhum provedor carregado.</p>
    </section>
  </div>
</template>
