<script setup lang="ts">
import { THEME_CHOICES, useTheme, type ThemeChoice } from './useTheme'

const THEME_LABELS: Record<ThemeChoice, string> = {
  light: 'Claro',
  dark: 'Escuro',
  system: 'Sistema',
}

const theme = useTheme()
</script>

<template>
  <div class="flex gap-0.5 rounded-control bg-surface-2 p-0.75" role="group" aria-label="Tema">
    <button
      v-for="option in THEME_CHOICES"
      :key="option"
      type="button"
      class="cursor-pointer whitespace-nowrap rounded-control-sm px-3 py-1.25 text-caption"
      :class="
        theme.choice === option
          ? 'bg-surface font-medium text-ink shadow-seg'
          : 'text-muted'
      "
      :aria-pressed="theme.choice === option"
      @click="theme.choose(option)"
    >
      {{ THEME_LABELS[option] }}
    </button>
  </div>
</template>
