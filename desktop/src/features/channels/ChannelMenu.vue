<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'

defineProps<{ name: string }>()
const emit = defineEmits<{ edit: []; remove: [] }>()

const TRIGGER_GLYPH = '⋯'
const DELETE_MESSAGE = 'Apaga o canal. As mensagens dele voltam para #geral.'
const DELETE_LABEL = 'Excluir canal'
const EDIT_LABEL = 'Editar canal'
const TRIGGER_CLASSES =
  'absolute right-1.5 top-1/2 grid size-6 -translate-y-1/2 cursor-pointer place-items-center rounded-full text-label font-bold text-muted transition-opacity duration-150 hover:bg-hover hover:text-ink focus-visible:opacity-100 focus:outline-2 focus:outline-solid focus:outline-accent motion-reduce:transition-none'
const TRIGGER_IDLE_CLASSES = 'opacity-0 group-hover:opacity-100'
const POPOVER_CLASSES =
  'absolute inset-x-0 top-full z-20 mt-1 flex flex-col gap-1 rounded-popover border border-line bg-surface p-1.5 shadow-float'
const ITEM_CLASSES =
  'flex w-full cursor-pointer items-center gap-2.5 rounded-control px-3 py-2 text-left text-label font-medium transition-colors duration-150 focus:outline-2 focus:outline-solid focus:outline-accent motion-reduce:transition-none'
const EDIT_ITEM_CLASSES = 'text-ink hover:bg-hover [&_svg]:text-muted hover:[&_svg]:text-ink'
const DELETE_ITEM_CLASSES = 'text-danger hover:bg-danger/10'
const ICON_CLASSES = 'size-4 shrink-0 transition-colors duration-150'
const CONFIRM_MESSAGE_CLASSES = 'px-2 pt-1 text-caption text-muted'
const CANCEL_LABEL = 'Cancelar'
const CONFIRM_LABEL = 'Excluir'

const open = ref(false)
const confirming = ref(false)
const root = ref<HTMLElement>()

function toggle(): void {
  open.value = !open.value
  confirming.value = false
}

function edit(): void {
  close()
  emit('edit')
}

function confirm(): void {
  close()
  emit('remove')
}

function close(): void {
  open.value = false
  confirming.value = false
}

function onKeydown(event: KeyboardEvent): void {
  if (open.value && event.key === 'Escape') {
    close()
  }
}

function onPointerDown(event: PointerEvent): void {
  if (open.value && event.target instanceof Node && !root.value?.contains(event.target)) {
    close()
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
      <template v-if="!confirming">
        <button type="button" role="menuitem" :class="[ITEM_CLASSES, EDIT_ITEM_CLASSES]" @click="edit">
          <svg :class="ICON_CLASSES" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M12 20h9" />
            <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4Z" />
          </svg>
          {{ EDIT_LABEL }}
        </button>
        <hr class="mx-1 border-t border-line" />
        <button type="button" role="menuitem" :class="[ITEM_CLASSES, DELETE_ITEM_CLASSES]" @click="confirming = true">
          <svg :class="ICON_CLASSES" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M3 6h18" />
            <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
            <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
            <path d="M10 11v6M14 11v6" />
          </svg>
          {{ DELETE_LABEL }}
        </button>
      </template>
      <template v-else>
        <p :class="CONFIRM_MESSAGE_CLASSES">{{ DELETE_MESSAGE }}</p>
        <div class="flex gap-1.5">
          <UiButton variant="quiet" size="sm" class="flex-1" @press="confirming = false">{{ CANCEL_LABEL }}</UiButton>
          <UiButton variant="danger" size="sm" class="flex-1" data-testid="confirm-accept" @press="confirm">{{ CONFIRM_LABEL }}</UiButton>
        </div>
      </template>
    </div>
  </div>
</template>
