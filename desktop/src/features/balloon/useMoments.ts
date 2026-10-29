import { useBalloons } from './useBalloons'
import type { BalloonAction } from './balloonTypes'
import { DISMISS_LABEL, MOMENT_COPY, type MomentKind } from './momentCopy'

export interface MomentReport {
  kind: MomentKind
  quote: string
  origin: string
}

const PRIMARY_CONFIRMATION: Record<MomentKind, string> = {
  memoria: 'Guardado',
  grafo: 'Adicionado ao grafo',
  decisao: 'ADR criada',
  skill: 'Skill criada',
  conflito: 'Plano ordenado',
  comemoracao: 'Abrindo o diff',
}

export function useMoments() {
  const balloons = useBalloons()

  function report(moment: MomentReport): void {
    const copy = MOMENT_COPY[moment.kind]
    const primary: BalloonAction = {
      id: 'primary',
      label: copy.primary,
      primary: true,
      run: () => PRIMARY_CONFIRMATION[moment.kind],
    }
    const dismiss: BalloonAction = { id: 'dismiss', label: DISMISS_LABEL }
    balloons.enqueue({
      id: `${moment.kind}:${moment.quote}`,
      kind: 'moment',
      source: copy.source,
      origin: moment.origin,
      title: copy.title,
      text: moment.quote,
      actions: [primary, dismiss],
    })
  }

  return { report }
}
