import { invoke } from '@tauri-apps/api/core'
import type { Agent, ConsoleEntry } from '../team/events'

const AGENT_CONSOLE_COMMAND = 'agent_console'

export function fetchAgentConsole(projectId: number, agent: Agent): Promise<ConsoleEntry[]> {
  return invoke<ConsoleEntry[]>(AGENT_CONSOLE_COMMAND, { projectId, agent })
}
