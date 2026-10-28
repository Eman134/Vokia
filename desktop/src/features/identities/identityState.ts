export const IDENTITY_STATES = ['executando', 'esperando', 'concluido', 'falha', 'pausado'] as const
export type IdentityState = (typeof IDENTITY_STATES)[number]
export const COMPARE_SIZES = [64, 32, 20, 16] as const
export const STATE_MARKS: Record<IdentityState, string> = {
  executando: '▶',
  esperando: '⋯',
  concluido: '✓',
  falha: '✕',
  pausado: '❚❚',
}
