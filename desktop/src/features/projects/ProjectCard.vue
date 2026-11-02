<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import ProjectIcon from './ProjectIcon.vue'
import { EDIT_LABEL, EMPTY_PATH_TEXT, OPEN_LABEL } from './projetosSample'
import type { Project } from './projectsApi'

const props = defineProps<{ project: Project }>()
const emit = defineEmits<{ open: []; edit: []; remove: [] }>()

const FOLDER_ORIGIN_LABEL = 'Pasta'
const GITHUB_ORIGIN_PREFIX = 'GitHub'

const CARD_CLASSES = 'flex min-w-0 flex-col gap-4 rounded-[26px] border border-line bg-surface p-5.5'
const HEADER_CLASSES = 'flex min-w-0 items-center gap-3.5'
const SYMBOL_CLASSES = 'grid size-11.5 shrink-0 place-items-center rounded-[15px] bg-accent-soft text-accent'
const NAME_CLASSES = 'truncate text-[20px] font-bold tracking-[-0.02em] text-ink'
const PATH_CLASSES = 'truncate font-mono text-[11.5px] text-muted'
const FOOTER_CLASSES = 'mt-auto flex items-center gap-2.5'

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
      <span :class="SYMBOL_CLASSES">
        <ProjectIcon :icon="project.symbol" class="size-6" data-testid="project-card-icon" />
      </span>
      <div class="min-w-0">
        <b :class="NAME_CLASSES" class="block">{{ project.name }}</b>
        <div :class="PATH_CLASSES" :title="path">{{ path }}</div>
      </div>
    </div>
    <p v-if="project.description" class="text-body text-muted">{{ project.description }}</p>
    <div :class="FOOTER_CLASSES">
      <UiBadge status="backlog" :label="originLabel" />
      <span class="ml-auto" />
      <UiButton variant="quiet" size="sm" data-testid="project-card-edit" @press="emit('edit')">
        {{ EDIT_LABEL }}
      </UiButton>
      <UiConfirm
        label="Excluir"
        message="Remove o projeto, conversas, backlog e planos do app. A pasta no disco não é apagada."
        confirm-label="Excluir projeto"
        @confirm="emit('remove')"
      />
      <UiButton variant="primary" size="lg" data-testid="project-card-open" @press="emit('open')">
        {{ OPEN_LABEL }}
      </UiButton>
    </div>
  </article>
</template>
