import { CURRENT_PROJECT, type RailPage } from '../../shell/railNav'
import type { TeamStatus } from './teamStatus'

export const PAGE_PLACEHOLDER: Partial<Record<RailPage, (project: string) => string>> = {
  projetos: () => 'Peça algo e escolha o projeto',
  conversas: (project) => `Mensagem para #${project}`,
}

export const QUEUE_SUFFIX = ' · entra na fila dele'

export function placeholderFor(page: RailPage | null, status: TeamStatus, statusPlaceholder: string): string {
  // Só o repouso usa o texto da página: erro e pausado precisam continuar visíveis.
  const pagePlaceholder = page === null || status !== 'rest' ? undefined : PAGE_PLACEHOLDER[page]
  return pagePlaceholder?.(CURRENT_PROJECT) ?? statusPlaceholder
}
