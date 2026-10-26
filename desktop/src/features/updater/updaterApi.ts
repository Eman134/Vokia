import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

export type { DownloadEvent, Update }

export function checkForUpdate(timeoutMs: number): Promise<Update | null> {
  return check({ timeout: timeoutMs })
}

export function relaunchApp(): Promise<void> {
  return relaunch()
}
