export type BalloonKind = 'moment' | 'limit' | 'request'
export type BalloonSource = 'conversa' | 'codigo' | 'chat' | 'grafo' | 'uso do plano'

export interface BalloonAction {
  id: string
  label: string
  primary?: boolean
  disabled?: boolean
  run?: () => string | void
}

export interface BalloonItem {
  id: string
  kind: BalloonKind
  source: BalloonSource
  origin?: string
  title: string
  text?: string
  actions: BalloonAction[]
}

export const MOMENT_TIMEOUT_MS = 8000
export const CONFIRM_VISIBLE_MS = 2000
