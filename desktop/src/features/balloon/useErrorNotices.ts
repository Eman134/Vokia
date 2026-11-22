import { watch } from 'vue'
import { defineStore } from 'pinia'
import { DEFAULT_RECIPIENT } from '../team/agents'
import { errorSummary } from '../team/errorSummary'
import { useFeed } from '../team/useFeed'
import { useRequests } from '../team/useRequests'
import { useProjects } from '../projects/useProjects'
import type { BalloonAction, BalloonItem } from './balloonTypes'
import { useBalloons } from './useBalloons'
import { buildWorktreeFixPrompt, FIX_LABEL, FIX_SENT_TEXT, worktreeConflictName } from './worktreeConflictFix'

const DISMISS_LABEL = 'Dispensar'
const ERROR_ID = (index: number) => `error:${index}`

function buildErrorBalloon(index: number, message: string, fix: BalloonAction | undefined): BalloonItem {
  const { title, detail } = errorSummary(message)
  const dismiss: BalloonAction = { id: 'dismiss', label: DISMISS_LABEL, primary: fix === undefined }
  return {
    id: ERROR_ID(index),
    kind: 'error',
    source: 'conversa',
    title,
    text: detail === '' ? undefined : detail,
    actions: fix ? [fix, dismiss] : [dismiss],
  }
}

const LIMIT_ERROR_TEXT = 'Limite esgotado'

// Erro sai do feed e vira balão que só fecha ao dispensar: o tipo 'error' não tem timer de sumir.
export const useErrorNotices = defineStore('errorNotices', () => {
  const feed = useFeed()
  const balloons = useBalloons()
  const requests = useRequests()
  const projects = useProjects()
  const announced = new WeakSet<object>()
  let count = 0

  // Conflito de nome de worktree: o pedido vai para a líder com o erro, para ela investigar e resolver.
  function fixFor(message: string): BalloonAction | undefined {
    const name = worktreeConflictName(message)
    if (name === undefined) return undefined
    return {
      id: 'fix',
      label: FIX_LABEL,
      primary: true,
      run: () => {
        void requests.send(buildWorktreeFixPrompt(name, message), DEFAULT_RECIPIENT, { projectId: projects.selected?.id })
        return FIX_SENT_TEXT
      },
    }
  }

  watch(
    () => feed.feed.length,
    () => {
      for (const entry of feed.feed) {
        if (entry.type !== 'error' || announced.has(entry)) continue
        announced.add(entry)
        if (entry.message === LIMIT_ERROR_TEXT) continue
        count += 1
        balloons.enqueue(buildErrorBalloon(count, entry.message, fixFor(entry.message)))
      }
    },
  )
})
