import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { VOKIA_EVENT, type VokiaEvent } from '../team/events'
import { useBalloons } from '../balloon/useBalloons'
import { useNavigation } from '../../shell/useNavigation'
import {
  applyLearningSuggestion,
  dismissLearningSuggestion,
  listLearningSuggestions,
  listProjectMemories,
  runMemoryReview,
  type LearningSuggestion,
  type ProjectMemory,
  type ReviewOutcome,
  type SkillScope,
} from './learningApi'

export const useLearningSuggestions = defineStore('learningSuggestions', () => {
  const suggestions = ref<LearningSuggestion[]>([])
  const memories = ref<ProjectMemory[]>([])
  const loading = ref(false)
  const reviewing = ref(false)
  const error = ref<string | null>(null)
  const busyIds = ref<Set<string>>(new Set())
  const activeProjectId = ref<number | undefined>(undefined)
  let unlisten: UnlistenFn | null = null

  const pendingSuggestions = computed(() =>
    suggestions.value.filter((item) => item.status === 'pending'),
  )
  const pendingCount = computed(() => pendingSuggestions.value.length)
  const pendingMemoryCount = computed(
    () => pendingSuggestions.value.filter((item) => item.kind === 'memory').length,
  )
  const pendingSkillCount = computed(
    () => pendingSuggestions.value.filter((item) => item.kind === 'skill').length,
  )

  async function load(projectId?: number): Promise<void> {
    activeProjectId.value = projectId
    loading.value = true
    error.value = null
    try {
      const [sugList, memList] = await Promise.all([
        listLearningSuggestions(projectId, 'pending'),
        projectId !== undefined ? listProjectMemories(projectId) : Promise.resolve([]),
      ])
      suggestions.value = sugList
      memories.value = memList
    } catch (err) {
      error.value = `Não foi possível carregar sugestões de aprendizado: ${String(err)}`
    } finally {
      loading.value = false
    }
  }

  async function apply(id: string, scope?: SkillScope): Promise<boolean> {
    const nextBusy = new Set(busyIds.value)
    nextBusy.add(id)
    busyIds.value = nextBusy
    error.value = null
    try {
      await applyLearningSuggestion(id, scope)
      suggestions.value = suggestions.value.filter((item) => item.id !== id)
      if (activeProjectId.value !== undefined) {
        memories.value = await listProjectMemories(activeProjectId.value)
      }
      return true
    } catch (err) {
      error.value = `Não foi possível aplicar a sugestão: ${String(err)}`
      return false
    } finally {
      const doneBusy = new Set(busyIds.value)
      doneBusy.delete(id)
      busyIds.value = doneBusy
    }
  }

  async function dismiss(id: string): Promise<boolean> {
    const nextBusy = new Set(busyIds.value)
    nextBusy.add(id)
    busyIds.value = nextBusy
    error.value = null
    try {
      await dismissLearningSuggestion(id)
      suggestions.value = suggestions.value.filter((item) => item.id !== id)
      return true
    } catch (err) {
      error.value = `Não foi possível descartar a sugestão: ${String(err)}`
      return false
    } finally {
      const doneBusy = new Set(busyIds.value)
      doneBusy.delete(id)
      busyIds.value = doneBusy
    }
  }

  async function triggerReview(projectId: number): Promise<ReviewOutcome | null> {
    reviewing.value = true
    error.value = null
    try {
      const outcome = await runMemoryReview(projectId)
      await load(projectId)
      return outcome
    } catch (err) {
      error.value = `Não foi possível executar a revisão: ${String(err)}`
      return null
    } finally {
      reviewing.value = false
    }
  }

  async function bindEvents(): Promise<void> {
    if (unlisten !== null) return
    unlisten = await listen<VokiaEvent>(VOKIA_EVENT, (event) => {
      const payload = event.payload
      if (payload.type !== 'learning-suggestions-changed') return
      void load(activeProjectId.value ?? payload.projectId)
      if (payload.newCount > 0) {
        const balloons = useBalloons()
        const navigation = useNavigation()
        balloons.enqueue({
          id: `learning-suggestions:${payload.projectId}:${Date.now()}`,
          kind: 'moment',
          source: 'conversa',
          title:
            payload.newCount === 1
              ? 'Nova sugestão de aprendizado'
              : `${payload.newCount} novas sugestões de aprendizado`,
          text: 'Revise e aprove memórias ou skills sugeridas em 1 clique.',
          actions: [
            {
              id: 'open-suggestions',
              label: 'Ver sugestões',
              primary: true,
              run: () => {
                navigation.goTo('memoria')
              },
            },
            { id: 'dismiss', label: 'Depois' },
          ],
        })
      }
    })
  }

  return {
    suggestions,
    memories,
    loading,
    reviewing,
    error,
    busyIds,
    pendingSuggestions,
    pendingCount,
    pendingMemoryCount,
    pendingSkillCount,
    load,
    apply,
    dismiss,
    triggerReview,
    bindEvents,
  }
})
