<script setup lang="ts">
import UiButton from './UiButton.vue'

defineProps<{
  fact: string
  author: string
  usage: string
}>()
const emit = defineEmits<{ forget: [] }>()
</script>

<template>
  <article class="flex min-w-0 flex-col gap-3 rounded-card border border-line bg-surface p-4">
    <p class="text-caption font-medium text-muted">Memória · fato com autor e uso</p>
    <p class="text-body text-ink">{{ fact }}</p>
    <footer class="flex items-center justify-between gap-3">
      <span class="text-small text-muted">{{ author }} · {{ usage }}</span>
      <UiButton variant="quiet" size="sm" @press="emit('forget')">Esquecer</UiButton>
    </footer>
  </article>
</template>
