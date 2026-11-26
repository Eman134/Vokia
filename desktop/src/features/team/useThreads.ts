import { computed } from 'vue'
import { defineStore } from 'pinia'
import { useBacklog } from '../backlog/useBacklog'
import type { BacklogColumn } from '../backlog/backlogTypes'
import { useDecisions } from '../decisions/useDecisions'
import { useProjects } from '../projects/useProjects'
import type { Agent } from './events'
import { showsAsThread, threadStatus, type ThreadStatus } from './threadStatus'
import { useFeed } from './useFeed'

export interface TaskThread {
  itemId: string
  publicId?: string
  title: string
  column: BacklogColumn
  assignee: Agent | null
  participants: Agent[]
  lastActivity: number
  messageCount: number
  isTriaging?: boolean
  status: ThreadStatus
}

export const useThreads = defineStore('threads', () => {
  const backlog = useBacklog()
  const feed = useFeed()

  const decisions = useDecisions()
  const projects = useProjects()

  const threads = computed<TaskThread[]>(() => {
    const byItem = new Map<string, { participants: Set<Agent>; lastActivity: number; messageCount: number }>()
    const ensureBucket = (itemId: string) => {
      let bucket = byItem.get(itemId)
      if (!bucket) {
        bucket = { participants: new Set<Agent>(), lastActivity: 0, messageCount: 0 }
        byItem.set(itemId, bucket)
      }
      return bucket
    }

    for (const entry of feed.feed) {
      if ((entry.type !== 'message' && entry.type !== 'task-result') || !entry.itemId) continue
      const bucket = ensureBucket(entry.itemId)
      bucket.messageCount += 1
      if (entry.time > bucket.lastActivity) bucket.lastActivity = entry.time
      if (entry.type === 'message') {
        if (entry.from !== 'user') bucket.participants.add(entry.from)
        if (entry.to !== 'user') bucket.participants.add(entry.to)
      } else {
        for (const agent of entry.agents) bucket.participants.add(agent)
      }
    }

    for (const [agent, itemId] of Object.entries(feed.stateItem)) {
      if (!itemId || feed.stateOf(agent) === 'idle') continue
      ensureBucket(itemId).participants.add(agent)
    }

    const waitingOnYou = new Set(feed.permissions.map((permission) => permission.agent))
    const projectId = projects.selected?.id

    return backlog.items
      .flatMap((item): TaskThread[] => {
        const column = item.column ?? 'Backlog'
        const bucket = byItem.get(item.id)
        const isTriaging = backlog.triagingIds.has(item.id)
        if (!showsAsThread({ column, isTriaging, messageCount: bucket?.messageCount ?? 0 })) return []
        const participants = [...(bucket?.participants ?? [])]
        const working = feed.activeAgentsForItems(new Set([item.id]))
        const status = threadStatus({
          column,
          isTriaging,
          hasOpenQuestion: decisions.pending.some((decision) => decision.itemId === item.id && decision.projectId === projectId),
          hasPendingPermission: [...working].some((agent) => waitingOnYou.has(agent)),
          hasWorkingAgent: working.size > 0,
        })
        return [
          {
            itemId: item.id,
            publicId: item.publicId,
            title: item.title,
            column,
            assignee: participants.at(-1) ?? null,
            participants,
            lastActivity: bucket?.lastActivity ?? 0,
            messageCount: bucket?.messageCount ?? 0,
            isTriaging,
            status,
          },
        ]
      })
      .sort((a, b) => b.lastActivity - a.lastActivity)
  })

  function find(itemId: string): TaskThread | undefined {
    return threads.value.find((thread) => thread.itemId === itemId)
  }

  return { threads, find }
})
