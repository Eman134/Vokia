import { ACTION_ID, STATE_ACTION, type ActionLabel } from './backlogLabels'
import { DEFAULT_COLUMN, READY_COLUMN, type BacklogColumn, type BacklogItem, type BacklogState, type TaskCardAction } from './backlogTypes'

export type CardAction = { label: ActionLabel; id: TaskCardAction }

// Card de agente nasce em `triagem` sem triagem rodando (triageStep vazio): precisa oferecer Triar e Vincular como o `vazio`.
function effectiveState(state: BacklogState, item: Pick<BacklogItem, 'triageStep'>): BacklogState {
  return state === 'triagem' && item.triageStep === undefined ? 'vazio' : state
}

export function cardActionFor(
  item: Pick<BacklogItem, 'error' | 'triageStep'>,
  column: BacklogColumn,
  state: BacklogState,
): CardAction | undefined {
  if (item.error) return undefined
  if (column === READY_COLUMN && state === 'vinculado') return { label: 'Iniciar', id: 'iniciar' }
  if (column !== DEFAULT_COLUMN) return undefined
  const label = STATE_ACTION[effectiveState(state, item)]
  return label && { label, id: ACTION_ID[label] }
}
