import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { useProjects } from '../projects/useProjects'
import {
  createArtifact,
  deleteArtifact,
  fetchArtifact,
  fetchArtifactKinds,
  fetchArtifacts,
  isArtifactConflict,
  PLAN_KIND,
  setArtifactCheck,
  updateArtifact,
  type ArtifactDocument,
  type ArtifactKindInfo,
  type ArtifactPatch,
  type ArtifactSummary,
  type NewArtifactInput,
} from './artifactApi'
import { checklistProgress, type ChecklistProgress } from './artifactChecks'

const CHECKLIST_KIND = 'checklist'

interface CachedProgress extends ChecklistProgress {
  updatedAt: number
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export const useArtifacts = defineStore('artifacts', () => {
  const kinds = ref<ArtifactKindInfo[]>([])
  const documents = ref<ArtifactSummary[]>([])
  const current = ref<ArtifactDocument | null>(null)
  const progress = ref<Record<string, CachedProgress>>({})
  const loadedProjectId = ref<number | null>(null)
  const loading = ref(false)
  const opening = ref(false)
  const saving = ref(false)
  const error = ref<string | null>(null)
  const conflict = ref(false)
  const projects = useProjects()
  // loadSeq muda a cada troca de projeto; refreshSeq ordena as releituras da lista dentro do mesmo projeto.
  let loadSeq = 0
  let refreshSeq = 0

  function clear(): void {
    loadSeq += 1
    documents.value = []
    current.value = null
    progress.value = {}
    loadedProjectId.value = null
    loading.value = false
    opening.value = false
    clearFeedback()
  }

  function clearFeedback(): void {
    error.value = null
    conflict.value = false
  }

  function fail(cause: unknown): void {
    const message = messageOf(cause)
    error.value = message
    conflict.value = isArtifactConflict(message)
  }

  // Os planos continuam vindo do backlog; aqui ficam só os documentos livres.
  async function readDocuments(projectId: number): Promise<ArtifactSummary[]> {
    const summaries = await fetchArtifacts(projectId)
    return summaries.filter((summary) => summary.kind !== PLAN_KIND)
  }

  // O resumo não traz o corpo; o progresso do checklist vem da leitura do documento, refeita só quando ele mudou.
  async function loadProgress(projectId: number, seq: number): Promise<void> {
    const stale = documents.value.filter(
      (summary) => summary.kind === CHECKLIST_KIND && progress.value[summary.id]?.updatedAt !== summary.updatedAt,
    )
    const loaded = await Promise.all(
      stale.map(async (summary) => {
        try {
          const document = await fetchArtifact(projectId, summary.id)
          return { id: summary.id, value: { ...checklistProgress(document.body), updatedAt: document.updatedAt } }
        } catch {
          return null
        }
      }),
    )
    if (seq !== loadSeq) return
    const next = { ...progress.value }
    for (const entry of loaded) if (entry) next[entry.id] = entry.value
    progress.value = next
  }

  async function load(projectId: number): Promise<void> {
    clear()
    const seq = loadSeq
    loading.value = true
    try {
      if (kinds.value.length === 0) kinds.value = await fetchArtifactKinds()
      const list = await readDocuments(projectId)
      if (seq !== loadSeq) return
      documents.value = list
      loadedProjectId.value = projectId
      await loadProgress(projectId, seq)
    } catch (cause) {
      if (seq === loadSeq) error.value = messageOf(cause)
    } finally {
      if (seq === loadSeq) loading.value = false
    }
  }

  async function refreshList(projectId: number): Promise<void> {
    const generation = loadSeq
    const order = (refreshSeq += 1)
    const list = await readDocuments(projectId)
    if (generation !== loadSeq || order !== refreshSeq || loadedProjectId.value !== projectId) return
    documents.value = list
    await loadProgress(projectId, generation)
  }

  async function refreshCurrent(projectId: number): Promise<void> {
    const open = current.value
    if (!open) return
    if (!documents.value.some((summary) => summary.id === open.id)) {
      current.value = null
      return
    }
    current.value = await fetchArtifact(projectId, open.id)
  }

  // Outro projeto, CLI ou agente gravou: só recarrega quando o evento é do projeto que está carregado.
  async function reloadIfCurrent(projectId: number): Promise<void> {
    if (loadedProjectId.value !== projectId) return
    try {
      await refreshList(projectId)
      await refreshCurrent(projectId)
    } catch (cause) {
      error.value = messageOf(cause)
    }
  }

  async function openDocument(id: string): Promise<void> {
    const projectId = loadedProjectId.value
    if (projectId === null) return
    opening.value = true
    current.value = null
    try {
      const document = await fetchArtifact(projectId, id)
      if (loadedProjectId.value === projectId) current.value = document
    } catch (cause) {
      error.value = messageOf(cause)
    } finally {
      opening.value = false
    }
  }

  function closeDocument(): void {
    current.value = null
  }

  async function reloadCurrent(): Promise<ArtifactDocument | null> {
    const projectId = loadedProjectId.value
    const open = current.value
    if (projectId === null || !open) return null
    clearFeedback()
    try {
      const document = await fetchArtifact(projectId, open.id)
      current.value = document
      await refreshList(projectId)
      return document
    } catch (cause) {
      fail(cause)
      return null
    }
  }

  // Uma gravação por vez: o duplo clique em "Criar" não gera dois artefatos.
  async function write<T>(action: (projectId: number) => Promise<T>): Promise<T | null> {
    const projectId = loadedProjectId.value
    if (projectId === null || saving.value) return null
    saving.value = true
    clearFeedback()
    try {
      const result = await action(projectId)
      // A gravação já valeu: devolver null aqui levaria a um novo clique em "Criar" e a um artefato duplicado.
      await refreshList(projectId).catch(fail)
      return result
    } catch (cause) {
      fail(cause)
      return null
    } finally {
      saving.value = false
    }
  }

  async function create(input: NewArtifactInput): Promise<ArtifactDocument | null> {
    return write((projectId) => createArtifact(projectId, input))
  }

  async function save(id: string, patch: ArtifactPatch): Promise<ArtifactDocument | null> {
    const document = await write((projectId) => updateArtifact(projectId, id, patch))
    if (document && current.value?.id === id) current.value = document
    return document
  }

  async function remove(id: string): Promise<boolean> {
    const removed = await write(async (projectId) => {
      await deleteArtifact(projectId, id)
      return true
    })
    if (removed && current.value?.id === id) current.value = null
    return removed === true
  }

  async function toggleCheck(id: string, index: number, done: boolean): Promise<void> {
    const document = await write((projectId) => setArtifactCheck(projectId, id, index, done))
    if (document && current.value?.id === id) current.value = document
  }

  watch(
    () => projects.selected?.id,
    (projectId) => {
      if (projectId === undefined) {
        clear()
        return
      }
      void load(projectId)
    },
    { immediate: true },
  )

  return {
    kinds,
    documents,
    current,
    progress,
    loading,
    opening,
    saving,
    error,
    conflict,
    loadedProjectId,
    load,
    reloadIfCurrent,
    reloadCurrent,
    openDocument,
    closeDocument,
    create,
    save,
    remove,
    toggleCheck,
    clearFeedback,
  }
})
