<script lang="ts">
export interface SearchPickerOption {
  id: string
  label: string
  disabled?: boolean
  keywords?: string
}

const DEFAULT_EMPTY_TEXT = 'Nenhum item disponível.'
const DEFAULT_PLACEHOLDER = 'Selecionar'
const DEFAULT_SEARCH_PLACEHOLDER = 'Buscar…'
const DEFAULT_SEARCH_LABEL = 'Filtrar opções'
</script>

<script setup lang="ts" generic="T extends SearchPickerOption">
import { computed, nextTick, onMounted, onUnmounted, ref, useId, watch } from 'vue'
import { INK_FOCUS_CLASSES } from './handDrawn'

const POPOVER_GAP = 6
const VIEWPORT_MARGIN = 12

const TRIGGER_BASE_CLASSES = `inline-flex min-w-0 cursor-pointer items-center justify-between gap-2 rounded-md text-left transition-colors ${INK_FOCUS_CLASSES} disabled:cursor-not-allowed disabled:opacity-40`
const TRIGGER_VARIANT_CLASSES: Record<'input' | 'compact' | 'ghost', string> = {
  input: 'h-9 w-full border border-line bg-surface px-2.5 text-control text-ink enabled:hover:border-faint',
  compact: 'h-7 border border-line bg-surface px-2 text-small font-medium text-ink enabled:hover:border-faint enabled:hover:bg-hover',
  ghost: 'h-7 px-1.5 text-small font-medium text-muted enabled:hover:bg-hover enabled:hover:text-ink',
}
const POPOVER_SURFACE_CLASSES =
  'flex flex-col overflow-hidden rounded-md border border-line bg-surface shadow-paper'
const SEARCH_INPUT_CLASSES =
  'h-8 w-full min-w-0 rounded-sm border border-line bg-surface-2 pl-8 pr-2.5 text-caption text-ink placeholder:text-faint transition-colors focus:border-accent focus:bg-surface focus:outline-none'
const OPTION_BASE_CLASSES =
  'flex w-full cursor-pointer items-center gap-2.5 rounded-sm px-2 py-1.5 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-45'

const props = withDefaults(
  defineProps<{
    items: readonly T[]
    label: string
    placeholder?: string
    searchPlaceholder?: string
    searchLabel?: string
    emptyText?: string
    emptyQueryText?: (query: string) => string
    disabled?: boolean
    externalFilter?: boolean
    placement?: 'bottom' | 'top'
    align?: 'start' | 'end'
    floating?: 'absolute' | 'fixed'
    popoverClass?: string
    triggerClass?: string
    triggerVariant?: 'input' | 'compact' | 'ghost'
    showCheck?: boolean
    title?: string
    testId?: string
  }>(),
  {
    placeholder: DEFAULT_PLACEHOLDER,
    searchPlaceholder: DEFAULT_SEARCH_PLACEHOLDER,
    searchLabel: DEFAULT_SEARCH_LABEL,
    emptyText: DEFAULT_EMPTY_TEXT,
    emptyQueryText: undefined,
    disabled: false,
    externalFilter: false,
    placement: 'bottom',
    align: 'start',
    floating: 'absolute',
    popoverClass: 'w-80',
    triggerClass: undefined,
    triggerVariant: 'input',
    showCheck: true,
    title: undefined,
    testId: undefined,
  },
)

const model = defineModel<string | null>({ default: null })
const open = defineModel<boolean>('open', { default: false })
const query = defineModel<string>('query', { default: '' })

const emit = defineEmits<{
  select: [item: T]
}>()

const uid = useId()
const listboxId = `${uid}-listbox`
const rootRef = ref<HTMLElement | null>(null)
const triggerRef = ref<HTMLButtonElement | null>(null)
const popoverRef = ref<HTMLElement | null>(null)
const searchInputRef = ref<HTMLInputElement | null>(null)
const listboxRef = ref<HTMLElement | null>(null)
const activeIndex = ref(-1)
const fixedPos = ref({ top: 0, left: 0 })

function normalizeText(value: string): string {
  return value.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase().trim()
}

const filteredItems = computed<readonly T[]>(() => {
  if (props.externalFilter) return props.items
  const needle = normalizeText(query.value)
  if (!needle) return props.items
  return props.items.filter((item) => {
    const haystack = normalizeText(`${item.label} ${item.keywords ?? ''}`)
    return haystack.includes(needle)
  })
})

const selectedItem = computed<T | null>(() => {
  if (model.value === null) return null
  return props.items.find((item) => item.id === model.value) ?? null
})

function optionId(index: number): string {
  return `${uid}-option-${index}`
}

const activeOptionId = computed<string | undefined>(() => {
  if (!open.value || activeIndex.value < 0 || activeIndex.value >= filteredItems.value.length) {
    return undefined
  }
  return optionId(activeIndex.value)
})

const emptyMessage = computed(() => {
  const trimmed = query.value.trim()
  if (trimmed !== '') {
    return props.emptyQueryText ? props.emptyQueryText(trimmed) : `Nenhum resultado para «${trimmed}»`
  }
  return props.emptyText
})

const absolutePositionClasses = computed(() => {
  const vertical = props.placement === 'top' ? 'bottom-full mb-1.5' : 'top-full mt-1.5'
  const horizontal = props.align === 'end' ? 'right-0' : 'left-0'
  return `absolute z-40 ${vertical} ${horizontal}`
})

function isSelected(item: T): boolean {
  return model.value !== null && item.id === model.value
}

function firstEnabledIndex(list: readonly T[]): number {
  return list.findIndex((item) => !item.disabled)
}

function lastEnabledIndex(list: readonly T[]): number {
  for (let i = list.length - 1; i >= 0; i -= 1) {
    if (!list[i]?.disabled) return i
  }
  return -1
}

function syncInitialActiveIndex(): void {
  const list = filteredItems.value
  const selectedIdx = list.findIndex((item) => isSelected(item) && !item.disabled)
  activeIndex.value = selectedIdx >= 0 ? selectedIdx : firstEnabledIndex(list)
}

function scrollActiveIntoView(): void {
  const id = activeOptionId.value
  if (!id || !listboxRef.value) return
  const target = listboxRef.value.querySelector<HTMLElement>(`#${CSS.escape(id)}`)
  target?.scrollIntoView({ block: 'nearest' })
}

function placePopover(): void {
  if (props.floating !== 'fixed' || !open.value || !triggerRef.value || !popoverRef.value) return
  const triggerRect = triggerRef.value.getBoundingClientRect()
  const popoverRect = popoverRef.value.getBoundingClientRect()
  const below = triggerRect.bottom + POPOVER_GAP
  const above = triggerRect.top - POPOVER_GAP - popoverRect.height
  const fitsBelow = below + popoverRect.height <= window.innerHeight - VIEWPORT_MARGIN
  const fitsAbove = above >= VIEWPORT_MARGIN

  let top: number
  if (props.placement === 'top') {
    top = fitsAbove || !fitsBelow ? above : below
  } else {
    top = fitsBelow || !fitsAbove ? below : above
  }

  const preferredLeft =
    props.align === 'end' ? triggerRect.right - popoverRect.width : triggerRect.left
  const left = Math.max(
    VIEWPORT_MARGIN,
    Math.min(preferredLeft, window.innerWidth - popoverRect.width - VIEWPORT_MARGIN),
  )

  fixedPos.value = {
    top: Math.max(VIEWPORT_MARGIN, Math.min(top, window.innerHeight - popoverRect.height - VIEWPORT_MARGIN)),
    left,
  }
}

async function openPicker(): Promise<void> {
  if (props.disabled || open.value) return
  query.value = ''
  open.value = true
}

async function close(restoreFocus: boolean): Promise<void> {
  if (!open.value) return
  open.value = false
  activeIndex.value = -1
  if (restoreFocus) {
    await nextTick()
    triggerRef.value?.focus()
  }
}

function toggle(): void {
  if (props.disabled) return
  if (open.value) {
    void close(true)
  } else {
    void openPicker()
  }
}

function selectItem(item: T): void {
  if (item.disabled) return
  model.value = item.id
  emit('select', item)
  void close(true)
}

function moveActive(delta: 1 | -1): void {
  const list = filteredItems.value
  const len = list.length
  if (len === 0) {
    activeIndex.value = -1
    return
  }
  const start = activeIndex.value
  for (let step = 1; step <= len; step += 1) {
    const candidate = start < 0 ? (delta === 1 ? step - 1 : len - step) : (start + delta * step + len) % len
    if (!list[candidate]?.disabled) {
      activeIndex.value = candidate
      void nextTick(scrollActiveIntoView)
      return
    }
  }
  activeIndex.value = -1
}

function onTriggerKeydown(event: KeyboardEvent): void {
  if (props.disabled) return
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    void openPicker()
    return
  }
  if (event.key === 'Escape' && open.value) {
    event.preventDefault()
    event.stopPropagation()
    void close(true)
  }
}

function onSearchKeydown(event: KeyboardEvent): void {
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    moveActive(1)
    return
  }
  if (event.key === 'ArrowUp') {
    event.preventDefault()
    moveActive(-1)
    return
  }
  if (event.key === 'Home') {
    event.preventDefault()
    activeIndex.value = firstEnabledIndex(filteredItems.value)
    void nextTick(scrollActiveIntoView)
    return
  }
  if (event.key === 'End') {
    event.preventDefault()
    activeIndex.value = lastEnabledIndex(filteredItems.value)
    void nextTick(scrollActiveIntoView)
    return
  }
  if (event.key === 'Enter') {
    event.preventDefault()
    const current = filteredItems.value[activeIndex.value]
    if (current && !current.disabled) {
      selectItem(current)
    }
    return
  }
  if (event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    void close(true)
    return
  }
  if (event.key === 'Tab') {
    void close(false)
  }
}

function onDocumentPointerDown(event: PointerEvent): void {
  if (!open.value) return
  if (event.target instanceof Node && !rootRef.value?.contains(event.target)) {
    void close(false)
  }
}

watch(open, async (isOpen) => {
  if (!isOpen) {
    activeIndex.value = -1
    return
  }
  query.value = ''
  await nextTick()
  syncInitialActiveIndex()
  if (props.floating === 'fixed') {
    placePopover()
  }
  searchInputRef.value?.focus()
})

watch(filteredItems, async (list) => {
  if (!open.value) return
  const current = list[activeIndex.value]
  if (!current || current.disabled) {
    activeIndex.value = firstEnabledIndex(list)
  }
  if (props.floating === 'fixed') {
    await nextTick()
    placePopover()
  }
})

watch(
  () => props.disabled,
  (isDisabled) => {
    if (isDisabled && open.value) {
      void close(false)
    }
  },
)

onMounted(() => {
  document.addEventListener('pointerdown', onDocumentPointerDown)
  window.addEventListener('resize', placePopover)
  window.addEventListener('scroll', placePopover, true)
})

onUnmounted(() => {
  document.removeEventListener('pointerdown', onDocumentPointerDown)
  window.removeEventListener('resize', placePopover)
  window.removeEventListener('scroll', placePopover, true)
})

defineExpose({
  open: openPicker,
  close,
  focusTrigger: () => triggerRef.value?.focus(),
})
</script>

<template>
  <div ref="rootRef" class="relative inline-flex min-w-0 max-w-full">
    <button
      ref="triggerRef"
      type="button"
      :disabled="disabled"
      :aria-label="label"
      :title="title"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-controls="open ? listboxId : undefined"
      :data-testid="testId"
      :class="[TRIGGER_BASE_CLASSES, TRIGGER_VARIANT_CLASSES[triggerVariant], triggerClass]"
      @click="toggle"
      @keydown="onTriggerKeydown"
    >
      <slot name="trigger" :selected="selectedItem" :open="open" :toggle="toggle">
        <span :class="['min-w-0 truncate', selectedItem ? 'text-ink' : 'text-muted']">
          {{ selectedItem?.label ?? placeholder }}
        </span>
        <svg
          class="size-3.5 shrink-0 text-muted transition-transform duration-150 motion-reduce:transition-none"
          :class="open && 'rotate-180'"
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
      </slot>
    </button>

    <div
      v-if="open"
      ref="popoverRef"
      :data-testid="testId ? `${testId}-popover` : undefined"
      :class="[
        POPOVER_SURFACE_CLASSES,
        popoverClass,
        'max-w-[calc(100vw-1.5rem)]',
        floating === 'fixed' ? 'fixed z-60' : absolutePositionClasses,
      ]"
      :style="floating === 'fixed' ? { top: `${fixedPos.top}px`, left: `${fixedPos.left}px` } : undefined"
      @keydown.esc.stop.prevent="close(true)"
    >
      <div class="border-b border-line p-2">
        <div class="relative flex items-center">
          <svg
            class="pointer-events-none absolute left-2.5 size-3.5 text-muted"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
          <input
            ref="searchInputRef"
            v-model="query"
            type="search"
            role="combobox"
            autocomplete="off"
            aria-autocomplete="list"
            :aria-expanded="open"
            :aria-controls="listboxId"
            :aria-activedescendant="activeOptionId"
            :aria-label="searchLabel"
            :placeholder="searchPlaceholder"
            :data-testid="testId ? `${testId}-search` : undefined"
            :class="SEARCH_INPUT_CLASSES"
            @keydown="onSearchKeydown"
          />
        </div>
      </div>

      <div
        :id="listboxId"
        ref="listboxRef"
        role="listbox"
        :aria-label="label"
        class="flex max-h-68 flex-col gap-0.5 overflow-y-auto p-1.5"
      >
        <p
          v-if="filteredItems.length === 0"
          role="status"
          :data-testid="testId ? `${testId}-empty` : undefined"
          class="px-2.5 py-5 text-center text-caption text-muted"
        >
          <slot name="empty" :query="query">
            {{ emptyMessage }}
          </slot>
        </p>

        <button
          v-for="(item, index) in filteredItems"
          :id="optionId(index)"
          :key="item.id"
          type="button"
          role="option"
          tabindex="-1"
          :disabled="item.disabled"
          :aria-selected="isSelected(item)"
          :data-testid="testId ? `${testId}-option-${item.id}` : undefined"
          :class="[
            OPTION_BASE_CLASSES,
            index === activeIndex
              ? 'bg-hover text-ink'
              : isSelected(item)
                ? 'bg-accent-soft/60 text-ink'
                : 'text-ink enabled:hover:bg-hover',
          ]"
          @mousedown.prevent
          @mouseenter="!item.disabled && (activeIndex = index)"
          @click="selectItem(item)"
        >
          <span v-if="$slots.leading" class="flex shrink-0 items-center">
            <slot name="leading" :item="item" :selected="isSelected(item)" :active="index === activeIndex" />
          </span>

          <span class="min-w-0 flex-1">
            <slot name="option" :item="item" :selected="isSelected(item)" :active="index === activeIndex">
              <span class="block truncate text-control font-medium text-ink">{{ item.label }}</span>
            </slot>
          </span>

          <span v-if="$slots.trailing || showCheck" class="flex shrink-0 items-center gap-1.5">
            <slot name="trailing" :item="item" :selected="isSelected(item)" :active="index === activeIndex" />
            <span
              v-if="showCheck"
              class="inline-flex size-4 shrink-0 items-center justify-center text-accent"
              aria-hidden="true"
            >
              <svg
                v-if="isSelected(item)"
                class="size-3.5"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.4"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </span>
          </span>
        </button>
      </div>
    </div>
  </div>
</template>
