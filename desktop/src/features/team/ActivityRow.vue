<script setup lang="ts">
import { nextTick, ref, useId } from 'vue'
import UiTooltip from '../../ui/UiTooltip.vue'
import type { ActivityRowModel, RowIconKind } from './activitySummary'


import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
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
  role: 'M6 4h12v16H6z M10 4v2h4V4 M9 12h6 M9 16h4',
  agent: 'M12 3v3 M6 7h12v11H6z M9 11h2v2H9z M13 11h2v2h-2z',
  flow: 'M3 4h6v5H3z M15 4h6v5h-6z M9 15h6v5H9z M6 9v3h12V9 M12 12v3',
  channel: 'M9 4L7 20 M17 4l-2 16 M4 9h16 M3 15h16',
  settings:
    'M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z M12 3v3 M12 18v3 M3 12h3 M18 12h3 M5.6 5.6l2.1 2.1 M16.3 16.3l2.1 2.1 M18.4 5.6l-2.1 2.1 M7.7 16.3l-2.1 2.1',
  backlog: 'M4 4h16v16H4z M9 4v16 M15 4v16 M6.5 7v5 M12 7v8 M17.5 7v3',
  project: 'M3 6h6l2 2h10v11H3z M12 10.5l1.2 2.4 2.6.4-1.9 1.8.5 2.6-2.4-1.3-2.4 1.3.5-2.6-1.9-1.8 2.6-.4z',
  provider: 'M9 3v5 M15 3v5 M6 8h12v4a6 6 0 0 1-12 0z M12 18v3',
  artifact: 'M7 3h7l5 5v13H7z M14 3v5h5 M10 13h6 M10 17h6',
  attachment: 'M20 11l-8 8a5 5 0 0 1-7-7l8-8a3.5 3.5 0 0 1 5 5l-8 8a2 2 0 0 1-3-3l7-7',
  check: 'M5 12.5l4.5 4.5L19 7',
  warning: 'M12 4L22 20H2z M12 10v4 M12 17v.5',
  team: 'M9 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6z M3 20a6 6 0 0 1 12 0 M17 11a2.5 2.5 0 1 0 0-5 M17 14a5 5 0 0 1 4 6',
}

const SVG_CLASSES = 'size-3.5 shrink-0 fill-none stroke-current stroke-2'
const CHIP_CLASSES = 'inline-flex min-w-0 max-w-full items-center gap-1 rounded-sm bg-surface-2 px-1.5 py-0.5 text-ink'
const COUNT_CLASSES = 'shrink-0 rounded-sm bg-surface-2 px-1.5 text-caption font-semibold text-muted'
const OPEN_HINT = 'Clique para abrir o arquivo'
const PREVIEW_CLASSES =
  'absolute left-0 z-10 flex w-96 max-w-full flex-col gap-1.5 rounded-md border border-line bg-surface p-3 text-caption shadow-paper'
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
      <UiTooltip v-if="row.chip" :text="row.chip.title" :class="CHIP_CLASSES" data-testid="activity-chip">
        <svg :class="SVG_CLASSES" viewBox="0 0 24 24" aria-hidden="true" :data-icon="row.chip.icon">
          <path :d="ICON_PATHS[row.chip.icon]" />
        </svg>
        <span class="truncate">{{ row.chip.label }}</span>
      </UiTooltip>
      <UiTooltip v-else-if="row.text" :text="row.text" class="min-w-0 truncate text-ink">{{ row.text }}</UiTooltip>
      <span v-if="row.meta" class="shrink-0 text-muted">{{ row.meta }}</span>
      <span v-if="row.count && row.count > 1" :class="COUNT_CLASSES" :title="`${row.count} ocorrências`" data-testid="activity-count">×{{ row.count }}</span>
      <button
        v-if="row.items"
        type="button"
        class="shrink-0 cursor-pointer text-muted hover:text-ink" :class="INK_FOCUS_CLASSES"
        :aria-expanded="open"
        :aria-controls="listId"
        @click="open = !open"
      >
        <span aria-hidden="true">{{ open ? '▾' : '▸' }}</span>
      </button>
    </div>
    <ul v-if="row.items?.length && open" :id="listId" class="flex flex-col gap-0.5 pl-6 text-muted">
      <li v-for="item in row.items" :key="item" class="truncate">
        <UiTooltip :text="item" class="block truncate">{{ item }}</UiTooltip>
      </li>
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
