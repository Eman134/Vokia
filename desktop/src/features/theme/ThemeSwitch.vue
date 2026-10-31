<script setup lang="ts">
import UiSegmented from '../../ui/UiSegmented.vue'
import { THEME_CHOICES, useTheme, type ThemeChoice } from './useTheme'

const THEME_LABELS: Record<ThemeChoice, string> = {
  light: 'Claro',
  dark: 'Escuro',
  system: 'Sistema',
}
const ERROR_CLASSES = 'text-small text-danger'
const WRAPPER_CLASSES = 'flex flex-col items-end gap-1'

const theme = useTheme()
const options = THEME_CHOICES.map((choice) => ({ label: THEME_LABELS[choice], value: choice }))
</script>

<template>
  <div :class="WRAPPER_CLASSES">
    <UiSegmented
      :options="options"
      :model-value="theme.choice"
      label="Tema"
      @update:model-value="theme.choose"
    />
    <p v-if="theme.persistError" role="alert" :class="ERROR_CLASSES">{{ theme.persistError }}</p>
  </div>
</template>
