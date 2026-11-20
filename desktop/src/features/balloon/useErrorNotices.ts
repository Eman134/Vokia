import { watch } from 'vue'
import { defineStore } from 'pinia'
import { errorSummary } from '../team/errorSummary'
import { useFeed } from '../team/useFeed'
import type { BalloonItem } from './balloonTypes'
import { useBalloons } from './useBalloons'

const DISMISS_LABEL = 'Dispensar'
const ERROR_ID = (index: number) => `error:${index}`

function buildErrorBalloon(index: number, message: string): BalloonItem {
  const { title, detail } = errorSummary(message)
  return {
    id: ERROR_ID(index),
    kind: 'error',
    source: 'conversa',
    title,
    text: detail === '' ? undefined : detail,
    actions: [{ id: 'dismiss', label: DISMISS_LABEL, primary: true }],
  }
}

// Erro sai do feed e vira balão que só fecha ao dispensar: o tipo 'error' não tem timer de sumir.
export const useErrorNotices = defineStore('errorNotices', () => {
  const feed = useFeed()
  const balloons = useBalloons()
  const announced = new WeakSet<object>()
  let count = 0

  watch(
    () => feed.feed.length,
    () => {
      for (const entry of feed.feed) {
        if (entry.type !== 'error' || announced.has(entry)) continue
        announced.add(entry)
        count += 1
        balloons.enqueue(buildErrorBalloon(count, entry.message))
      }
    },
  )
})
