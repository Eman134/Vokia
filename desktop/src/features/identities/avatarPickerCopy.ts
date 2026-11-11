import type { AvatarExpression } from './avatarProfiles'

export const NEW_PROFILE_LABEL = 'Novo perfil'
export const PROFILE_NAME_LABEL = 'Nome do perfil'
export const PROFILE_SHAPE_LABEL = 'Forma'
export const PROFILE_COLOR_LABEL = 'Cor'
export const PROFILE_EXPRESSION_LABEL = 'Expressão'
export const SAVE_PROFILE_LABEL = 'Salvar perfil'
export const CANCEL_PROFILE_LABEL = 'Cancelar'
export const PROFILE_PREVIEW_LABEL = 'Prévia do perfil'

export const EXPRESSION_LABELS: Record<AvatarExpression, string> = {
  feliz: 'Feliz',
  curioso: 'Curioso',
  focado: 'Focado',
  sonolento: 'Sonolento',
}

export const PROFILE_NAME_INVALID = 'Nome inválido: use de 2 a 20 caracteres, com ao menos 2 letras ou dígitos.'
export const PROFILE_NAME_IN_USE = 'Nome em uso: já existe um perfil com esse nome.'
export const PROFILE_COMBINATION_INVALID = 'Combinação de forma, cor e expressão inválida.'
export const PROFILE_LOAD_BAD_FORMAT = 'formato inválido.'

export function profileSaveFailed(reason: string): string {
  return `Não foi possível salvar o perfil: ${reason}`
}

export function profileLoadUnreadable(reason: string): string {
  return `Não foi possível ler os perfis de avatar salvos: ${reason}`
}

export function profilesDiscarded(count: number): string {
  return count === 1 ? '1 perfil de avatar salvo inválido ignorado.' : `${count} perfis de avatar salvos inválidos ignorados.`
}
