import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { ModelTier } from '../team/teamApi'
import { fetchSettings, saveSetting } from '../settings/settingsApi'

export const ACTIVE_LLM_SETTING_KEY = 'active_llm'
export const LLM_FALLBACK_SETTING_KEY = 'llm_fallback'

export const useOnboardingLlm = defineStore('onboardingLlm', () => {
  const defaultCli = ref('')
  const effortLevel = ref<ModelTier>('intermediate')
  const fallbackCli = ref('')
  const initialized = ref(false)

  async function initFromSettings(connectedIds: string[]): Promise<void> {
    if (initialized.value) return
    try {
      const settings = await fetchSettings()
      if (settings[ACTIVE_LLM_SETTING_KEY]) {
        const parsed = JSON.parse(settings[ACTIVE_LLM_SETTING_KEY]) as {
          cli?: string
          modelTier?: ModelTier | null
          tier?: ModelTier | null
        }
        if (parsed.cli) defaultCli.value = parsed.cli
        const tier = parsed.modelTier ?? parsed.tier
        if (tier) effortLevel.value = tier
      }
      if (settings[LLM_FALLBACK_SETTING_KEY]) {
        const raw = JSON.parse(settings[LLM_FALLBACK_SETTING_KEY]) as
          | { cli?: string; chain?: { cli?: string }[] }
          | { cli?: string }[]
        if (Array.isArray(raw)) {
          if (raw[0]?.cli) fallbackCli.value = raw[0].cli
        } else if (Array.isArray(raw.chain) && raw.chain[0]?.cli) {
          fallbackCli.value = raw.chain[0].cli
        } else if (raw.cli) {
          fallbackCli.value = raw.cli
        }
      }
    } catch {
      // Settings opcionais no onboarding inicial
    }
    // Sem provedor padrão obrigatório: só pré-seleciona se houver algum provedor já conectado.
    if (!defaultCli.value && connectedIds.length > 0) {
      defaultCli.value = connectedIds[0]
    }
    initialized.value = true
  }

  async function persist(): Promise<void> {
    if (defaultCli.value) {
      await saveSetting(
        ACTIVE_LLM_SETTING_KEY,
        JSON.stringify({ cli: defaultCli.value, modelTier: effortLevel.value, model: null }),
      )
    }
    if (fallbackCli.value) {
      await saveSetting(
        LLM_FALLBACK_SETTING_KEY,
        JSON.stringify({
          enabled: true,
          cooldownSecs: 300,
          chain: [{ cli: fallbackCli.value, modelTier: effortLevel.value, model: null }],
        }),
      )
    }
  }

  return { defaultCli, effortLevel, fallbackCli, initialized, initFromSettings, persist }
})
