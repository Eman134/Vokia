<script lang="ts">
export const BADGE_STATUS = ['trabalhando', 'precisa de você', 'livre', 'erro', 'backlog'] as const
</script>

<script setup lang="ts">
import { useHighlightOnce } from './useHighlightOnce'

type BadgeStatus = (typeof BADGE_STATUS)[number] | 'lote'

const props = defineProps<{
  status: BadgeStatus
  label?: string
  highlight?: boolean
}>()

const HIGHLIGHT_KEY = 'ui-badge'
const BASE_CLASSES = 'inline-flex h-6 shrink-0 items-center rounded-chip px-2 text-caption font-medium'
const TEXT_CLASSES: Record<BadgeStatus, string> = {
  trabalhando: 'text-ok',
  'precisa de você': 'text-warn-text',
  livre: 'text-faint',
  erro: 'text-danger',
  backlog: 'text-muted',
  lote: 'text-accent',
}
const SURFACE_CLASSES: Record<BadgeStatus, string> = {
  trabalhando: 'bg-ok-soft',
  'precisa de você': 'bg-warn-soft',
  livre: 'bg-surface-2',
  erro: 'bg-danger/10',
  backlog: 'bg-surface-2',
  lote: 'bg-accent-soft',
}
const HIGHLIGHT_SURFACE_CLASSES = 'bg-sticky'

// Lido uma vez no setup: o destaque acontece na montagem do primeiro selo marcado.
const highlighted = props.highlight && useHighlightOnce(HIGHLIGHT_KEY)
</script>

<template>
  <span
    :class="[
      BASE_CLASSES,
      TEXT_CLASSES[status],
      highlighted ? HIGHLIGHT_SURFACE_CLASSES : SURFACE_CLASSES[status],
    ]"
  >
    {{ label ?? status }}
  </span>
</template>
