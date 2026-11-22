<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, useId } from 'vue'
import { HAND_DRAWN_RADIUS, INK_FOCUS_CLASSES } from '../../ui/handDrawn'

defineProps<{ label: string }>()

// Lacuna da frase do pedido: parece uma caixinha de seleção e abre o popover com as opções.
const SLOT_CLASSES = `group mx-0.5 inline-flex cursor-pointer items-center gap-1.5 whitespace-nowrap border-[1.5px] px-2 py-0.5 align-middle text-control font-medium text-ink shadow-paper ${HAND_DRAWN_RADIUS} ${INK_FOCUS_CLASSES}`
const SLOT_IDLE = 'border-graphite bg-surface hover:bg-surface-2'
const SLOT_OPEN = 'border-ink bg-surface-2'
const CHEVRON_PATH = 'M3 5.5l3 3 3-3'
// fixed escapa do overflow do diálogo (que não tem transform): o popover flutua por cima sem criar barra de rolagem.
const POPOVER_CLASSES =
  'fixed z-60 w-72 max-w-[calc(100vw-2rem)] max-h-[calc(100dvh-2rem)] overflow-y-auto rounded-md border border-line bg-surface p-2 font-sans text-base font-normal leading-normal shadow-paper'
const POPOVER_GAP = 6
const VIEWPORT_MARGIN = 16

const open = ref(false)
const wrap = ref<HTMLElement>()
const trigger = ref<HTMLButtonElement>()
const popover = ref<HTMLElement>()
const position = ref({ top: 0, left: 0 })
const popoverId = useId()

// Abre para baixo; sem espaço embaixo e com espaço em cima, abre para cima.
function place(): void {
  if (!open.value || !trigger.value || !popover.value) return
  const slot = trigger.value.getBoundingClientRect()
  const { width, height } = popover.value.getBoundingClientRect()
  const below = slot.bottom + POPOVER_GAP
  const above = slot.top - POPOVER_GAP - height
  const fitsBelow = below + height <= window.innerHeight - VIEWPORT_MARGIN
  const top = fitsBelow || above < VIEWPORT_MARGIN ? below : above
  const left = Math.max(VIEWPORT_MARGIN, Math.min(slot.left, window.innerWidth - width - VIEWPORT_MARGIN))
  position.value = { top: Math.max(VIEWPORT_MARGIN, top), left }
}

async function close(): Promise<void> {
  if (!open.value) return
  open.value = false
  await nextTick()
  trigger.value?.focus()
}

async function toggle(): Promise<void> {
  if (open.value) {
    await close()
    return
  }
  open.value = true
  await nextTick()
  place()
  popover.value?.querySelector<HTMLElement>('[aria-checked="true"], button')?.focus()
}

function onPointerDown(event: PointerEvent): void {
  if (!open.value) return
  if (event.target instanceof Node && !wrap.value?.contains(event.target)) open.value = false
}

onMounted(() => {
  document.addEventListener('pointerdown', onPointerDown)
  window.addEventListener('resize', place)
  window.addEventListener('scroll', place, true)
})
onUnmounted(() => {
  document.removeEventListener('pointerdown', onPointerDown)
  window.removeEventListener('resize', place)
  window.removeEventListener('scroll', place, true)
})
</script>

<template>
  <span ref="wrap" class="inline-block" @keydown.esc.stop.prevent="close">
    <button
      ref="trigger"
      type="button"
      :class="[SLOT_CLASSES, open ? SLOT_OPEN : SLOT_IDLE]"
      :aria-label="label"
      aria-haspopup="dialog"
      :aria-expanded="open"
      :aria-controls="open ? popoverId : undefined"
      @click="toggle"
    >
      <slot name="value" />
      <svg
        width="12"
        height="12"
        viewBox="0 0 12 12"
        aria-hidden="true"
        class="text-muted transition-transform duration-150 motion-reduce:transition-none"
        :class="open && 'rotate-180'"
      >
        <path :d="CHEVRON_PATH" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </button>
    <div
      v-if="open"
      :id="popoverId"
      ref="popover"
      role="dialog"
      :aria-label="label"
      :class="POPOVER_CLASSES"
      :style="{ top: `${position.top}px`, left: `${position.left}px` }"
    >
      <slot :close="close" />
    </div>
  </span>
</template>
