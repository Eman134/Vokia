import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from '../team/events'
import { useFeed } from '../team/useFeed'
import { fetchAgentActivity, type AgentActivity, type AgentRun } from './agentActivity'

// Atividade dos agentes em todos os projetos: recarregada a cada mudança de estado de agente e de backlog.
export const useAgentActivity = defineStore('agentActivity', () => {
  const feed = useFeed()
  const groups = ref<AgentActivity[]>([])
  let loadSeq = 0

  async function load(): Promise<void> {
    const seq = (loadSeq += 1)
    try {
      const next = await fetchAgentActivity()
      if (seq === loadSeq) groups.value = next
    } catch (error) {
      if (seq === loadSeq) feed.feed.push({ type: 'error', message: error instanceof Error ? error.message : String(error) })
    }
  }

  function runsOf(agent: Agent): AgentRun[] {
    return groups.value.find((group) => group.agent === agent)?.runs ?? []
  }

  return { load, runsOf }
})
