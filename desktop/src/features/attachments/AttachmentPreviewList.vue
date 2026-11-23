<script setup lang="ts">
import { ref, watch } from 'vue'
import UiIcon from '../../ui/UiIcon.vue'
import { attachmentIconPath, isImageAttachmentPath, type Attachment } from './attachmentBlock'
import { loadAttachmentPreview } from './attachmentsApi'

const props = withDefaults(
  defineProps<{
    attachments: Attachment[]
    compact?: boolean
    align?: 'start' | 'end'
  }>(),
  {
    compact: false,
    align: 'start',
  },
)

const previews = ref<Record<string, string>>({})

const LIST_BASE_CLASSES = 'flex flex-wrap gap-1.5'
const ITEM_DEFAULT_CLASSES =
  'flex min-w-0 max-w-full items-center gap-2 rounded-md border border-line bg-surface px-2.5 py-1.5 text-caption'
const ITEM_COMPACT_CLASSES =
  'flex min-w-0 max-w-full items-center gap-1.5 rounded-sm border border-line bg-surface-2 px-1.5 py-1 text-meta'
const THUMB_DEFAULT_CLASSES = 'size-10 shrink-0 rounded-sm border border-line object-cover bg-surface-2'
const THUMB_COMPACT_CLASSES = 'size-7 shrink-0 rounded-sm border border-line object-cover bg-surface-2'

watch(
  () => props.attachments,
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
</script>

<template>
  <ul
    v-if="attachments.length > 0"
    :class="[LIST_BASE_CLASSES, align === 'end' ? 'justify-end' : 'justify-start']"
  >
    <li
      v-for="attachment in attachments"
      :key="attachment.path"
      :title="attachment.size ? `${attachment.name} (${attachment.size})` : attachment.name"
      :data-attachment-name="attachment.name"
      :class="compact ? ITEM_COMPACT_CLASSES : ITEM_DEFAULT_CLASSES"
    >
      <img
        v-if="previews[attachment.path]"
        :src="previews[attachment.path]"
        :alt="attachment.name"
        :class="compact ? THUMB_COMPACT_CLASSES : THUMB_DEFAULT_CLASSES"
      />
      <UiIcon
        v-else
        :path="attachmentIconPath(attachment.name)"
        :class="compact ? 'size-3.5 shrink-0 text-muted' : 'size-4 shrink-0 text-muted'"
      />
      <div class="flex min-w-0 flex-col">
        <span class="truncate font-semibold text-ink">{{ attachment.name }}</span>
        <span v-if="!compact && attachment.size" class="shrink-0 text-muted">
          {{ attachment.label }} · {{ attachment.size }}
        </span>
      </div>
    </li>
  </ul>
</template>
