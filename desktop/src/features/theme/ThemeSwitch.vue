<script setup lang="ts">
import UiSegmented from '../../ui/UiSegmented.vue'
import { THEME_CHOICES, useTheme, type ThemeChoice } from './useTheme'

const THEME_LABELS: Record<ThemeChoice, string> = {
  light: 'Claro',
  dark: 'Escuro',
  system: 'Sistema',
}

const theme = useTheme()
const options = THEME_CHOICES.map((choice) => ({ label: THEME_LABELS[choice], value: choice }))
</script>

<template>
  <UiSegmented
    :options="options"
    :model-value="theme.choice"
    label="Tema"
    @update:model-value="theme.choose"
  />
</template>
