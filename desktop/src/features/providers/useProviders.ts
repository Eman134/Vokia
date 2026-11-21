import { ref } from 'vue'
import { defineStore } from 'pinia'
import { useRoster } from '../team/useRoster'
import {
  autoConfigureProvider,
  buildSetupPrompt,
  deleteProvider,
  discoverProviderModels,
  inspectCli,
  listProviders,
  saveProvider,
  testProviderConfig,
  type DiscoverModelsResult,
  type InspectCliResult,
  type ProviderConfig,
  type ProviderWithStatus,
  type TestProviderResult,
} from './providersApi'

export const useProviders = defineStore('providers', () => {
  const providers = ref<ProviderWithStatus[]>([])
  const loading = ref(false)
  const discovering = ref<Record<string, boolean>>({})
  const error = ref<string | null>(null)

  const setupOpen = ref(false)
  const setupInitialProvider = ref('')

  function openSetup(provider?: string): void {
    setupInitialProvider.value = provider?.trim() ?? ''
    setupOpen.value = true
  }

  function closeSetup(): void {
    setupOpen.value = false
    setupInitialProvider.value = ''
  }

  async function load(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      providers.value = await listProviders()
    } catch (err) {
      error.value = String(err)
      throw err
    } finally {
      loading.value = false
    }
  }

  async function discover(id: string): Promise<DiscoverModelsResult> {
    discovering.value = { ...discovering.value, [id]: true }
    error.value = null
    try {
      const result = await discoverProviderModels(id)
      providers.value = providers.value.map((provider) =>
        provider.id === id ? { ...provider, models: result.models } : provider,
      )
      return result
    } catch (err) {
      error.value = String(err)
      throw err
    } finally {
      discovering.value = { ...discovering.value, [id]: false }
    }
  }

  async function save(config: ProviderConfig): Promise<ProviderConfig> {
    error.value = null
    try {
      const saved = await saveProvider(config)
      await Promise.all([load(), useRoster().load()])
      return saved
    } catch (err) {
      error.value = String(err)
      throw err
    }
  }

  async function remove(id: string): Promise<void> {
    error.value = null
    try {
      await deleteProvider(id)
      await Promise.all([load(), useRoster().load()])
    } catch (err) {
      error.value = String(err)
      throw err
    }
  }

  function inspect(command: string): Promise<InspectCliResult> {
    return inspectCli(command)
  }

  function buildPrompt(cliName: string, command: string, helpText: string): Promise<string> {
    return buildSetupPrompt(cliName, command, helpText)
  }

  function autoConfigure(cliName: string, command: string, helpText: string): Promise<ProviderConfig> {
    return autoConfigureProvider(cliName, command, helpText)
  }

  function testConfig(config: ProviderConfig, model?: string): Promise<TestProviderResult> {
    return testProviderConfig(config, model)
  }

  return {
    providers,
    loading,
    discovering,
    error,
    setupOpen,
    setupInitialProvider,
    openSetup,
    closeSetup,
    load,
    discover,
    save,
    remove,
    inspect,
    buildPrompt,
    autoConfigure,
    testConfig,
  }
})
