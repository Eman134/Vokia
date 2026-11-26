export interface BatchSelectionState {
  selected: Set<string>
  anchorId: string | null
}

export interface SelectionClickParams {
  selected: ReadonlySet<string>
  anchorId: string | null
  orderedIds: readonly string[]
  targetId: string
  shiftKey: boolean
}

export function createEmptySelection(): BatchSelectionState {
  return { selected: new Set(), anchorId: null }
}

export function toggleItemSelection(selected: ReadonlySet<string>, id: string): Set<string> {
  const next = new Set(selected)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  return next
}

export function selectRange(
  orderedIds: readonly string[],
  anchorId: string | null,
  targetId: string,
  selected: ReadonlySet<string>,
): Set<string> {
  const next = new Set(selected)
  const targetIndex = orderedIds.indexOf(targetId)
  if (targetIndex === -1) return next

  const anchorIndex = anchorId === null ? -1 : orderedIds.indexOf(anchorId)
  if (anchorIndex === -1) {
    next.add(targetId)
    return next
  }

  const start = Math.min(anchorIndex, targetIndex)
  const end = Math.max(anchorIndex, targetIndex)
  for (let i = start; i <= end; i += 1) {
    next.add(orderedIds[i])
  }
  return next
}

export function applySelectionClick(params: SelectionClickParams): BatchSelectionState {
  const { selected, anchorId, orderedIds, targetId, shiftKey } = params
  if (shiftKey && anchorId !== null) {
    return {
      selected: selectRange(orderedIds, anchorId, targetId, selected),
      anchorId: targetId,
    }
  }
  const nextSelected = toggleItemSelection(selected, targetId)
  return {
    selected: nextSelected,
    anchorId: nextSelected.has(targetId) ? targetId : null,
  }
}

export function reconcileSelection(
  state: BatchSelectionState,
  orderedIds: readonly string[],
): BatchSelectionState {
  if (state.selected.size === 0 && state.anchorId === null) return state
  const valid = new Set(orderedIds)
  const nextSelected = new Set([...state.selected].filter((id) => valid.has(id)))
  const nextAnchor = state.anchorId !== null && valid.has(state.anchorId) ? state.anchorId : null
  if (nextSelected.size === state.selected.size && nextAnchor === state.anchorId) return state
  return { selected: nextSelected, anchorId: nextAnchor }
}
