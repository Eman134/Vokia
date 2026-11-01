<script setup lang="ts">
import { AGENTS, DEFAULT_RECIPIENT, useTeam, type QueuedRequest } from './useTeam'
import { QUEUE_EDIT, QUEUE_REMOVE, QUEUED_TAG } from './queueCopy'
import QueueItemEditor from './QueueItemEditor.vue'
import UiButton from '../../ui/UiButton.vue'

const USER_INITIAL = 'V'

defineProps<{ request: QueuedRequest }>()

const team = useTeam()

function recipientName(id: string): string {
  return AGENTS.find((agent) => agent.id === id)?.name ?? id
}
</script>

<template>
  <div class="flex items-start justify-end gap-2.5 opacity-60">
    <div class="flex max-w-3/4 flex-col items-end gap-1">
      <span class="flex items-center gap-2 text-caption text-muted">
        <span class="rounded-control bg-surface-2 px-1.5 font-semibold">{{ QUEUED_TAG }}</span>
        <span v-if="request.to !== DEFAULT_RECIPIENT">Para {{ recipientName(request.to) }}</span>
      </span>
      <QueueItemEditor v-if="request.editing" :request="request" class="w-full" />
      <template v-else>
        <p
          class="wrap-break-word whitespace-pre-wrap rounded-card rounded-tr-tail bg-surface-2 px-4 py-3 text-control text-muted"
        >
          {{ request.text }}
        </p>
        <div class="flex gap-1">
          <UiButton variant="quiet" size="sm" @press="team.startEdit(request.id)">{{ QUEUE_EDIT }}</UiButton>
          <UiButton variant="quiet" size="sm" @press="team.removeQueued(request.id)">{{ QUEUE_REMOVE }}</UiButton>
        </div>
      </template>
    </div>
    <span class="grid size-7 shrink-0 place-items-center rounded-full bg-ink text-small font-semibold text-bg">
      {{ USER_INITIAL }}
    </span>
  </div>
</template>
