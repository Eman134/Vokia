import { DEFAULT_COLUMN, type BacklogColumn } from './backlogTypes'

// Em andamento, em revisão e concluído não se iniciam: Done só volta a rodar reaberto ou movido de coluna.
const NON_RUNNABLE_COLUMNS: readonly BacklogColumn[] = ['In progress', 'In review', 'Done']

export function isRunnableColumn(column: BacklogColumn | undefined): boolean {
  return !NON_RUNNABLE_COLUMNS.includes(column ?? DEFAULT_COLUMN)
}

// O botão some só em Done; nas colunas em andamento ele continua à vista, desabilitado.
export function showsStartButton(column: BacklogColumn | undefined): boolean {
  return column !== 'Done'
}
