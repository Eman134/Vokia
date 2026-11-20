import { computed, onScopeDispose, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { useDecisions } from '../decisions/useDecisions'
import { useUsage } from '../usage/useUsage'
import { useUsageLimit } from '../usage/useUsageLimit'
import { usageLimitText } from '../usage/usageLimitCopy'
import { useFeed } from './useFeed'
import {
  DONE_HOLD_MS,
  errorCopy,
  needsYouCopy,
  resolveTeamStatus,
  STATUS_COPY,
  STATUS_RING,
  type TeamStatus,
  workingCopy,
} from './status'

const DONE_TICK_MS = 1000

export const useTeamStatus = defineStore('teamStatus', () => {
  const feed = useFeed()
  const usage = useUsage()
  const usageLimit = useUsageLimit()
  const decisions = useDecisions()
  const now = ref(Date.now())
  const doneUntil = ref(0)
  let doneTimer: ReturnType<typeof setInterval> | null = null

  // Mensagens do usuário também entram no feed; só erro ou resposta de agente mudam o estado.
  const outcomes = computed(() =>
    feed.feed.filter((entry) => entry.type === 'error' || (entry.type === 'message' && entry.from !== 'user')),
  )
  const lastOutcome = computed(() => outcomes.value.at(-1))
  const lastIsError = computed(() => lastOutcome.value?.type === 'error')

  const status = computed<TeamStatus>(() =>
    resolveTeamStatus({
      fiveHour: usage.fiveHour,
      lastIsError: lastIsError.value,
      pendingDecisions: decisions.pendingCount,
      activeAgents: feed.activeAgents,
      doneUntil: doneUntil.value,
      now: now.value,
    }),
  )
  const copy = computed(() => {
    if (status.value === 'working') {
      return workingCopy(feed.activeAgents)
    }
    if (status.value === 'error') {
      const last = lastOutcome.value
      return errorCopy(last?.type === 'error' ? last.message : '')
    }
    return status.value === 'needs-you' ? needsYouCopy() : STATUS_COPY[status.value]
  })
  const limitExhausted = computed(() => status.value !== 'working' && usageLimit.visible && usageLimit.notice !== null)
  const ring = computed(() => (limitExhausted.value ? 'stroke-warn' : STATUS_RING[status.value]))
  const statusText = computed(() => {
    if (limitExhausted.value && usageLimit.notice !== null) {
      return usageLimitText(usageLimit.notice, usage.now)
    }
    return [copy.value.status, copy.value.detail].filter(Boolean).join(' · ')
  })
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
    stopDoneTimer()
    now.value = Date.now()
    doneUntil.value = now.value + DONE_HOLD_MS
    doneTimer = setInterval(tickDone, DONE_TICK_MS)
  }

  function lastIsAgentReply(): boolean {
    const last = feed.feed.at(-1)
    return last?.type === 'message' && last.to === 'user' && !last.isError && last.text !== 'Limite esgotado'
  }

  watch(
    () => feed.activeAgents.length,
    (count, previous) => {
      if (count > 0 && previous === 0) {
        stopDoneTimer()
        doneUntil.value = 0
      } else if (count === 0 && previous > 0 && lastIsAgentReply()) {
        startDone()
      }
    },
  )

  watch(status, (current) => {
    if (current !== 'done') {
      stopDoneTimer()
      doneUntil.value = 0
    }
  })

  onScopeDispose(stopDoneTimer)

  return { status, ring, limitExhausted, statusText, placeholder }
})
