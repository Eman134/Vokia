<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import ProjectIcon from '../projects/ProjectIcon.vue'
import { isProjectError } from '../projects/projectsApi'
import { useProjects } from '../projects/useProjects'
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'

const PIN_LABEL = 'Fixar um projeto'
const MENU_TITLE = 'Fixar projeto'
const EMPTY_MENU_TEXT = 'Nenhum projeto com pasta para fixar.'
const UNPIN_LABEL = 'Desafixar'
const PIN_GLYPH_PATH = 'M5.5 2.5h5M6 2.5v4L4.5 9h7L10 6.5v-4M8 9v5'
const ROOT_CLASSES = 'relative flex shrink-0 items-center'
const MENU_CLASSES = 'absolute bottom-full left-0 pb-2'
const MENU_FRAME_CLASSES = 'w-60 rounded-popover border border-line bg-surface p-1.5 shadow-float'
const MENU_TITLE_CLASSES = 'px-2 py-1.5 text-caption text-muted'
const PIN_ICON_CLASSES = 'size-4 text-faint'

const projects = useProjects()
const root = ref<HTMLElement | null>(null)
const open = ref(false)
const busy = ref(false)
const failure = ref<string | null>(null)

const withFolder = computed(() => projects.projects.filter((project) => project.path.trim() !== ''))
const toggleLabel = computed(() => (projects.pinned ? `Fixado: ${projects.pinned.name}` : PIN_LABEL))

function toggleMenu(): void {
  open.value = !open.value
  failure.value = null
}

async function choose(id: number | null): Promise<void> {
  failure.value = null
  busy.value = true
  try {
    await projects.pin(id)
    open.value = false
  } catch (cause) {
    failure.value = isProjectError(cause) ? cause.message : String(cause)
  } finally {
    busy.value = false
  }
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && !event.defaultPrevented) {
    open.value = false
  }
}

function onPointerDown(event: PointerEvent): void {
  if (!open.value || !(event.target instanceof Node) || root.value?.contains(event.target)) {
    return
  }
  open.value = false
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  document.addEventListener('pointerdown', onPointerDown)
})
onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
  document.removeEventListener('pointerdown', onPointerDown)
})
</script>

<template>
  <div ref="root" :class="ROOT_CLASSES">
    <UiButton
      variant="quiet"
      size="sm"
      data-testid="pin-toggle"
      aria-haspopup="menu"
      :aria-expanded="open"
      :aria-label="toggleLabel"
      :title="toggleLabel"
      @press="toggleMenu"
    >
      <ProjectIcon v-if="projects.pinned" :icon="projects.pinned.symbol" class="size-4" />
      <svg
        v-else
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.4"
        stroke-linecap="round"
        stroke-linejoin="round"
        :class="PIN_ICON_CLASSES"
        aria-hidden="true"
      >
        <path :d="PIN_GLYPH_PATH" />
      </svg>
    </UiButton>

    <div v-if="open" :class="MENU_CLASSES">
      <div :class="MENU_FRAME_CLASSES">
        <UiNotice v-if="failure" class="mb-1.5" :text="failure" @close="failure = null" />
        <p :class="MENU_TITLE_CLASSES">{{ MENU_TITLE }}</p>
        <ul v-if="withFolder.length > 0" role="menu" class="flex flex-col gap-0.5">
          <li v-for="project in withFolder" :key="project.id" role="none">
            <UiButton
              variant="quiet"
              size="sm"
              role="menuitem"
              class="w-full"
              :disabled="busy"
              :aria-current="project.pinned ? 'true' : undefined"
              @press="choose(project.id)"
            >
              <span class="flex w-full items-center gap-2">
                <ProjectIcon :icon="project.symbol" class="size-4" />
                <span class="truncate">{{ project.name }}</span>
              </span>
            </UiButton>
          </li>
          <li v-if="projects.pinned" role="none">
            <UiButton variant="quiet" size="sm" role="menuitem" class="w-full" :disabled="busy" @press="choose(null)">
              {{ UNPIN_LABEL }}
            </UiButton>
          </li>
        </ul>
        <p v-else :class="MENU_TITLE_CLASSES">{{ EMPTY_MENU_TEXT }}</p>
      </div>
    </div>
  </div>
</template>
