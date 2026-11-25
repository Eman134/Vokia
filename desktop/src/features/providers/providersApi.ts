import { invoke } from '@tauri-apps/api/core'
import type { ModelTier, ModelTiers, ToolTransport } from '../team/teamApi'

export type { ModelTier, ModelTiers, ToolTransport }

const LIST_PROVIDERS_COMMAND = 'list_providers'
const SAVE_PROVIDER_COMMAND = 'save_provider'
const DELETE_PROVIDER_COMMAND = 'delete_provider'
const DISCOVER_PROVIDER_MODELS_COMMAND = 'discover_provider_models'
const INSPECT_CLI_COMMAND = 'inspect_cli'
const BUILD_SETUP_PROMPT_COMMAND = 'build_setup_prompt'
const TEST_PROVIDER_CONFIG_COMMAND = 'test_provider_config'
const AUTO_CONFIGURE_PROVIDER_COMMAND = 'auto_configure_provider'

export type ProviderProtocol =
  | 'claude-live'
  | 'codex-app-server'
  | 'codex-jsonl'
  | 'open-code-json'
  | 'antigravity-stream'
  | 'generic-cli'

export type ModelDiscoveryKind = 'command-lines' | 'codex-cache-file' | 'json-array'

export interface ModelDiscoveryConfig {
  kind?: ModelDiscoveryKind
  args: string[]
  jsonPath?: string | null
  cacheFile?: string | null
}

export type GenericOutputFormat = 'text' | 'jsonl' | 'json'

export interface GenericCliSpec {
  outputFormat?: GenericOutputFormat
  promptArgs: string[]
  promptViaStdin: boolean
  modelFlag?: string | null
  dirFlag?: string | null
  jsonOutputArgs: string[]
  textJsonPaths: string[]
  errorJsonPaths: string[]
  sandboxArgs: Record<string, string[]>
}

export interface ProviderConfig {
  id: string
  label: string
  protocol: ProviderProtocol
  defaultCommand: string
  defaultModel: string
  models: string[]
  freeModels?: string[]
  tiers: ModelTiers
  supportsMcp: boolean
  toolTransport?: ToolTransport | null
  supportsSandbox: boolean
  reservedFlags: string[]
  rateLimitPatterns: string[]
  modelUnavailablePatterns: string[]
  discovery?: ModelDiscoveryConfig | null
  generic?: GenericCliSpec | null
  builtinSeed: boolean
}

export interface ProviderWithStatus extends ProviderConfig {
  installed: boolean
}

export interface DiscoverModelsResult {
  models: string[]
  source: 'live' | 'cache'
  warning: string | null
}

export interface InspectCliResult {
  found: boolean
  programPath: string | null
  helpText: string
}

export interface TestProviderResult {
  ok: boolean
  latencyMs: number
  reply: string
  errorKind: 'limit' | 'model-unavailable' | 'other' | null
  errorMessage: string | null
}

export function listProviders(): Promise<ProviderWithStatus[]> {
  return invoke<ProviderWithStatus[]>(LIST_PROVIDERS_COMMAND)
}

export function saveProvider(config: ProviderConfig): Promise<ProviderConfig> {
  return invoke<ProviderConfig>(SAVE_PROVIDER_COMMAND, { config })
}

export function deleteProvider(id: string): Promise<void> {
  return invoke<void>(DELETE_PROVIDER_COMMAND, { id })
}

export function discoverProviderModels(id: string): Promise<DiscoverModelsResult> {
  return invoke<DiscoverModelsResult>(DISCOVER_PROVIDER_MODELS_COMMAND, { id })
}

export function inspectCli(command: string): Promise<InspectCliResult> {
  return invoke<InspectCliResult>(INSPECT_CLI_COMMAND, { command })
}

export function buildSetupPrompt(cliName: string, command: string, helpText: string): Promise<string> {
  return invoke<string>(BUILD_SETUP_PROMPT_COMMAND, { cliName, command, helpText })
}

export function testProviderConfig(config: ProviderConfig, model?: string): Promise<TestProviderResult> {
  return invoke<TestProviderResult>(TEST_PROVIDER_CONFIG_COMMAND, { config, model: model ?? null })
}

export function autoConfigureProvider(cliName: string, command: string, helpText: string): Promise<ProviderConfig> {
  return invoke<ProviderConfig>(AUTO_CONFIGURE_PROVIDER_COMMAND, { cliName, command, helpText })
}
