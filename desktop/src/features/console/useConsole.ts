import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import type { Agent, ConsoleEntry, VokiaConsoleLine } from '../team/events'
import { projectAgentKey } from '../team/projectAgentKey'
import { useProjects } from '../projects/useProjects'
import { fetchAgentConsole } from './consoleApi'

export const CONSOLE_BUFFER_LINES = 500

export const useConsole = defineStore('console', () => {
  const entries = ref<Record<string, ConsoleEntry[]>>({})
  const openAgents = ref<Agent[]>([])
  const activeAgent = ref<Agent | null>(null)
  const loadErrors = ref<Record<Agent, string>>({})
  const projects = useProjects()

  function resetPanel(): void {
    openAgents.value = []
    activeAgent.value = null
    loadErrors.value = {}
  }

  watch(() => projects.selected?.id, resetPanel)

  function merge(key: string, incoming: readonly ConsoleEntry[]): void {
    const current = entries.value[key] ?? []
    const known = new Set(current.map((entry) => entry.seq))
    const merged = [...current, ...incoming.filter((entry) => !known.has(entry.seq))]
    merged.sort((first, second) => first.seq - second.seq)
    entries.value[key] = merged.slice(-CONSOLE_BUFFER_LINES)
  }

  function apply(event: VokiaConsoleLine): void {
    const { seq, kind, text, time } = event
    merge(projectAgentKey(event.projectId, event.agent), [{ seq, kind, text, time }])
  }

  function entriesOf(agent: Agent): ConsoleEntry[] {
    const project = projects.selected
    return project === null ? [] : (entries.value[projectAgentKey(project.id, agent)] ?? [])
  }

  async function load(agent: Agent): Promise<void> {
    delete loadErrors.value[agent]
    const project = projects.selected
    if (project === null) return
    try {
      merge(projectAgentKey(project.id, agent), await fetchAgentConsole(project.id, agent))
    } catch (error) {
      if (projects.selected?.id === project.id) {
        loadErrors.value[agent] = `Não foi possível carregar o console: ${String(error)}`
      }
    }
  }

  async function open(agent: Agent): Promise<void> {
    if (!openAgents.value.includes(agent)) {
      openAgents.value.push(agent)
    }
    activeAgent.value = agent
    await load(agent)
  }

  function activate(agent: Agent): void {
    activeAgent.value = agent
  }

  function close(agent: Agent): void {
    const index = openAgents.value.indexOf(agent)
    if (index === -1) return
    openAgents.value.splice(index, 1)
    delete loadErrors.value[agent]
    if (activeAgent.value === agent) {
      activeAgent.value = openAgents.value[index] ?? openAgents.value[index - 1] ?? null
    }
  }

  function dismissError(agent: Agent): void {
    delete loadErrors.value[agent]
  }

  function forgetProject(projectId: number): void {
    const prefix = projectAgentKey(projectId, '')
    entries.value = Object.fromEntries(Object.entries(entries.value).filter(([key]) => !key.startsWith(prefix)))
  }

  function forgetAll(): void {
    entries.value = {}
    resetPanel()
  }

  return {
    entries,
    openAgents,
    activeAgent,
    loadErrors,
    apply,
    entriesOf,
    open,
    load,
    activate,
    close,
    dismissError,
    forgetProject,
    forgetAll,
  }
})
