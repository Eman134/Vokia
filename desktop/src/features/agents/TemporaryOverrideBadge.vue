<script setup lang="ts">
import { computed, ref } from 'vue'
import { resetCountdownLabel } from '../usage/usagePlans'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
export interface TemporaryOverrideInfo {
  cli: string
  model: string
  originalCli: string
  originalModel: string
  reason: string
  expiresAt?: number | null
}

const props = withDefaults(
  defineProps<{
    override: TemporaryOverrideInfo
    restoring?: boolean
  }>(),
  {
    restoring: false,
  },
)

const emit = defineEmits<{
  restore: []
}>()

const REASON_LABELS: Record<string, string> = {
  'usage-limit': 'Limite esgotado',
  usage_limit: 'Limite esgotado',
  limit_exhausted: 'Limite esgotado',
  'model-unavailable': 'Modelo sem acesso',
  model_unavailable: 'Modelo sem acesso',
  'manual-temp': 'Troca temporária',
  manual: 'Troca temporária',
}

const rootRef = ref<HTMLElement | null>(null)
const popoverRef = ref<HTMLElement | null>(null)
const isOpen = ref(false)
const popoverPos = ref({ top: 0, left: 0 })

const reasonLabel = computed(() => REASON_LABELS[props.override.reason] ?? props.override.reason)

const countdownText = computed(() => {
  const raw = props.override.expiresAt
  if (raw == null) return null
  const seconds = raw > 1e12 ? Math.floor(raw / 1000) : raw
  return resetCountdownLabel(seconds, Date.now())
})

const tooltipText = computed(() => {
  const parts = [
    `Temporário: ${props.override.cli} · ${props.override.model}`,
    `Original: ${props.override.originalCli} · ${props.override.originalModel}`,
    `Motivo: ${reasonLabel.value}`,
  ]
  if (countdownText.value) {
    parts.push(`Expira ${countdownText.value}`)
  }
  return parts.join('\n')
})

function togglePopover() {
  if (!isOpen.value && rootRef.value) {
    const rect = rootRef.value.getBoundingClientRect()
    popoverPos.value = {
      top: rect.bottom + 6,
      left: Math.max(8, Math.min(rect.left, window.innerWidth - 264)),
    }
  }
  isOpen.value = !isOpen.value
}

function closePopover() {
  isOpen.value = false
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && isOpen.value) {
    event.preventDefault()
    event.stopPropagation()
    closePopover()
  }
}

function onFocusOut(event: FocusEvent) {
  const nextTarget = event.relatedTarget as Node | null
  if (nextTarget && (rootRef.value?.contains(nextTarget) || popoverRef.value?.contains(nextTarget))) return
  closePopover()
}

function onRestore() {
  if (props.restoring) return
  emit('restore')
}
</script>

<template>
  <span
    ref="rootRef"
    class="relative inline-flex items-center"
    @keydown="onKeydown"
    @focusout="onFocusOut"
  >
    <button
      type="button"
      data-testid="temporary-override-badge"
      :title="tooltipText"
      :aria-label="`Uso temporário: ${override.cli} ${override.model}. Original: ${override.originalCli} ${override.originalModel}`"
      :aria-expanded="isOpen"
      aria-haspopup="dialog"
      class="inline-flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-sm bg-warn-soft text-warn-text hover:opacity-90" :class="INK_FOCUS_CLASSES"
      @click.stop="togglePopover"
    >
      <svg
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
        <path d="M3 3v5h5" />
        <path d="M12 7v5l3 2" />
      </svg>
    </button>

    <Teleport to="body">
      <div
        v-if="isOpen"
        ref="popoverRef"
        role="dialog"
        aria-label="Detalhes da troca temporária"
        class="fixed z-70 w-64 rounded-md border border-line bg-surface p-3 text-caption text-ink shadow-seg"
        :style="{ top: `${popoverPos.top}px`, left: `${popoverPos.left}px` }"
        @keydown="onKeydown"
        @focusout="onFocusOut"
        @click.stop
      >
      <dl class="flex flex-col gap-1.5">
        <div class="flex flex-col gap-0.5">
          <dt class="text-label font-medium text-muted">Temporário</dt>
          <dd class="font-medium text-warn-text">{{ override.cli }} · {{ override.model }}</dd>
        </div>
        <div class="flex flex-col gap-0.5">
          <dt class="text-label font-medium text-muted">Original</dt>
          <dd class="text-ink">{{ override.originalCli }} · {{ override.originalModel }}</dd>
        </div>
        <div class="flex items-center justify-between gap-2 text-small text-muted">
          <span>{{ reasonLabel }}</span>
          <span v-if="countdownText">Expira {{ countdownText }}</span>
        </div>
      </dl>

      <div class="mt-2.5 flex justify-end border-t border-line pt-2">
        <button
          type="button"
          data-testid="temporary-override-restore"
          :disabled="restoring"
          class="inline-flex h-7 cursor-pointer items-center justify-center rounded-md bg-warn-soft px-2.5 text-label font-medium text-warn-text hover:opacity-90 disabled:cursor-default disabled:opacity-40" :class="INK_FOCUS_CLASSES"
          @click="onRestore"
        >
          {{ restoring ? 'Restaurando...' : 'Restaurar original' }}
        </button>
      </div>
      </div>
    </Teleport>
  </span>
</template>
