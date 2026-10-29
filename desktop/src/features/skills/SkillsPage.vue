<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import { SAMPLE_DATA_LABEL } from '../../shell/railNav'
import SkillCard from './SkillCard.vue'
import SkillMatrix from './SkillMatrix.vue'
import {
  SKILL_SAMPLES,
  SKILLS_CONTEXT_LABEL,
  SKILLS_INSTALL_ACTION_LABEL,
  SKILLS_MEMORY_ACTION_LABEL,
  SKILLS_SUBTITLE,
  SKILLS_TITLE,
} from './skillsSample'

const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto pt-7 pr-8 pb-[150px] pl-2'
const HEADER_CLASSES = 'flex flex-wrap items-end gap-4'
const TITLE_BLOCK_CLASSES = 'flex min-w-0 flex-col gap-1'
const TITLE_CLASSES = 'text-[32px] font-bold tracking-[-0.03em] text-ink'
const SUBTITLE_CLASSES = 'max-w-[64ch] text-body text-muted'
const ACTIONS_CLASSES = 'ml-auto flex gap-2'
const CARD_GRID_CLASSES = 'grid grid-cols-[repeat(auto-fit,minmax(300px,1fr))] gap-4'
</script>

<template>
  <section :class="PAGE_CLASSES">
    <div :class="HEADER_CLASSES">
      <div :class="TITLE_BLOCK_CLASSES">
        <span class="text-caption text-muted">{{ SKILLS_CONTEXT_LABEL }} · {{ SAMPLE_DATA_LABEL }}</span>
        <h1 :class="TITLE_CLASSES">{{ SKILLS_TITLE }}</h1>
        <p :class="SUBTITLE_CLASSES">{{ SKILLS_SUBTITLE }}</p>
      </div>
      <div :class="ACTIONS_CLASSES">
        <UiButton variant="quiet" size="lg" class="border border-line" disabled aria-disabled="true">
          {{ SKILLS_MEMORY_ACTION_LABEL }}
        </UiButton>
        <UiButton variant="primary" size="lg" disabled aria-disabled="true">
          {{ SKILLS_INSTALL_ACTION_LABEL }}
        </UiButton>
      </div>
    </div>
    <div :class="CARD_GRID_CLASSES">
      <SkillCard v-for="skill in SKILL_SAMPLES" :key="skill.name" :skill="skill" />
    </div>
    <SkillMatrix />
  </section>
</template>
