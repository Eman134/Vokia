<script setup lang="ts">
import UiHandLine from '../../ui/UiHandLine.vue'
import UiIcon from '../../ui/UiIcon.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import { DEFAULT_JITTER, sketchCheck } from '../../ui/sketch/sketchPath'
import type { BacklogCriterion } from './backlogTypes'

defineProps<{
  criteria: readonly BacklogCriterion[]
}>()

const emit = defineEmits<{
  toggle: [id: string]
}>()

const CHECK_BOX = 16
const CHECK_SEED = 7
const CHECK_PATH = sketchCheck(CHECK_BOX, CHECK_BOX, CHECK_SEED, DEFAULT_JITTER)
const ITEM_BUTTON_CLASSES = `flex w-full cursor-pointer items-start gap-2.5 rounded-md px-1 py-1 text-left ${INK_FOCUS_CLASSES}`
const BOX_CLASSES = 'mt-0.5 flex size-4.25 shrink-0 items-center justify-center rounded-sm border bg-surface'
const BOX_CHECKED_CLASSES = 'border-ink-pen text-ink-pen'
const BOX_UNCHECKED_CLASSES = 'border-graphite'
const STRIKE_CLASSES = 'pointer-events-none absolute inset-x-0 top-1/2 h-2 -translate-y-1/2'
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
          <span aria-hidden="true" :class="[BOX_CLASSES, criterion.done ? BOX_CHECKED_CLASSES : BOX_UNCHECKED_CLASSES]">
            <UiIcon v-if="criterion.done" :path="CHECK_PATH" :size="CHECK_BOX" />
          </span>
          <span :class="['relative min-w-0 flex-1 wrap-break-word text-control', criterion.done ? 'text-muted' : 'text-ink']">
            {{ criterion.text }}
            <UiHandLine v-if="criterion.done" variant="strike" :class="STRIKE_CLASSES" />
          </span>
        </button>
      </li>
    </ul>
  </section>
</template>
