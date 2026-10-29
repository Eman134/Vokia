<script setup lang="ts">
import { computed } from 'vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import { ARTIFACT_KIND_INFO, type ArtifactSample } from './artefatosSample'

const AVATAR_SIZE = 20
const CARD_CLASSES = 'flex min-w-0 flex-col overflow-hidden rounded-dialog border border-line bg-surface'
const BODY_CLASSES = 'flex flex-col gap-2 p-3.5'

const props = defineProps<{ artifact: ArtifactSample }>()

const kind = computed(() => ARTIFACT_KIND_INFO[props.artifact.kind])
</script>

<template>
  <article :class="CARD_CLASSES">
    <div :class="BODY_CLASSES">
      <p class="flex items-center gap-1.5 text-small font-semibold text-accent">
        <svg
          class="size-3.25 flex-none"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          stroke-width="1.4"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path :d="kind.iconPath" />
        </svg>
        {{ kind.label }}
      </p>
      <h3 class="truncate text-lead text-ink">{{ artifact.title }}</h3>
      <p class="flex items-center gap-1.5 text-small text-muted">
        <AgentPortrait v-if="artifact.portrait" :who="artifact.portrait" :size="AVATAR_SIZE" />
        <span class="truncate">{{ artifact.author }} · {{ artifact.time }}</span>
      </p>
    </div>
  </article>
</template>
