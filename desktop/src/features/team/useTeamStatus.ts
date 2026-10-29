import { computed, onScopeDispose, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { useUsage } from '../usage/useUsage'
import { AGENTS, useTeam } from './useTeam'
import {
  DONE_HOLD_MS,
  resolveTeamStatus,
  STATUS_COPY,
  STATUS_RING,
  type TeamStatus,
  workingCopy,
} from './teamStatus'

const DONE_TICK_MS = 1000

export const useTeamStatus = defineStore('teamStatus', () => {
  const team = useTeam()
  const usage = useUsage()
  const now = ref(Date.now())
  const doneUntil = ref(0)
  let doneTimer: ReturnType<typeof setInterval> | null = null

  const activeAgents = computed(() => AGENTS.filter((agent) => team.states[agent.id] !== 'idle'))
  // Mensagens do usuário também entram no feed; só erro ou resposta de agente mudam o estado.
  const outcomes = computed(() =>
    team.feed.filter((entry) => entry.type === 'error' || (entry.type === 'message' && entry.from !== 'user')),
  )
  const lastIsError = computed(() => outcomes.value.at(-1)?.type === 'error')

  const status = computed<TeamStatus>(() =>
    resolveTeamStatus({
      fiveHour: usage.fiveHour,
      lastIsError: lastIsError.value,
      activeAgents: activeAgents.value,
      doneUntil: doneUntil.value,
      now: now.value,
    }),
  )
  const copy = computed(() =>
    status.value === 'working' ? workingCopy(activeAgents.value) : STATUS_COPY[status.value],
  )
  const ring = computed(() => STATUS_RING[status.value])
  const statusText = computed(() => `${copy.value.status} · ${copy.value.detail}`)
  const placeholder = computed(() => copy.value.placeholder)

  function stopDoneTimer(): void {
    if (doneTimer !== null) {
      clearInterval(doneTimer)
      doneTimer = null
    }
  }

  function tickDone(): void {
    now.value = Date.now()
    if (now.value >= doneUntil.value) {
      stopDoneTimer()
    }
  }

  function startDone(): void {
    now.value = Date.now()
    doneUntil.value = now.value + DONE_HOLD_MS
    doneTimer = setInterval(tickDone, DONE_TICK_MS)
  }

  function lastIsAgentReply(): boolean {
    const last = team.feed.at(-1)
    return last?.type === 'message' && last.to === 'user'
  }

  watch(
    () => activeAgents.value.length,
    (count, previous) => {
      if (count > 0 && previous === 0) {
        stopDoneTimer()
        doneUntil.value = 0
      } else if (count === 0 && previous > 0 && lastIsAgentReply()) {
        startDone()
      }
    },
  )

  onScopeDispose(stopDoneTimer)

  return { status, ring, statusText, placeholder }
})
