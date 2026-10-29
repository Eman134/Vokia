<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  CURRENT_PROJECT,
  PAGE_LABELS,
  PROJECT_TOOLS,
  PROJECTS,
  RAIL_COLLAPSED_WIDTH,
  RAIL_EXPANDED_WIDTH,
  WORKSPACE_ITEMS,
  type AppPage,
} from './railNav'
import { useNavigation } from './useNavigation'
import UsageAvatar from '../features/usage/UsageAvatar.vue'
import VokiaLogo from '../features/brand/VokiaLogo.vue'

const PROJECT_LIST_PAGE: AppPage = 'projetos'
const NEW_PROJECT_PAGE: AppPage = 'onboarding'

const ITEM = 'relative flex h-9.5 items-center rounded-control whitespace-nowrap text-left'
const ITEM_EXPANDED_PAD = 'pr-2.5'
// Caixa fixa do ícone: mantém x e y do ícone iguais nos dois modos do trilho.
const ICON_BOX = 'grid size-9.5 flex-none place-items-center'
const ITEM_ON = 'bg-accent-soft text-accent'
const ITEM_OFF = 'text-muted'
const TOGGLE = 'cursor-pointer bg-transparent text-ink'
const CARD = 'flex flex-col gap-0.5 rounded-card border border-line bg-surface p-1.25 shadow-float'
const LABEL = 'overflow-hidden text-ellipsis text-label'
const RAIL_MOTION = 'duration-180 ease-[ease]'
const SUBLIST = `grid transition-[grid-template-rows,opacity] ${RAIL_MOTION}`
const SUBLIST_OPEN = 'grid-rows-[1fr] opacity-100'
const SUBLIST_CLOSED = 'grid-rows-[0fr] opacity-0'

const navigation = useNavigation()
// Novo projeto é página fora do trilho: o item Projetos segue ativo enquanto ela aparece.
const railPage = computed<AppPage>(() =>
  navigation.page === 'novo-projeto' ? PROJECT_LIST_PAGE : navigation.page,
)
const expanded = ref(false)
const openProject = ref<string | null>(CURRENT_PROJECT)
const itemSize = computed(() => [ITEM, expanded.value && ITEM_EXPANDED_PAD])
const toggleLabel = computed(() => (expanded.value ? 'Recolher menu' : 'Expandir menu'))

function isActive(page: AppPage) {
  return page === railPage.value
}

function isOpen(projectName: string) {
  return openProject.value === projectName
}

function isActiveSubpage(projectName: string, page: AppPage) {
  return isOpen(projectName) && isActive(page)
}

function groupId(projectName: string) {
  return `rail-projeto-${projectName}`
}

function toggleProject(projectName: string) {
  openProject.value = isOpen(projectName) ? null : projectName
}

function toggle() {
  expanded.value = !expanded.value
}
</script>

<template>
  <div class="relative shrink-0" :style="{ width: `${RAIL_COLLAPSED_WIDTH}px` }">
    <div
      :class="[
        `absolute inset-y-0 left-0 z-30 flex flex-col items-start gap-3 bg-bg px-4.75 py-5 font-sans transition-[width] ${RAIL_MOTION}`,
        expanded && 'shadow-float',
      ]"
      :style="{ width: `${expanded ? RAIL_EXPANDED_WIDTH : RAIL_COLLAPSED_WIDTH}px` }"
    >
      <div class="flex shrink-0 items-center gap-2.5 pl-1.25">
        <button
          type="button"
          class="grid size-10 flex-none place-items-center rounded-control-lg bg-ink text-bg [--logo-cut:var(--color-ink)]"
          aria-label="Vokia, início"
          title="Vokia"
          @click="navigation.goTo(PROJECT_LIST_PAGE)"
        >
          <VokiaLogo variant="cheio" :size="24" />
        </button>
        <span v-if="expanded" class="order-2 text-lead text-ink">Vokia</span>
      </div>

      <nav aria-label="Workspace" :class="[CARD, 'shrink-0', expanded && 'w-full']">
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
            <svg
              class="size-4.25 flex-none"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              stroke-width="1.4"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path :d="item.path" />
            </svg>
          </span>
          <span v-if="expanded" :class="LABEL">{{ item.label }}</span>
        </button>
      </nav>

      <nav
        aria-label="Projetos"
        :class="[CARD, 'min-h-0 flex-1 overflow-y-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden', expanded && 'w-full']"
      >
        <div v-for="project in PROJECTS" :key="project.name" class="flex flex-col">
          <button
            type="button"
            :class="[itemSize, ITEM_OFF, isOpen(project.name) && 'bg-surface-2']"
            :aria-label="`${project.name} · ${project.working} trabalhando`"
            :aria-expanded="isOpen(project.name)"
            :aria-controls="groupId(project.name)"
            :title="`${project.name} · ${project.working} trabalhando`"
            @click="toggleProject(project.name)"
          >
            <span :class="ICON_BOX">
              <svg
                class="size-4"
                viewBox="0 0 18 18"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <path :d="project.path" />
              </svg>
            </span>
            <span v-if="expanded" :class="LABEL">{{ project.name }}</span>
            <span
              :class="[
                'size-1.75 rounded-full shadow-[0_0_0_2px_var(--color-surface)]',
                project.dot === 'warn' ? 'bg-warn' : 'bg-ok',
                expanded ? 'ml-auto' : 'absolute bottom-1.5 right-1.5',
              ]"
            />
          </button>
          <div
            :id="groupId(project.name)"
            :class="[SUBLIST, isOpen(project.name) ? SUBLIST_OPEN : SUBLIST_CLOSED]"
            :inert="!isOpen(project.name)"
          >
            <div class="flex min-h-0 flex-col gap-0.5 overflow-hidden">
              <span class="mx-1.5 my-0.75 h-px bg-line" />
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
                  <svg
                    class="size-4.25 flex-none"
                    viewBox="0 0 16 16"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.4"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    aria-hidden="true"
                  >
                    <path :d="item.path" />
                  </svg>
                </span>
                <span v-if="expanded" :class="LABEL">{{ item.label }}</span>
              </button>
            </div>
          </div>
        </div>
        <span class="mx-1.5 my-0.75 h-px bg-line" />
        <button
          type="button"
          :class="[itemSize, ITEM_OFF]"
          aria-label="Novo projeto"
          title="Novo projeto"
          @click="navigation.goTo(NEW_PROJECT_PAGE)"
        >
          <span :class="ICON_BOX">
            <svg
              class="size-4"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              aria-hidden="true"
            >
              <path d="M8 3v10M3 8h10" />
            </svg>
          </span>
          <span v-if="expanded" :class="LABEL">Novo projeto</span>
        </button>
      </nav>

      <div class="mt-2 flex shrink-0 flex-col items-start gap-1 pl-1.5">
        <button
          type="button"
          :class="[itemSize, isActive('config') ? ITEM_ON : ITEM_OFF]"
          :aria-current="isActive('config') ? 'page' : undefined"
          :aria-label="PAGE_LABELS.config"
          :title="PAGE_LABELS.config"
          @click="navigation.goTo('config')"
        >
          <span :class="ICON_BOX">
            <svg
              class="size-4.25 flex-none"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              stroke-width="1.4"
              stroke-linecap="round"
              aria-hidden="true"
            >
              <circle cx="8" cy="8" r="2.2" />
              <path
                d="M8 3.2 L8.67 3.25 L9.6 1.6 L10.89 2.07 L10.54 3.93 L11.39 4.61 L11.83 5.11 L13.66 4.6 L14.24 5.85 L12.68 6.92 L12.8 8 L12.75 8.67 L14.4 9.6 L13.93 10.89 L12.07 10.54 L11.39 11.39 L10.89 11.83 L11.4 13.66 L10.15 14.24 L9.08 12.68 L8 12.8 L7.33 12.75 L6.4 14.4 L5.11 13.93 L5.46 12.07 L4.61 11.39 L4.17 10.89 L2.34 11.4 L1.76 10.15 L3.32 9.08 L3.2 8 L3.25 7.33 L1.6 6.4 L2.07 5.11 L3.93 5.46 L4.61 4.61 L5.11 4.17 L4.6 2.34 L5.85 1.76 L6.92 3.32 Z"
              />
            </svg>
          </span>
          <span v-if="expanded" :class="LABEL">{{ PAGE_LABELS.config }}</span>
        </button>
        <button
          type="button"
          :class="[itemSize, TOGGLE]"
          :aria-label="toggleLabel"
          :aria-expanded="expanded"
          :title="toggleLabel"
          @click="toggle"
        >
          <span :class="ICON_BOX">
            <svg
              class="size-4.25 flex-none"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              stroke-width="1.4"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path d="M2 2.5h12v11H2zM6 2.5v11" />
              <path :d="expanded ? 'M11 6L9 8l2 2' : 'M9 6l2 2-2 2'" />
            </svg>
          </span>
          <span v-if="expanded" :class="LABEL">Recolher</span>
        </button>
        <UsageAvatar class="-ml-0.75" :expanded="expanded" />
      </div>
    </div>
  </div>
</template>
