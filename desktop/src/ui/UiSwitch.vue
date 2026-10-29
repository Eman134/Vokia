<script setup lang="ts">
const props = defineProps<{
  label: string
  disabled?: boolean
}>()
const model = defineModel<boolean>({ required: true })

const TRACK_CLASSES =
  'inline-flex h-5 w-9 shrink-0 items-center rounded-full p-0.5 focus:outline-2 focus:outline-solid focus:outline-accent'
const KNOB_CLASSES = 'block size-4 rounded-full shadow-seg'

function toggle() {
  if (props.disabled) return
  model.value = !model.value
}
</script>

<template>
  <button
    type="button"
    role="switch"
    :class="[
      TRACK_CLASSES,
      model ? 'bg-accent' : 'bg-line',
      disabled ? 'cursor-default opacity-40' : 'cursor-pointer',
    ]"
    :aria-checked="model"
    :aria-label="label"
    :aria-disabled="disabled"
    :disabled="disabled"
    @click="toggle"
  >
    <span :class="[KNOB_CLASSES, model ? 'translate-x-4 bg-accent-ink' : 'bg-ink']" />
  </button>
</template>
