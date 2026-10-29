<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useBalloons } from './useBalloons'
import UiButton from '../../ui/UiButton.vue'

const store = useBalloons()

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
    class="pointer-events-auto absolute bottom-full mb-3 w-80 rounded-popover border border-line bg-surface p-3 shadow-float"
  >
    <p class="text-tiny text-muted">{{ store.current.origin ?? store.current.source }}</p>
    <p class="text-label font-semibold text-ink">{{ store.current.title }}</p>
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
