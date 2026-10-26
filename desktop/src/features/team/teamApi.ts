import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { VOKIA_EVENT, type CliProblem, type VokiaEvent } from './events'

export type { UnlistenFn }

const CLI_PROBLEM_COMMAND = 'cli_problem'
const SEND_REQUEST_COMMAND = 'send_request'
const CHECK_CONNECTION_COMMAND = 'check_connection'

export function fetchCliProblem(): Promise<CliProblem | null> {
  return invoke<CliProblem | null>(CLI_PROBLEM_COMMAND)
}

export function sendRequest(text: string): Promise<void> {
  return invoke<void>(SEND_REQUEST_COMMAND, { text })
}

export function listenTeamEvents(onEvent: (event: VokiaEvent) => void): Promise<UnlistenFn> {
  return listen<VokiaEvent>(VOKIA_EVENT, (event) => onEvent(event.payload))
}

export function checkConnection(): Promise<boolean> {
  // Offline é estado, não erro: uma exceção do comando conta como falso.
  return invoke<boolean>(CHECK_CONNECTION_COMMAND).catch(() => false)
}

export function listenConnectionChanges(onOnline: () => void, onOffline: () => void): () => void {
  window.addEventListener('online', onOnline)
  window.addEventListener('offline', onOffline)
  return () => {
    window.removeEventListener('online', onOnline)
    window.removeEventListener('offline', onOffline)
  }
}
