import { invoke } from '@tauri-apps/api/core'

export function fetchBypassMode(projectId: number): Promise<boolean> {
  return invoke<boolean>('get_bypass_mode', { projectId })
}

export function saveBypassMode(projectId: number, enabled: boolean): Promise<void> {
  return invoke<void>('set_bypass_mode', { projectId, enabled })
}
