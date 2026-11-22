<script setup lang="ts">
import AgentPortrait from '../team/AgentPortrait.vue'
import UserAvatar from '../profile/UserAvatar.vue'
import UiMemoryCard from '../../ui/UiMemoryCard.vue'
import { MEMORY_FACTS, MEMORY_NOTE } from './memoriaSample'

const MARKER_SIZE = 30
const HEADER_CLASSES = 'flex flex-col items-start gap-2.5 px-4.5 pt-4'
const LIST_CLASSES = 'flex flex-col gap-3 px-4.5 py-4'
const ROW_CLASSES = 'flex items-start gap-3'
const SUGGESTION_ROW_CLASSES = 'flex items-start gap-3 rounded-md bg-accent-soft p-2.5'
const CONTENT_CLASSES = 'flex min-w-0 flex-1 flex-col items-end gap-2'
const NOTE_CLASSES = 'border-t border-line px-4.5 py-3 text-small text-muted'
</script>

<template>
  <div :class="HEADER_CLASSES">
    <h2 class="text-lead text-ink">Memória</h2>
  </div>
  <ol :class="LIST_CLASSES">
    <li v-for="fact in MEMORY_FACTS" :key="fact.text" :class="fact.suggested ? SUGGESTION_ROW_CLASSES : ROW_CLASSES">
      <UserAvatar v-if="fact.who === null" :size="MARKER_SIZE" />
      <AgentPortrait v-else :who="fact.who" :size="MARKER_SIZE" />
      <div :class="CONTENT_CLASSES">
        <UiMemoryCard class="w-full" :fact="fact.text" :author="fact.author" :usage="fact.usage" />
      </div>
    </li>
  </ol>
  <p :class="NOTE_CLASSES">{{ MEMORY_NOTE }}</p>
</template>
