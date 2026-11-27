import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from './events'
import type { ConversationScreen } from './timeline'

export type ComposerScope = 'conversation' | 'bar'

export interface SavedDraft {
  text: string
  confirmed: Agent[]
}

const STORAGE_KEY = 'vokia:composer-drafts:v1'
const MAX_STORED_KEYS = 40

function loadFromStorage(): Record<string, SavedDraft> {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw) as Record<string, unknown>
    if (!parsed || typeof parsed !== 'object') return {}
    const result: Record<string, SavedDraft> = {}
    for (const [key, value] of Object.entries(parsed)) {
      if (
        value &&
        typeof value === 'object' &&
        typeof (value as SavedDraft).text === 'string'
      ) {
        const confirmed = Array.isArray((value as SavedDraft).confirmed)
          ? (value as SavedDraft).confirmed.filter((item): item is Agent => typeof item === 'string')
          : []
        if ((value as SavedDraft).text !== '' || confirmed.length > 0) {
          result[key] = { text: (value as SavedDraft).text, confirmed }
        }
      }
    }
    return result
  } catch {
    return {}
  }
}

function persistToStorage(map: Record<string, SavedDraft>): void {
  try {
    const entries = Object.entries(map).slice(-MAX_STORED_KEYS)
    if (entries.length === 0) {
      window.localStorage.removeItem(STORAGE_KEY)
      return
    }
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(Object.fromEntries(entries)))
  } catch {
    // Ignora falha de quota/acesso ao localStorage: o rascunho segue em memória.
  }
}

export function composerDraftKey(
  scope: ComposerScope,
  projectId: number | null | undefined,
  screen: ConversationScreen,
): string {
  if (scope === 'bar') {
    return `bar:${projectId ?? 'none'}`
  }
  const projectPart = projectId ?? 'none'
  switch (screen.kind) {
    case 'agent':
      return `conv:${projectPart}:agent:${screen.agent}`
    case 'channel':
      return `conv:${projectPart}:channel:${screen.channel}`
    case 'thread':
      return `conv:${projectPart}:thread:${screen.itemId}`
    case 'triage':
      return `conv:${projectPart}:triage`
    case 'logs':
      return `conv:${projectPart}:logs`
    case 'all':
    default:
      return `conv:${projectPart}:all`
  }
}

export const useConversationDrafts = defineStore('conversationDrafts', () => {
  const drafts = ref<Record<string, SavedDraft>>(loadFromStorage())

  function getText(key: string): string {
    return drafts.value[key]?.text ?? ''
  }

  function getConfirmed(key: string): Set<Agent> {
    return new Set(drafts.value[key]?.confirmed ?? [])
  }

  function setText(key: string, text: string): void {
    const current = drafts.value[key]
    const confirmed = current?.confirmed ?? []
    if (text === '' && confirmed.length === 0) {
      clear(key)
      return
    }
    const next = { ...drafts.value, [key]: { text, confirmed } }
    drafts.value = next
    persistToStorage(next)
  }

  function setConfirmed(key: string, confirmedSet: ReadonlySet<Agent>): void {
    const current = drafts.value[key]
    const text = current?.text ?? ''
    const confirmed = [...confirmedSet]
    if (text === '' && confirmed.length === 0) {
      clear(key)
      return
    }
    const next = { ...drafts.value, [key]: { text, confirmed } }
    drafts.value = next
    persistToStorage(next)
  }

  function clear(key: string): void {
    if (drafts.value[key] === undefined) return
    const next = { ...drafts.value }
    delete next[key]
    drafts.value = next
    persistToStorage(next)
  }

  return {
    drafts,
    getText,
    getConfirmed,
    setText,
    setConfirmed,
    clear,
  }
})
