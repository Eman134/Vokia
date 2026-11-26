import { invoke } from '@tauri-apps/api/core'
import type {
  EffortMode,
  LlmTarget,
  ModelEffortRule,
  ModelGroupingMode,
  ModelTier,
  ModelTiers,
  ReasoningEffort,
  ReasoningEffortSpec,
  ToolTransport,
} from '../team/teamApi'

export type {
  EffortMode,
  LlmTarget,
  ModelEffortRule,
  ModelGroupingMode,
  ModelTier,
  ModelTiers,
  ReasoningEffort,
  ReasoningEffortSpec,
  ToolTransport,
}

const LIST_PROVIDERS_COMMAND = 'list_providers'
const CHECK_PROVIDER_STATUS_COMMAND = 'check_provider_status'
const GET_ACTIVE_LLM_COMMAND = 'get_active_llm'
const SET_ACTIVE_LLM_COMMAND = 'set_active_llm'
const SET_PROVIDER_ENABLED_COMMAND = 'set_provider_enabled'
const SAVE_PROVIDER_COMMAND = 'save_provider'
const DELETE_PROVIDER_COMMAND = 'delete_provider'
const DISCOVER_PROVIDER_MODELS_COMMAND = 'discover_provider_models'
const INSPECT_CLI_COMMAND = 'inspect_cli'
const BUILD_SETUP_PROMPT_COMMAND = 'build_setup_prompt'
const TEST_PROVIDER_CONFIG_COMMAND = 'test_provider_config'
const AUTO_CONFIGURE_PROVIDER_COMMAND = 'auto_configure_provider'
const START_PROVIDER_LOGIN_COMMAND = 'start_provider_login'
const GET_PROVIDER_LOGIN_STATE_COMMAND = 'get_provider_login_state'
const SUBMIT_PROVIDER_LOGIN_CODE_COMMAND = 'submit_provider_login_code'
const CANCEL_PROVIDER_LOGIN_COMMAND = 'cancel_provider_login'
const LOGOUT_PROVIDER_COMMAND = 'logout_provider'
const LIST_PROVIDER_ENV_COMMAND = 'list_provider_env'
const SET_PROVIDER_ENV_COMMAND = 'set_provider_env'
const DELETE_PROVIDER_ENV_COMMAND = 'delete_provider_env'

export type ProviderConnectionStatus = 'connected' | 'signed-out' | 'missing' | 'error'
export type ProviderDiagnosticState = 'not_installed' | 'installed_signed_out' | 'connected' | 'error'
export type ExecutableSource = 'path' | 'known-dir' | 'explicit'
export type AuthFlowKind = 'browser' | 'device-code' | 'paste-code' | 'api-key' | 'free-no-login'
export type LoginSessionPhase =
  | 'starting'
  | 'awaiting-browser'
  | 'awaiting-device-code'
  | 'awaiting-paste-code'
  | 'completed'
  | 'cancelled'
  | 'error'

export interface ProviderAuthProbe {
  envVars?: string[]
  credentialFiles?: string[]
  freeWithoutLogin?: boolean
}

export interface ProviderAuthFlowSpec {
  id: string
  kind: AuthFlowKind
  label: string
  description?: string | null
  commandArgs?: string[]
  envVar?: string | null
  sensitive?: boolean
  supportsAppServerRpc?: boolean
}

export interface ProviderLogoutSpec {
  commandArgs?: string[]
  sharesTerminalLogin?: boolean
  warning?: string | null
  clearEnvVars?: string[]
}

export interface ProviderEnvPreset {
  key: string
  label: string
  placeholder?: string | null
  sensitive?: boolean
  description?: string | null
}

export interface ProviderLoginState {
  providerId: string
  flowId: string
  kind: AuthFlowKind
  phase: LoginSessionPhase
  url?: string | null
  userCode?: string | null
  message?: string | null
  errorCategory?: string | null
}

export interface ProviderLogoutResult {
  ok: boolean
  providerId: string
  sharesTerminalLogin: boolean
  warning?: string | null
  message: string
}

export interface ProviderEnvVarView {
  key: string
  sensitive: boolean
  stored: boolean
  status: 'missing' | 'saved' | 'unreadable' | string
  value?: string | null
  storageKind?: string | null
}

export interface ProviderStatusSummary {
  id: string
  label: string
  installed: boolean
  connectionStatus: ProviderConnectionStatus
  diagnosticState: ProviderDiagnosticState
  version?: string | null
  executablePath?: string | null
  executableSource?: ExecutableSource | null
  errorCategory?: string | null
  statusDetail?: string | null
  installHint?: string | null
  connectHint?: string | null
  supportsFreeWithoutLogin: boolean
}

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
  enabled?: boolean
  disabledReason?: string | null
  protocol: ProviderProtocol
  defaultCommand: string
  defaultModel: string
  models: string[]
  freeModels?: string[]
  tiers: ModelTiers
  supportsMcp: boolean
  toolTransport?: ToolTransport | null
  hasNativeWebSearch?: boolean
  supportsSandbox: boolean
  reservedFlags: string[]
  rateLimitPatterns: string[]
  modelUnavailablePatterns: string[]
  discovery?: ModelDiscoveryConfig | null
  generic?: GenericCliSpec | null
  modelGrouping?: ModelGroupingMode
  effort?: ReasoningEffortSpec | null
  authProbe?: ProviderAuthProbe | null
  authFlows?: ProviderAuthFlowSpec[]
  logout?: ProviderLogoutSpec | null
  envPresets?: ProviderEnvPreset[]
  installHint?: string | null
  connectHint?: string | null
  badgeCode?: string | null
  displayOrder?: number | null
  builtinSeed: boolean
}

export interface ProviderWithStatus extends ProviderConfig {
  installed: boolean
  connectionStatus?: ProviderConnectionStatus
  diagnosticState?: ProviderDiagnosticState
  version?: string | null
  executablePath?: string | null
  executableSource?: ExecutableSource | null
  errorCategory?: string | null
  statusDetail?: string | null
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

export function checkProviderStatus(id?: string): Promise<ProviderStatusSummary[]> {
  return invoke<ProviderStatusSummary[]>(CHECK_PROVIDER_STATUS_COMMAND, {
    id: id ?? null,
  })
}

export function getActiveLlm(): Promise<LlmTarget | null> {
  return invoke<LlmTarget | null>(GET_ACTIVE_LLM_COMMAND)
}

export function setActiveLlm(params: {
  cli: string
  modelTier?: ModelTier | null
  model?: string | null
  reasoningEffort?: ReasoningEffort | null
}): Promise<LlmTarget> {
  return invoke<LlmTarget>(SET_ACTIVE_LLM_COMMAND, {
    cli: params.cli,
    modelTier: params.modelTier ?? null,
    model: params.model ?? null,
    reasoningEffort: params.reasoningEffort ?? null,
  })
}

export function saveProvider(config: ProviderConfig): Promise<ProviderConfig> {
  return invoke<ProviderConfig>(SAVE_PROVIDER_COMMAND, { config })
}

export function setProviderEnabled(
  id: string,
  enabled: boolean,
  reason?: string | null,
): Promise<ProviderConfig> {
  return invoke<ProviderConfig>(SET_PROVIDER_ENABLED_COMMAND, {
    id,
    enabled,
    reason: reason ?? null,
  })
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

export function startProviderLogin(
  id: string,
  flowId?: string | null,
  apiKey?: string | null,
): Promise<ProviderLoginState> {
  return invoke<ProviderLoginState>(START_PROVIDER_LOGIN_COMMAND, {
    id,
    flowId: flowId ?? null,
    apiKey: apiKey ?? null,
  })
}

export function getProviderLoginState(id: string): Promise<ProviderLoginState | null> {
  return invoke<ProviderLoginState | null>(GET_PROVIDER_LOGIN_STATE_COMMAND, { id })
}

export function submitProviderLoginCode(id: string, code: string): Promise<ProviderLoginState> {
  return invoke<ProviderLoginState>(SUBMIT_PROVIDER_LOGIN_CODE_COMMAND, { id, code })
}

export function cancelProviderLogin(id: string): Promise<ProviderLoginState> {
  return invoke<ProviderLoginState>(CANCEL_PROVIDER_LOGIN_COMMAND, { id })
}

export function logoutProvider(id: string): Promise<ProviderLogoutResult> {
  return invoke<ProviderLogoutResult>(LOGOUT_PROVIDER_COMMAND, { id })
}

export function listProviderEnv(id: string): Promise<ProviderEnvVarView[]> {
  return invoke<ProviderEnvVarView[]>(LIST_PROVIDER_ENV_COMMAND, { id })
}

export function setProviderEnv(
  id: string,
  key: string,
  value: string,
  sensitive = false,
): Promise<ProviderEnvVarView> {
  return invoke<ProviderEnvVarView>(SET_PROVIDER_ENV_COMMAND, { id, key, value, sensitive })
}

export function deleteProviderEnv(id: string, key: string): Promise<boolean> {
  return invoke<boolean>(DELETE_PROVIDER_ENV_COMMAND, { id, key })
}
