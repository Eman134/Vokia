<script setup lang="ts">
import { onMounted, watch } from 'vue'
import { useFlow } from '../useFlow'
import { useProjects } from '../../projects/useProjects'
import { useLearningSuggestions } from '../../memory/useLearningSuggestions'
import UiButton from '../../../ui/UiButton.vue'
import MemorySuggestionsPopover from './MemorySuggestionsPopover.vue'

const open = defineModel<boolean>('open')

const flow = useFlow()
const projects = useProjects()
const learning = useLearningSuggestions()

watch(
  () => projects.selected?.id,
  (projectId) => {
    void learning.load(projectId)
  },
  { immediate: true },
)

watch(
  () => learning.pendingCount,
  (count) => {
    if (count === 0) open.value = false
  },
)

onMounted(() => {
  void learning.bindEvents()
})
</script>

<template>
  <div v-if="!flow.active && learning.pendingCount > 0" class="relative">
    <UiButton
      variant="quiet"
      size="sm"
      class="gap-1.5 px-2"
      aria-label="Sugestões de memória pendentes"
      title="Sugestões de memória pendentes"
      aria-haspopup="dialog"
      :aria-expanded="open"
      data-testid="memory-suggestions-button"
      @press="open = !open"
    >
      <span
        class="grid h-4.5 min-w-4.5 place-items-center rounded-full bg-accent px-1.25 text-tiny font-semibold text-bg"
        data-testid="memory-suggestions-count"
      >
        {{ learning.pendingCount }}
      </span>
      <svg
        class="size-4 shrink-0"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M5.5 3.5a2.5 2.5 0 0 1 5 0c1.2.3 2 1.3 2 2.6 0 .8-.3 1.5-.8 2 .4.5.6 1.1.6 1.8a2.5 2.5 0 0 1-2.5 2.5H6.2A2.5 2.5 0 0 1 3.7 9.9c0-.7.2-1.3.6-1.8-.5-.5-.8-1.2-.8-2 0-1.3.8-2.3 2-2.6Z" />
        <path d="M8 3.5v9" />
      </svg>
    </UiButton>
    <MemorySuggestionsPopover v-if="open" @close="open = false" />
  </div>
</template>
