<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useBalloons } from './useBalloons'
import UiButton from '../../ui/UiButton.vue'
import VokiaMascot from '../brand/VokiaMascot.vue'

const store = useBalloons()
// Centralizado sobre a barra; a largura cabe na tela com 16 px de margem de cada lado.
const POSITION_CLASSES = 'left-1/2 -translate-x-1/2 w-80 max-w-[calc(100vw-2rem)]'

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
    :class="['pointer-events-auto absolute bottom-full mb-3 rounded-popover border border-line bg-surface p-3 shadow-float', POSITION_CLASSES]"
  >
    <div class="flex items-start gap-2">
      <VokiaMascot v-if="store.current.mascot" :look="store.current.mascot" />
      <div class="min-w-0">
        <p class="text-tiny text-muted">{{ store.current.origin ?? store.current.source }}</p>
        <p class="text-label font-semibold text-ink">{{ store.current.title }}</p>
      </div>
    </div>
    <p v-if="store.current.text" class="text-caption text-muted">{{ store.current.text }}</p>
    <p v-if="store.confirmation !== null" class="mt-2 text-label font-semibold text-ok">
      {{ store.confirmation }}
    </p>
    <div v-else class="mt-2 flex justify-end gap-2">
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
