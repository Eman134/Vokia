import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from '../team/events'

export const useAgentControl = defineStore('agentControl', () => {
  const paused = ref<Set<Agent>>(new Set())

  function pause(agent: Agent): void {
    paused.value.add(agent)
  }

  function resume(agent: Agent): void {
    paused.value.delete(agent)
  }

  function isPaused(agent: Agent): boolean {
    return paused.value.has(agent)
  }

  return { paused, pause, resume, isPaused }
})
