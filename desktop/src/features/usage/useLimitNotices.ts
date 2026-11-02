import { ref, watch, type WatchStopHandle } from 'vue'
import { defineStore } from 'pinia'
import { useBalloons } from '../balloon/useBalloons'
import { useUsage } from './useUsage'
import {
  LIMIT_LEVELS,
  buildLimitBalloon,
  levelOf,
  type LimitContext,
  type LimitLevel,
  type LimitNoticeKind,
} from './limitNotices'

export const useLimitNotices = defineStore('limitNotices', () => {
  const usage = useUsage()
  const balloons = useBalloons()
  const lastLevel = ref<LimitLevel>('none')
  let warnBalloonId: string | null = null
  let stopWatch: WatchStopHandle | null = null

  function rankOf(level: LimitLevel): number {
    return LIMIT_LEVELS.indexOf(level)
  }

  function show(kind: LimitNoticeKind, context: LimitContext): void {
    const item = buildLimitBalloon(kind, context)
    if (kind === 'warn75') {
      warnBalloonId = item.id
    }
    const replaced =
      kind === 'pause90' && warnBalloonId !== null && balloons.replace(warnBalloonId, item)
    if (!replaced) {
      balloons.enqueue(item)
    }
  }

  function onFiveHourChange(percent: number | null): void {
    const claude = usage.claude
    if (claude === null) {
      return
    }
    const previous = lastLevel.value
    const next = levelOf(percent)
    lastLevel.value = next
    const context: LimitContext = {
      fiveHourPercent: percent ?? 0,
      sevenDayPercent: claude.sevenDay.percent,
      resetsAt: claude.fiveHour.resetsAt,
      nowMs: usage.now,
    }
    if (previous === 'pause90' && next !== 'pause90') {
      show('renewed', context)
      return
    }
    if (rankOf(next) > rankOf(previous) && next !== 'none') {
      show(next, context)
    }
  }

  function start(): void {
    if (stopWatch !== null) {
      return
    }
    stopWatch = watch(() => usage.fiveHour, onFiveHourChange)
  }

  function stop(): void {
    stopWatch?.()
    stopWatch = null
  }

  return { start, stop }
})
