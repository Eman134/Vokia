<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, useId, watch } from 'vue'
import BypassWarning from '../../permissions/BypassWarning.vue'
import { EXECUTION_MODE_INFO, EXECUTION_MODES, type ExecutionMode } from '../../permissions/executionModes'
import { useExecutionMode } from '../../permissions/useExecutionMode'
import { useModeChoice } from '../../permissions/useModeChoice'
import { useProjects } from '../../projects/useProjects'
import UiButton from '../../../ui/UiButton.vue'
import { INK_FOCUS_CLASSES } from '../../../ui/handDrawn'

const open = defineModel<boolean>('open', { required: true })

const LABEL_PREFIX = 'Modo de execução:'
const LIST_LABEL = 'Modo de execução'
const OPTION_CLASSES = `flex w-full cursor-pointer items-start gap-2.5 rounded-md px-2.5 py-2 text-left ${INK_FOCUS_CLASSES}`
const ICON_CLASSES = 'size-4 shrink-0'

const store = useExecutionMode()
const projects = useProjects()
const projectId = computed(() => projects.selected?.id ?? null)
const mode = computed(() => store.modeOf(projectId.value))
const current = computed(() => EXECUTION_MODE_INFO[mode.value])
const buttonLabel = computed(() => `${LABEL_PREFIX} ${current.value.label}`)
const { warningOpen, choose, acceptWarning } = useModeChoice(projectId)
const rootRef = ref<HTMLElement>()
const triggerRef = ref<{ $el: HTMLElement } | null>(null)
const optionRefs = ref<HTMLElement[]>([])
const listId = useId()
const modeAnimating = ref(false)
let modeAnimTimer: ReturnType<typeof setTimeout> | null = null

watch(mode, (next, prev) => {
  if (prev === undefined || next === prev) return
  modeAnimating.value = true
  if (modeAnimTimer !== null) clearTimeout(modeAnimTimer)
  modeAnimTimer = setTimeout(() => {
    modeAnimating.value = false
    modeAnimTimer = null
  }, 650)
})

watch(
  projectId,
  (id) => {
    if (id !== null) void store.load(id)
  },
  { immediate: true },
)

watch(open, async (isOpen) => {
  if (!isOpen) return
  await nextTick()
  optionRefs.value[EXECUTION_MODES.indexOf(mode.value)]?.focus()
})

function close(): void {
  open.value = false
  triggerRef.value?.$el.focus()
}

function select(next: ExecutionMode): void {
  close()
  choose(next)
}

function onListKeydown(event: KeyboardEvent): void {
  const delta = event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0
  if (delta === 0) return
  event.preventDefault()
  const focused = optionRefs.value.findIndex((option) => option === document.activeElement)
  const count = EXECUTION_MODES.length
  optionRefs.value[(focused + delta + count) % count]?.focus()
}

function onKeydown(event: KeyboardEvent): void {
  if (open.value && event.key === 'Escape' && !event.defaultPrevented) close()
}

function onDocumentMouseDown(event: MouseEvent): void {
  if (open.value && event.target instanceof Node && !rootRef.value?.contains(event.target)) {
    open.value = false
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  document.addEventListener('mousedown', onDocumentMouseDown)
})

onUnmounted(() => {
  if (modeAnimTimer !== null) clearTimeout(modeAnimTimer)
  window.removeEventListener('keydown', onKeydown)
  document.removeEventListener('mousedown', onDocumentMouseDown)
})
</script>

<template>
  <div v-if="projectId !== null" ref="rootRef" class="relative inline-flex items-center">
    <UiButton
      ref="triggerRef"
      variant="quiet"
      size="sm"
      class="relative w-7 px-0"
      :class="[current.colorClass, modeAnimating && 'mode-shake']"
      :aria-label="buttonLabel"
      :title="buttonLabel"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-controls="open ? listId : undefined"
      :data-mode="mode"
      data-testid="execution-mode-selector"
      @press="open = !open"
    >
      <svg :class="[ICON_CLASSES, current.colorClass]" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path :d="current.icon" />
      </svg>
    </UiButton>
    <div v-if="open" class="absolute left-0 bottom-full z-40 pb-2">
      <ul
        :id="listId"
        role="listbox"
        :aria-label="LIST_LABEL"
        class="flex w-72 flex-col gap-0.5 rounded-md border border-line bg-surface p-1.5 shadow-paper"
        data-testid="execution-mode-list"
        @keydown="onListKeydown"
      >
        <li v-for="option in EXECUTION_MODES" :key="option" role="presentation">
          <button
            ref="optionRefs"
            type="button"
            role="option"
            :aria-selected="option === mode"
            :class="[OPTION_CLASSES, option === mode ? 'bg-hover' : 'hover:bg-hover']"
            :data-testid="`execution-mode-option-${option}`"
            @click="select(option)"
          >
            <span
              class="mt-0.5 inline-flex size-5 shrink-0 items-center justify-center"
              :class="EXECUTION_MODE_INFO[option].colorClass"
            >
              <svg :class="[ICON_CLASSES, current.colorClass]" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path :d="EXECUTION_MODE_INFO[option].icon" />
              </svg>
            </span>
            <span class="flex min-w-0 flex-col">
              <span class="text-caption font-semibold text-ink">{{ EXECUTION_MODE_INFO[option].label }}</span>
              <span class="text-small text-muted">{{ EXECUTION_MODE_INFO[option].description }}</span>
            </span>
          </button>
        </li>
      </ul>
    </div>
    <BypassWarning v-model:open="warningOpen" @accept="acceptWarning" />
  </div>
</template>

<style scoped>
@keyframes mode-shake {
  0%, 100% {
    transform: translateX(0) rotate(0deg);
  }
  20% {
    transform: translateX(-2px) rotate(-6deg);
  }
  40% {
    transform: translateX(2px) rotate(6deg);
  }
  60% {
    transform: translateX(-1.5px) rotate(-4deg);
  }
  80% {
    transform: translateX(1.5px) rotate(4deg);
  }
}

.mode-shake {
  animation: mode-shake 0.45s ease-in-out;
}

@media (prefers-reduced-motion: reduce) {
  .mode-shake {
    animation: none;
  }
}
</style>
