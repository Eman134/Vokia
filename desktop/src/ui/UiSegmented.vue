<script lang="ts">
import { INK_FOCUS_CLASSES, handReveal } from './handDrawn'
import UiHandLine from './UiHandLine.vue'

export const SEGMENTED_GROUP_CLASSES = 'flex gap-0.5 rounded-md bg-surface-2 p-0.75'
export const SEGMENTED_OPTION_CLASSES = `group relative whitespace-nowrap rounded-sm px-3 py-1 text-caption ${INK_FOCUS_CLASSES}`
export const SEGMENTED_ACTIVE_CLASSES = 'bg-surface font-medium text-ink'
export const SEGMENTED_INACTIVE_CLASSES = 'text-muted enabled:hover:text-ink'
export const SEGMENTED_MARK_CLASSES = 'absolute bottom-0.5 left-3 h-1.5 w-[calc(100%-1.5rem)] text-ink-pen'
</script>

<script setup lang="ts" generic="T extends string">
const props = defineProps<{
  options: readonly { label: string; value: T }[]
  label: string
  disabled?: boolean
  wrap?: boolean
}>()
const model = defineModel<T>({ required: true })

function select(value: T) {
  if (props.disabled || value === model.value) return
  model.value = value
}
</script>

<template>
  <div
    role="group"
    :aria-label="label"
    :aria-disabled="disabled"
    :class="[SEGMENTED_GROUP_CLASSES, wrap ? 'flex-wrap' : 'overflow-x-auto', disabled && 'opacity-40']"
  >
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      :disabled="disabled"
      :class="[
        SEGMENTED_OPTION_CLASSES,
        disabled ? 'cursor-default' : 'cursor-pointer',
        option.value === model ? SEGMENTED_ACTIVE_CLASSES : SEGMENTED_INACTIVE_CLASSES,
      ]"
      :aria-pressed="option.value === model"
      @click="select(option.value)"
    >
      {{ option.label }}
      <UiHandLine :class="[SEGMENTED_MARK_CLASSES, handReveal(option.value === model)]" />
    </button>
  </div>
</template>
