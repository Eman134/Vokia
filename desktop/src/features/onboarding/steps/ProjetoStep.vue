<script setup lang="ts">
import UiSegmented from '../../../ui/UiSegmented.vue'
import { BRANCH_SAMPLE, REPO_SAMPLES } from '../onboardingSample'
import ChoiceCard from './ChoiceCard.vue'
import StepHeading from './StepHeading.vue'

const SOURCE_OPTIONS = [
  { label: 'Pasta local', value: 'local' },
  { label: 'Do GitHub', value: 'github' },
]
const START_OPTIONS = [
  { label: 'Branch', value: 'branch' },
  { label: 'Nova branch', value: 'nova' },
  { label: 'Pull request', value: 'pr' },
]
const SOURCE_SAMPLE = 'github'
const START_SAMPLE = 'branch'
const BRANCH_CHIP_CLASSES = 'ml-auto rounded-control-sm bg-surface-2 px-2.25 py-1 font-mono text-caption'
</script>

<template>
  <div class="flex w-full flex-col gap-7">
    <StepHeading
      title="Escolha um projeto"
      text="Uma pasta com git ou um repositório do GitHub. Cada feature ganha uma branch e um worktree próprios."
    />
    <div class="flex flex-col gap-3.5">
      <div class="flex justify-center">
        <UiSegmented :options="SOURCE_OPTIONS" :model-value="SOURCE_SAMPLE" label="Origem" disabled />
      </div>
      <div class="flex flex-col gap-2">
        <ChoiceCard v-for="repo in REPO_SAMPLES" :key="repo.name" :selected="repo.selected">
          <span class="flex min-w-0 flex-col gap-0.5">
            <b class="font-mono text-[13.5px] font-medium">{{ repo.name }}</b>
            <span class="text-small text-muted">{{ repo.detail }}</span>
          </span>
        </ChoiceCard>
        <div class="flex items-center gap-2.5 pt-1.5">
          <span class="text-label text-muted">Começar de</span>
          <UiSegmented :options="START_OPTIONS" :model-value="START_SAMPLE" label="Começar de" disabled />
          <span :class="BRANCH_CHIP_CLASSES">{{ BRANCH_SAMPLE }}</span>
        </div>
      </div>
    </div>
  </div>
</template>
