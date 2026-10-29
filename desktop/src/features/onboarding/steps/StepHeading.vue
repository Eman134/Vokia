<script setup lang="ts">
defineProps<{
  title: string
  text: string
}>()

const TITLE_CLASSES = 'text-[34px] font-bold leading-[1.1] tracking-[-0.03em] text-ink'
const TEXT_CLASSES = 'max-w-[46ch] text-[15px] leading-normal text-muted'
</script>

<template>
  <div class="flex flex-col items-center gap-3.5 text-center">
    <slot name="mascot" />
    <h1 :class="TITLE_CLASSES">{{ title }}</h1>
    <p :class="TEXT_CLASSES">{{ text }}</p>
  </div>
</template>
