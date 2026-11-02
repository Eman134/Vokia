<script setup lang="ts">
import ProjectIcon from './ProjectIcon.vue'
import { PROJECT_ICON_KEYS, PROJECT_ICONS } from './projectIcons'

const model = defineModel<string>({ required: true })

const ICON_LABEL = 'Ícone do projeto'
const ICON_BUTTON_CLASSES =
  'grid size-10 place-items-center rounded-control border focus:outline-2 focus:outline-solid focus:outline-accent'
const ICON_ACTIVE_CLASSES = 'border-accent bg-accent-soft text-accent'
const ICON_INACTIVE_CLASSES = 'border-line text-muted'
</script>

<template>
  <div class="flex flex-col gap-1.5">
    <span id="project-icon-label" class="text-label font-medium text-muted">{{ ICON_LABEL }}</span>
    <div role="radiogroup" aria-labelledby="project-icon-label" class="flex flex-wrap gap-2" data-testid="field-symbol">
      <button
        v-for="key in PROJECT_ICON_KEYS"
        :key="key"
        type="button"
        role="radio"
        :aria-checked="model === key"
        :aria-label="PROJECT_ICONS[key].label"
        :title="PROJECT_ICONS[key].label"
        :data-testid="`project-icon-${key}`"
        :class="[ICON_BUTTON_CLASSES, model === key ? ICON_ACTIVE_CLASSES : ICON_INACTIVE_CLASSES]"
        @click="model = key"
      >
        <ProjectIcon :icon="key" class="size-5" />
      </button>
    </div>
  </div>
</template>
