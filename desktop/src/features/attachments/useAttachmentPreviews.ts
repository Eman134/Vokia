import { ref, watch, type Ref } from 'vue'
import { isImageAttachmentPath } from './attachmentBlock'
import { loadAttachmentPreview } from './attachmentsApi'

export interface PreviewableAttachment {
  path: string
  name: string
}

/** Carrega miniaturas base64 para anexos de imagem e controla qual está ampliado no viewer. */
export function useAttachmentPreviews<T extends PreviewableAttachment>(
  attachments: () => readonly T[],
): {
  previews: Ref<Record<string, string>>
  expanded: Ref<T | null>
} {
  const previews = ref<Record<string, string>>({})
  const expanded = ref<T | null>(null) as Ref<T | null>

  watch(
    attachments,
    (list) => {
      for (const attachment of list) {
        if (!isImageAttachmentPath(attachment.path || attachment.name)) continue
        if (previews.value[attachment.path] !== undefined) continue
        void loadAttachmentPreview(attachment.path).then((dataUrl) => {
          if (dataUrl) {
            previews.value = { ...previews.value, [attachment.path]: dataUrl }
          }
        })
      }
    },
    { immediate: true },
  )

  return { previews, expanded }
}
