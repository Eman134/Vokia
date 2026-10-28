<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  ACTIVE_PAGE,
  CURRENT_PROJECT,
  PROJECT_TOOLS,
  PROJECTS,
  RAIL_COLLAPSED_WIDTH,
  RAIL_EXPANDED_WIDTH,
  WORKSPACE_ITEMS,
  type RailPage,
} from './railNav'
import UsageAvatar from '../features/usage/UsageAvatar.vue'
import VokiaLogo from '../features/brand/VokiaLogo.vue'

const ITEM = 'relative flex h-9.5 items-center rounded-control whitespace-nowrap'
const ITEM_EXPANDED_PAD = 'pr-2.5'
// Caixa fixa do ícone: mantém x e y do ícone iguais nos dois modos do trilho.
const ICON_BOX = 'grid size-9.5 flex-none place-items-center'
const ITEM_ON = 'bg-accent-soft text-accent'
const ITEM_OFF = 'text-muted'
const TOGGLE = 'cursor-pointer bg-transparent text-ink'
const CARD = 'flex flex-col gap-0.5 rounded-card border border-line bg-surface p-1.25 shadow-float'
const LABEL = 'overflow-hidden text-ellipsis text-label'

const expanded = ref(false)
const inProject = computed(() => PROJECT_TOOLS.some((item) => item.id === ACTIVE_PAGE))
const itemSize = computed(() => [ITEM, expanded.value && ITEM_EXPANDED_PAD])
const toggleLabel = computed(() => (expanded.value ? 'Recolher menu' : 'Expandir menu'))
const projectSwitchLabel = `Trocar de projeto: ${CURRENT_PROJECT}`
const projectSwitchTitle = `${CURRENT_PROJECT} · trocar`

function isActive(id: RailPage) {
  return id === ACTIVE_PAGE
}

function toggle() {
  expanded.value = !expanded.value
}
</script>

<template>
  <div class="relative shrink-0" :style="{ width: `${RAIL_COLLAPSED_WIDTH}px` }">
    <div
      :class="[
        'absolute inset-y-0 left-0 z-30 flex flex-col items-start gap-3 bg-bg px-4.75 py-5 font-sans transition-[width] duration-180 ease-[ease]',
        expanded && 'shadow-float',
      ]"
      :style="{ width: `${expanded ? RAIL_EXPANDED_WIDTH : RAIL_COLLAPSED_WIDTH}px` }"
    >
      <div class="flex items-center gap-2.5 pl-1.25">
        <a
          class="grid size-10 flex-none place-items-center rounded-control-lg bg-ink text-bg [--logo-cut:var(--color-ink)]"
          aria-label="Vokia, início"
          title="Vokia"
        >
          <VokiaLogo variant="cheio" :size="24" />
        </a>
        <span v-if="expanded" class="order-2 text-lead text-ink">Vokia</span>
      </div>

      <nav aria-label="Workspace" :class="[CARD, expanded && 'w-full']">
        <a
          v-for="item in WORKSPACE_ITEMS"
          :key="item.id"
          :class="[itemSize, isActive(item.id) ? ITEM_ON : ITEM_OFF]"
          :aria-current="isActive(item.id) ? 'page' : undefined"
          :aria-label="item.label"
          :title="item.label"
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
        </a>
      </nav>

      <nav v-if="inProject" :aria-label="`Projeto ${CURRENT_PROJECT}`" :class="[CARD, expanded && 'w-full']">
        <a
          :class="[ITEM, expanded && ITEM_EXPANDED_PAD, 'bg-surface-2']"
          :aria-label="projectSwitchLabel"
          :title="projectSwitchTitle"
        >
          <span :class="ICON_BOX">
            <svg class="size-4 flex-none" viewBox="0 0 18 18" aria-hidden="true">
              <circle cx="9" cy="9" r="6.5" fill="none" class="stroke-ink" stroke-width="1.5" />
              <circle cx="9" cy="9" r="2.5" class="fill-ink" />
            </svg>
          </span>
          <span v-if="expanded" class="flex min-w-0 flex-1 flex-col leading-[1.15]">
            <b class="text-label font-semibold text-ink">{{ CURRENT_PROJECT }}</b>
            <span class="text-tiny text-muted">trocar de projeto</span>
          </span>
        </a>
        <span class="mx-1.5 my-0.75 h-px bg-line" />
        <a
          v-for="item in PROJECT_TOOLS"
          :key="item.id"
          :class="[itemSize, isActive(item.id) ? ITEM_ON : ITEM_OFF]"
          :aria-current="isActive(item.id) ? 'page' : undefined"
          :aria-label="item.label"
          :title="item.label"
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
        </a>
      </nav>

      <nav v-else aria-label="Projetos" :class="[CARD, expanded && 'w-full']">
        <a
          v-for="project in PROJECTS"
          :key="project.name"
          :class="[itemSize, ITEM_OFF]"
          :aria-label="`${project.name} · ${project.working} trabalhando`"
          :title="`${project.name} · ${project.working} trabalhando`"
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
        </a>
        <span class="mx-1.5 my-0.75 h-px bg-line" />
        <a :class="[itemSize, ITEM_OFF]" aria-label="Novo projeto" title="Novo projeto">
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
        </a>
      </nav>

      <div class="mt-2 flex flex-col items-start gap-1 pl-1.5">
        <a
          :class="[itemSize, isActive('config') ? ITEM_ON : ITEM_OFF]"
          :aria-current="isActive('config') ? 'page' : undefined"
          aria-label="Configurações"
          title="Configurações"
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
          <span v-if="expanded" :class="LABEL">Configurações</span>
        </a>
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
