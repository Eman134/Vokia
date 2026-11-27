import { ref } from 'vue'
import { defineStore } from 'pinia'
import {
  checkConnection,
  listenConnectionChanges,
  listenTeamEvents,
  type UnlistenFn,
} from './teamApi'
import { useDecisions } from '../decisions/useDecisions'
import { fetchDbProblem, type DbProblem } from '../projects/projectsApi'
import { useFeed } from './useFeed'
import { useRoster } from './useRoster'

const CONNECTIVITY_POLL_INTERVAL_MS = 15000

export const useConnection = defineStore('connection', () => {
  const ready = ref(false)
  const startupError = ref<string | null>(null)
  const online = ref(true)
  const checking = ref(false)
  const roster = useRoster()
  const feed = useFeed()
  let offlineSignals = 0
  let unlisten: UnlistenFn | null = null
  let stopListeningConnection: (() => void) | null = null
  let pollTimer: ReturnType<typeof setInterval> | null = null
  let pendingCheck: Promise<boolean> | null = null

  function refreshConnection(): Promise<boolean> {
    if (pendingCheck === null) {
      const offlineSignalsAtStart = offlineSignals
      pendingCheck = checkConnection()
        .then((reachable) => {
          const stillOnline = reachable && offlineSignalsAtStart === offlineSignals
          online.value = stillOnline
          return stillOnline
        })
        .finally(() => {
          pendingCheck = null
        })
    }
    return pendingCheck
  }

  async function confirmOnline(): Promise<boolean> {
    checking.value = true
    try {
      return await refreshConnection()
    } finally {
      checking.value = false
    }
  }

  // Nunca lança: uma falha da própria consulta vai ao feed e vale como "sem problema de banco" para quem chama.
  async function checkDbProblem(): Promise<DbProblem | null> {
    try {
      return await fetchDbProblem()
    } catch (error) {
      feed.feed.push({ type: 'error', message: String(error) })
      return null
    }
  }

  // Com banco indisponível, o motivo do banco é a mensagem única: restoreHistory (CLI ok) ou reportDbProblem (CLI com problema).
  async function loadRoster(): Promise<void> {
    try {
      await roster.load()
    } catch (error) {
      if ((await checkDbProblem()) === null) {
        feed.feed.push({ type: 'error', message: String(error) })
      }
    }
  }

  async function start(): Promise<void> {
    try {
      await loadRoster()
      await feed.restoreHistory()
      unlisten = await listenTeamEvents(feed.apply)
      stopListeningConnection = listenConnectionChanges(
        () => void refreshConnection(),
        () => {
          offlineSignals += 1
          online.value = false
        },
      )
      void refreshConnection()
      pollTimer = setInterval(() => void refreshConnection(), CONNECTIVITY_POLL_INTERVAL_MS)
      ready.value = true
    } catch (error) {
      startupError.value = `Não foi possível iniciar o Vokia: ${String(error)}`
    }
  }

  function stop(): void {
    unlisten?.()
    unlisten = null
    stopListeningConnection?.()
    stopListeningConnection = null
    useDecisions().stop()
    if (pollTimer !== null) {
      clearInterval(pollTimer)
      pollTimer = null
    }
  }

  return {
    ready,
    startupError,
    online,
    checking,
    refreshConnection,
    confirmOnline,
    start,
    stop,
  }
})
