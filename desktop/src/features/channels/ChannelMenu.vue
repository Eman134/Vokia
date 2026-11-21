<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import UiConfirm from '../../ui/UiConfirm.vue'

defineProps<{ name: string }>()
const emit = defineEmits<{ remove: [] }>()

const TRIGGER_GLYPH = '⋯'
const DELETE_MESSAGE = 'Apaga o canal. As mensagens dele voltam para #geral.'
const DELETE_LABEL = 'Excluir canal'
const TRIGGER_CLASSES =
  'absolute right-1.5 top-1/2 grid size-6 -translate-y-1/2 cursor-pointer place-items-center rounded-full text-label font-bold text-muted transition-opacity duration-150 hover:bg-hover hover:text-ink focus-visible:opacity-100 focus:outline-2 focus:outline-solid focus:outline-accent motion-reduce:transition-none'
const TRIGGER_IDLE_CLASSES = 'opacity-0 group-hover:opacity-100'
const POPOVER_CLASSES =
  'absolute inset-x-0 top-full z-20 mt-1 rounded-popover border border-line bg-surface p-2 shadow-float'

const open = ref(false)
const root = ref<HTMLElement>()

function toggle(): void {
  open.value = !open.value
}

function confirm(): void {
  open.value = false
  emit('remove')
}

function onKeydown(event: KeyboardEvent): void {
  if (open.value && event.key === 'Escape') {
    open.value = false
  }
}

function onPointerDown(event: PointerEvent): void {
  if (open.value && event.target instanceof Node && !root.value?.contains(event.target)) {
    open.value = false
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  window.addEventListener('pointerdown', onPointerDown)
})
onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
  window.removeEventListener('pointerdown', onPointerDown)
})
</script>

<template>
  <div ref="root">
    <button
      type="button"
      :class="[TRIGGER_CLASSES, !open && TRIGGER_IDLE_CLASSES]"
      :aria-label="`Ações do canal #${name}`"
      :title="`Ações do canal #${name}`"
      aria-haspopup="menu"
      :aria-expanded="open"
      @click="toggle"
    >
      {{ TRIGGER_GLYPH }}
    </button>
    <div v-if="open" role="menu" :aria-label="`Ações do canal #${name}`" :class="POPOVER_CLASSES">
      <UiConfirm :label="`${DELETE_LABEL} #${name}`" :message="DELETE_MESSAGE" :confirm-label="DELETE_LABEL" @confirm="confirm" />
    </div>
  </div>
</template>
