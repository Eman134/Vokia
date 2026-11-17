<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import ProjectBadge from './ProjectBadge.vue'
import { projectColorOf } from './projectColors'
import { EDIT_LABEL, EMPTY_PATH_TEXT, OPEN_LABEL } from './projetosSample'
import type { Project } from './projectsApi'

const props = defineProps<{ project: Project }>()
const emit = defineEmits<{ open: []; edit: []; pin: [] }>()

const FOLDER_ORIGIN_LABEL = 'Pasta'
const GITHUB_ORIGIN_PREFIX = 'GitHub'
const PIN_LABEL = 'Fixar'
const UNPIN_LABEL = 'Desafixar'
const PINNED_BADGE_LABEL = 'Fixado'

const CARD_CLASSES = 'flex min-w-0 flex-col gap-4 rounded-[26px] border border-line bg-surface p-5.5'
const HEADER_CLASSES = 'flex min-w-0 items-center gap-3.5'
const BADGE_CLASSES = 'size-7 shrink-0'
const NAME_CLASSES = 'truncate text-[20px] font-bold tracking-[-0.02em] text-ink'
const PATH_CLASSES = 'truncate font-mono text-[11.5px] text-muted'
const FOOTER_CLASSES = 'mt-auto flex flex-wrap items-center gap-2.5'

const path = computed(() => props.project.path || EMPTY_PATH_TEXT)
const originLabel = computed(() =>
  props.project.originKind === 'github'
    ? `${GITHUB_ORIGIN_PREFIX} · ${props.project.originRepo ?? ''}`
    : FOLDER_ORIGIN_LABEL,
)
</script>

<template>
  <article :class="CARD_CLASSES" data-testid="project-card">
    <div :class="HEADER_CLASSES">
      <ProjectBadge :color="projectColorOf(project)" :class="BADGE_CLASSES" data-testid="project-card-icon" />
      <div class="min-w-0">
        <div class="flex min-w-0 items-center gap-2">
          <b :class="NAME_CLASSES" class="min-w-0 block">{{ project.name }}</b>
          <UiBadge v-if="project.pinned" status="lote" :label="PINNED_BADGE_LABEL" />
        </div>
        <div :class="PATH_CLASSES" :title="path">{{ path }}</div>
      </div>
    </div>
    <p v-if="project.description" class="text-body text-muted">{{ project.description }}</p>
    <div :class="FOOTER_CLASSES">
      <UiBadge status="backlog" :label="originLabel" />
      <span class="ml-auto" />
      <UiButton
        variant="quiet"
        size="sm"
        data-testid="project-card-pin"
        :disabled="!project.pinned && project.path.trim() === ''"
        @press="emit('pin')"
      >
        {{ project.pinned ? UNPIN_LABEL : PIN_LABEL }}
      </UiButton>
      <UiButton variant="quiet" size="sm" data-testid="project-card-edit" @press="emit('edit')">
        {{ EDIT_LABEL }}
      </UiButton>
      <UiButton variant="primary" size="lg" data-testid="project-card-open" @press="emit('open')">
        {{ OPEN_LABEL }}
      </UiButton>
    </div>
  </article>
</template>
