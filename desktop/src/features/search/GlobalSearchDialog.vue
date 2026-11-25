<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiIcon from '../../ui/UiIcon.vue'
import { useProjects } from '../projects/useProjects'
import { splitHighlightedSnippet } from './searchApi'
import {
  FILTER_CHIPS,
  useGlobalSearch,
  type PaletteGroup,
  type PaletteItem,
} from './useGlobalSearch'

const OVERLAY_CLASSES =
  'fixed inset-0 z-50 flex items-start justify-center bg-overlay px-4 pt-[min(12vh,5.5rem)] pb-6'
const FRAME_CLASSES =
  'flex w-full max-w-2xl max-h-[min(76dvh,640px)] flex-col overflow-hidden rounded-md border border-line bg-surface shadow-paper'
const HEADER_CLASSES = 'flex items-center gap-2.5 border-b border-line px-4 py-3'
const INPUT_CLASSES =
  'min-w-0 flex-1 bg-transparent text-body text-ink placeholder:text-faint outline-none'
const CLEAR_BUTTON_CLASSES =
  `inline-flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-sm text-muted hover:bg-hover hover:text-ink disabled:cursor-not-allowed ${INK_FOCUS_CLASSES}`
const KBD_CLASSES =
  'inline-flex shrink-0 items-center rounded-sm border border-line bg-surface-2 px-1.5 py-0.5 font-mono text-tiny text-muted'
const FILTER_BAR_CLASSES =
  'flex items-center gap-1.5 overflow-x-auto border-b border-line bg-paper px-3.5 py-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden'
const CHIP_BASE_CLASSES =
  `inline-flex shrink-0 cursor-pointer items-center gap-1 rounded-full border px-2.5 py-1 text-small transition-colors disabled:cursor-not-allowed ${INK_FOCUS_CLASSES}`
const CHIP_ACTIVE_CLASSES = 'border-accent bg-accent-soft font-medium text-accent'
const CHIP_IDLE_CLASSES = 'border-line bg-surface text-muted hover:bg-hover hover:text-ink'
const LIST_CLASSES = 'flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-2.5 py-2.5'
const GROUP_HEADER_CLASSES =
  'flex items-center justify-between px-2 pb-1 text-tiny font-semibold uppercase tracking-wide text-muted'
const ROW_BASE_CLASSES =
  `flex w-full cursor-pointer flex-col gap-0.5 rounded-md px-2.5 py-2 text-left transition-colors disabled:cursor-not-allowed ${INK_FOCUS_CLASSES}`
const ROW_ACTIVE_CLASSES = 'bg-accent-soft text-ink'
const ROW_IDLE_CLASSES = 'text-ink hover:bg-hover'
const BADGE_CLASSES =
  'inline-flex shrink-0 items-center rounded-sm bg-surface-2 px-1.5 py-0.5 text-tiny font-medium text-muted'
const PROJECT_BADGE_CLASSES =
  'inline-flex shrink-0 items-center rounded-sm border border-line bg-paper px-1.5 py-0.5 text-tiny text-faint'
const SNIPPET_CLASSES = 'line-clamp-2 text-caption text-muted'
const MARK_CLASSES = 'rounded-sm bg-mark-yellow px-0.5 text-mark-ink'
const LOAD_MORE_CLASSES =
  `mt-0.5 w-fit cursor-pointer rounded-sm px-2.5 py-1 text-small font-medium text-accent hover:bg-accent-soft disabled:cursor-not-allowed ${INK_FOCUS_CLASSES}`
const FOOTER_CLASSES =
  'flex flex-wrap items-center justify-between gap-2 border-t border-line bg-paper px-4 py-2 text-tiny text-muted'

const SEARCH_ICON_PATH = 'M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10z M10.8 10.8 14 14'
const PLACEHOLDER = 'Buscar projetos, cards (#), agentes (@), ações (>), artefatos e mensagens...'
const DIALOG_LABEL = 'Busca global'
const EMPTY_TITLE = 'Nenhum resultado encontrado.'
const EMPTY_HINT = 'Tente outro termo ou troque o filtro acima.'
const LOADING_TEXT = 'Buscando no workspace...'
const CLEAR_LABEL = 'Limpar busca'
const CURRENT_PROJECT_LABEL = 'Projeto atual'

const search = useGlobalSearch()
const projects = useProjects()
const dialogId = useId()
const inputRef = ref<HTMLInputElement | null>(null)
const listRef = ref<HTMLElement | null>(null)
const activeIndex = ref(0)

let previousFocus: HTMLElement | null = null
let restoreFocusOnClose = true

interface FlatEntry {
  item: PaletteItem
  groupIndex: number
  flatIndex: number
}

const flatEntries = computed<FlatEntry[]>(() => {
  const entries: FlatEntry[] = []
  let flatIndex = 0
  search.groups.forEach((group, groupIndex) => {
    for (const item of group.items) {
      entries.push({ item, groupIndex, flatIndex })
      flatIndex += 1
    }
  })
  return entries
})

const groupStartIndex = computed<number[]>(() => {
  const starts: number[] = []
  let offset = 0
  for (const group of search.groups) {
    starts.push(offset)
    offset += group.items.length
  }
  return starts
})

watch(
  () => search.isOpen,
  async (open) => {
    if (open) {
      restoreFocusOnClose = true
      previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
      activeIndex.value = 0
      await nextTick()
      inputRef.value?.focus()
      inputRef.value?.select()
      return
    }
    const target = previousFocus
    previousFocus = null
    if (restoreFocusOnClose && target && target.isConnected) {
      await nextTick()
      target.focus()
    }
  },
)

watch(
  () => flatEntries.value.map((entry) => entry.item.key).join('|'),
  () => {
    if (activeIndex.value >= flatEntries.value.length) {
      activeIndex.value = 0
    }
  },
)

function scrollActiveIntoView(): void {
  void nextTick(() => {
    const active = listRef.value?.querySelector<HTMLElement>('[data-search-active="true"]')
    active?.scrollIntoView({ block: 'nearest' })
  })
}

function dismiss(): void {
  restoreFocusOnClose = true
  search.closeDialog()
}

function selectItem(item: PaletteItem): void {
  restoreFocusOnClose = false
  void search.execute(item)
}

function moveSelection(delta: 1 | -1): void {
  const total = flatEntries.value.length
  if (total === 0) return
  activeIndex.value = (activeIndex.value + delta + total) % total
  scrollActiveIntoView()
}

function jumpGroup(delta: 1 | -1): void {
  const count = search.groups.length
  if (count <= 1) return
  const current = flatEntries.value[activeIndex.value]
  const currentGroup = current?.groupIndex ?? 0
  const nextGroup = (currentGroup + delta + count) % count
  activeIndex.value = groupStartIndex.value[nextGroup] ?? 0
  scrollActiveIntoView()
}

function onDialogKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    dismiss()
    return
  }
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    moveSelection(1)
    return
  }
  if (event.key === 'ArrowUp') {
    event.preventDefault()
    moveSelection(-1)
    return
  }
  if (event.key === 'Tab') {
    event.preventDefault()
    jumpGroup(event.shiftKey ? -1 : 1)
    return
  }
  if (event.key === 'Enter' && !event.isComposing) {
    const current = flatEntries.value[activeIndex.value]
    if (!current) return
    event.preventDefault()
    selectItem(current.item)
  }
}

function itemFlatIndex(groupIndex: number, indexInGroup: number): number {
  return (groupStartIndex.value[groupIndex] ?? 0) + indexInGroup
}

function canLoadMore(group: PaletteGroup): boolean {
  return Boolean(group.nextCursor && group.backendKind)
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="search.isOpen"
      :class="OVERLAY_CLASSES"
      data-testid="global-search-overlay"
      @click.self="dismiss"
    >
      <section
        :id="dialogId"
        role="dialog"
        aria-modal="true"
        :aria-label="DIALOG_LABEL"
        data-testid="global-search-dialog"
        :class="FRAME_CLASSES"
        @keydown="onDialogKeydown"
      >
        <div :class="HEADER_CLASSES">
          <UiIcon class="shrink-0 text-muted" :path="SEARCH_ICON_PATH" :size="17" />
          <input
            ref="inputRef"
            v-model="search.query"
            type="text"
            role="combobox"
            aria-autocomplete="list"
            :aria-expanded="flatEntries.length > 0"
            :aria-label="DIALOG_LABEL"
            :placeholder="PLACEHOLDER"
            data-testid="global-search-input"
            :class="INPUT_CLASSES"
          />
          <button
            v-if="search.query"
            type="button"
            :class="CLEAR_BUTTON_CLASSES"
            :aria-label="CLEAR_LABEL"
            :title="CLEAR_LABEL"
            @click="search.query = ''; inputRef?.focus()"
          >
            ×
          </button>
          <kbd :class="KBD_CLASSES">Esc</kbd>
        </div>

        <div role="toolbar" aria-label="Filtros da busca" :class="FILTER_BAR_CLASSES">
          <button
            v-for="chip in FILTER_CHIPS"
            :key="chip.id"
            type="button"
            :aria-pressed="search.effectiveChip === chip.id"
            :class="[
              CHIP_BASE_CLASSES,
              search.effectiveChip === chip.id ? CHIP_ACTIVE_CLASSES : CHIP_IDLE_CLASSES,
            ]"
            @click="search.selectChip(chip.id); inputRef?.focus()"
          >
            <span v-if="chip.prefix" class="font-mono text-tiny opacity-75">{{ chip.prefix }}</span>
            <span>{{ chip.label }}</span>
          </button>

          <button
            v-if="projects.selected"
            type="button"
            :aria-pressed="search.currentProjectOnly"
            :class="[
              CHIP_BASE_CLASSES,
              'ml-auto',
              search.currentProjectOnly ? CHIP_ACTIVE_CLASSES : CHIP_IDLE_CLASSES,
            ]"
            :title="`Limitar busca a ${projects.selected.name}`"
            @click="search.currentProjectOnly = !search.currentProjectOnly; inputRef?.focus()"
          >
            {{ CURRENT_PROJECT_LABEL }}
          </button>
        </div>

        <div ref="listRef" role="listbox" :aria-label="DIALOG_LABEL" :class="LIST_CLASSES">
          <p v-if="search.error" class="px-2.5 py-2 text-caption text-danger">
            {{ search.error }}
          </p>

          <template v-if="search.groups.length > 0">
            <section
              v-for="(group, groupIndex) in search.groups"
              :key="group.id"
              :aria-label="group.label"
              class="flex flex-col gap-0.5"
            >
              <div :class="GROUP_HEADER_CLASSES">
                <span>{{ group.label }}</span>
                <span class="tabular-nums">{{ group.total }}</span>
              </div>

              <button
                v-for="(item, indexInGroup) in group.items"
                :key="item.key"
                type="button"
                role="option"
                :aria-selected="itemFlatIndex(groupIndex, indexInGroup) === activeIndex"
                :data-search-active="itemFlatIndex(groupIndex, indexInGroup) === activeIndex ? 'true' : undefined"
                :class="[
                  ROW_BASE_CLASSES,
                  itemFlatIndex(groupIndex, indexInGroup) === activeIndex ? ROW_ACTIVE_CLASSES : ROW_IDLE_CLASSES,
                ]"
                @mouseenter="activeIndex = itemFlatIndex(groupIndex, indexInGroup)"
                @click="selectItem(item)"
              >
                <div class="flex min-w-0 items-center gap-2">
                  <span :class="BADGE_CLASSES">{{ item.badge }}</span>
                  <span class="min-w-0 flex-1 truncate text-control font-medium text-ink">
                    {{ item.title }}
                  </span>
                  <span v-if="item.projectName" :class="PROJECT_BADGE_CLASSES">
                    {{ item.projectName }}
                  </span>
                </div>
                <span v-if="item.subtitle && !item.snippet" class="truncate text-caption text-muted">
                  {{ item.subtitle }}
                </span>
                <p v-else-if="item.snippet" :class="SNIPPET_CLASSES">
                  <span v-if="item.subtitle" class="mr-1.5 text-faint">{{ item.subtitle }} ·</span>
                  <template v-for="(segment, segIdx) in splitHighlightedSnippet(item.snippet)" :key="segIdx">
                    <mark v-if="segment.highlight" :class="MARK_CLASSES">{{ segment.text }}</mark>
                    <span v-else>{{ segment.text }}</span>
                  </template>
                </p>
              </button>

              <button
                v-if="canLoadMore(group)"
                type="button"
                :class="LOAD_MORE_CLASSES"
                @click="group.backendKind && search.loadMore(group.backendKind)"
              >
                Mostrar mais em {{ group.label }}
              </button>
            </section>
          </template>

          <div
            v-else-if="search.loading"
            class="flex items-center justify-center py-10 text-caption text-muted"
          >
            {{ LOADING_TEXT }}
          </div>

          <div
            v-else
            class="flex flex-col items-center justify-center gap-1 py-10 text-center"
          >
            <p class="text-control font-medium text-ink">{{ EMPTY_TITLE }}</p>
            <p class="text-caption text-muted">{{ EMPTY_HINT }}</p>
          </div>
        </div>

        <footer :class="FOOTER_CLASSES">
          <div class="flex flex-wrap items-center gap-3">
            <span class="inline-flex items-center gap-1">
              <kbd :class="KBD_CLASSES">↑</kbd>
              <kbd :class="KBD_CLASSES">↓</kbd>
              navegar
            </span>
            <span class="inline-flex items-center gap-1">
              <kbd :class="KBD_CLASSES">Tab</kbd>
              grupos
            </span>
            <span class="inline-flex items-center gap-1">
              <kbd :class="KBD_CLASSES">Enter</kbd>
              abrir
            </span>
          </div>
          <div class="flex items-center gap-2 font-mono text-tiny text-faint">
            <span>#cards</span>
            <span>@agentes</span>
            <span>&gt;ações</span>
          </div>
        </footer>
      </section>
    </div>
  </Teleport>
</template>
