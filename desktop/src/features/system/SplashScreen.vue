<script lang="ts">
export const SPLASH_TEXT = 'Conectando Claude Code…'
</script>

<script setup lang="ts">
import VokiaSignature from '../brand/VokiaSignature.vue'
</script>

<template>
  <main class="flex h-screen flex-col items-center justify-center gap-4 bg-bg p-8 text-center text-ink">
    <VokiaSignature orientation="vertical" />
    <p class="text-body text-muted">{{ SPLASH_TEXT }}</p>
  </main>
</template>
