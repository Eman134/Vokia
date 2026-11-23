import { ref } from 'vue'
import { defineStore } from 'pinia'
import { useTemporaryOverrides } from '../providers/useTemporaryOverrides'
import { replicaOf } from './agentCopies'
import type { Agent } from './events'
import {
  createAgent,
  deleteAgent,
  listAgents,
  setAgentInstructions,
  updateAgent,
  updateEmberLlm,
  type AgentDraft,
  type AgentProfile,
  type CliPreset,
  type LlmTarget,
  type ModelTier,
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
    void useTemporaryOverrides().load()
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

  async function updateEmber(params: {
    cli?: string | null
    modelTier?: ModelTier | null
    model?: string | null
    fallback?: LlmTarget[] | null
  }): Promise<AgentProfile> {
    const profile = await updateEmberLlm(params)
    agents.value = agents.value.map((agent) => (agent.id === profile.id ? profile : agent))
    return profile
  }

  async function setInstructions(id: Agent, text: string): Promise<AgentProfile> {
    const profile = await setAgentInstructions(id, text)
    agents.value = agents.value.map((agent) => (agent.id === id ? profile : agent))
    return profile
  }

  // Os cards do agente já foram para o Ember no banco; a recarga do backlog vem pelo evento backlog-changed.
  async function remove(id: Agent): Promise<void> {
    await deleteAgent(id)
    agents.value = agents.value.filter((agent) => agent.id !== id)
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
    const profile = profileOf(id)
    if (profile) return profile.name
    const named = instanceNames.value.get(id)
    if (named) return named
    const replica = replicaOf(id)
    if (replica) {
      const base = profileOf(replica.base)
      if (base) return `${base.name} ${replica.number}`
    }
    return id
  }

  function avatarOf(id: string): string {
    const profile = profileOf(id)
    if (profile) return profile.avatar
    const replica = replicaOf(id)
    if (replica) return profileOf(replica.base)?.avatar ?? replica.base.trim().toLowerCase()
    return id.trim().toLowerCase()
  }

  return {
    agents,
    presets,
    load,
    create,
    update,
    updateEmber,
    setInstructions,
    remove,
    profileOf,
    nameOf,
    nameInstance,
    avatarOf,
  }
})
