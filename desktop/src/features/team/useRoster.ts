import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from './events'
import { createAgent, listAgents, type AgentDraft, type AgentProfile, type CliPreset } from './teamApi'

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

  function profileOf(id: Agent): AgentProfile | undefined {
    return agents.value.find((agent) => agent.id === id)
  }

  function nameOf(id: Agent): string {
    return profileOf(id)?.name ?? id
  }

  function avatarOf(id: string): string {
    return profileOf(id)?.avatar ?? id.trim().toLowerCase()
  }

  return { agents, presets, load, create, profileOf, nameOf, avatarOf }
})
