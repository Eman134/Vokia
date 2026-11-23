<script setup lang="ts">
import type { Attachment } from './attachmentsApi'
import { formatSize } from './attachmentBlock'
import UiIcon from '../../ui/UiIcon.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'

const SAVING_LABEL = 'Anexando…'

const ICON_IMAGE = 'M4 6a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6Zm4.5 3.5a1.5 1.5 0 1 0 0 .01M6 17l4-4 3 3 2-2 3 3'
const ICON_CODE = 'm8 9-3 3 3 3m8-6 3 3-3 3m-5 2 2-10'
const ICON_DOC = 'M7 3h7l5 5v13a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Zm7 0v5h5M9 13h6M9 17h4'
const ICON_TEXT = 'M6 4h12v16H6V4Zm3 5h6M9 13h6M9 17h4'
const ICON_GENERIC = 'M7 3h7l5 5v13a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Zm7 0v5h5'

const CODE_EXTS = new Set([
  'ts', 'tsx', 'js', 'jsx', 'vue', 'rs', 'py', 'go', 'java', 'c', 'cpp', 'h', 'hpp',
  'css', 'scss', 'html', 'json', 'yaml', 'yml', 'toml', 'sh', 'sql', 'xml',
])
const IMAGE_EXTS = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp', 'ico'])
const DOC_EXTS = new Set(['pdf', 'doc', 'docx', 'odt', 'rtf'])
const TEXT_EXTS = new Set(['txt', 'md', 'csv', 'log'])

function attachmentIconPath(attachment: Attachment): string {
  const ext = attachment.name.split('.').pop()?.toLowerCase() ?? ''
  if (IMAGE_EXTS.has(ext)) return ICON_IMAGE
  if (CODE_EXTS.has(ext)) return ICON_CODE
  if (DOC_EXTS.has(ext)) return ICON_DOC
  if (TEXT_EXTS.has(ext)) return ICON_TEXT
  return ICON_GENERIC
}

defineProps<{ attachments: Attachment[]; saving: boolean }>()
const emit = defineEmits<{ remove: [id: string] }>()
</script>

<template>
  <ul v-if="attachments.length > 0 || saving" class="flex min-w-0 shrink items-center gap-1 overflow-x-auto">
    <li
      v-for="attachment in attachments"
      :key="attachment.id"
      :data-attachment="attachment.id"
      class="flex h-6 min-w-0 shrink-0 items-center gap-1 rounded-full bg-surface px-2 pr-1 text-caption text-ink"
    >
      <UiIcon :path="attachmentIconPath(attachment)" class="size-3.5 shrink-0 text-muted" />
      <span class="max-w-28 truncate" :title="`${attachment.name} (${formatSize(attachment.size)})`">
        {{ attachment.name }}
      </span>
      <button
        type="button"
        :aria-label="`Remover ${attachment.name}`"
        class="flex size-4 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted hover:bg-hover hover:text-ink"
        :class="INK_FOCUS_CLASSES"
        @click="emit('remove', attachment.id)"
      >
        ×
      </button>
    </li>
    <li v-if="saving" class="flex h-6 shrink-0 items-center rounded-full bg-surface px-2 text-caption text-muted">
      {{ SAVING_LABEL }}
    </li>
  </ul>
</template>
