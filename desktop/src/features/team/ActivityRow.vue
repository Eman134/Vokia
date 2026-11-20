<script setup lang="ts">
import { nextTick, ref, useId } from 'vue'
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
  delegate: 'M4 12h14 M13 6l6 6-6 6',
  web: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z M3 12h18',
  thinking: 'M9 4a3 3 0 0 0-3 3v1a3 3 0 0 0-2 5 3 3 0 0 0 2 5 3 3 0 0 0 3 3 3 3 0 0 0 3-3V7a3 3 0 0 0-3-3z M15 4a3 3 0 0 1 3 3v1a3 3 0 0 1 2 5 3 3 0 0 1-2 5 3 3 0 0 1-3 3 3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z',
  other: 'M12 4v16 M4 12h16',
}

const SVG_CLASSES = 'size-3.5 shrink-0 fill-none stroke-current stroke-2'
const CHIP_CLASSES = 'inline-flex min-w-0 max-w-full items-center gap-1 rounded-chip bg-surface-2 px-1.5 py-0.5 text-ink'
const OPEN_HINT = 'Clique para abrir o arquivo'
const PREVIEW_CLASSES =
  'absolute left-0 z-10 flex w-96 max-w-full flex-col gap-1.5 rounded-card border border-line bg-surface p-3 text-caption shadow-float'
const PREVIEW_OFFSET_PX = 4
const PREVIEW_BELOW_CLASSES = 'top-full mt-1'
const PREVIEW_ABOVE_CLASSES = 'bottom-full mb-1'

const open = ref(false)
const listId = useId()
const itemEl = ref<HTMLElement | null>(null)
const panelEl = ref<HTMLElement | null>(null)
const hovering = ref(false)
const openUp = ref(false)

// O painel é cortado pela área de rolagem da conversa, então o espaço útil é o dela, não a janela.
function scrollAreaOf(element: HTMLElement): HTMLElement | null {
  for (let parent = element.parentElement; parent; parent = parent.parentElement) {
    const { overflowY } = getComputedStyle(parent)
    if (overflowY === 'auto' || overflowY === 'scroll' || overflowY === 'hidden') return parent
  }
  return null
}

function fitsBelow(item: HTMLElement, panel: HTMLElement): boolean {
  const limit = scrollAreaOf(item)?.getBoundingClientRect().bottom ?? window.innerHeight
  return item.getBoundingClientRect().bottom + PREVIEW_OFFSET_PX + panel.offsetHeight <= limit
}

// O painel só existe com o mouse em cima: medido já aberto, a direção vem do espaço real antes de pintar.
async function showPreview(): Promise<void> {
  hovering.value = true
  await nextTick()
  if (itemEl.value && panelEl.value) {
    openUp.value = !fitsBelow(itemEl.value, panelEl.value)
  }
}
</script>

<template>
  <li
    ref="itemEl"
    class="relative flex min-w-0 flex-col gap-0.5"
    :data-icon="row.icon"
    data-testid="activity-row"
    @pointerenter="showPreview"
    @pointerleave="hovering = false"
  >
    <div class="flex min-w-0 items-center gap-1.5" :class="row.failed ? 'text-danger' : ''">
      <svg :class="SVG_CLASSES" viewBox="0 0 24 24" aria-hidden="true">
        <path :d="ICON_PATHS[row.icon]" />
      </svg>
      <span class="shrink-0 font-semibold text-ink">{{ row.verb }}</span>
      <span v-if="row.chip" :class="CHIP_CLASSES" :title="row.chip.title" data-testid="activity-chip">
        <svg :class="SVG_CLASSES" viewBox="0 0 24 24" aria-hidden="true" :data-icon="row.chip.icon">
          <path :d="ICON_PATHS[row.chip.icon]" />
        </svg>
        <span class="truncate">{{ row.chip.label }}</span>
      </span>
      <span v-else-if="row.text" class="min-w-0 truncate text-ink" :title="row.text">{{ row.text }}</span>
      <span v-if="row.meta" class="shrink-0 text-muted">{{ row.meta }}</span>
      <button
        v-if="row.items"
        type="button"
        class="shrink-0 cursor-pointer text-muted hover:text-ink focus:outline-2 focus:outline-solid focus:outline-accent"
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
    <div
      v-if="row.preview && hovering"
      ref="panelEl"
      :class="[PREVIEW_CLASSES, openUp ? PREVIEW_ABOVE_CLASSES : PREVIEW_BELOW_CLASSES]"
      data-testid="activity-preview"
    >
      <p v-if="row.preview.removed" class="line-clamp-4 wrap-break-word whitespace-pre-wrap text-muted line-through">{{ row.preview.removed }}</p>
      <p v-if="row.preview.added" class="line-clamp-4 wrap-break-word whitespace-pre-wrap text-ink">{{ row.preview.added }}</p>
      <span class="text-muted">{{ OPEN_HINT }}</span>
    </div>
  </li>
</template>
