import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { AVATAR_PROFILES_SETTING_KEY, fetchSettings, saveSetting } from '../settings/settingsApi'
import { normalizeName } from '../team/useRoster'
import {
  PROFILE_COMBINATION_INVALID,
  PROFILE_LOAD_BAD_FORMAT,
  PROFILE_NAME_INVALID,
  PROFILE_NAME_IN_USE,
  profileLoadUnreadable,
  profileSaveFailed,
  profilesDiscarded,
} from './avatarPickerCopy'
import { AVATAR_PROFILES, isRenderable, type AvatarExpression, type AvatarProfile, type AvatarShapeId } from './avatarProfiles'

const STORAGE_VERSION = 1
const PROFILE_NAME_MIN_CHARS = 2
const PROFILE_NAME_MAX_CHARS = 20

export class AvatarProfileError extends Error {}

export interface AvatarProfileDraft {
  nome: string
  forma: AvatarShapeId
  cor: string
  expressao: AvatarExpression
}

export function slugifyProfileName(name: string): string {
  return normalizeName(name)
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

function isNameLengthValid(nome: string): boolean {
  return nome.length >= PROFILE_NAME_MIN_CHARS && nome.length <= PROFILE_NAME_MAX_CHARS
}

function isValidStored(entry: unknown, takenIds: Set<string>): entry is AvatarProfile {
  if (typeof entry !== 'object' || entry === null) return false
  const { id, nome, forma, cor, expressao } = entry as Record<string, unknown>
  if (![id, nome, forma, cor, expressao].every((field) => typeof field === 'string')) return false
  const profile = entry as AvatarProfile
  if (!isNameLengthValid(profile.nome) || profile.id !== slugifyProfileName(profile.nome)) return false
  if (profile.id.length < PROFILE_NAME_MIN_CHARS || takenIds.has(profile.id) || !isRenderable(profile)) return false
  takenIds.add(profile.id)
  return true
}

function parseStoredProfiles(raw: string): { profiles: AvatarProfile[]; discarded: number } {
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    throw new Error(PROFILE_LOAD_BAD_FORMAT)
  }
  if (typeof parsed !== 'object' || parsed === null) throw new Error(PROFILE_LOAD_BAD_FORMAT)
  const { version, profiles } = parsed as Record<string, unknown>
  if (version !== STORAGE_VERSION || !Array.isArray(profiles)) throw new Error(PROFILE_LOAD_BAD_FORMAT)
  const takenIds = new Set(AVATAR_PROFILES.map((profile) => profile.id))
  const valid = profiles.filter((entry): entry is AvatarProfile => isValidStored(entry, takenIds))
  return { profiles: valid, discarded: profiles.length - valid.length }
}

export const useAvatarProfiles = defineStore('avatarProfiles', () => {
  const profiles = ref<AvatarProfile[]>([])
  const loadNotice = ref<string | null>(null)
  // Perfis salvos ilegíveis: gravar a lista só com o novo perfil apagaria os que existem na chave.
  let loadFailure: string | null = null
  const all = computed(() => [...AVATAR_PROFILES, ...profiles.value])

  async function load(): Promise<void> {
    try {
      const stored = (await fetchSettings())[AVATAR_PROFILES_SETTING_KEY]
      loadFailure = null
      if (stored === undefined) {
        profiles.value = []
        loadNotice.value = null
        return
      }
      const parsed = parseStoredProfiles(stored)
      profiles.value = parsed.profiles
      loadNotice.value = parsed.discarded > 0 ? profilesDiscarded(parsed.discarded) : null
    } catch (error) {
      profiles.value = []
      loadFailure = profileLoadUnreadable(error instanceof Error ? error.message : String(error))
      loadNotice.value = loadFailure
    }
  }

  async function create(draft: AvatarProfileDraft): Promise<AvatarProfile> {
    if (loadFailure !== null) throw new AvatarProfileError(loadFailure)
    const nome = draft.nome.trim()
    const id = slugifyProfileName(nome)
    if (!isNameLengthValid(nome) || id.length < PROFILE_NAME_MIN_CHARS) throw new AvatarProfileError(PROFILE_NAME_INVALID)
    if (all.value.some((existing) => existing.id === id)) throw new AvatarProfileError(PROFILE_NAME_IN_USE)
    const profile: AvatarProfile = { id, nome, forma: draft.forma, cor: draft.cor, expressao: draft.expressao }
    if (!isRenderable(profile)) throw new AvatarProfileError(PROFILE_COMBINATION_INVALID)
    const next = [...profiles.value, profile]
    try {
      await saveSetting(AVATAR_PROFILES_SETTING_KEY, JSON.stringify({ version: STORAGE_VERSION, profiles: next }))
    } catch (error) {
      throw new AvatarProfileError(profileSaveFailed(String(error)))
    }
    profiles.value = next
    return profile
  }

  function dismissLoadNotice(): void {
    loadNotice.value = null
  }

  return { profiles, loadNotice, all, load, create, dismissLoadNotice }
})
