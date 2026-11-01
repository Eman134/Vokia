<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useTeam, type QueuedRequest } from './useTeam'
import { QUEUE_KEEP, QUEUE_SAVE } from './queueCopy'
import UiButton from '../../ui/UiButton.vue'

const props = defineProps<{ request: QueuedRequest }>()

const team = useTeam()
const text = ref(props.request.text)
const field = ref<HTMLTextAreaElement | null>(null)

onMounted(() => field.value?.focus())

function save(): void {
  team.saveEdit(props.request.id, text.value)
}

function keep(): void {
  team.cancelEdit(props.request.id)
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault()
    save()
  } else if (event.key === 'Escape') {
    // Sem isso o Esc também fecharia o popup que hospeda o editor.
    event.preventDefault()
    keep()
  }
}
</script>

<template>
  <div class="flex flex-col gap-1.5">
    <textarea
      ref="field"
      v-model="text"
      rows="2"
      aria-label="Editar pedido na fila"
      class="w-full resize-none rounded-control border border-line bg-surface-2 px-3 py-2 text-label text-ink outline-none focus:outline-2 focus:outline-solid focus:outline-accent"
      @keydown="onKeydown"
    />
    <div class="flex justify-end gap-1">
      <UiButton variant="quiet" size="sm" @press="keep">{{ QUEUE_KEEP }}</UiButton>
      <UiButton variant="primary" size="sm" :disabled="text.trim() === ''" @press="save">{{ QUEUE_SAVE }}</UiButton>
    </div>
  </div>
</template>
