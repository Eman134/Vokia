<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, useId } from 'vue'
import UiSketchBox from './UiSketchBox.vue'

const props = defineProps<{ text?: string }>()

const SHOW_DELAY_MS = 200
const EDGE_GAP_PX = 8
const OFFSET_PX = 8
const BUBBLE_CLASSES = 'pointer-events-none fixed z-70 max-w-sm rounded-md bg-surface shadow-paper'
const TEXT_CLASSES = 'wrap-break-word whitespace-pre-wrap px-3 py-2 text-caption text-ink'

const id = useId()
const trigger = ref<HTMLElement>()
const bubble = ref<HTMLElement>()
const open = ref(false)
const position = ref({ top: 0, left: 0 })
let timer: ReturnType<typeof setTimeout> | null = null

function cancelTimer(): void {
  if (timer !== null) {
    clearTimeout(timer)
    timer = null
  }
}

// A bolha vive no body: o texto da linha é truncado (overflow oculto) e cortaria um tooltip absoluto.
async function place(): Promise<void> {
  open.value = true
  await nextTick()
  if (!trigger.value || !bubble.value) return
  const anchor = trigger.value.getBoundingClientRect()
  const { width, height } = bubble.value.getBoundingClientRect()
  const above = anchor.top - height - OFFSET_PX
  const top = above >= EDGE_GAP_PX ? above : anchor.bottom + OFFSET_PX
  const left = Math.min(Math.max(anchor.left, EDGE_GAP_PX), Math.max(window.innerWidth - width - EDGE_GAP_PX, EDGE_GAP_PX))
  position.value = { top, left }
}

function showSoon(): void {
  if (!props.text) return
  cancelTimer()
  timer = setTimeout(() => void place(), SHOW_DELAY_MS)
}

function hide(): void {
  cancelTimer()
  open.value = false
}

// Foco por clique não abre: só o foco de teclado, como o outline do projeto.
function onFocusIn(event: FocusEvent): void {
  if (!props.text) return
  if (event.target instanceof HTMLElement && event.target.matches(':focus-visible')) {
    cancelTimer()
    void place()
  }
}

onBeforeUnmount(cancelTimer)
</script>

<template>
  <span
    ref="trigger"
    :aria-describedby="open ? id : undefined"
    @pointerenter="showSoon"
    @pointerleave="hide"
    @focusin="onFocusIn"
    @focusout="hide"
    @keydown.esc="hide"
  >
    <slot />
    <Teleport to="body">
      <div v-if="open && text" :id="id" ref="bubble" role="tooltip" :class="BUBBLE_CLASSES" :style="{ top: `${position.top}px`, left: `${position.left}px` }">
        <UiSketchBox>
          <p :class="TEXT_CLASSES">{{ text }}</p>
        </UiSketchBox>
      </div>
    </Teleport>
  </span>
</template>
