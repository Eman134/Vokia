<script setup lang="ts">
import { THEME_CHOICES, useTheme, type ThemeChoice } from '../stores/useTheme'

const THEME_LABELS: Record<ThemeChoice, string> = {
  light: 'Claro',
  dark: 'Escuro',
  system: 'Sistema',
}

const theme = useTheme()
</script>

<template>
  <div class="flex gap-0.5 rounded-xl bg-surface-2 p-[3px]" role="group" aria-label="Tema">
    <button
      v-for="option in THEME_CHOICES"
      :key="option"
      type="button"
      class="h-[30px] cursor-pointer whitespace-nowrap rounded-[9px] px-[11px] text-[12.5px]"
      :class="
        theme.choice === option
          ? 'bg-surface font-semibold text-ink shadow-[0_1px_2px_rgba(0,0,0,0.08)]'
          : 'text-muted'
      "
      :aria-pressed="theme.choice === option"
      @click="theme.choose(option)"
    >
      {{ THEME_LABELS[option] }}
    </button>
  </div>
</template>
