<script setup lang="ts">
import type { Attachment } from './attachmentsApi'
import { formatSize } from './attachmentBlock'

const SAVING_LABEL = 'Anexando…'

defineProps<{ attachments: Attachment[]; saving: boolean }>()
const emit = defineEmits<{ remove: [id: string] }>()
</script>

<template>
  <ul v-if="attachments.length > 0 || saving" class="flex min-w-0 max-w-[50%] shrink items-center gap-1.5 overflow-x-auto">
    <li
      v-for="attachment in attachments"
      :key="attachment.id"
      :data-attachment="attachment.id"
      class="flex h-7 min-w-0 shrink-0 items-center gap-1.5 rounded-full bg-surface-2 pl-2.5 pr-1 text-caption text-ink"
    >
      <span class="max-w-32 truncate" :title="attachment.name">{{ attachment.name }}</span>
      <span class="shrink-0 text-muted">{{ formatSize(attachment.size) }}</span>
      <button
        type="button"
        :aria-label="`Remover ${attachment.name}`"
        class="flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted hover:bg-hover hover:text-ink focus:outline-2 focus:outline-solid focus:outline-accent"
        @click="emit('remove', attachment.id)"
      >
        ×
      </button>
    </li>
    <li v-if="saving" class="flex h-7 shrink-0 items-center rounded-full bg-surface-2 px-2.5 text-caption text-muted">
      {{ SAVING_LABEL }}
    </li>
  </ul>
</template>
