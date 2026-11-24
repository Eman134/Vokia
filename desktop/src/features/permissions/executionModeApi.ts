import { invoke } from '@tauri-apps/api/core'
import type { ExecutionMode } from './executionModes'

export function fetchExecutionMode(projectId: number): Promise<ExecutionMode> {
  return invoke<ExecutionMode>('get_execution_mode', { projectId })
}

export function saveExecutionMode(projectId: number, mode: ExecutionMode): Promise<void> {
  return invoke<void>('set_execution_mode', { projectId, mode })
}
