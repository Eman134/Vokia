import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from './events'
import {
  createAgent,
  listAgents,
  updateAgent,
  type AgentDraft,
  type AgentProfile,
  type CliPreset,
} from './teamApi'

export function normalizeName(text: string): string {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()
}

export const useRoster = defineStore('roster', () => {
  const agents = ref<AgentProfile[]>([])
  const presets = ref<CliPreset[]>([])

  async function load(): Promise<void> {
    const listing = await listAgents()
    agents.value = listing.agents
    presets.value = listing.presets
  }

  async function create(draft: AgentDraft): Promise<AgentProfile> {
    const profile = await createAgent(draft)
    agents.value = [...agents.value, profile]
    return profile
  }

  async function update(id: Agent, draft: AgentDraft): Promise<AgentProfile> {
    const profile = await updateAgent(id, draft)
    agents.value = agents.value.map((agent) => (agent.id === id ? profile : agent))
    return profile
  }

  function profileOf(id: Agent): AgentProfile | undefined {
    return agents.value.find((agent) => agent.id === id)
  }

  // Réplica não está no elenco: o nome de exibição (ex.: "Davi 2") chega do backend pelo evento agent-named.
  const instanceNames = ref(new Map<Agent, string>())

  function nameInstance(id: Agent, name: string): void {
    instanceNames.value = new Map(instanceNames.value).set(id, name)
  }

  function nameOf(id: Agent): string {
    return profileOf(id)?.name ?? instanceNames.value.get(id) ?? id
  }

  function avatarOf(id: string): string {
    return profileOf(id)?.avatar ?? id.trim().toLowerCase()
  }

  return {
    agents,
    presets,
    load,
    create,
    update,
    profileOf,
    nameOf,
    nameInstance,
    avatarOf,
  }
})
