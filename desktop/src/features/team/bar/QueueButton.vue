<script setup lang="ts">
import { watch } from 'vue'
import { useRequests } from '../useRequests'
import { QUEUE_BUTTON_LABEL } from '../queueCopy'
import QueuePopover from '../QueuePopover.vue'
import UiButton from '../../../ui/UiButton.vue'
import UiIcon from '../../../ui/UiIcon.vue'

const QUEUE_ICON_PATH = 'M2.5 4h11M2.5 8h8M2.5 12h5'
const QUEUE_ICON_SIZE = 14

const open = defineModel<boolean>('open')
const requests = useRequests()

// Sem fila o botão e o popover somem; sem isto o pai ficaria com a barra expandida por um aberto invisível.
watch(
  () => requests.projectQueue.length,
  (length) => {
    if (length === 0) {
      open.value = false
    }
  },
)
</script>

<template>
  <div v-if="requests.projectQueue.length > 0" class="relative">
    <UiButton
      variant="quiet"
      size="sm"
      class="gap-1.5"
      :aria-label="QUEUE_BUTTON_LABEL(requests.projectQueue.length)"
      :title="QUEUE_BUTTON_LABEL(requests.projectQueue.length)"
      aria-haspopup="dialog"
      :aria-expanded="open"
      @press="open = !open"
    >
      <UiIcon :path="QUEUE_ICON_PATH" :size="QUEUE_ICON_SIZE" />
      <span>{{ requests.projectQueue.length }}</span>
    </UiButton>
    <QueuePopover v-if="open" @close="open = false" />
  </div>
</template>
