<script setup lang="ts">
import { onMounted, onUnmounted, watch } from 'vue'
import { agentName, useTeam } from './useTeam'
import {
  QUEUE_DOWN,
  QUEUE_EDIT,
  QUEUE_POPOVER_TITLE,
  QUEUE_REMOVE,
  QUEUE_UP,
} from './queueCopy'
import QueueItemEditor from './QueueItemEditor.vue'
import UiButton from '../../ui/UiButton.vue'

const emit = defineEmits<{ close: [] }>()

const team = useTeam()

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && !event.defaultPrevented) {
    emit('close')
  }
}

watch(
  () => team.queue.length,
  (length) => {
    if (length === 0) {
      emit('close')
    }
  },
  { immediate: true },
)

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="absolute bottom-full left-0 pb-2">
    <div class="w-80 rounded-popover border border-line bg-surface p-1.5 shadow-float">
      <p class="px-2 py-1.5 text-caption text-muted">{{ QUEUE_POPOVER_TITLE }}</p>
      <ol :aria-label="QUEUE_POPOVER_TITLE" class="flex flex-col gap-1">
        <li v-for="(request, index) in team.queue" :key="request.id" class="rounded-control px-2 py-1.5">
          <span class="text-caption font-semibold text-ink">{{ agentName(request.to) }}</span>
          <QueueItemEditor v-if="request.editing" :request="request" />
          <template v-else>
            <p class="wrap-break-word line-clamp-2 whitespace-pre-wrap text-label text-muted">{{ request.text }}</p>
            <div class="mt-1 flex flex-wrap gap-1">
              <UiButton
                variant="quiet"
                size="sm"
                :disabled="index === 0"
                :aria-label="`${QUEUE_UP} pedido para ${agentName(request.to)}`"
                @press="team.moveQueued(request.id, 'up')"
              >
                {{ QUEUE_UP }}
              </UiButton>
              <UiButton
                variant="quiet"
                size="sm"
                :disabled="index === team.queue.length - 1"
                :aria-label="`${QUEUE_DOWN} pedido para ${agentName(request.to)}`"
                @press="team.moveQueued(request.id, 'down')"
              >
                {{ QUEUE_DOWN }}
              </UiButton>
              <UiButton
                variant="quiet"
                size="sm"
                :aria-label="`${QUEUE_EDIT} pedido para ${agentName(request.to)}`"
                @press="team.startEdit(request.id)"
              >
                {{ QUEUE_EDIT }}
              </UiButton>
              <UiButton
                variant="quiet"
                size="sm"
                :aria-label="`${QUEUE_REMOVE} pedido para ${agentName(request.to)}`"
                @press="team.removeQueued(request.id)"
              >
                {{ QUEUE_REMOVE }}
              </UiButton>
            </div>
          </template>
        </li>
      </ol>
    </div>
  </div>
</template>
