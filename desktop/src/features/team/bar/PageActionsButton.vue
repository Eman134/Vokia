<script setup lang="ts">
import UiButton from '../../../ui/UiButton.vue'
import { BAR_MOTION_CLASSES } from './barClasses'

const REVEAL_ENTER_FROM_CLASS = 'opacity-0'
const REVEAL_ENTER_ACTIVE_CLASS = `transition-opacity ${BAR_MOTION_CLASSES}`
// Saindo, o botão deixa o fluxo: senão a largura final da pílula o conta e ela salta quando ele some.
const REVEAL_LEAVE_ACTIVE_CLASS = `absolute ${REVEAL_ENTER_ACTIVE_CLASS}`
const REVEAL_LEAVE_TO_CLASS = REVEAL_ENTER_FROM_CLASS

defineProps<{ visible: boolean }>()

const open = defineModel<boolean>('open')
</script>

<template>
  <Transition
    :enter-from-class="REVEAL_ENTER_FROM_CLASS"
    :enter-active-class="REVEAL_ENTER_ACTIVE_CLASS"
    :leave-active-class="REVEAL_LEAVE_ACTIVE_CLASS"
    :leave-to-class="REVEAL_LEAVE_TO_CLASS"
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
