import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
export const INSTRUCTIONS_MAX_CHARS = 4000
export const INSTANCES_MIN = 1
export const INSTANCES_MAX = 4
export const DEFAULT_INSTANCES = 1
export const WORKTREES_MIN = 0
export const WORKTREES_MAX = 8
// Sem máximo de worktrees por padrão: 0 na tela é "sem limite" e vira vazio no backend.
export const DEFAULT_WORKTREES = 0

export const TEXTAREA_CLASSES =
  `w-full min-w-0 rounded-md border border-line bg-surface px-3 py-2 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`
