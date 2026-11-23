<script setup lang="ts">
import UiButton from '../../../ui/UiButton.vue'
import { BAR_MOTION_CLASSES } from './barClasses'

const REVEAL_ENTER_FROM_CLASS = 'opacity-0'
const REVEAL_ENTER_ACTIVE_CLASS = `transition-opacity ${BAR_MOTION_CLASSES}`

defineProps<{ visible: boolean }>()

const open = defineModel<boolean>('open')
</script>

<template>
  <Transition
    :enter-from-class="REVEAL_ENTER_FROM_CLASS"
    :enter-active-class="REVEAL_ENTER_ACTIVE_CLASS"
  >
    <span v-if="visible" class="flex shrink-0 items-center gap-2">
      <span class="h-5.5 w-px shrink-0 bg-line" aria-hidden="true" />
      <UiButton
        variant="quiet"
        size="sm"
        class="w-7 px-0"
        aria-label="Ações da página"
        title="Ações da página"
        aria-haspopup="menu"
        :aria-expanded="open"
        @press="open = !open"
      >
        ☰
      </UiButton>
    </span>
  </Transition>
</template>
