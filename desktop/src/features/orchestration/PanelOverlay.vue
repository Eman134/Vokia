<script setup lang="ts">
const emit = defineEmits<{ close: [] }>()
</script>

<template>
  <!-- Janela sobre o canvas: o card do painel é o próprio diálogo; clicar fora ou Esc (tratado pela tela) fecha. -->
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4" @click.self="emit('close')">
    <div role="dialog" aria-modal="true" class="max-h-full w-120 max-w-full overflow-y-auto">
      <slot />
    </div>
  </div>
</template>
