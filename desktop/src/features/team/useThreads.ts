import { computed } from 'vue'
import { defineStore } from 'pinia'
import { useBacklog } from '../backlog/useBacklog'
import type { BacklogColumn } from '../backlog/backlogTypes'
import type { Agent } from './events'
import { useFeed } from './useFeed'

export interface TaskThread {
  itemId: string
  title: string
  column: BacklogColumn
  assignee: Agent | null
  participants: Agent[]
  lastActivity: number
  messageCount: number
  isTriaging?: boolean
}

const ACTIVE_THREAD_COLUMNS: ReadonlySet<BacklogColumn> = new Set(['In progress', 'In review'])

export const useThreads = defineStore('threads', () => {
  const backlog = useBacklog()
  const feed = useFeed()

  const threads = computed<TaskThread[]>(() => {
    const activeItems = backlog.items.filter(
      (item) => (item.column !== undefined && ACTIVE_THREAD_COLUMNS.has(item.column)) || item.triageStep !== undefined,
    )
    if (activeItems.length === 0) return []

    const byItem = new Map<string, { participants: Set<Agent>; lastActivity: number; messageCount: number }>()
    for (const entry of feed.feed) {
      if ((entry.type !== 'message' && entry.type !== 'task-result') || !entry.itemId) continue
      let bucket = byItem.get(entry.itemId)
      if (!bucket) {
        bucket = { participants: new Set<Agent>(), lastActivity: 0, messageCount: 0 }
        byItem.set(entry.itemId, bucket)
      }
      bucket.messageCount += 1
      if (entry.time > bucket.lastActivity) bucket.lastActivity = entry.time
      if (entry.type === 'message') {
        if (entry.from !== 'user') bucket.participants.add(entry.from)
        if (entry.to !== 'user') bucket.participants.add(entry.to)
      } else {
        for (const agent of entry.agents) bucket.participants.add(agent)
      }
    }

    return activeItems
      .map((item) => {
        const bucket = byItem.get(item.id)
        const participants = [...(bucket?.participants ?? [])]
        return {
          itemId: item.id,
          title: item.title,
          column: item.column ?? 'Backlog',
          assignee: participants.at(-1) ?? null,
          participants,
          lastActivity: bucket?.lastActivity ?? 0,
          messageCount: bucket?.messageCount ?? 0,
          isTriaging: item.triageStep !== undefined,
        }
      })
      .sort((a, b) => b.lastActivity - a.lastActivity)
  })

  function find(itemId: string): TaskThread | undefined {
    return threads.value.find((thread) => thread.itemId === itemId)
  }

  return { threads, find }
})
