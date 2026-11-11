import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { useTeam } from '../team/useTeam'
import {
  fetchBacklog,
  moveBacklogItem,
  saveBacklogItem,
  saveItemPlan,
  type ItemPlanEntry,
  type ItemPlanInput,
} from './backlogApi'
import { DEFAULT_COLUMN, MOVABLE_COLUMNS, type BacklogColumn, type BacklogItem, type TriageQuestion } from './backlogTypes'
import { normalizeTag } from './tags'

interface ItemQueue {
  tail: Promise<void>
  generation: number
}

// moveTo recusa In progress e In review (MOVABLE_COLUMNS); só a execução move o card para elas.
const RUN_COLUMNS = ['Ready', 'In progress', 'In review'] as const
export type RunColumn = (typeof RUN_COLUMNS)[number]

export const useBacklog = defineStore('backlog', () => {
  const items = ref<BacklogItem[]>([])
  const planEntries = ref<ItemPlanEntry[]>([])
  const openId = ref<string | null>(null)
  const loadedProjectId = ref<number | null>(null)
  const loading = ref(false)
  const loadError = ref<string | null>(null)
  const team = useTeam()
  const queues = new Map<string, ItemQueue>()
  let loadSeq = 0

  const openItem = computed(() => items.value.find((item) => item.id === openId.value) ?? null)

  function find(id: string): BacklogItem | undefined {
    return items.value.find((item) => item.id === id)
  }

  // Item com erro de leitura é mostrado, mas não abre nem muda: o banco guarda dado que o app não consegue ler.
  function editable(id: string): BacklogItem | undefined {
    const item = find(id)
    return item?.error ? undefined : item
  }

  function open(id: string): void {
    if (editable(id)) openId.value = id
  }

  function close(): void {
    openId.value = null
  }

  function reportError(error: unknown): void {
    team.feed.push({ type: 'error', message: error instanceof Error ? error.message : String(error) })
  }

  function clearLoaded(): void {
    items.value = []
    planEntries.value = []
    loadedProjectId.value = null
    openId.value = null
  }

  function clear(): void {
    loadSeq += 1
    clearLoaded()
    loading.value = false
    loadError.value = null
  }

  // Resposta de uma carga antiga (outro projeto ou carga mais nova) é descartada.
  async function load(projectId: number): Promise<boolean> {
    const seq = (loadSeq += 1)
    clearLoaded()
    loadError.value = null
    loading.value = true
    return applySnapshot(projectId, seq)
  }

  async function applySnapshot(projectId: number, seq: number): Promise<boolean> {
    try {
      const snapshot = await fetchBacklog(projectId)
      if (seq !== loadSeq) return false
      items.value = snapshot.items
      planEntries.value = snapshot.plans
      loadedProjectId.value = projectId
      loadError.value = null
      return true
    } catch (error) {
      if (seq === loadSeq) {
        loadError.value = error instanceof Error ? error.message : String(error)
        reportError(error)
      }
      return false
    } finally {
      if (seq === loadSeq) loading.value = false
    }
  }

  // Sem zerar a lista: planos em geração de outros itens continuam, porque o plan store preserva os que aguardam resposta.
  async function reloadAfterFailure(projectId: number): Promise<void> {
    if (loadedProjectId.value !== projectId) return
    await applySnapshot(projectId, (loadSeq += 1))
  }

  // Gravações do mesmo item rodam em ordem. Uma falha descarta as que ainda não foram enviadas (geração antiga)
  // e recarrega do banco; o conteúdo é lido no envio, então nada reaplica a mudança que falhou.
  function enqueue(itemId: string, write: (projectId: number) => Promise<void>): void {
    const projectId = loadedProjectId.value
    if (projectId === null) return
    const queue = queues.get(itemId) ?? { tail: Promise.resolve(), generation: 0 }
    queues.set(itemId, queue)
    const generation = queue.generation
    const run = queue.tail.then(async () => {
      if (queue.generation !== generation) return
      try {
        await write(projectId)
      } catch (error) {
        queue.generation += 1
        reportError(error)
        await reloadAfterFailure(projectId)
      }
    })
    queue.tail = run
    void run.then(() => {
      if (queue.tail === run) queues.delete(itemId)
    })
  }

  function saveItem(itemId: string): void {
    enqueue(itemId, async (projectId) => {
      const item = editable(itemId)
      if (item) await saveBacklogItem(projectId, JSON.parse(JSON.stringify(item)))
    })
  }

  function savePlan(itemId: string, plan: ItemPlanInput): void {
    enqueue(itemId, (projectId) => saveItemPlan(projectId, itemId, plan))
  }

  function moveTo(id: string, column: BacklogColumn): void {
    const item = editable(id)
    if (!item || !MOVABLE_COLUMNS.includes(column) || column === (item.column ?? DEFAULT_COLUMN)) return
    item.column = column
    enqueue(id, (projectId) => moveBacklogItem(projectId, id, column))
  }

  function addActivity(id: string, text: string): void {
    const item = editable(id)
    if (!item) return
    item.activity = [
      ...(item.activity ?? []),
      { id: crypto.randomUUID(), text, at: new Date().toLocaleString('pt-BR') },
    ]
    saveItem(id)
  }

  function setRunColumn(id: string, column: RunColumn, note: string): void {
    const item = editable(id)
    if (!item) return
    item.column = column
    addActivity(id, note)
  }

  function addTags(id: string, raws: string[]): void {
    const item = editable(id)
    if (!item) return
    const fresh = [...new Set(raws.map(normalizeTag).filter((tag): tag is string => tag !== null && !item.tags.includes(tag)))]
    if (fresh.length === 0) return
    item.tags = [...item.tags, ...fresh]
    saveItem(id)
  }

  function removeTag(id: string, tag: string): void {
    const item = editable(id)
    if (!item || !item.tags.includes(tag)) return
    item.tags = item.tags.filter((current) => current !== tag)
    saveItem(id)
  }

  function answer(itemId: string, questionId: string, option: string): void {
    const question = editable(itemId)?.questions?.find((current) => current.id === questionId)
    if (!question) return
    question.selected = option
    saveItem(itemId)
  }

  function addQuestion(itemId: string, question: TriageQuestion): void {
    const item = editable(itemId)
    if (!item) return
    item.questions = [...(item.questions ?? []), question]
    saveItem(itemId)
  }

  function clearAnswer(itemId: string, questionId: string): void {
    const question = editable(itemId)?.questions?.find((current) => current.id === questionId)
    if (!question || question.selected === undefined) return
    delete question.selected
    saveItem(itemId)
  }

  function removeQuestions(itemId: string, ids: string[]): void {
    const item = editable(itemId)
    if (!item?.questions || ids.length === 0) return
    item.questions = item.questions.filter((question) => !ids.includes(question.id))
    saveItem(itemId)
  }

  function toggleCriterion(itemId: string, criterionId: string): void {
    const criterion = editable(itemId)?.criteria?.find((current) => current.id === criterionId)
    if (!criterion) return
    criterion.done = !criterion.done
    saveItem(itemId)
  }

  return {
    items,
    planEntries,
    loadedProjectId,
    openId,
    openItem,
    loading,
    loadError,
    open,
    close,
    clear,
    load,
    moveTo,
    addTags,
    removeTag,
    addActivity,
    setRunColumn,
    answer,
    addQuestion,
    clearAnswer,
    removeQuestions,
    toggleCriterion,
    savePlan,
  }
})
