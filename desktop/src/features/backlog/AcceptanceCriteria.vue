<script setup lang="ts">
import type { BacklogCriterion } from './backlogTypes'

defineProps<{
  criteria: readonly BacklogCriterion[]
}>()

const emit = defineEmits<{
  toggle: [id: string]
}>()

const ITEM_BUTTON_CLASSES =
  'flex w-full cursor-pointer items-start gap-2.5 rounded-control px-1 py-1 text-left focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-accent'
const BOX_BASE_CLASSES =
  'mt-0.5 flex size-[17px] shrink-0 items-center justify-center rounded-sm border text-tiny font-bold transition-colors'
const BOX_CHECKED_CLASSES = 'border-accent bg-accent text-accent-ink'
const BOX_UNCHECKED_CLASSES = 'border-faint bg-surface'
</script>

<template>
  <section data-testid="acceptance-criteria">
    <h3 class="mb-1.5 text-label font-semibold text-ink">Critérios de aceite</h3>
    <ul class="flex flex-col gap-1">
      <li v-for="criterion in criteria" :key="criterion.id">
        <button
          type="button"
          role="checkbox"
          :aria-checked="criterion.done"
          :class="ITEM_BUTTON_CLASSES"
          @click="emit('toggle', criterion.id)"
        >
          <span
            aria-hidden="true"
            :class="[BOX_BASE_CLASSES, criterion.done ? BOX_CHECKED_CLASSES : BOX_UNCHECKED_CLASSES]"
          >
            {{ criterion.done ? '✓' : '' }}
          </span>
          <span :class="['min-w-0 flex-1 wrap-break-word text-control', criterion.done ? 'text-muted line-through' : 'text-ink']">
            {{ criterion.text }}
          </span>
        </button>
      </li>
    </ul>
  </section>
</template>
