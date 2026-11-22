<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import { ACTION_TITLE, type ActionLabel } from './backlogLabels'
import type { TaskCardAction } from './backlogTypes'

defineProps<{ action: { label: ActionLabel; id: TaskCardAction } }>()
const emit = defineEmits<{ action: [action: TaskCardAction] }>()

const MAIN_CLASSES = 'h-6.5 gap-1.5 rounded-sm pr-2.25 pl-1.75 text-small font-semibold'
const LINK_CLASSES = `inline-flex size-6.5 shrink-0 cursor-pointer items-center justify-center rounded-sm border border-line text-muted hover:bg-hover hover:text-ink ${INK_FOCUS_CLASSES}`
const TRIAGE_ICON_PATH = 'M2 3h12l-4.5 5.5V13l-3 1.5V8.5z'
const LINK_ICON_PATH = 'M6.5 9.5l3-3M7 4.5l1-1a2.5 2.5 0 013.5 3.5l-1 1M9 11.5l-1 1A2.5 2.5 0 014.5 9l1-1'
</script>

<!-- O click.stop impede que o clique nas ações abra o card que as contém. -->
<template>
  <div class="flex items-center gap-1">
    <UiButton variant="primary" size="sm" :class="MAIN_CLASSES" :title="ACTION_TITLE[action.id]" @click.stop @press="emit('action', action.id)">
      <svg v-if="action.id === 'triar'" width="12" height="12" viewBox="0 0 16 16" aria-hidden="true">
        <path :d="TRIAGE_ICON_PATH" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
      </svg>
      {{ action.label }}
    </UiButton>
    <button
      v-if="action.id === 'triar'"
      type="button"
      :class="LINK_CLASSES"
      :aria-label="ACTION_TITLE.vincular"
      :title="ACTION_TITLE.vincular"
      @click.stop="emit('action', 'vincular')"
    >
      <svg width="13" height="13" viewBox="0 0 16 16" aria-hidden="true">
        <path :d="LINK_ICON_PATH" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
      </svg>
    </button>
  </div>
</template>
