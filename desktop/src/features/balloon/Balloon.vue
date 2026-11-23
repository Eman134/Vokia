<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useBalloons } from './useBalloons'
import UiButton from '../../ui/UiButton.vue'
import { INK_FOCUS_CLASSES, STICKY_INK_VARS } from '../../ui/handDrawn'
import VokiaMascot from '../brand/VokiaMascot.vue'

const store = useBalloons()

const DISMISS_ALL_LABEL = 'Fechar todos'
// Centralizado sobre a barra; a largura cabe na tela com 16 px de margem de cada lado.
const CONTAINER_CLASSES =
  'pointer-events-none absolute bottom-full left-1/2 mb-3 flex w-80 max-w-[calc(100vw-2rem)] -translate-x-1/2 flex-col items-stretch gap-2'
const STACK_BASE_CLASSES = 'px-1 py-1'
const STACK_FANNED_CLASSES = `${STACK_BASE_CLASSES} flex max-h-[min(60vh,28rem)] flex-col gap-2 overflow-y-auto`
const STACK_PILED_CLASSES = `${STACK_BASE_CLASSES} grid`
const STICKY_PILED_CELL_CLASSES = '[grid-area:1/1] transition-transform duration-150 motion-reduce:transition-none'
const PILE_VISIBLE_DEPTH = 3
const PILE_PEEK_PX = 8
// A inclinação fica ≤ 1,5°; alterna o lado por camada para parecer post-its soltos.
const PILE_TILT_DEG = 1.5
const STICKY_CLASSES = `pointer-events-auto rounded-sm bg-mark-yellow p-3 text-ink shadow-paper ${STICKY_INK_VARS}`
const DISMISS_ALL_CLASSES =
  `pointer-events-auto self-end cursor-pointer rounded-sm border border-line bg-surface px-2.5 py-1 text-tiny font-medium text-muted shadow-paper hover:text-ink ${INK_FOCUS_CLASSES}`

const fanned = ref(false)
const pileLayout = computed(() => store.items.length >= 2 && !fanned.value)

function pileStyle(index: number): Record<string, string | number> {
  const depth = store.items.length - 1 - index
  const tilt = depth % 2 === 0 ? -PILE_TILT_DEG : PILE_TILT_DEG
  return {
    transform: `translateY(${-depth * PILE_PEEK_PX}px) rotate(${tilt}deg)`,
    zIndex: index,
    opacity: depth < PILE_VISIBLE_DEPTH ? 1 : 0,
    pointerEvents: depth === 0 ? 'auto' : 'none',
  }
}

function onStackFocusOut(event: FocusEvent): void {
  const stack = event.currentTarget as HTMLElement
  if (!stack.contains(event.relatedTarget as Node | null)) {
    fanned.value = false
  }
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Escape' || event.defaultPrevented || store.panelOpen || store.items.length === 0) {
    return
  }
  const latest = store.items[store.items.length - 1]
  if (latest !== undefined) {
    store.dismiss(latest.id)
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div v-if="store.items.length > 0 && !store.panelOpen" :class="CONTAINER_CLASSES">
    <button
      v-if="store.items.length >= 2"
      type="button"
      :class="DISMISS_ALL_CLASSES"
      @click="store.dismissAll()"
    >
      {{ DISMISS_ALL_LABEL }}
    </button>
    <div
      :class="pileLayout ? STACK_PILED_CLASSES : STACK_FANNED_CLASSES"
      @mouseenter="fanned = true"
      @mouseleave="fanned = false"
      @focusin="fanned = true"
      @focusout="onStackFocusOut"
    >
      <div
        v-for="(item, index) in store.items"
        :key="item.id"
        role="status"
        :class="[STICKY_CLASSES, pileLayout ? STICKY_PILED_CELL_CLASSES : '-rotate-1']"
        :style="pileLayout ? pileStyle(index) : undefined"
        @mouseenter="store.setHovered(item.id, true)"
        @mouseleave="store.setHovered(item.id, false)"
      >
        <div class="flex items-start gap-2">
          <VokiaMascot v-if="item.mascot" :look="item.mascot" />
          <div class="min-w-0">
            <p v-if="item.kind !== 'error'" class="text-tiny text-muted">
              {{ item.origin ?? item.source }}
            </p>
            <p class="text-label font-semibold text-ink">{{ item.title }}</p>
          </div>
        </div>
        <p v-if="item.text" class="text-caption text-muted">{{ item.text }}</p>
        <p v-if="store.confirmations[item.id] !== undefined" class="mt-2 text-label font-semibold text-ok">
          {{ store.confirmations[item.id] }}
        </p>
        <div v-else-if="item.actions.length > 0" class="mt-2 flex justify-end gap-2">
          <UiButton
            v-for="action in item.actions"
            :key="action.id"
            :variant="action.primary ? 'primary' : 'quiet'"
            size="sm"
            :disabled="action.disabled"
            @press="store.act(item.id, action.id)"
          >
            {{ action.label }}
          </UiButton>
        </div>
      </div>
    </div>
  </div>
</template>
