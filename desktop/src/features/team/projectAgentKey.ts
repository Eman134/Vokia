import type { Agent } from './events'

export function projectAgentKey(projectId: number, agent: Agent): string {
  return `${projectId}:${agent}`
}
