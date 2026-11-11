import { defineStore } from 'pinia'
import { useNavigation } from '../../shell/useNavigation'
import type { Agent } from '../team/events'
import { agentName, useTeam } from '../team/useTeam'
import type { BalloonItem } from './balloonTypes'
import { useBalloons } from './useBalloons'

const SENT_MEMORY_LIMIT = 20
const SENT_TITLE = (agent: string) => `Mandado para ${agent}`
const SENT_TEXT_STARTED = 'Já começou. Você vê o andamento nas Conversas.'
const SENT_TEXT_QUEUED = (position: number) =>
  `Entrou na fila, em ${position}º. Você vê o andamento nas Conversas.`
const SENT_TEXT_PAUSED = (agent: string) =>
  `Pausado para ${agent}: o pedido sai da fila quando você retomar.`
const SENT_TEXT_BACKEND_QUEUED = (agent: string, position: number) =>
  `Entrou na fila de ${agent}, em ${position}º. Já foi enviado e não dá para desfazer. Você vê o andamento nas Conversas.`
const ACTION_VIEW = 'Ver conversa'
const ACTION_UNDO = 'Desfazer'
const UNDONE_TEXT = 'Pedido retirado da fila'
const ALREADY_STARTED_TEXT = 'Já começou, não dá para desfazer'

interface SentInfo {
  id: number
  to: Agent
  ahead: number | null
  dispatched: boolean
  externalBusy: boolean
}

const sentBalloonId = (id: number) => `request:sent:${id}`

export const useRequestNotices = defineStore('requestNotices', () => {
  const balloons = useBalloons()
  const team = useTeam()
  const navigation = useNavigation()
  const sent = new Map<number, SentInfo>()

  function sentText(info: SentInfo): string {
    if (info.dispatched && !info.externalBusy) return SENT_TEXT_STARTED
    if (info.dispatched) return SENT_TEXT_BACKEND_QUEUED(agentName(info.to), (info.ahead ?? 0) + 1)
    if (info.ahead === null) return SENT_TEXT_PAUSED(agentName(info.to))
    return SENT_TEXT_QUEUED(info.ahead + 1)
  }

  function buildSentBalloon(info: SentInfo, text: string): BalloonItem {
    return {
      id: sentBalloonId(info.id),
      kind: 'request',
      source: 'conversa',
      title: SENT_TITLE(agentName(info.to)),
      text,
      actions: [
        { id: 'view', label: ACTION_VIEW, primary: true, run: () => navigation.goTo('conversas') },
        {
          id: 'undo',
          label: ACTION_UNDO,
          disabled: info.dispatched,
          run: () => {
            if (!team.queue.some((request) => request.id === info.id)) return ALREADY_STARTED_TEXT
            team.removeQueued(info.id)
            return UNDONE_TEXT
          },
        },
      ],
    }
  }

  function remember(info: SentInfo): void {
    sent.set(info.id, info)
    if (sent.size <= SENT_MEMORY_LIMIT) return
    const oldest = sent.keys().next().value
    if (oldest !== undefined) sent.delete(oldest)
  }

  function announceSent(info: SentInfo): void {
    // Conversas mostra o feed de todos os agentes: o pedido já aparece ali, o balão seria redundante.
    if (navigation.page === 'conversas') return
    remember(info)
    balloons.enqueue(buildSentBalloon(info, sentText(info)))
  }

  function markBackendQueued(id: number): void {
    const info = sent.get(id)
    if (info === undefined || !info.dispatched || info.externalBusy) return
    const corrected: SentInfo = { ...info, externalBusy: true }
    remember(corrected)
    const text = SENT_TEXT_BACKEND_QUEUED(agentName(info.to), (info.ahead ?? 0) + 2)
    balloons.replace(sentBalloonId(id), buildSentBalloon(corrected, text))
  }

  return { announceSent, markBackendQueued }
})
