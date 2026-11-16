<script setup lang="ts">
import { watch } from 'vue'
import { useRequests } from '../useRequests'
import { QUEUE_BADGE } from '../queueCopy'
import QueuePopover from '../QueuePopover.vue'
import UiButton from '../../../ui/UiButton.vue'

const open = defineModel<boolean>('open')
const requests = useRequests()

// Sem fila o botão e o popover somem; sem isto o pai ficaria com a barra expandida por um aberto invisível.
watch(
  () => requests.queue.length,
  (length) => {
    if (length === 0) {
      open.value = false
    }
  },
)
</script>

<template>
  <UiButton
    v-if="requests.queue.length > 0"
    variant="quiet"
    size="sm"
    aria-haspopup="dialog"
    :aria-expanded="open"
    @press="open = !open"
  >
    {{ QUEUE_BADGE(requests.queue.length) }}
  </UiButton>
  <QueuePopover v-if="open && requests.queue.length > 0" @close="open = false" />
</template>
