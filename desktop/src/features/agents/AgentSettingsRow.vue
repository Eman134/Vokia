<script setup lang="ts">
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'

withDefaults(
  defineProps<{
    label: string
    summary?: string
    chevron?: 'right' | 'down' | 'plus' | 'none'
    disabled?: boolean
    swatchClass?: string | null
  }>(),
  {
    summary: '',
    chevron: 'right',
    disabled: false,
    swatchClass: null,
  },
)

const emit = defineEmits<{ press: [event: MouseEvent] }>()

const ROW_CLASSES =
  `flex w-full min-w-0 cursor-pointer items-center justify-between gap-3 px-3.5 py-2.5 text-left transition-colors hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-50 ${INK_FOCUS_CLASSES}`
</script>

<template>
  <button
    type="button"
    :class="ROW_CLASSES"
    :disabled="disabled"
    @click="emit('press', $event)"
  >
    <span class="flex min-w-0 items-center gap-2.5">
      <span
        v-if="$slots.icon"
        class="grid size-5 shrink-0 place-items-center text-muted"
        aria-hidden="true"
      >
        <slot name="icon" />
      </span>
      <span class="truncate text-control font-medium text-ink">{{ label }}</span>
    </span>
    <span class="flex min-w-0 shrink-0 items-center gap-2">
      <span
        v-if="swatchClass"
        class="size-3 shrink-0 rounded-full border border-project-line"
        :class="swatchClass"
        aria-hidden="true"
      />
      <span v-if="summary" class="max-w-44 truncate text-caption text-muted">{{ summary }}</span>
      <span v-if="chevron === 'right'" class="text-caption text-faint" aria-hidden="true">&rsaquo;</span>
      <span v-else-if="chevron === 'down'" class="text-caption text-faint" aria-hidden="true">&#8964;</span>
      <span v-else-if="chevron === 'plus'" class="text-caption text-faint" aria-hidden="true">+</span>
    </span>
  </button>
</template>
