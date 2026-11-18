<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import { useProjects } from '../projects/useProjects'
import type { InterruptedRun } from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import {
  DISCARD_LABEL,
  INTERRUPTED_NOTE,
  INTERRUPTED_TITLE,
  RESUME_LEADER_LABEL,
  RESUME_REQUEST_LABEL,
} from './nowCopy'
import { useAgentControl } from './useAgentControl'
import { useInterruptedRuns } from './useInterruptedRuns'

const interrupted = useInterruptedRuns()
const roster = useRoster()
const projects = useProjects()
const control = useAgentControl()

function agentName(run: InterruptedRun): string {
  return roster.profileOf(run.agent)?.name ?? run.agent
}

function projectName(run: InterruptedRun): string {
  return projects.projects.find((project) => project.id === run.projectId)?.name ?? ''
}

function isPausedRun(run: InterruptedRun): boolean {
  return control.isPaused(run.agent, run.projectId)
}
</script>

<template>
  <details
    v-if="interrupted.runs.length > 0"
    open
    class="flex flex-col gap-2 rounded-card border border-line bg-surface px-3 py-2"
    data-testid="interrupted-runs"
  >
    <summary class="cursor-pointer text-label font-semibold text-ink">
      {{ INTERRUPTED_TITLE }} {{ interrupted.runs.length }}
    </summary>
    <p class="text-caption text-muted">{{ INTERRUPTED_NOTE }}</p>
    <ul class="flex flex-col gap-2">
      <li
        v-for="run in interrupted.runs"
        :key="run.id"
        class="flex flex-col gap-1.5"
        :data-testid="`interrupted-run-${run.id}`"
      >
        <span class="text-caption text-muted">{{ agentName(run) }} · {{ projectName(run) }}</span>
        <span class="wrap-break-word line-clamp-2 text-label text-ink">{{ run.request }}</span>
        <span class="flex gap-1.5">
          <UiButton
            v-if="run.leader"
            variant="primary"
            size="sm"
            :disabled="isPausedRun(run)"
            data-testid="interrupted-resume-leader"
            @press="interrupted.resume(run)"
          >
            {{ RESUME_LEADER_LABEL }}
          </UiButton>
          <UiButton
            v-else
            variant="approve"
            size="sm"
            :disabled="isPausedRun(run)"
            data-testid="interrupted-resume"
            @press="interrupted.resume(run)"
          >
            {{ RESUME_REQUEST_LABEL }}
          </UiButton>
          <UiButton variant="quiet" size="sm" data-testid="interrupted-discard" @press="interrupted.discard(run)">
            {{ DISCARD_LABEL }}
          </UiButton>
        </span>
      </li>
    </ul>
  </details>
</template>
