import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  VOKIA_EVENT,
  type Agent,
  type AgentState,
  type CliProblem,
  type VokiaError,
  type VokiaEvent,
  type VokiaMessage,
} from '../types/events'

export interface AgentProfile {
  id: Agent
  name: string
  role: string
}

export type FeedEntry = VokiaMessage | VokiaError

export const AGENTS: readonly AgentProfile[] = [
  { id: 'iara', name: 'Iara', role: 'Tech lead' },
  { id: 'davi', name: 'Davi', role: 'Dev frontend' },
]

export const useTeam = defineStore('team', () => {
  const cliChecked = ref(false)
  const cliProblem = ref<CliProblem | null>(null)
  const feed = ref<FeedEntry[]>([])
  const states = ref<Record<Agent, AgentState>>({ iara: 'idle', davi: 'idle' })
  const busy = ref(false)
  let unlisten: UnlistenFn | null = null

  const thinkingAgent = computed<AgentProfile | undefined>(() =>
    AGENTS.find((agent) => states.value[agent.id] === 'thinking'),
  )
  const statusText = computed<string>(() =>
    thinkingAgent.value ? `${thinkingAgent.value.name} pensando` : 'Tudo em dia',
  )

  async function start(): Promise<void> {
    cliProblem.value = await invoke<CliProblem | null>('cli_problem')
    if (cliProblem.value === null) {
      unlisten = await listen<VokiaEvent>(VOKIA_EVENT, (event) => apply(event.payload))
    }
    cliChecked.value = true
  }

  function stop(): void {
    unlisten?.()
    unlisten = null
  }

  function apply(event: VokiaEvent): void {
    switch (event.type) {
      case 'message':
      case 'error':
        feed.value.push(event)
        break
      case 'agent-state':
        states.value[event.agent] = event.state
        break
    }
  }

  async function send(text: string): Promise<void> {
    busy.value = true
    try {
      await invoke<void>('send_request', { text })
    } catch (error) {
      feed.value.push({ type: 'error', message: String(error) })
    } finally {
      busy.value = false
    }
  }

  return { cliChecked, cliProblem, feed, states, busy, thinkingAgent, statusText, start, stop, send }
})
