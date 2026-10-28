<script setup lang="ts">
import UiButton from './UiButton.vue'

defineProps<{ title: string }>()
const emit = defineEmits<{ create: []; adjust: [] }>()
</script>

<template>
  <article class="flex min-w-0 flex-col gap-3 rounded-card border border-line bg-surface p-4">
    <p class="text-caption font-medium text-muted">Proposta para o kanban</p>
    <p class="text-lead text-ink">{{ title }}</p>
    <footer class="flex justify-end gap-2">
      <UiButton variant="quiet" size="sm" @press="emit('adjust')">Ajustar</UiButton>
      <UiButton variant="approve" size="sm" @press="emit('create')">Criar no kanban</UiButton>
    </footer>
  </article>
</template>
