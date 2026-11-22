<script setup lang="ts">
import { INK_FOCUS_CLASSES, handReveal } from './handDrawn'
import UiHandLine from './UiHandLine.vue'

const props = withDefaults(
  defineProps<{
    label: string
    selected?: boolean
    kind?: 'option' | 'add'
  }>(),
  { kind: 'option' },
)
const emit = defineEmits<{ press: [] }>()

const BASE_CLASSES =
  `group relative inline-flex h-7 shrink-0 cursor-pointer items-center rounded-full px-3 text-label font-medium ${INK_FOCUS_CLASSES}`
const OPTION_CLASSES: Record<'selected' | 'unselected', string> = {
  selected: 'bg-surface-2 text-ink',
  unselected: 'bg-surface-2 text-muted hover:text-ink',
}
const ADD_CLASSES = 'border border-dashed border-line text-muted hover:border-faint hover:text-ink'
const MARK_CLASSES = 'absolute bottom-0.5 left-3.5 h-1.5 w-[calc(100%-1.75rem)] text-ink-pen'
</script>

<template>
  <button
    type="button"
    :class="[
      BASE_CLASSES,
      kind === 'add' ? ADD_CLASSES : OPTION_CLASSES[selected ? 'selected' : 'unselected'],
    ]"
    :aria-pressed="kind === 'option' ? selected : undefined"
    @click="emit('press')"
  >
    {{ label }}
    <UiHandLine v-if="kind === 'option'" :class="[MARK_CLASSES, handReveal(props.selected === true)]" />
  </button>
</template>
