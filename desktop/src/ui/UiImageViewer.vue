<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { trapFocus } from '../features/backlog/dialogFocus'
import UiCloseButton from './UiCloseButton.vue'

const props = defineProps<{
  src: string | null
  alt: string
}>()
const emit = defineEmits<{ close: [] }>()

// Mesma camada do UiConfirm: o viewer pode abrir de dentro dos modais das telas (z-50).
const OVERLAY_CLASSES = 'fixed inset-0 z-60 flex items-center justify-center bg-paper/70 p-4'
const FRAME_CLASSES = 'relative max-h-full max-w-full rounded-md border border-line bg-surface p-2 shadow-paper'
// O backdrop tem p-4 e a moldura p-2 (3rem no total): a imagem nunca passa da janela.
const IMAGE_CLASSES = 'block max-h-[calc(100dvh-3rem)] max-w-[calc(100dvw-3rem)] rounded-sm object-contain'
const CLOSE_CLASSES = 'absolute right-3 top-3 z-10 bg-surface/80'

const dialog = ref<HTMLElement>()
const closeButton = ref<{ $el: HTMLElement } | null>(null)
let origin: HTMLElement | null = null

watch(
  () => props.src,
  async (src, previous) => {
    if (!src || previous) return
    origin = document.activeElement instanceof HTMLElement ? document.activeElement : null
    await nextTick()
    closeButton.value?.$el.focus()
  },
)

function close(): void {
  emit('close')
  origin?.focus()
  origin = null
}

// Os ouvintes de clique fora das telas rodam no document: parar a propagação evita que fechem o pai junto.
function onOverlayClick(event: MouseEvent): void {
  if (event.target === event.currentTarget) close()
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="src"
      :class="OVERLAY_CLASSES"
      data-testid="image-viewer"
      @click.stop="onOverlayClick"
      @pointerdown.stop
      @mousedown.stop
    >
      <section
        ref="dialog"
        role="dialog"
        aria-modal="true"
        :aria-label="alt"
        tabindex="-1"
        :class="FRAME_CLASSES"
        @keydown.esc.stop="close"
        @keydown.tab="trapFocus($event, dialog)"
      >
        <UiCloseButton ref="closeButton" :class="CLOSE_CLASSES" @press="close" />
        <img :src="src" :alt="alt" :class="IMAGE_CLASSES" />
      </section>
    </div>
  </Teleport>
</template>
