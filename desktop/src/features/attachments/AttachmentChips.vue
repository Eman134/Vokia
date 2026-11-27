<script setup lang="ts">
import type { Attachment } from './attachmentsApi'
import { attachmentIconPath, formatSize } from './attachmentBlock'
import { useAttachmentPreviews } from './useAttachmentPreviews'
import UiIcon from '../../ui/UiIcon.vue'
import UiImageViewer from '../../ui/UiImageViewer.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'

const SAVING_LABEL = 'Anexando…'
const EXPAND_LABEL_PREFIX = 'Ampliar'
const EXPAND_BUTTON_CLASSES = `flex min-w-0 cursor-zoom-in items-center gap-1 rounded-full hover:text-accent ${INK_FOCUS_CLASSES}`

const props = defineProps<{ attachments: Attachment[]; saving: boolean }>()
const emit = defineEmits<{ remove: [id: string] }>()

const { previews, expanded } = useAttachmentPreviews(() => props.attachments)
</script>

<template>
  <ul v-if="attachments.length > 0 || saving" class="flex min-w-0 shrink items-center gap-1 overflow-x-auto">
    <li
      v-for="attachment in attachments"
      :key="attachment.id"
      :data-attachment="attachment.id"
      class="flex h-6 min-w-0 shrink-0 items-center gap-1 rounded-full bg-surface px-2 pr-1 text-caption text-ink"
    >
      <button
        v-if="previews[attachment.path]"
        type="button"
        :class="EXPAND_BUTTON_CLASSES"
        :aria-label="`${EXPAND_LABEL_PREFIX} ${attachment.name}`"
        :title="`${attachment.name} (${formatSize(attachment.size)})`"
        data-testid="attachment-expand"
        @click.stop="expanded = attachment"
      >
        <UiIcon :path="attachmentIconPath(attachment.name)" class="size-3.5 shrink-0 text-muted" />
        <span class="max-w-28 truncate">{{ attachment.name }}</span>
      </button>
      <template v-else>
        <UiIcon :path="attachmentIconPath(attachment.name)" class="size-3.5 shrink-0 text-muted" />
        <span class="max-w-28 truncate" :title="`${attachment.name} (${formatSize(attachment.size)})`">
          {{ attachment.name }}
        </span>
      </template>
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
    <!-- Dentro da ul para manter raiz única: os pais passam class; o Teleport só deixa âncoras aqui. -->
    <UiImageViewer
      :src="expanded ? (previews[expanded.path] ?? null) : null"
      :alt="expanded?.name ?? ''"
      @close="expanded = null"
    />
  </ul>
</template>
