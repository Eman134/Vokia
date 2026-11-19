import { ref } from 'vue'
import { defineStore } from 'pinia'
import { fetchOrchestration } from '../orchestration/orchestrationApi'
import { useProjects } from '../projects/useProjects'
import { useFeed } from '../team/useFeed'
import { useRequests } from '../team/useRequests'
import { useRoster } from '../team/useRoster'
import { isUnassigned, type BacklogItem } from './backlogTypes'
import { buildTriagePrompt } from './triagePrompt'

export const useTriage = defineStore('triage', () => {
  const projects = useProjects()
  const requests = useRequests()
  const roster = useRoster()
  const feed = useFeed()
  // Itens que pediram triagem sem líder configurado; só na sessão, para a tela dizer por que seguem sem responsável.
  const withoutLeader = ref<string[]>([])

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
      await requests.send(buildTriagePrompt(item, snapshot, roster.agents), leader.id, { projectId: project.id })
    } catch (error) {
      feed.feed.push({ type: 'error', message: error instanceof Error ? error.message : String(error) })
    }
  }

  return { isWithoutLeader, requestTriage }
})
