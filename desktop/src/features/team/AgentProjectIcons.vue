<script setup lang="ts">
import { computed } from 'vue'
import ProjectIcon from '../projects/ProjectIcon.vue'
import { useProjects } from '../projects/useProjects'
import type { Agent } from './events'
import { useTeam } from './useTeam'

const MAX_ICONS = 2

const props = defineProps<{ agent: Agent }>()

const team = useTeam()
const projects = useProjects()

const working = computed(() => team.activeAgents.some((profile) => profile.id === props.agent))
const active = computed(() =>
  working.value
    ? team.projectsOf(props.agent).flatMap((id) => projects.projects.find((project) => project.id === id) ?? [])
    : [],
)
const shown = computed(() => active.value.slice(0, MAX_ICONS))
const hidden = computed(() => active.value.length - shown.value.length)
</script>

<template>
  <span v-if="active.length > 0" class="inline-flex items-center gap-1 text-muted" data-testid="agent-project-icons">
    <span v-for="project in shown" :key="project.id" role="img" :title="project.name" :aria-label="project.name">
      <ProjectIcon :icon="project.symbol" class="size-3.5" />
    </span>
    <span v-if="hidden > 0" class="text-caption">+{{ hidden }}</span>
  </span>
</template>
