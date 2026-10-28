<script setup lang="ts">
const model = defineModel<string>({ required: true })

defineProps<{
  label: string
  hint?: string
  error?: string
}>()

const INPUT_CLASSES =
  'h-9 w-full min-w-0 rounded-control border bg-surface px-3 text-control text-ink placeholder:text-faint focus:outline-2 focus:outline-solid focus:outline-accent'
const INPUT_BORDER_CLASSES = 'border-line'
const INPUT_ERROR_BORDER_CLASSES = 'border-danger'
</script>

<template>
  <label class="flex flex-col gap-1.5">
    <span class="flex items-baseline justify-between gap-3 text-label font-medium text-muted">
      {{ label }}
      <kbd v-if="hint" class="font-mono text-tiny font-normal text-faint">{{ hint }}</kbd>
    </span>
    <input
      v-model="model"
      :class="[INPUT_CLASSES, error ? INPUT_ERROR_BORDER_CLASSES : INPUT_BORDER_CLASSES]"
      :aria-invalid="Boolean(error)"
    />
    <span v-if="error" role="alert" class="text-small text-danger">{{ error }}</span>
  </label>
</template>
