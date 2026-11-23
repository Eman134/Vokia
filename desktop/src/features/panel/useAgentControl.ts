import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { useProjects } from '../projects/useProjects'
import type { Agent } from '../team/events'
import { projectAgentKey } from '../team/projectAgentKey'
import { listAgentPauses, setAgentPaused, type AgentPause } from '../team/teamApi'

const NO_PROJECT_MESSAGE = 'Selecione um projeto para pausar ou retomar um agente.'

export const useAgentControl = defineStore('agentControl', () => {
  const projects = useProjects()
  const paused = ref<Set<string>>(new Set())
  const pausedList = computed<AgentPause[]>(() =>
    [...paused.value].map((key) => {
      const [projectId, agent] = key.split(':') as [string, Agent]
      return { projectId: Number(projectId), agent }
    }),
  )

  function isPaused(agent: Agent, projectId: number | null = projects.selected?.id ?? null): boolean {
    return projectId !== null && paused.value.has(projectAgentKey(projectId, agent))
  }

  async function setPaused(agent: Agent, value: boolean): Promise<void> {
    const projectId = projects.selected?.id
    if (projectId === undefined) {
      throw new Error(NO_PROJECT_MESSAGE)
    }
    await setAgentPaused(projectId, agent, value)
    const next = new Set(paused.value)
    const key = projectAgentKey(projectId, agent)
    if (value) {
      next.add(key)
    } else {
      next.delete(key)
    }
    paused.value = next
  }

  function applyPauses(pauses: readonly AgentPause[]): void {
    paused.value = new Set(pauses.map((pause) => projectAgentKey(pause.projectId, pause.agent)))
  }

  async function load(): Promise<void> {
    applyPauses(await listAgentPauses())
  }

  return {
    paused,
    pausedList,
    isPaused,
    applyPauses,
    pause: (agent: Agent) => setPaused(agent, true),
    resume: (agent: Agent) => setPaused(agent, false),
    load,
  }
})
