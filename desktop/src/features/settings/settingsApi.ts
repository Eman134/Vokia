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

export function fetchSettings(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>(GET_SETTINGS_COMMAND)
}

export function saveSetting(key: string, value: string): Promise<void> {
  return invoke<void>(SET_SETTING_COMMAND, { key, value })
}

export function eraseAllData(): Promise<void> {
  return invoke<void>(ERASE_ALL_DATA_COMMAND)
}
