import { ref } from 'vue'
import {
  discardAttachment,
  pickAttachmentFiles,
  saveAttachmentBytes,
  saveAttachmentFiles,
  type Attachment,
} from './attachmentsApi'
import { useFeed } from '../team/useFeed'
import { NO_PROJECT_NOTICE } from '../team/projectResolution'
import { useProjects } from '../projects/useProjects'

export const ATTACHMENTS_PER_MESSAGE = 10

export interface AttachmentBatch {
  ids: string[]
  projectId: number | null
  attachments: Attachment[]
}

export function useComposerAttachments() {
  const feed = useFeed()
  const projects = useProjects()
  const attachments = ref<Attachment[]>([])
  const projectId = ref<number | null>(null)
  const saving = ref(false)

  // Devolve o projeto onde guardar, ou null (com aviso) quando o lote não pode entrar.
  function targetProject(incoming: number): number | null {
    const selected = projects.selected
    if (selected === null) {
      feed.notify(NO_PROJECT_NOTICE)
      return null
    }
    if (projectId.value !== null && projectId.value !== selected.id) {
      const owner = projects.projects.find((project) => project.id === projectId.value)
      feed.notify(
        `Os anexos pendentes são de #${owner?.name ?? projectId.value}. Envie ou remova antes de anexar em outro projeto.`,
      )
      return null
    }
    if (attachments.value.length + incoming > ATTACHMENTS_PER_MESSAGE) {
      feed.notify(`Máximo de ${ATTACHMENTS_PER_MESSAGE} anexos por mensagem.`)
      return null
    }
    return selected.id
  }

  function append(project: number, saved: Attachment[]): void {
    if (saved.length === 0) {
      return
    }
    projectId.value = project
    attachments.value = [...attachments.value, ...saved]
  }

  async function guard(incoming: number, save: (project: number, sink: Attachment[]) => Promise<void>): Promise<void> {
    if (incoming === 0 || saving.value) {
      return
    }
    const project = targetProject(incoming)
    if (project === null) {
      return
    }
    saving.value = true
    const saved: Attachment[] = []
    try {
      await save(project, saved)
    } catch (error) {
      feed.feed.push({ type: 'error', message: String(error) })
    } finally {
      append(project, saved)
      saving.value = false
    }
  }

  async function addPaths(paths: string[]): Promise<void> {
    await guard(paths.length, async (project, sink) => {
      sink.push(...(await saveAttachmentFiles(project, paths)))
    })
  }

  async function pickFiles(): Promise<void> {
    try {
      await addPaths(await pickAttachmentFiles())
    } catch (error) {
      feed.feed.push({ type: 'error', message: String(error) })
    }
  }

  async function addFiles(files: File[]): Promise<void> {
    await guard(files.length, async (project, sink) => {
      for (const file of files) {
        sink.push(await saveAttachmentBytes(project, file))
      }
    })
  }

  async function remove(id: string): Promise<void> {
    const owner = projectId.value
    if (owner === null) {
      return
    }
    try {
      await discardAttachment(owner, id)
    } catch (error) {
      feed.feed.push({ type: 'error', message: String(error) })
      return
    }
    attachments.value = attachments.value.filter((attachment) => attachment.id !== id)
    if (attachments.value.length === 0) {
      projectId.value = null
    }
  }

  function take(): AttachmentBatch {
    const batch = {
      ids: attachments.value.map((attachment) => attachment.id),
      projectId: projectId.value,
      attachments: attachments.value,
    }
    attachments.value = []
    projectId.value = null
    return batch
  }

  function restore(batch: AttachmentBatch): void {
    attachments.value = batch.attachments
    projectId.value = batch.projectId
  }

  return { attachments, projectId, saving, pickFiles, addFiles, addPaths, remove, take, restore }
}
