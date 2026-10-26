import { ref } from 'vue'
import { defineStore } from 'pinia'

export const THEME_CHOICES = ['light', 'dark', 'system'] as const

export type ThemeChoice = (typeof THEME_CHOICES)[number]

export const useTheme = defineStore('theme', () => {
  const choice = ref<ThemeChoice>('system')

  function choose(next: ThemeChoice): void {
    choice.value = next
    const root = document.documentElement
    if (next === 'system') {
      root.removeAttribute('data-theme')
    } else {
      root.dataset.theme = next
    }
  }

  return { choice, choose }
})
