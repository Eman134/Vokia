<script setup lang="ts">
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
  'inline-flex h-7 shrink-0 cursor-pointer items-center rounded-full px-3 text-label font-medium focus:outline-2 focus:outline-solid focus:outline-accent'
const OPTION_CLASSES: Record<'selected' | 'unselected', string> = {
  selected: 'bg-accent-soft text-accent hover:bg-accent/15',
  unselected: 'bg-surface-2 text-muted hover:bg-hover hover:text-ink',
}
const ADD_CLASSES = 'border border-dashed border-line text-muted hover:border-faint hover:text-ink'
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
  </button>
</template>
