<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import {
  PAGE_LABELS,
  PROJECT_LIST_PAGE,
  PROJECT_TOOLS,
  RAIL_COLLAPSED_WIDTH,
  RAIL_EXPANDED_WIDTH,
  WORKSPACE_ITEMS,
  isProjectPage,
  type AppPage,
} from './railNav'
import { useNavigation } from './useNavigation'
import { useRail } from './useRail'
import UiHandDivider from '../ui/UiHandDivider.vue'
import UiHandLine from '../ui/UiHandLine.vue'
import UiIcon from '../ui/UiIcon.vue'
import { HAND_MARK_CLASSES, INK_FOCUS_CLASSES, handReveal } from '../ui/handDrawn'
import UsageAvatar from '../features/usage/UsageAvatar.vue'
import VokiaLogo from '../features/brand/VokiaLogo.vue'
import ProjectIcon from '../features/projects/ProjectIcon.vue'
import { projectColorOf } from '../features/projects/projectColors'
import type { Project } from '../features/projects/projectsApi'
import { useProjects } from '../features/projects/useProjects'
import { useGlobalSearch } from '../features/search/useGlobalSearch'
import { useFeed } from '../features/team/useFeed'

const NEW_PROJECT_PAGE: AppPage = 'novo-projeto'

const ITEM = `group relative flex h-9.5 [@media(max-height:860px)]:h-8.5 [@media(max-height:740px)]:h-7.5 cursor-pointer items-center rounded-md whitespace-nowrap text-left ${INK_FOCUS_CLASSES}`
const ITEM_EXPANDED_PAD = 'pr-2.5'
// Caixa fixa do ícone: mantém x e y do ícone iguais nos dois modos do trilho.
const ICON_BOX = 'grid size-9.5 [@media(max-height:860px)]:size-8.5 [@media(max-height:740px)]:size-7.5 flex-none place-items-center'
const PROJECT_BADGE = 'size-4.5 flex-none'
// SVG não estica por left/right como uma div: a largura precisa ser explícita para o traço não vazar do botão.
const ACTIVE_MARK = `${HAND_MARK_CLASSES} text-ink-pen! bottom-0.5 left-2.5 w-[calc(100%-1.25rem)]`
const ITEM_ON = 'text-ink-pen'
const ITEM_OFF = 'text-muted hover:bg-hover hover:text-ink'
const TOGGLE = 'cursor-pointer bg-transparent text-ink'
const GROUP = 'flex flex-col gap-0.5'
const GROUP_LABEL = 'pl-1.25 text-caption text-muted'
const ICON_SIZE = 17
const SEARCH_ICON = 'M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10z M10.8 10.8 14 14'
const SEARCH_LABEL = 'Buscar'
const SEARCH_SHORTCUT = typeof navigator !== 'undefined' && /mac/i.test(navigator.platform) ? '⌘K' : 'Ctrl+K'
const SEARCH_KBD = 'ml-auto rounded-sm border border-line bg-surface-2 px-1.5 py-0.5 font-mono text-tiny text-faint'
const NEW_PROJECT_ICON = 'M8 3v10M3 8h10'
const PIN_GLYPH_PATH = 'M5.5 2.5h5M6 2.5v4L4.5 9h7L10 6.5v-4M8 9v5'
const SETTINGS_ICON =
  'M8 5.8a2.2 2.2 0 1 0 0 4.4 2.2 2.2 0 1 0 0-4.4z M8 3.2 L8.67 3.25 L9.6 1.6 L10.89 2.07 L10.54 3.93 L11.39 4.61 L11.83 5.11 L13.66 4.6 L14.24 5.85 L12.68 6.92 L12.8 8 L12.75 8.67 L14.4 9.6 L13.93 10.89 L12.07 10.54 L11.39 11.39 L10.89 11.83 L11.4 13.66 L10.15 14.24 L9.08 12.68 L8 12.8 L7.33 12.75 L6.4 14.4 L5.11 13.93 L5.46 12.07 L4.61 11.39 L4.17 10.89 L2.34 11.4 L1.76 10.15 L3.32 9.08 L3.2 8 L3.25 7.33 L1.6 6.4 L2.07 5.11 L3.93 5.46 L4.61 4.61 L5.11 4.17 L4.6 2.34 L5.85 1.76 L6.92 3.32 Z'
const PANEL_ICON = 'M2 2.5h12v11H2zM6 2.5v11'
const PANEL_COLLAPSE_ARROW = 'M11 6L9 8l2 2'
const PANEL_EXPAND_ARROW = 'M9 6l2 2-2 2'
const LABEL = 'overflow-hidden text-ellipsis text-label'
// O nome do projeto saiu do cabeçalho das páginas e vive aqui, no estilo manuscrito.
const PROJECT_LABEL = 'overflow-hidden text-ellipsis font-hand text-annotation'
const RAIL_MOTION = 'duration-180 ease-[ease]'
const SUBLIST = `grid transition-[grid-template-rows,opacity] ${RAIL_MOTION}`
const SUBLIST_OPEN = 'grid-rows-[1fr] opacity-100'
const SUBLIST_CLOSED = 'grid-rows-[0fr] opacity-0'

const navigation = useNavigation()
// Novo projeto é página fora do trilho: o item Projetos segue ativo enquanto ela aparece.
const railPage = computed<AppPage>(() =>
  navigation.page === 'novo-projeto' ? PROJECT_LIST_PAGE : navigation.page,
)
const rail = useRail()
const { expanded } = storeToRefs(rail)
const projectsStore = useProjects()
const globalSearch = useGlobalSearch()
const feed = useFeed()
const openProject = ref<string | null>(projectsStore.selected?.name ?? null)
watch(
  () => projectsStore.selected?.name ?? null,
  (name) => {
    openProject.value = name
  },
)
const itemSize = computed(() => [ITEM, expanded.value && ITEM_EXPANDED_PAD])
const togglePath = computed(() => `${PANEL_ICON} ${expanded.value ? PANEL_COLLAPSE_ARROW : PANEL_EXPAND_ARROW}`)
const toggleLabel = computed(() => (expanded.value ? 'Recolher menu' : 'Expandir menu'))
const hasSingleProject = computed(() => projectsStore.projects.length === 1)
const pinBusyId = ref<number | null>(null)

function canPinProject(project: Project): boolean {
  return projectsStore.projects.length > 1 && project.path.trim() !== ''
}

async function togglePinProject(project: Project): Promise<void> {
  if (pinBusyId.value !== null) return
  pinBusyId.value = project.id
  try {
    await projectsStore.pin(project.pinned ? null : project.id)
  } catch (cause) {
    feed.feed.push({ type: 'error', message: cause instanceof Error ? cause.message : String(cause) })
  } finally {
    pinBusyId.value = null
  }
}

function isActive(page: AppPage) {
  return page === railPage.value
}

function isOpen(projectName: string) {
  return hasSingleProject.value || openProject.value === projectName
}

function isActiveSubpage(projectName: string, page: AppPage) {
  return isOpen(projectName) && isActive(page)
}

function groupId(projectName: string) {
  return `rail-projeto-${projectName}`
}

async function enterProject(project: Project) {
  try {
    await navigation.enterProject(project)
  } catch (cause) {
    feed.feed.push({ type: 'error', message: cause instanceof Error ? cause.message : String(cause) })
  }
}

function toggleProject(project: Project) {
  if (projectsStore.selected?.id === project.id && !isProjectPage(navigation.page)) {
    openProject.value = project.name
    navigation.goTo(navigation.rememberedPage(project.id))
  } else if (isOpen(project.name) && !hasSingleProject.value) {
    openProject.value = null
  } else if (projectsStore.selected?.id === project.id) {
    openProject.value = project.name
  } else {
    void enterProject(project)
  }
}
</script>

<template>
  <div
    :class="`relative flex h-full shrink-0 flex-col items-start gap-3 bg-paper px-4.75 py-5 [@media(max-height:860px)]:gap-2 [@media(max-height:860px)]:py-3.5 [@media(max-height:740px)]:gap-1.5 [@media(max-height:740px)]:px-3.5 [@media(max-height:740px)]:py-2.5 font-sans transition-[width] motion-reduce:transition-none ${RAIL_MOTION}`"
    :style="{ width: `${expanded ? RAIL_EXPANDED_WIDTH : RAIL_COLLAPSED_WIDTH}px` }"
  >
    <UiHandDivider class="absolute inset-y-0 right-0 h-full text-line!" />
    <div class="flex shrink-0 items-center gap-2.5 pl-1.25 [@media(max-height:740px)]:pl-0.5">
      <button
        type="button"
        :class="`grid size-10 [@media(max-height:860px)]:size-9 [@media(max-height:740px)]:size-8 flex-none place-items-center rounded-md text-ink [--logo-cut:var(--color-paper)] ${INK_FOCUS_CLASSES}`"
        aria-label="Vokia, início"
        title="Vokia"
        @click="navigation.goTo(PROJECT_LIST_PAGE)"
      >
        <VokiaLogo variant="cheio" :size="24" />
      </button>
      <span v-if="expanded" class="order-2 text-lead text-ink">Vokia</span>
    </div>

    <div class="flex min-h-0 w-full flex-1 flex-col gap-3 [@media(max-height:860px)]:gap-2 [@media(max-height:740px)]:gap-1.5 overflow-y-auto pr-0.5">
    <button
      type="button"
      data-testid="rail-global-search"
      :class="[itemSize, ITEM_OFF, 'shrink-0', expanded && 'w-full']"
      :aria-label="`${SEARCH_LABEL} (${SEARCH_SHORTCUT})`"
      :title="`${SEARCH_LABEL} (${SEARCH_SHORTCUT})`"
      @click="globalSearch.openDialog()"
    >
      <span :class="ICON_BOX">
        <UiIcon class="flex-none" :path="SEARCH_ICON" :size="ICON_SIZE" />
      </span>
      <span v-if="expanded" :class="LABEL">{{ SEARCH_LABEL }}</span>
      <kbd v-if="expanded" :class="SEARCH_KBD">{{ SEARCH_SHORTCUT }}</kbd>
    </button>

    <p v-if="expanded" :class="GROUP_LABEL" aria-hidden="true">workspace</p>
    <nav aria-label="Workspace" :class="[GROUP, 'shrink-0', expanded && 'w-full']">
      <button
        v-for="item in WORKSPACE_ITEMS"
        :key="item.page"
        type="button"
        :class="[itemSize, isActive(item.page) ? ITEM_ON : ITEM_OFF]"
        :aria-current="isActive(item.page) ? 'page' : undefined"
        :aria-label="item.label"
        :title="item.label"
        @click="navigation.goTo(item.page)"
      >
        <span :class="ICON_BOX">
          <UiIcon class="flex-none" :path="item.path" :size="ICON_SIZE" />
        </span>
        <span v-if="expanded" :class="LABEL">{{ item.label }}</span>
        <UiHandLine :class="[ACTIVE_MARK, handReveal(isActive(item.page))]" />
      </button>
    </nav>

    <p v-if="expanded && (projectsStore.projects.length > 0 || projectsStore.loaded)" :class="GROUP_LABEL" aria-hidden="true">projetos</p>
    <nav
      v-if="projectsStore.projects.length > 0"
      aria-label="Projetos"
      :class="[GROUP, 'shrink-0', expanded && 'w-full']"
    >
      <div v-for="project in projectsStore.projects" :key="project.id" class="flex flex-col">
        <div class="group/proj relative flex items-center">
          <button
            type="button"
            :class="[itemSize, ITEM_OFF, isOpen(project.name) && 'bg-surface-2', expanded && 'w-full', expanded && canPinProject(project) && 'pr-8']"
            :aria-label="project.name"
            :aria-expanded="isOpen(project.name)"
            :aria-controls="groupId(project.name)"
            :title="project.name"
            @click="toggleProject(project)"
          >
            <span :class="ICON_BOX">
              <ProjectIcon :icon="project.symbol" :color="projectColorOf(project)" :class="PROJECT_BADGE" />
            </span>
            <span v-if="expanded" :class="PROJECT_LABEL">{{ project.name }}</span>
          </button>
          <button
            v-if="expanded && canPinProject(project)"
            type="button"
            data-testid="rail-project-pin"
            :disabled="pinBusyId === project.id"
            :class="[
              'absolute right-1.5 grid size-6 place-items-center rounded-sm transition-opacity',
              INK_FOCUS_CLASSES,
              project.pinned
                ? 'text-accent opacity-100 hover:bg-hover'
                : 'text-faint opacity-0 group-hover/proj:opacity-100 focus-visible:opacity-100 hover:bg-hover hover:text-ink',
            ]"
            :aria-label="project.pinned ? `Desafixar ${project.name}` : `Fixar ${project.name}`"
            :title="project.pinned ? `Desafixar ${project.name}` : `Fixar ${project.name}`"
            :aria-pressed="project.pinned"
            @click.stop="togglePinProject(project)"
          >
            <UiIcon :path="PIN_GLYPH_PATH" :size="13" />
          </button>
        </div>
        <div
          :id="groupId(project.name)"
          :class="[SUBLIST, isOpen(project.name) ? SUBLIST_OPEN : SUBLIST_CLOSED]"
          :inert="!isOpen(project.name)"
        >
          <div class="flex min-h-0 flex-col gap-0.5 overflow-hidden">
            <UiHandLine class="mx-1.5 my-0.75 h-1.5 text-line" />
            <button
              v-for="item in PROJECT_TOOLS"
              :key="item.page"
              type="button"
              :class="[itemSize, isActiveSubpage(project.name, item.page) ? ITEM_ON : ITEM_OFF]"
              :aria-current="isActiveSubpage(project.name, item.page) ? 'page' : undefined"
              :aria-label="item.label"
              :title="item.label"
              @click="navigation.goTo(item.page)"
            >
              <span :class="ICON_BOX">
                <UiIcon class="flex-none" :path="item.path" :size="ICON_SIZE" />
              </span>
              <span v-if="expanded" :class="LABEL">{{ item.label }}</span>
              <UiHandLine :class="[ACTIVE_MARK, handReveal(isActiveSubpage(project.name, item.page))]" />
            </button>
          </div>
        </div>
      </div>
    </nav>
    <div v-else class="min-h-0 flex-1">
      <nav v-if="projectsStore.loaded" aria-label="Projetos" :class="[GROUP, 'shrink-0', expanded && 'w-full']">
        <button
          type="button"
          :class="[itemSize, ITEM_OFF]"
          :aria-label="PAGE_LABELS[NEW_PROJECT_PAGE]"
          :title="PAGE_LABELS[NEW_PROJECT_PAGE]"
          @click="navigation.goTo(NEW_PROJECT_PAGE)"
        >
          <span :class="ICON_BOX">
            <UiIcon class="flex-none" :path="NEW_PROJECT_ICON" :size="ICON_SIZE" />
          </span>
          <span v-if="expanded" :class="LABEL">{{ PAGE_LABELS[NEW_PROJECT_PAGE] }}</span>
        </button>
      </nav>
    </div>
    </div>

    <div class="mt-auto flex shrink-0 flex-col items-start gap-1 pl-0.5">
      <button
        type="button"
        :class="[itemSize, isActive('config') ? ITEM_ON : ITEM_OFF]"
        :aria-current="isActive('config') ? 'page' : undefined"
        :aria-label="PAGE_LABELS.config"
        :title="PAGE_LABELS.config"
        @click="navigation.goTo('config')"
      >
        <span :class="ICON_BOX">
          <UiIcon class="flex-none" :path="SETTINGS_ICON" :size="ICON_SIZE" />
        </span>
        <span v-if="expanded" :class="LABEL">{{ PAGE_LABELS.config }}</span>
        <UiHandLine :class="[ACTIVE_MARK, handReveal(isActive('config'))]" />
      </button>
      <button
        v-if="rail.canExpand"
        type="button"
        :class="[itemSize, TOGGLE]"
        :aria-label="toggleLabel"
        :aria-expanded="expanded"
        :title="toggleLabel"
        @click="rail.toggle"
      >
        <span :class="ICON_BOX">
          <UiIcon class="flex-none" :path="togglePath" :size="ICON_SIZE" />
        </span>
        <span v-if="expanded" :class="LABEL">Recolher</span>
      </button>
      <UsageAvatar class="-ml-0.75" :expanded="expanded" />
    </div>
  </div>
</template>
