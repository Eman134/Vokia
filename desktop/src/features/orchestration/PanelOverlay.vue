<script setup lang="ts">
import UiCloseButton from '../../ui/UiCloseButton.vue'

const emit = defineEmits<{ close: [] }>()
</script>

<template>
  <!-- Janela sobre o canvas: o card do painel é o próprio diálogo; clicar fora ou Esc (tratado pela tela) fecha. -->
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-paper/70 p-4" @click.self="emit('close')">
    <div role="dialog" aria-modal="true" class="relative max-h-full w-120 max-w-full overflow-y-auto rounded-md">
      <UiCloseButton class="absolute right-3 top-3 z-10" @press="emit('close')" />
      <slot />
    </div>
  </div>
</template>
