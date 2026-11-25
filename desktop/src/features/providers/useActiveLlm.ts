import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'
import { defineStore } from 'pinia'
import {
  ACTIVE_LLM_SETTING_KEY,
  LLM_FALLBACK_SETTING_KEY,
  fetchSettings,
  saveSetting,
} from '../settings/settingsApi'
import type { OverrideReason, TemporaryOverride } from '../team/events'
import type { ModelTier, ReasoningEffort } from '../team/teamApi'
import { normalizeReasoningEffort } from './providerModelCatalog'
import { setActiveLlm } from './providersApi'

export interface ActiveLlmConfig {
  cli: string
  modelTier?: ModelTier | null
  model?: string | null
  reasoningEffort?: ReasoningEffort | null
}

export interface FallbackChainItem {
  cli: string
  modelTier?: ModelTier | null
  model?: string | null
  reasoningEffort?: ReasoningEffort | null
}

export interface FallbackConfig {
  enabled: boolean
  cooldownSecs: number
  chain: FallbackChainItem[]
}

const DEFAULT_COOLDOWN_SECS = 300

export const useActiveLlm = defineStore('activeLlm', () => {
  // Sem provedor padrão obrigatório: enquanto não houver escolha salva, fica vazio até o catálogo resolver.
  const active = ref<ActiveLlmConfig>({
    cli: '',
    modelTier: null,
    model: null,
    reasoningEffort: null,
  })
  const fallback = ref<FallbackConfig>({ enabled: true, cooldownSecs: DEFAULT_COOLDOWN_SECS, chain: [] })
  const overrides = ref<TemporaryOverride[]>([])
  const loaded = ref(false)

  async function load(): Promise<void> {
    try {
      const settings = await fetchSettings()
      if (settings[ACTIVE_LLM_SETTING_KEY]) {
        const parsed = JSON.parse(settings[ACTIVE_LLM_SETTING_KEY]) as Partial<ActiveLlmConfig>
        if (parsed.cli) {
          active.value = {
            cli: parsed.cli,
            modelTier: parsed.modelTier ?? null,
            model: parsed.model ?? null,
            reasoningEffort: normalizeReasoningEffort(parsed.reasoningEffort),
          }
        }
      }
      if (settings[LLM_FALLBACK_SETTING_KEY]) {
        const raw = JSON.parse(settings[LLM_FALLBACK_SETTING_KEY]) as
          | Partial<FallbackConfig>
          | FallbackChainItem[]
        if (Array.isArray(raw)) {
          fallback.value = { enabled: true, cooldownSecs: DEFAULT_COOLDOWN_SECS, chain: raw }
        } else {
          fallback.value = {
            enabled: raw.enabled ?? true,
            cooldownSecs: typeof raw.cooldownSecs === 'number' ? raw.cooldownSecs : DEFAULT_COOLDOWN_SECS,
            chain: Array.isArray(raw.chain) ? raw.chain : [],
          }
        }
      }
    } catch {
      // Configurações ainda não gravadas ou JSON legado: mantém estado inicial sem assumir provedor
    }
    await loadOverrides()
    loaded.value = true
  }

  async function saveActive(config: ActiveLlmConfig): Promise<void> {
    const normalized: ActiveLlmConfig = {
      cli: config.cli,
      modelTier: config.modelTier ?? null,
      model: config.model ?? null,
      reasoningEffort: normalizeReasoningEffort(config.reasoningEffort),
    }
    active.value = normalized
    try {
      const saved = await setActiveLlm({
        cli: normalized.cli,
        modelTier: normalized.modelTier ?? null,
        model: normalized.model ?? null,
        reasoningEffort: normalized.reasoningEffort ?? null,
      })
      active.value = {
        cli: saved.cli,
        modelTier: saved.modelTier ?? null,
        model: saved.model ?? null,
        reasoningEffort: normalizeReasoningEffort(saved.reasoningEffort),
      }
    } catch {
      await saveSetting(ACTIVE_LLM_SETTING_KEY, JSON.stringify(active.value))
    }
  }

  async function saveFallback(config: FallbackConfig): Promise<void> {
    fallback.value = {
      enabled: config.enabled,
      cooldownSecs: config.cooldownSecs,
      chain: [...config.chain],
    }
    await saveSetting(LLM_FALLBACK_SETTING_KEY, JSON.stringify(fallback.value))
  }

  async function loadOverrides(): Promise<void> {
    try {
      overrides.value = await invoke<TemporaryOverride[]>('list_temporary_overrides')
    } catch {
      overrides.value = []
    }
  }

  function syncOverrides(list: TemporaryOverride[]): void {
    overrides.value = list
  }

  async function applyOverride(params: {
    agent: string
    cli: string
    modelTier?: ModelTier | null
    model?: string | null
    reasoningEffort?: ReasoningEffort | null
    reason?: OverrideReason
    expiresInMs?: number | null
  }): Promise<TemporaryOverride> {
    const ov = await invoke<TemporaryOverride>('set_temporary_override', {
      agent: params.agent,
      cli: params.cli,
      modelTier: params.modelTier ?? null,
      model: params.model ?? null,
      reasoningEffort: params.reasoningEffort ?? null,
      reason: params.reason ?? 'manual-temp',
      expiresInMs: params.expiresInMs ?? null,
    })
    await loadOverrides()
    return ov
  }

  async function clearOverride(agent: string): Promise<void> {
    await invoke<void>('clear_temporary_override', { agent })
    await loadOverrides()
  }

  async function clearAllOverrides(): Promise<void> {
    await invoke<void>('clear_all_temporary_overrides')
    overrides.value = []
  }

  return {
    active,
    fallback,
    overrides,
    loaded,
    load,
    saveActive,
    saveFallback,
    loadOverrides,
    syncOverrides,
    applyOverride,
    clearOverride,
    clearAllOverrides,
  }
})
