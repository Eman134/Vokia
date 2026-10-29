<script setup lang="ts">
import UiBadge from '../../ui/UiBadge.vue'
import UiChip from '../../ui/UiChip.vue'
import { SKILL_FROM_MEMORY_LABEL, type SkillSample } from './skillsSample'

const FEATURED_ICON_PATH = 'M9 1.5L3.5 9h4L6.5 14.5 12.5 6.5h-4z'
const CARD_CLASSES = 'flex min-w-0 flex-col gap-2.5 rounded-dialog p-5'
const CARD_FEATURED_CLASSES = 'bg-accent-soft'
const CARD_PLAIN_CLASSES = 'border border-line bg-surface'
const HEADER_CLASSES = 'flex min-w-0 items-center gap-2.5'
const NAME_CLASSES = 'text-[18px] font-semibold text-ink'
const ICON_CLASSES = 'size-5 shrink-0 text-accent'
const ORIGIN_CLASSES = 'ml-auto font-mono text-tiny text-muted'
const DESCRIPTION_CLASSES = 'text-control text-muted'
const BOUND_CLASSES = 'text-caption text-ink'

defineProps<{ skill: SkillSample }>()
</script>

<template>
  <article :class="[CARD_CLASSES, skill.featured ? CARD_FEATURED_CLASSES : CARD_PLAIN_CLASSES]">
    <header :class="HEADER_CLASSES">
      <svg v-if="skill.featured" :class="ICON_CLASSES" viewBox="0 0 16 16" aria-hidden="true">
        <path :d="FEATURED_ICON_PATH" fill="currentColor" />
      </svg>
      <h2 :class="NAME_CLASSES">{{ skill.name }}</h2>
      <span v-if="skill.origin" :class="ORIGIN_CLASSES">{{ skill.origin }}</span>
      <UiBadge v-else-if="skill.fromMemory" class="ml-auto" status="lote" :label="SKILL_FROM_MEMORY_LABEL" />
    </header>
    <p :class="DESCRIPTION_CLASSES">{{ skill.description }}</p>
    <div v-if="skill.tags" class="flex flex-wrap gap-1.5">
      <UiChip v-for="tag in skill.tags" :key="tag" :label="tag" />
    </div>
    <p v-if="skill.boundTo" :class="BOUND_CLASSES">Ligada para {{ skill.boundTo }}</p>
  </article>
</template>
