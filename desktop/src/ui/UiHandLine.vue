<script setup lang="ts">
const HAND_PATH = 'M1 3.4C14 2 22 4.6 36 3.2S58 2.2 70 3.6 88 4.4 99 2.8'
</script>

<template>
  <svg viewBox="0 0 100 6" preserveAspectRatio="none" fill="none" stroke="currentColor" aria-hidden="true">
    <path :d="HAND_PATH" stroke-width="1.4" stroke-linecap="round" vector-effect="non-scaling-stroke" />
  </svg>
</template>
