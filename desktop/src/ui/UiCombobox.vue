<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue'

const props = withDefaults(
  defineProps<{
    modelValue: string
    options: readonly string[]
    placeholder?: string
    disabled?: boolean
    loading?: boolean
    refreshLabel?: string
    showRefresh?: boolean
    id?: string
  }>(),
  {
    placeholder: undefined,
    disabled: false,
    loading: false,
    refreshLabel: 'Atualizar',
    showRefresh: false,
    id: undefined,
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
  refresh: []
}>()

const generatedId = useId()
const inputId = computed(() => props.id ?? `combobox-${generatedId}`)
const listboxId = computed(() => `${inputId.value}-listbox`)

const rootRef = ref<HTMLElement | null>(null)
const inputRef = ref<HTMLInputElement | null>(null)
const isOpen = ref(false)
const activeIndex = ref(-1)
const filterActive = ref(false)

const filteredOptions = computed(() => {
  if (!filterActive.value) return props.options
  const query = props.modelValue.trim().toLowerCase()
  if (!query) return props.options
  return props.options.filter((option) => option.toLowerCase().includes(query))
})

const showDropdown = computed(
  () => !props.disabled && isOpen.value && (filteredOptions.value.length > 0 || props.loading),
)

const activeOptionId = computed(() =>
  showDropdown.value && activeIndex.value >= 0 && activeIndex.value < filteredOptions.value.length
    ? `${inputId.value}-option-${activeIndex.value}`
    : undefined,
)

watch(filteredOptions, (list) => {
  if (list.length === 0) {
    activeIndex.value = -1
  } else if (activeIndex.value >= list.length) {
    activeIndex.value = list.length - 1
  }
})

function openDropdown(enableFilter: boolean) {
  if (props.disabled) return
  filterActive.value = enableFilter
  isOpen.value = true
  const currentIdx = filteredOptions.value.indexOf(props.modelValue)
  activeIndex.value = currentIdx >= 0 ? currentIdx : filteredOptions.value.length > 0 ? 0 : -1
}

function closeDropdown() {
  isOpen.value = false
  activeIndex.value = -1
  filterActive.value = false
}

function toggleDropdown() {
  if (props.disabled) return
  if (isOpen.value) {
    closeDropdown()
  } else {
    openDropdown(false)
    inputRef.value?.focus()
  }
}

function onInput(event: Event) {
  const value = (event.target as HTMLInputElement).value
  emit('update:modelValue', value)
  if (!isOpen.value) {
    openDropdown(true)
  } else {
    filterActive.value = true
    activeIndex.value = filteredOptions.value.length > 0 ? 0 : -1
  }
}

function selectOption(option: string) {
  emit('update:modelValue', option)
  closeDropdown()
  inputRef.value?.focus()
}

function onKeydown(event: KeyboardEvent) {
  if (props.disabled) return

  if (event.key === 'ArrowDown') {
    event.preventDefault()
    if (!isOpen.value) {
      openDropdown(false)
      return
    }
    const len = filteredOptions.value.length
    if (len > 0) {
      activeIndex.value = activeIndex.value < len - 1 ? activeIndex.value + 1 : 0
    }
    return
  }

  if (event.key === 'ArrowUp') {
    event.preventDefault()
    if (!isOpen.value) {
      openDropdown(false)
      if (filteredOptions.value.length > 0) {
        activeIndex.value = filteredOptions.value.length - 1
      }
      return
    }
    const len = filteredOptions.value.length
    if (len > 0) {
      activeIndex.value = activeIndex.value > 0 ? activeIndex.value - 1 : len - 1
    }
    return
  }

  if (event.key === 'Enter') {
    if (showDropdown.value && activeIndex.value >= 0 && activeIndex.value < filteredOptions.value.length) {
      event.preventDefault()
      selectOption(filteredOptions.value[activeIndex.value])
    }
    return
  }

  if (event.key === 'Escape') {
    if (isOpen.value) {
      event.preventDefault()
      event.stopPropagation()
      closeDropdown()
    }
    return
  }

  if (event.key === 'Tab') {
    closeDropdown()
  }
}

function onFocusOut(event: FocusEvent) {
  const nextTarget = event.relatedTarget as Node | null
  if (nextTarget && rootRef.value?.contains(nextTarget)) return
  closeDropdown()
}

function onRefresh() {
  if (props.disabled || props.loading) return
  emit('refresh')
}

const INPUT_CLASSES =
  'h-9 w-full min-w-0 rounded-control border border-line bg-surface pl-3 pr-9 text-control text-ink placeholder:text-faint transition-colors enabled:hover:border-faint focus:outline-2 focus:outline-solid focus:outline-accent disabled:cursor-default disabled:opacity-40'
const REFRESH_CLASSES =
  'inline-flex h-9 shrink-0 cursor-pointer items-center gap-1.5 rounded-control border border-line bg-surface px-2.5 text-label font-medium text-muted enabled:hover:border-faint enabled:hover:bg-hover enabled:hover:text-ink focus:outline-2 focus:outline-solid focus:outline-accent disabled:cursor-default disabled:opacity-40'
</script>

<template>
  <div ref="rootRef" class="flex items-center gap-2" @focusout="onFocusOut">
    <div class="relative min-w-0 flex-1">
      <input
        :id="inputId"
        ref="inputRef"
        type="text"
        role="combobox"
        autocomplete="off"
        :value="modelValue"
        :placeholder="placeholder"
        :disabled="disabled"
        :aria-disabled="disabled || undefined"
        :aria-expanded="showDropdown"
        aria-autocomplete="list"
        :aria-controls="listboxId"
        :aria-activedescendant="activeOptionId"
        :class="INPUT_CLASSES"
        @input="onInput"
        @focus="openDropdown(false)"
        @keydown="onKeydown"
      />
      <button
        type="button"
        tabindex="-1"
        :disabled="disabled"
        aria-label="Alternar sugestões"
        class="absolute top-1/2 right-1.5 flex size-6 -translate-y-1/2 cursor-pointer items-center justify-center rounded-control-sm text-muted enabled:hover:text-ink disabled:cursor-default disabled:opacity-40"
        @mousedown.prevent
        @click="toggleDropdown"
      >
        <svg
          class="size-4 transition-transform"
          :class="showDropdown ? 'rotate-180' : ''"
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
      </button>

      <ul
        v-if="showDropdown"
        :id="listboxId"
        role="listbox"
        class="absolute right-0 left-0 z-30 mt-1 max-h-56 overflow-y-auto rounded-control border border-line bg-surface py-1 shadow-float"
      >
        <li v-if="loading && filteredOptions.length === 0" class="px-3 py-1.5 text-caption text-muted">
          Carregando...
        </li>
        <li
          v-for="(option, index) in filteredOptions"
          :id="`${inputId}-option-${index}`"
          :key="option"
          role="option"
          :aria-selected="option === modelValue"
          :class="[
            'cursor-pointer px-3 py-1.5 text-control',
            index === activeIndex
              ? 'bg-hover text-ink'
              : option === modelValue
                ? 'bg-accent-soft text-accent'
                : 'text-ink hover:bg-hover',
          ]"
          @mousedown.prevent
          @mouseenter="activeIndex = index"
          @click="selectOption(option)"
        >
          {{ option }}
        </li>
      </ul>
    </div>

    <button
      v-if="showRefresh"
      type="button"
      :disabled="disabled || loading"
      :aria-label="refreshLabel"
      :class="REFRESH_CLASSES"
      @click="onRefresh"
    >
      <svg
        class="size-3.5"
        :class="loading ? 'animate-spin' : ''"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M21 12a9 9 0 1 1-2.64-6.36" />
        <path d="M21 3v6h-6" />
      </svg>
      <span>{{ refreshLabel }}</span>
    </button>
  </div>
</template>
