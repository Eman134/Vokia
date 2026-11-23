<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useBalloons } from './useBalloons'
import UiButton from '../../ui/UiButton.vue'
import { INK_FOCUS_CLASSES, STICKY_INK_VARS } from '../../ui/handDrawn'
import VokiaMascot from '../brand/VokiaMascot.vue'

const store = useBalloons()

const DISMISS_ALL_LABEL = 'Fechar todos'
// Centralizado sobre a barra; a largura cabe na tela com 16 px de margem de cada lado.
const CONTAINER_CLASSES =
  'pointer-events-none absolute bottom-full left-1/2 mb-3 flex w-80 max-w-[calc(100vw-2rem)] -translate-x-1/2 flex-col items-stretch gap-2'
const STACK_LIST_CLASSES = 'flex max-h-[min(60vh,28rem)] flex-col gap-2 overflow-y-auto px-1 py-1'
// A inclinação fica ≤ 1,5°.
const STICKY_CLASSES = `pointer-events-auto -rotate-1 rounded-sm bg-mark-yellow p-3 text-ink shadow-paper ${STICKY_INK_VARS}`
const DISMISS_ALL_CLASSES =
  `pointer-events-auto self-end cursor-pointer rounded-sm border border-line bg-surface px-2.5 py-1 text-tiny font-medium text-muted shadow-paper hover:text-ink ${INK_FOCUS_CLASSES}`

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
    <div :class="STACK_LIST_CLASSES">
      <div
        v-for="item in store.items"
        :key="item.id"
        role="status"
        :class="STICKY_CLASSES"
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
