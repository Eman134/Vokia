import { ref } from 'vue'
import { defineStore } from 'pinia'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useNavigation } from '../../shell/useNavigation'
import type { MascotLook } from '../brand/mascotArt'
import { fetchSettings, MOMENTS_CONFIG_SETTING_KEY, saveSetting } from '../settings/settingsApi'
import { VOKIA_EVENT, type VokiaEvent } from '../team/events'
import type { BalloonSource } from './balloonTypes'
import { useBalloons } from './useBalloons'

export type MomentKind = 'memory' | 'graph' | 'decision' | 'skill' | 'conflict' | 'celebration'

export type MomentsPreferences = Record<MomentKind, boolean>

export interface MomentDefinition {
  kind: MomentKind
  label: string
  hint: string
  defaultEnabled: boolean
  mascot: MascotLook
  source: BalloonSource
}

export const MOMENT_DEFINITIONS: readonly MomentDefinition[] = [
  {
    kind: 'memory',
    label: 'Memória',
    hint: 'fato novo numa conversa',
    defaultEnabled: true,
    mascot: 'piscando',
    source: 'conversa',
  },
  {
    kind: 'graph',
    label: 'Grafo',
    hint: 'arquivos ou artefatos ligados',
    defaultEnabled: true,
    mascot: 'animado',
    source: 'grafo',
  },
  {
    kind: 'decision',
    label: 'Decisão',
    hint: 'algo que vira ADR',
    defaultEnabled: true,
    mascot: 'piscando',
    source: 'chat',
  },
  {
    kind: 'skill',
    label: 'Skill',
    hint: 'um jeito de trabalhar repetido',
    defaultEnabled: true,
    mascot: 'animado',
    source: 'conversa',
  },
  {
    kind: 'conflict',
    label: 'Conflito',
    hint: 'dois agentes no mesmo arquivo',
    defaultEnabled: true,
    mascot: 'precisa-de-voce',
    source: 'codigo',
  },
  {
    kind: 'celebration',
    label: 'Festa',
    hint: 'feature concluída',
    defaultEnabled: false,
    mascot: 'feliz',
    source: 'conversa',
  },
]

const DEFAULT_PREFERENCES: MomentsPreferences = {
  memory: true,
  graph: true,
  decision: true,
  skill: true,
  conflict: true,
  celebration: false,
}

// Após 3 dispensas seguidas do mesmo tipo na sessão, o momento silencia automaticamente para evitar fadiga.
const ADAPTIVE_DISMISS_THRESHOLD = 3
const DISMISS_ACTION_LABEL = 'Agora não'

export interface EmitMomentInput {
  kind: MomentKind
  dedupeKey: string
  title: string
  text: string
  actionLabel: string
  confirmText?: string
  onAccept: () => void | Promise<void>
}

export const useMoments = defineStore('moments', () => {
  const preferences = ref<MomentsPreferences>({ ...DEFAULT_PREFERENCES })
  const loaded = ref(false)
  const saveError = ref<string | null>(null)
  const emittedKeys = new Set<string>()
  const consecutiveDismissals = new Map<MomentKind, number>()
  let unlisten: UnlistenFn | null = null

  async function load(): Promise<void> {
    try {
      const settings = await fetchSettings()
      const raw = settings[MOMENTS_CONFIG_SETTING_KEY]
      if (raw) {
        const parsed = JSON.parse(raw) as Partial<Record<MomentKind, unknown>>
        const next: MomentsPreferences = { ...DEFAULT_PREFERENCES }
        for (const def of MOMENT_DEFINITIONS) {
          if (typeof parsed[def.kind] === 'boolean') {
            next[def.kind] = parsed[def.kind] as boolean
          }
        }
        preferences.value = next
      }
    } catch {
      // Mantém padrões quando ainda não há preferência salva.
    } finally {
      loaded.value = true
    }
  }

  async function setEnabled(kind: MomentKind, enabled: boolean): Promise<void> {
    const previous = preferences.value[kind]
    preferences.value = { ...preferences.value, [kind]: enabled }
    if (enabled) {
      consecutiveDismissals.delete(kind)
    }
    try {
      await saveSetting(MOMENTS_CONFIG_SETTING_KEY, JSON.stringify(preferences.value))
      saveError.value = null
    } catch (error) {
      preferences.value = { ...preferences.value, [kind]: previous }
      saveError.value = `Não foi possível salvar preferência de momento: ${String(error)}`
    }
  }

  function isKindActive(kind: MomentKind): boolean {
    if (!preferences.value[kind]) return false
    return (consecutiveDismissals.get(kind) ?? 0) < ADAPTIVE_DISMISS_THRESHOLD
  }

  function emitMoment(input: EmitMomentInput): boolean {
    if (!isKindActive(input.kind)) {
      return false
    }
    if (emittedKeys.has(input.dedupeKey)) {
      return false
    }
    emittedKeys.add(input.dedupeKey)

    const def = MOMENT_DEFINITIONS.find((entry) => entry.kind === input.kind)
    const balloons = useBalloons()
    balloons.enqueue({
      id: `moment:${input.kind}:${input.dedupeKey}`,
      kind: 'moment',
      source: def?.source ?? 'conversa',
      mascot: def?.mascot ?? 'animado',
      title: input.title,
      text: input.text,
      actions: [
        {
          id: 'accept',
          label: input.actionLabel,
          primary: true,
          run: async () => {
            consecutiveDismissals.set(input.kind, 0)
            await input.onAccept()
            return input.confirmText ?? 'Feito'
          },
        },
        {
          id: 'later',
          label: DISMISS_ACTION_LABEL,
          run: () => {
            consecutiveDismissals.set(
              input.kind,
              (consecutiveDismissals.get(input.kind) ?? 0) + 1,
            )
          },
        },
      ],
    })
    return true
  }

  function handleEvent(event: VokiaEvent): void {
    const navigation = useNavigation()
    if (event.type === 'backlog-changed') {
      return
    }
    if (event.type === 'error' && event.message.toLowerCase().includes('conflit')) {
      emitMoment({
        kind: 'conflict',
        dedupeKey: `conflict:${event.message.slice(0, 60)}`,
        title: 'Dois agentes podem colidir no mesmo arquivo',
        text: event.message,
        actionLabel: 'Ordenar no plano',
        confirmText: 'Aberto no Backlog',
        onAccept: () => {
          navigation.goTo('backlog')
        },
      })
    }
  }

  async function start(): Promise<void> {
    if (!loaded.value) {
      await load()
    }
    if (unlisten !== null) return
    unlisten = await listen<VokiaEvent>(VOKIA_EVENT, ({ payload }) => {
      handleEvent(payload)
    })
  }

  function stop(): void {
    unlisten?.()
    unlisten = null
  }

  return {
    preferences,
    loaded,
    saveError,
    load,
    setEnabled,
    isKindActive,
    emitMoment,
    start,
    stop,
  }
})
