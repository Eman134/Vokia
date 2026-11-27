export interface InlinePermissionState {
  flowActive: boolean
  pendingCount: number
  hasContent: boolean
}

// Com texto, anexo ou menção confirmada na barra, a decisão inline cede o lugar ao campo; a permissão segue pendente em "N Decisão".
export function showsInlinePermission({ flowActive, pendingCount, hasContent }: InlinePermissionState): boolean {
  return !flowActive && pendingCount > 0 && !hasContent
}

export function hasComposerContent(draft: string, attachmentCount: number, confirmedCount: number): boolean {
  return draft !== '' || attachmentCount > 0 || confirmedCount > 0
}
