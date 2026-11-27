import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { listRoles } from '../agents/rolesApi'
import { fetchOrchestration } from '../orchestration/orchestrationApi'
import { useProjects } from '../projects/useProjects'
import { useFeed } from '../team/useFeed'
import { useRequests } from '../team/useRequests'
import { useRoster } from '../team/useRoster'
import { isUnassigned, type BacklogItem } from './backlogTypes'
import { buildTriagePrompt } from './triagePrompt'
import { useItemPlan } from './useItemPlan'

export const useTriage = defineStore('triage', () => {
  const projects = useProjects()
  const requests = useRequests()
  const roster = useRoster()
  const feed = useFeed()
  // Itens que pediram triagem sem líder configurado; só na sessão, para a tela dizer por que seguem sem responsável.
  const withoutLeader = ref<string[]>([])
  const selectedTriageId = ref<string | null>(null)
  const queue = ref<string[]>([])
  const activeIds = ref<string[]>([])
  const activeId = computed(() => activeIds.value[0] ?? null)
  const TRIAGE_CONCURRENCY = 3
  let activeWorkers = 0

  const isBatchActive = computed(() => activeIds.value.length > 0 || queue.value.length > 0)

  function isQueued(itemId: string): boolean {
    return queue.value.includes(itemId)
  }

  function isRunning(itemId: string): boolean {
    return activeIds.value.includes(itemId)
  }

  function clearBatch(): void {
    queue.value = []
    activeIds.value = []
  }

  watch(
    () => projects.selected?.id,
    () => {
      clearBatch()
    },
  )

  async function runTriageWorker(batchProjectId: number | undefined, itemPlan: ReturnType<typeof useItemPlan>): Promise<void> {
    activeWorkers += 1
    try {
      while (queue.value.length > 0 && projects.selected?.id === batchProjectId) {
        const nextId = queue.value[0]
        queue.value = queue.value.slice(1)
        activeIds.value = [...activeIds.value, nextId]
        try {
          await itemPlan.requestAndWait(nextId)
        } catch (error) {
          feed.feed.push({ type: 'error', message: error instanceof Error ? error.message : String(error) })
        } finally {
          activeIds.value = activeIds.value.filter((id) => id !== nextId)
        }
      }
    } finally {
      activeWorkers -= 1
    }
  }

  function drainBatch(): void {
    const batchProjectId = projects.selected?.id
    const itemPlan = useItemPlan()
    const slots = Math.min(TRIAGE_CONCURRENCY, queue.value.length)
    while (activeWorkers < slots) {
      void runTriageWorker(batchProjectId, itemPlan)
    }
  }

  function enqueueBatch(itemIds: readonly string[]): void {
    const fresh = itemIds.filter((id) => !activeIds.value.includes(id) && !queue.value.includes(id))
    if (fresh.length === 0) return
    queue.value = [...queue.value, ...fresh]
    drainBatch()
  }

  function setWithoutLeader(itemId: string, missing: boolean): void {
    const others = withoutLeader.value.filter((id) => id !== itemId)
    withoutLeader.value = missing ? [...others, itemId] : others
  }

  function isWithoutLeader(itemId: string): boolean {
    return withoutLeader.value.includes(itemId)
  }

  // Só o líder recebe o pedido: a ferramenta backlog_set_responsible grava o responsável, não o líder direto.
  async function requestTriage(item: BacklogItem): Promise<void> {
    const project = projects.selected
    if (project === null || !isUnassigned(item)) return
    try {
      const snapshot = await fetchOrchestration()
      const leader = roster.profileOf(snapshot.settings.leaderAgent)
      setWithoutLeader(item.id, leader === undefined)
      if (leader === undefined) return
      const roles = await listRoles()
      await requests.send(buildTriagePrompt(item, snapshot, roster.agents, roles), leader.id, { projectId: project.id })
    } catch (error) {
      feed.feed.push({ type: 'error', message: error instanceof Error ? error.message : String(error) })
    }
  }

  return {
    isWithoutLeader,
    requestTriage,
    selectedTriageId,
    queue,
    activeId,
    isBatchActive,
    isQueued,
    isRunning,
    enqueueBatch,
    clearBatch,
  }
})
