<script setup lang="ts" generic="T extends string">
const props = defineProps<{
  options: readonly { label: string; value: T }[]
  label: string
  disabled?: boolean
  wrap?: boolean
}>()
const model = defineModel<T>({ required: true })

const ROOT_CLASSES = 'flex gap-0.5 rounded-control bg-surface-2 p-0.75'
const OPTION_CLASSES =
  'whitespace-nowrap rounded-control-sm px-3 py-1.25 text-caption focus:outline-2 focus:outline-solid focus:outline-accent'
const ACTIVE_CLASSES = 'bg-surface font-medium text-ink shadow-seg'
const INACTIVE_CLASSES = 'text-muted'

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
    :class="[ROOT_CLASSES, wrap ? 'flex-wrap' : 'overflow-x-auto', disabled && 'opacity-40']"
  >
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      :disabled="disabled"
      :class="[
        OPTION_CLASSES,
        disabled ? 'cursor-default' : 'cursor-pointer',
        option.value === model ? ACTIVE_CLASSES : INACTIVE_CLASSES,
      ]"
      :aria-pressed="option.value === model"
      @click="select(option.value)"
    >
      {{ option.label }}
    </button>
  </div>
</template>
