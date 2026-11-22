<script lang="ts">
export const SPLASH_TEXT = 'Conectando Claude Code…'
</script>

<script setup lang="ts">
import VokiaLogo from '../brand/VokiaLogo.vue'

const LOGO_SIZE = 64
</script>

<template>
  <main class="flex h-screen flex-col items-center justify-center gap-4 bg-bg p-8 text-center text-ink">
    <VokiaLogo variant="contorno" :size="LOGO_SIZE" class="splash-draw text-ink" />
    <span class="text-lead">Vokia</span>
    <p class="text-body text-muted">{{ SPLASH_TEXT }}</p>
  </main>
</template>

<style scoped>
/* O contorno do logo aparece da esquerda para a direita, uma vez; menos de 600 ms. */
.splash-draw {
  animation: splash-draw 560ms ease-out 1 both;
}

@keyframes splash-draw {
  from {
    clip-path: inset(0 100% 0 0);
  }
  to {
    clip-path: inset(0 0 0 0);
  }
}

@media (prefers-reduced-motion: reduce) {
  .splash-draw {
    animation: none;
  }
}
</style>
