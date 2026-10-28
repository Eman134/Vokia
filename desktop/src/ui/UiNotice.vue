<script setup lang="ts">
import UiButton from './UiButton.vue'

defineProps<{
  text: string
  undoable?: boolean
}>()
const emit = defineEmits<{ undo: []; close: [] }>()
</script>

<template>
  <div class="flex min-w-0 items-center gap-3 rounded-control bg-surface-2 py-2 pl-3.5 pr-2 text-caption text-ink">
    <p class="min-w-0 flex-1">{{ text }}</p>
    <UiButton v-if="undoable" variant="quiet" size="sm" @press="emit('undo')">Desfazer</UiButton>
    <UiButton variant="quiet" size="sm" aria-label="Fechar" @press="emit('close')">×</UiButton>
  </div>
</template>
