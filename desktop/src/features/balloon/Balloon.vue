<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useBalloons } from './useBalloons'
import UiButton from '../../ui/UiButton.vue'
import { STICKY_INK_VARS } from '../../ui/handDrawn'
import VokiaMascot from '../brand/VokiaMascot.vue'

const store = useBalloons()
// Centralizado sobre a barra; a largura cabe na tela com 16 px de margem de cada lado.
// A inclinação fica ≤ 1,5°.
const STICKY_CLASSES = `pointer-events-auto absolute bottom-full mb-3 -rotate-1 rounded-sm bg-mark-yellow p-3 text-ink shadow-paper ${STICKY_INK_VARS}`
const POSITION_CLASSES = 'left-1/2 -translate-x-1/2 w-80 max-w-[calc(100vw-2rem)]'
const DISMISS_LABEL = 'Dispensar'

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && !event.defaultPrevented && store.current !== null) {
    store.dismiss()
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div
    v-if="store.current && !store.panelOpen"
    role="status"
    :class="[STICKY_CLASSES, POSITION_CLASSES]"
  >
    <div class="flex items-start gap-2">
      <VokiaMascot v-if="store.current.mascot" :look="store.current.mascot" />
      <div class="min-w-0 flex-1">
        <p v-if="store.current.kind !== 'error'" class="text-tiny text-muted">{{ store.current.origin ?? store.current.source }}</p>
        <p class="text-label font-semibold text-ink">{{ store.current.title }}</p>
      </div>
      <UiButton
        variant="quiet"
        size="sm"
        :aria-label="DISMISS_LABEL"
        :title="DISMISS_LABEL"
        @press="store.dismiss()"
      >
        ×
      </UiButton>
    </div>
    <p v-if="store.current.text" class="text-caption text-muted">{{ store.current.text }}</p>
    <p v-if="store.confirmation !== null" class="mt-2 text-label font-semibold text-ok">
      {{ store.confirmation }}
    </p>
    <div v-else-if="store.current.actions.length > 0" class="mt-2 flex justify-end gap-2">
      <UiButton
        v-for="action in store.current.actions"
        :key="action.id"
        :variant="action.primary ? 'primary' : 'quiet'"
        size="sm"
        :disabled="action.disabled"
        @press="store.act(action.id)"
      >
        {{ action.label }}
      </UiButton>
    </div>
  </div>
</template>
