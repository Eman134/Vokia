<script setup lang="ts">
import { ref, useId } from 'vue'
import type { ActivityRowModel, RowIconKind } from './activitySummary'

defineProps<{ row: ActivityRowModel }>()

const ICON_PATHS: Readonly<Record<RowIconKind, string>> = {
  document: 'M7 3h7l5 5v13H7z M14 3v5h5',
  sheet: 'M4 4h16v16H4z M4 10h16 M10 4v16',
  code: 'M8 8l-4 4 4 4 M16 8l4 4-4 4',
  generic: 'M6 3h8l4 4v14H6z',
  folder: 'M3 6h6l2 2h10v11H3z',
  search: 'M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14z M20 20l-4-4',
  edit: 'M4 20h4L19 9l-4-4L4 16z',
  command: 'M5 7l5 5-5 5 M12 19h7',
  web: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z M3 12h18',
  other: 'M12 4v16 M4 12h16',
}

const SVG_CLASSES = 'size-3.5 shrink-0 fill-none stroke-current stroke-2'
const CHIP_CLASSES = 'inline-flex min-w-0 max-w-full items-center gap-1 rounded-chip bg-surface-2 px-1.5 py-0.5 text-ink'

const open = ref(false)
const listId = useId()
</script>

<template>
  <li class="flex min-w-0 flex-col gap-0.5" :data-icon="row.icon" data-testid="activity-row">
    <div class="flex min-w-0 flex-wrap items-center gap-1.5" :class="row.failed ? 'text-danger' : ''">
      <svg :class="SVG_CLASSES" viewBox="0 0 24 24" aria-hidden="true">
        <path :d="ICON_PATHS[row.icon]" />
      </svg>
      <span class="font-semibold text-ink">{{ row.verb }}</span>
      <span v-if="row.chip" :class="CHIP_CLASSES" :title="row.chip.title" data-testid="activity-chip">
        <svg :class="SVG_CLASSES" viewBox="0 0 24 24" aria-hidden="true" :data-icon="row.chip.icon">
          <path :d="ICON_PATHS[row.chip.icon]" />
        </svg>
        <span class="truncate">{{ row.chip.label }}</span>
      </span>
      <span v-else-if="row.text" class="min-w-0 truncate text-ink" :title="row.text">{{ row.text }}</span>
      <span v-if="row.meta" class="text-muted">{{ row.meta }}</span>
      <button
        v-if="row.items"
        type="button"
        class="cursor-pointer text-muted hover:text-ink focus:outline-2 focus:outline-solid focus:outline-accent"
        :aria-expanded="open"
        :aria-controls="listId"
        @click="open = !open"
      >
        <span aria-hidden="true">{{ open ? '▾' : '▸' }}</span>
      </button>
    </div>
    <ul v-if="row.items && open" :id="listId" class="flex flex-col gap-0.5 pl-6 text-muted">
      <li v-for="item in row.items" :key="item" class="truncate" :title="item">{{ item }}</li>
    </ul>
  </li>
</template>
