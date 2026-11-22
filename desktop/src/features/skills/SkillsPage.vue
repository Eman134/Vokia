<script setup lang="ts">
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import SkillCard from './SkillCard.vue'
import SkillMatrix from './SkillMatrix.vue'
import {
  SKILL_SAMPLES,
  SKILLS_TITLE,
} from './skillsSample'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const CARD_GRID_CLASSES = 'grid grid-cols-[repeat(auto-fit,minmax(min(300px,100%),1fr))] gap-4'
</script>

<template>
  <section :class="PAGE_CLASSES">
    <UiPageHeader :title="SKILLS_TITLE" />
    <div :class="CARD_GRID_CLASSES">
      <SkillCard v-for="skill in SKILL_SAMPLES" :key="skill.name" :skill="skill" />
    </div>
    <SkillMatrix />
  </section>
</template>
