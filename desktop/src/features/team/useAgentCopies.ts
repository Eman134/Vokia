import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from './events'
import { fetchAgentCopies, type AgentCopy } from './agentCopies'
import { useFeed } from './useFeed'
import { useProjects } from '../projects/useProjects'

export const useAgentCopies = defineStore('agentCopies', () => {
  const projects = useProjects()
  const feed = useFeed()
  const copies = ref<AgentCopy[]>([])
  const selectedId = ref<string | null>(null)

  async function load(): Promise<void> {
    const projectId = projects.selected?.id
    if (projectId === undefined) {
      copies.value = []
      return
    }
    try {
      copies.value = await fetchAgentCopies(projectId)
    } catch (error) {
      feed.feed.push({ type: 'error', message: error instanceof Error ? error.message : String(error) })
    }
  }

  function copiesOf(agent: Agent): AgentCopy[] {
    return copies.value.filter((copy) => copy.agent === agent)
  }

  function select(id: string): void {
    selectedId.value = selectedId.value === id ? null : id
  }

  return { selectedId, load, copiesOf, select }
})
