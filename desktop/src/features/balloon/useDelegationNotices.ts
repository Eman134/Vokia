import { watch } from 'vue'
import { defineStore } from 'pinia'
import { agentName } from '../team/agents'
import type { Agent, Speaker, VokiaMessage } from '../team/events'
import { isBetweenAgents } from '../team/timeline'
import { useFeed } from '../team/useFeed'
import { useBalloons } from './useBalloons'

const DELEGATED_TITLE = 'Task delegada'
const ARROW = ' → '
const REQUEST_PREVIEW_CHARS = 90

function isAgent(speaker: Speaker): speaker is Agent {
  return speaker !== 'user'
}

function preview(text: string): string {
  const line = text.trim().split('\n')[0] ?? ''
  return line.length > REQUEST_PREVIEW_CHARS ? `${line.slice(0, REQUEST_PREVIEW_CHARS)}…` : line
}

// Só o encaminhamento vira aviso: a volta do resultado ao agente que delegou não é uma nova task.
export const useDelegationNotices = defineStore('delegationNotices', () => {
  const feed = useFeed()
  const balloons = useBalloons()
  const announced = new WeakSet<object>()
  const delegatorOf = new Map<Agent, Agent>()

  function isReturn(message: VokiaMessage & { from: Agent; to: Agent }): boolean {
    return delegatorOf.get(message.from) === message.to
  }

  watch(
    () => feed.feed.length,
    () => {
      for (const entry of feed.feed) {
        if (entry.type !== 'message' || announced.has(entry)) continue
        announced.add(entry)
        if (entry.isError || !isBetweenAgents(entry) || !isAgent(entry.from) || !isAgent(entry.to)) continue
        const message = { ...entry, from: entry.from, to: entry.to }
        if (message.from === message.to) continue
        if (isReturn(message)) {
          delegatorOf.delete(message.from)
          continue
        }
        delegatorOf.set(message.to, message.from)
        balloons.alert(DELEGATED_TITLE, `${agentName(message.from)}${ARROW}${agentName(message.to)}: ${preview(message.text)}`)
      }
    },
  )
})
