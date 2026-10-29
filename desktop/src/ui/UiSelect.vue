<script setup lang="ts">
defineProps<{
  options: readonly { label: string; value: string }[]
  label: string
  placeholder?: string
  disabled?: boolean
}>()
const model = defineModel<string>()

const SELECT_CLASSES =
  'h-9 appearance-none rounded-control border border-line bg-surface pl-3 pr-9 text-control text-ink focus:outline-2 focus:outline-solid focus:outline-accent'
</script>

<template>
  <div class="relative inline-flex">
    <select
      :value="model ?? ''"
      :disabled="disabled"
      :aria-disabled="disabled"
      :aria-label="label"
      :class="[SELECT_CLASSES, disabled ? 'cursor-default opacity-40' : 'cursor-pointer']"
      @change="model = ($event.target as HTMLSelectElement).value"
    >
      <option v-if="placeholder" value="" disabled>{{ placeholder }}</option>
      <option v-for="option in options" :key="option.value" :value="option.value">{{ option.label }}</option>
    </select>
    <svg
      class="pointer-events-none absolute top-1/2 right-3 size-4 -translate-y-1/2 text-muted"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <path d="m6 9 6 6 6-6" />
    </svg>
  </div>
</template>
