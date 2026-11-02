import { ref } from 'vue'
import { defineStore } from 'pinia'
import { THEME_SETTING_KEY, fetchSettings, saveSetting } from '../settings/settingsApi'

export const THEME_CHOICES = ['light', 'dark', 'system'] as const

export type ThemeChoice = (typeof THEME_CHOICES)[number]

const DEFAULT_THEME: ThemeChoice = 'system'

function isThemeChoice(value: string | undefined): value is ThemeChoice {
  return THEME_CHOICES.some((choice) => choice === value)
}

function applyTheme(theme: ThemeChoice): void {
  const root = document.documentElement
  if (theme === 'system') {
    root.removeAttribute('data-theme')
  } else {
    root.dataset.theme = theme
  }
}

export const useTheme = defineStore('theme', () => {
  const choice = ref<ThemeChoice>(DEFAULT_THEME)
  const persistError = ref<string | null>(null)

  async function load(): Promise<void> {
    try {
      const stored = (await fetchSettings())[THEME_SETTING_KEY]
      const theme = isThemeChoice(stored) ? stored : DEFAULT_THEME
      choice.value = theme
      applyTheme(theme)
    } catch (error) {
      // O motivo do banco indisponível já aparece no feed (useTeam.restoreHistory); aqui só mantém o tema padrão.
      console.error(`Não foi possível ler o tema salvo: ${String(error)}`)
    }
  }

  async function choose(next: ThemeChoice): Promise<void> {
    choice.value = next
    applyTheme(next)
    try {
      await saveSetting(THEME_SETTING_KEY, next)
      persistError.value = null
    } catch (error) {
      persistError.value = `Não foi possível salvar o tema: ${String(error)}`
    }
  }

  return { choice, persistError, load, choose }
})
