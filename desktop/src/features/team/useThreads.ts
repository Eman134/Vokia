import { computed } from 'vue'
import { defineStore } from 'pinia'
import { useBacklog } from '../backlog/useBacklog'
import type { BacklogColumn } from '../backlog/backlogTypes'
import { useDecisions } from '../decisions/useDecisions'
import { useProjects } from '../projects/useProjects'
import type { Agent } from './events'
import { showsAsThread, threadStatus, type ThreadStatus } from './threadStatus'
import { isBetweenAgents } from './timeline'
import { useFeed } from './useFeed'

export interface TaskThread {
  itemId: string
  publicId?: string
  title: string
  column: BacklogColumn
  parentId?: string
  assignee: Agent | null
  participants: Agent[]
  lastActivity: number
  messageCount: number
  exchangeCount: number
  isTriaging?: boolean
  status: ThreadStatus
  subthreads: TaskThread[]
}

export const useThreads = defineStore('threads', () => {
  const backlog = useBacklog()
  const feed = useFeed()

  const decisions = useDecisions()
  const projects = useProjects()

  const threads = computed<TaskThread[]>(() => {
    const byItem = new Map<string, { participants: Set<Agent>; lastActivity: number; messageCount: number; exchangeCount: number }>()
    const ensureBucket = (itemId: string) => {
      let bucket = byItem.get(itemId)
      if (!bucket) {
        bucket = { participants: new Set<Agent>(), lastActivity: 0, messageCount: 0, exchangeCount: 0 }
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
        if (isBetweenAgents(entry)) bucket.exchangeCount += 1
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

    const projectId = projects.selected?.id
    const waitingOnYou = new Set(
      feed.permissions
        .filter((permission) => permission.projectId === null || permission.projectId === projectId)
        .map((permission) => permission.agent),
    )

    const nodeById = new Map<string, TaskThread>()
    for (const item of backlog.items) {
      const column = item.column ?? 'Backlog'
      const bucket = byItem.get(item.id)
      const isTriaging = backlog.triagingIds.has(item.id)
      const participants = [...(bucket?.participants ?? [])]
      const working = feed.activeAgentsForItems(new Set([item.id]))
      const status = threadStatus({
        column,
        isTriaging,
        hasOpenQuestion: decisions.pending.some((decision) => decision.itemId === item.id && decision.projectId === projectId),
        hasPendingPermission: [...working].some((agent) => waitingOnYou.has(agent)),
        hasWorkingAgent: working.size > 0,
      })
      nodeById.set(item.id, {
        itemId: item.id,
        publicId: item.publicId,
        title: item.title,
        column,
        parentId: item.parent,
        assignee: participants.at(-1) ?? null,
        participants,
        lastActivity: bucket?.lastActivity ?? 0,
        messageCount: bucket?.messageCount ?? 0,
        exchangeCount: bucket?.exchangeCount ?? 0,
        isTriaging,
        status,
        subthreads: [],
      })
    }

    const roots: TaskThread[] = []
    for (const node of nodeById.values()) {
      const parent = node.parentId ? nodeById.get(node.parentId) : undefined
      if (parent) {
        parent.subthreads.push(node)
      } else {
        roots.push(node)
      }
    }

    function rollup(node: TaskThread): { maxActivity: number; hasMessages: boolean } {
      let maxActivity = node.lastActivity
      let hasMessages = node.messageCount > 0
      for (const sub of node.subthreads) {
        const subInfo = rollup(sub)
        if (subInfo.maxActivity > maxActivity) maxActivity = subInfo.maxActivity
        if (subInfo.hasMessages) hasMessages = true
      }
      node.lastActivity = maxActivity
      return { maxActivity, hasMessages }
    }
    const hasMessagesByRoot = new Map<string, boolean>()
    for (const root of roots) {
      hasMessagesByRoot.set(root.itemId, rollup(root).hasMessages)
    }

    return roots
      .filter((root) =>
        showsAsThread({
          column: root.column,
          isTriaging: Boolean(root.isTriaging),
          messageCount: root.messageCount,
          lastActivity: root.lastActivity,
          hasSubthreadMessages: hasMessagesByRoot.get(root.itemId) ?? false,
        }),
      )
      .sort((a, b) => b.lastActivity - a.lastActivity)
  })

  function findInTree(list: readonly TaskThread[], itemId: string): TaskThread | undefined {
    for (const thread of list) {
      if (thread.itemId === itemId) return thread
      const nested = findInTree(thread.subthreads, itemId)
      if (nested) return nested
    }
    return undefined
  }

  function find(itemId: string): TaskThread | undefined {
    return findInTree(threads.value, itemId)
  }

  return { threads, find }
})
