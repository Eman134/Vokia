<script setup lang="ts">
import UiAnnotation from './UiAnnotation.vue'

defineProps<{
  title: string
  text?: string
  hint?: string
}>()
</script>

<template>
  <div class="flex flex-col items-center gap-2 text-center">
    <slot name="mascot" />
    <p class="text-section text-ink">{{ title }}</p>
    <p v-if="text" class="text-body text-muted">{{ text }}</p>
    <template v-if="$slots.action">
      <UiAnnotation v-if="hint" :text="hint" direction="down" />
      <slot name="action" />
    </template>
  </div>
</template>
