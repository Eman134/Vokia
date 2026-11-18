import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from './events'
import { fetchAgentCopies, type AgentCopies, type AgentCopy } from './agentCopies'
import { useFeed } from './useFeed'
import { useProjects } from '../projects/useProjects'

export const useAgentCopies = defineStore('agentCopies', () => {
  const projects = useProjects()
  const feed = useFeed()
  const groups = ref<AgentCopies[]>([])
  const selectedId = ref<string | null>(null)

  async function load(): Promise<void> {
    const projectId = projects.selected?.id
    if (projectId === undefined) {
      groups.value = []
      return
    }
    try {
      groups.value = await fetchAgentCopies(projectId)
    } catch (error) {
      feed.feed.push({ type: 'error', message: error instanceof Error ? error.message : String(error) })
    }
  }

  function copiesOf(agent: Agent): AgentCopy[] {
    return groups.value.find((group) => group.agent === agent)?.copies ?? []
  }

  function select(id: string): void {
    selectedId.value = selectedId.value === id ? null : id
  }

  return { selectedId, load, copiesOf, select }
})
