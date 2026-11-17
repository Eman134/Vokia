import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { fetchSettings, saveSetting, USER_AVATAR_SETTING_KEY, USER_NAME_SETTING_KEY } from '../settings/settingsApi'
import { findUserIcon, type UserAvatarChoice } from './userIcons'

export const USER_NAME_MIN_CHARS = 2
export const USER_NAME_MAX_CHARS = 20
const DEFAULT_DISPLAY_NAME = 'Você'
const DEFAULT_INITIAL = 'V'

export function isUserNameValid(name: string): boolean {
  const length = [...name.trim()].length
  return length >= USER_NAME_MIN_CHARS && length <= USER_NAME_MAX_CHARS && !/[\r\n]/.test(name)
}

export function initialOf(name: string): string {
  return ([...name.trim()][0] ?? DEFAULT_INITIAL).toUpperCase()
}

function parseAvatar(raw: string | undefined): UserAvatarChoice | null {
  if (raw === undefined) return null
  try {
    const parsed: unknown = JSON.parse(raw)
    if (typeof parsed !== 'object' || parsed === null) return null
    const { icone } = parsed as Record<string, unknown>
    if (typeof icone === 'string' && findUserIcon(icone)) return { icone }
    console.warn('Foto do perfil salva não existe mais; usando a inicial.')
  } catch {
    console.warn('Foto do perfil salva ilegível; usando a inicial.')
  }
  return null
}

export const useUserProfile = defineStore('userProfile', () => {
  const name = ref<string | null>(null)
  const avatar = ref<UserAvatarChoice | null>(null)
  // Falso enquanto get_settings não respondeu ou falhou: sem isso, erro de leitura pareceria "nome não existe".
  const loaded = ref(false)

  const displayName = computed(() => name.value ?? DEFAULT_DISPLAY_NAME)
  const initial = computed(() => initialOf(name.value ?? ''))
  const needsProfile = computed(() => loaded.value && name.value === null)

  async function load(): Promise<void> {
    try {
      const settings = await fetchSettings()
      const stored = settings[USER_NAME_SETTING_KEY]?.trim()
      name.value = stored ? stored : null
      avatar.value = parseAvatar(settings[USER_AVATAR_SETTING_KEY])
      loaded.value = true
    } catch (error) {
      console.error(`Não foi possível ler o perfil salvo: ${String(error)}`)
    }
  }

  // O nome vai por último: sem ele o perfil continua "inexistente" (needsProfile), então uma falha no meio
  // não deixa um perfil pela metade; a foto anterior é restaurada para o caso de edição.
  async function save(nextName: string, nextAvatar: UserAvatarChoice | null): Promise<void> {
    const trimmed = nextName.trim()
    const previousAvatar = JSON.stringify(avatar.value)
    try {
      await saveSetting(USER_AVATAR_SETTING_KEY, JSON.stringify(nextAvatar))
    } catch (error) {
      throw new Error(`Não foi possível salvar a foto do perfil: ${String(error)}`)
    }
    try {
      await saveSetting(USER_NAME_SETTING_KEY, trimmed)
    } catch (error) {
      await saveSetting(USER_AVATAR_SETTING_KEY, previousAvatar).catch((revertError) =>
        console.error(`Não foi possível restaurar a foto anterior: ${String(revertError)}`),
      )
      throw new Error(`Não foi possível salvar o nome do perfil: ${String(error)}`)
    }
    name.value = trimmed
    avatar.value = nextAvatar
  }

  return { name, avatar, loaded, displayName, initial, needsProfile, load, save }
})
