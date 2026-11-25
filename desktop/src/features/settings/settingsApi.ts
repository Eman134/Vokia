import { invoke } from '@tauri-apps/api/core'

const GET_SETTINGS_COMMAND = 'get_settings'
const SET_SETTING_COMMAND = 'set_setting'
const ERASE_ALL_DATA_COMMAND = 'erase_all_data'

export const THEME_SETTING_KEY = 'theme'
export const AVATAR_PROFILES_SETTING_KEY = 'avatar_profiles'
export const USER_NAME_SETTING_KEY = 'user_name'
export const USER_AVATAR_SETTING_KEY = 'user_avatar'
export const ACTIVE_LLM_SETTING_KEY = 'active_llm'
export const LLM_FALLBACK_SETTING_KEY = 'llm_fallback'
export const AUTO_PAUSE_ON_LIMIT_SETTING_KEY = 'auto_pause_on_limit'
export const CONTINUATION_ENABLED_SETTING_KEY = 'continuationEnabled'
export const CONTINUATION_MAX_COUNT_SETTING_KEY = 'continuationMaxCount'
export const WEB_SEARCH_SETTING_KEY = 'web_search'

export type WebSearchMode = 'auxiliary_provider' | 'mcp_connector' | 'disabled'

export interface McpSearchConnectorConfig {
  preset?: string | null
  serverName: string
  command: string
  args: string[]
  url?: string | null
  toolName?: string | null
  env: Record<string, string>
}

export interface WebSearchSettings {
  mode: WebSearchMode
  auxiliaryProviderId?: string | null
  mcpConnector?: McpSearchConnectorConfig | null
}

export interface McpSearchPreset {
  id: string
  label: string
  serverName: string
  command: string
  args: string[]
  toolName: string
  envHint?: string
}

export const MCP_SEARCH_PRESETS: readonly McpSearchPreset[] = [
  {
    id: 'context7',
    label: 'Context7 (Documentação)',
    serverName: 'context7',
    command: 'npx',
    args: ['-y', '@upstash/context7-mcp'],
    toolName: 'query-docs',
  },
  {
    id: 'brave',
    label: 'Brave Search',
    serverName: 'brave-search',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-brave-search'],
    toolName: 'brave_web_search',
    envHint: 'BRAVE_API_KEY',
  },
  {
    id: 'exa',
    label: 'Exa Search',
    serverName: 'exa',
    command: 'npx',
    args: ['-y', 'exa-mcp-server'],
    toolName: 'web_search_exa',
    envHint: 'EXA_API_KEY',
  },
  {
    id: 'tavily',
    label: 'Tavily Search',
    serverName: 'tavily',
    command: 'npx',
    args: ['-y', 'tavily-mcp'],
    toolName: 'tavily-search',
    envHint: 'TAVILY_API_KEY',
  },
]

export function fetchSettings(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>(GET_SETTINGS_COMMAND)
}

export function saveSetting(key: string, value: string): Promise<void> {
  return invoke<void>(SET_SETTING_COMMAND, { key, value })
}

export function eraseAllData(): Promise<void> {
  return invoke<void>(ERASE_ALL_DATA_COMMAND)
}
