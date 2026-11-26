<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { BATCH_BAR_WRAP_CLASSES } from '../team/bar/barClasses'

const props = defineProps<{
  selectedCount: number
  queueCount?: number
  batchActive?: boolean
  disabled?: boolean
}>()

const emit = defineEmits<{
  triage: []
  clear: []
}>()

const PILL_CLASSES =
  'pointer-events-auto flex items-center gap-2.5 rounded-full border border-line bg-surface py-1.5 pr-2 pl-3.5 shadow-bar'
const CLEAR_TITLE = 'Limpar seleção (Esc)'

const barRef = ref<HTMLElement | null>(null)
let previousFocus: HTMLElement | null = null

const isVisible = computed(() => props.selectedCount > 0 || props.batchActive === true)
const countLabel = computed(() =>
  props.selectedCount === 1 ? '1 selecionado' : `${props.selectedCount} selecionados`,
)
const triageLabel = computed(() => `Triar ${countLabel.value}`)
const batchStatusLabel = computed(() => {
  const remaining = props.queueCount ?? 0
  return remaining > 0 ? `Triando lote · ${remaining} na fila` : 'Triando lote…'
})

function onFocusIn(event: FocusEvent): void {
  const related = event.relatedTarget
  if (related instanceof HTMLElement && !barRef.value?.contains(related)) {
    previousFocus = related
  }
}

function restoreFocusIfInside(): void {
  if (barRef.value?.contains(document.activeElement)) {
    previousFocus?.focus()
  }
  previousFocus = null
}

function onClear(): void {
  restoreFocusIfInside()
  emit('clear')
}

defineExpose({ restoreFocusIfInside })
</script>

<template>
  <div
    v-if="isVisible"
    ref="barRef"
    :class="BATCH_BAR_WRAP_CLASSES"
    role="region"
    aria-label="Triagem em lote"
    data-testid="batch-triage-bar"
    @focusin="onFocusIn"
  >
    <div :class="PILL_CLASSES">
      <template v-if="selectedCount > 0">
        <span class="text-caption font-medium text-ink tabular-nums" data-testid="batch-selected-count">
          {{ countLabel }}
        </span>
        <span v-if="queueCount && queueCount > 0" class="text-meta text-muted tabular-nums">
          · {{ queueCount }} na fila
        </span>
        <UiButton
          variant="primary"
          size="sm"
          class="h-7 rounded-full px-3 text-small font-semibold"
          :disabled="disabled"
          data-testid="batch-triage-submit"
          @press="emit('triage')"
        >
          {{ triageLabel }}
        </UiButton>
        <UiButton
          variant="quiet"
          size="sm"
          class="h-7 rounded-full px-2 text-small"
          :title="CLEAR_TITLE"
          aria-label="Limpar seleção"
          data-testid="batch-triage-clear"
          @press="onClear"
        >
          Limpar
        </UiButton>
      </template>
      <span v-else class="pr-1.5 text-caption text-muted tabular-nums" data-testid="batch-triage-status">
        {{ batchStatusLabel }}
      </span>
    </div>
  </div>
</template>
