import { invoke } from '@tauri-apps/api/core'

const GET_SETTINGS_COMMAND = 'get_settings'
const SET_SETTING_COMMAND = 'set_setting'

export const THEME_SETTING_KEY = 'theme'

export function fetchSettings(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>(GET_SETTINGS_COMMAND)
}

export function saveSetting(key: string, value: string): Promise<void> {
  return invoke<void>(SET_SETTING_COMMAND, { key, value })
}
