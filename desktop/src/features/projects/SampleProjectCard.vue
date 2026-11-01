<script setup lang="ts">
import AgentPortrait from '../team/AgentPortrait.vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiProgress from '../../ui/UiProgress.vue'
import ProjectIcon from './ProjectIcon.vue'
import {
  MOUNT_TEAM_LABEL,
  OPEN_LABEL,
  RESUME_LABEL,
  SAMPLE_BADGE_LABEL,
  STATE_NEXT,
  STATE_PENDING,
  STATE_WORKING,
  type SampleProject,
} from './projetosSample'

defineProps<{ project: SampleProject }>()

const PORTRAIT_SIZE = 30
const ICON_BOX_CLASSES = 'grid size-11.5 shrink-0 place-items-center rounded-[15px]'
const CARD_CLASSES = 'flex min-w-0 flex-col gap-4 rounded-[26px] border border-line bg-surface p-5.5'
const PAUSED_CARD_CLASSES = 'flex min-w-0 flex-col gap-3.5 rounded-[26px] border-[1.5px] border-dashed border-line p-5.5'
const HEADER_CLASSES = 'flex min-w-0 items-center gap-3.5'
const NAME_CLASSES = 'truncate text-[20px] font-bold tracking-[-0.02em]'
const PATH_CLASSES = 'truncate font-mono text-[11.5px] text-muted'
const FEATURE_ROW_CLASSES = 'grid grid-cols-[minmax(0,1fr)_140px_70px] items-center gap-3 text-body'
const STATE_GRID_CLASSES = 'grid grid-cols-3 gap-2'
const STATE_BLOCK_CLASSES = 'flex min-w-0 flex-col gap-1 rounded-[14px] bg-surface-2 px-3 py-2.5'
const STATE_TITLE_CLASSES = 'flex items-center gap-1.5 text-[11.5px] text-muted'
const STATE_TEXT_CLASSES = 'whitespace-pre-line text-[12.5px] leading-[1.4]'
const FOOTER_CLASSES = 'flex flex-wrap items-center gap-2.5'
</script>

<template>
  <article :class="project.pausedText ? PAUSED_CARD_CLASSES : CARD_CLASSES" data-testid="sample-project-card">
    <div :class="HEADER_CLASSES">
      <span
        :class="[ICON_BOX_CLASSES, project.pausedText ? 'bg-surface-2 text-faint' : 'bg-accent-soft text-accent']"
      >
        <ProjectIcon :icon="project.icon" class="size-6" />
      </span>
      <div class="min-w-0">
        <b :class="[NAME_CLASSES, project.pausedText && 'text-muted']" class="block">{{ project.name }}</b>
        <div :class="PATH_CLASSES">{{ project.path }}</div>
      </div>
      <span v-if="project.working" class="ml-auto flex items-center gap-1.5 text-[12.5px] font-semibold text-ok">
        <span class="size-1.75 rounded-full bg-ok" />{{ project.working }} trabalhando
      </span>
      <UiBadge status="backlog" :label="SAMPLE_BADGE_LABEL" :class="project.working ? '' : 'ml-auto'" />
    </div>

    <p v-if="project.pausedText" class="text-body text-muted">{{ project.pausedText }}</p>
    <div v-else class="flex flex-col gap-2.5">
      <div v-for="feature in project.features" :key="feature.name" :class="FEATURE_ROW_CLASSES">
        <span>{{ feature.name }}</span>
        <UiProgress :value="feature.percent" />
        <span class="text-right text-[12px] text-muted">{{ feature.label }}</span>
      </div>
    </div>

    <div aria-label="Estado do projeto" :class="STATE_GRID_CLASSES">
      <div :class="STATE_BLOCK_CLASSES">
        <span :class="STATE_TITLE_CLASSES"><span class="size-1.75 rounded-full bg-ok" />{{ STATE_WORKING }}</span>
        <span :class="STATE_TEXT_CLASSES">{{ project.workingNow.join('\n') }}</span>
      </div>
      <div :class="STATE_BLOCK_CLASSES">
        <span :class="STATE_TITLE_CLASSES"><span class="size-1.75 rounded-full bg-warn" />{{ STATE_PENDING }}</span>
        <span :class="STATE_TEXT_CLASSES">{{ project.pending }}</span>
      </div>
      <div :class="STATE_BLOCK_CLASSES">
        <span :class="STATE_TITLE_CLASSES">{{ STATE_NEXT }}</span>
        <span :class="STATE_TEXT_CLASSES">{{ project.next }}</span>
      </div>
    </div>

    <div v-if="project.pausedText" class="flex gap-2">
      <UiButton variant="primary" size="lg" disabled aria-disabled="true">{{ MOUNT_TEAM_LABEL }}</UiButton>
      <UiButton variant="quiet" size="lg" disabled aria-disabled="true">{{ RESUME_LABEL }}</UiButton>
    </div>
    <div v-else :class="FOOTER_CLASSES">
      <div class="flex">
        <AgentPortrait
          v-for="(who, index) in project.portraits"
          :key="who"
          :who="who"
          :size="PORTRAIT_SIZE"
          :class="index > 0 && '-ml-2'"
        />
      </div>
      <span class="text-[12.5px] text-muted">{{ project.team }}</span>
      <UiBadge v-if="project.pendingBadge" status="precisa de você" :label="project.pendingBadge" />
      <UiButton class="ml-auto" variant="quiet" size="lg" disabled aria-disabled="true">{{ OPEN_LABEL }}</UiButton>
    </div>
  </article>
</template>
