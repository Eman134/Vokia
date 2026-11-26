<script setup lang="ts">
import { computed } from 'vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiButton from '../../ui/UiButton.vue'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import SkillCard from './SkillCard.vue'
import SkillMatrix from './SkillMatrix.vue'
import LearningSuggestionsPanel from '../memory/LearningSuggestionsPanel.vue'
import { useLearningSuggestions } from '../memory/useLearningSuggestions'
import { useProjects } from '../projects/useProjects'
import {
  SKILL_SAMPLES,
  SKILLS_TITLE,
} from './skillsSample'

const learningStore = useLearningSuggestions()
const projects = useProjects()
const currentProjectId = computed(() => projects.selected?.id)

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const CARD_GRID_CLASSES = 'grid grid-cols-[repeat(auto-fit,minmax(min(300px,100%),1fr))] gap-4'
</script>

<template>
  <section :class="PAGE_CLASSES">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="flex items-center gap-3">
        <UiPageHeader :title="SKILLS_TITLE" />
        <span
          v-if="learningStore.pendingSkillCount > 0"
          class="inline-flex items-center rounded-full bg-accent-soft px-2.5 py-0.5 text-tiny font-semibold text-accent"
        >
          {{ learningStore.pendingSkillCount }} pendente{{ learningStore.pendingSkillCount === 1 ? '' : 's' }}
        </span>
      </div>
      <UiButton
        variant="quiet"
        size="sm"
        :disabled="currentProjectId === undefined || learningStore.reviewing"
        @press="currentProjectId !== undefined && learningStore.triggerReview(currentProjectId)"
      >
        {{ learningStore.reviewing ? 'Analisando memória...' : 'Criar da memória' }}
      </UiButton>
    </div>
    <LearningSuggestionsPanel filter-kind="skill" title="Skills › Sugestões" />
    <div :class="CARD_GRID_CLASSES">
      <SkillCard v-for="skill in SKILL_SAMPLES" :key="skill.name" :skill="skill" />
    </div>
    <SkillMatrix />
  </section>
</template>
