import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { formatSize, BYTES_PER_MB } from './attachmentBlock'

const PICK_ATTACHMENT_FILES_COMMAND = 'pick_attachment_files'
const SAVE_ATTACHMENT_FILES_COMMAND = 'save_attachment_files'
const SAVE_ATTACHMENT_BYTES_COMMAND = 'save_attachment_bytes'
const DISCARD_ATTACHMENT_COMMAND = 'discard_attachment'
const PROJECT_HEADER = 'x-vokia-project'
const NAME_HEADER = 'x-vokia-name'
const PASTED_IMAGE_NAME = 'imagem-colada.png'
const DROP_EVENT_TYPE = 'drop'
// Igual a ATTACHMENT_MAX_BYTES do Rust: recusar antes de ler evita carregar na memória um arquivo que o backend rejeitaria.
export const ATTACHMENT_MAX_BYTES = 20_971_520

export type AttachmentKind = 'image' | 'pdf' | 'file'

export interface Attachment {
  id: string
  name: string
  kind: AttachmentKind
  size: number
  path: string
}

export function pickAttachmentFiles(): Promise<string[]> {
  return invoke<string[]>(PICK_ATTACHMENT_FILES_COMMAND)
}

export function saveAttachmentFiles(projectId: number, paths: string[]): Promise<Attachment[]> {
  return invoke<Attachment[]>(SAVE_ATTACHMENT_FILES_COMMAND, { projectId, paths })
}

export async function saveAttachmentBytes(projectId: number, file: File): Promise<Attachment> {
  const name = file.name === '' ? PASTED_IMAGE_NAME : file.name
  if (file.size > ATTACHMENT_MAX_BYTES) {
    return Promise.reject(
      `O arquivo "${name}" tem ${formatSize(file.size)} e passa do limite de ${ATTACHMENT_MAX_BYTES / BYTES_PER_MB} MB por anexo.`,
    )
  }
  return invoke<Attachment>(SAVE_ATTACHMENT_BYTES_COMMAND, await file.arrayBuffer(), {
    headers: { [PROJECT_HEADER]: String(projectId), [NAME_HEADER]: encodeURIComponent(name) },
  })
}

export function discardAttachment(projectId: number, id: string): Promise<void> {
  return invoke<void>(DISCARD_ATTACHMENT_COMMAND, { projectId, id })
}

export function listenFileDrops(onPaths: (paths: string[]) => void): Promise<UnlistenFn> {
  return getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === DROP_EVENT_TYPE) {
      onPaths(event.payload.paths)
    }
  })
}
