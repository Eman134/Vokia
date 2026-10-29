<script setup lang="ts">
defineProps<{ selected: boolean }>()

const CARD_CLASSES = 'flex w-full items-center gap-3 rounded-card px-4 py-3.5 text-left text-ink'
const SELECTED_CLASSES = 'border-[1.5px] border-accent bg-accent-soft'
const IDLE_CLASSES = 'border border-line bg-surface'
// Sem interação no onboarding de exemplo: o cartão só mostra a escolha inicial do protótipo.
const RADIO_CLASSES = 'ml-auto size-4 flex-none rounded-full box-border'
const RADIO_SELECTED_CLASSES = 'border-[5px] border-accent'
const RADIO_IDLE_CLASSES = 'border-[1.5px] border-faint'
</script>

<template>
  <button type="button" disabled :class="[CARD_CLASSES, selected ? SELECTED_CLASSES : IDLE_CLASSES]">
    <slot />
    <span
      aria-hidden="true"
      :class="[RADIO_CLASSES, selected ? RADIO_SELECTED_CLASSES : RADIO_IDLE_CLASSES]"
    />
  </button>
</template>
