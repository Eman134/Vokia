<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from './UiButton.vue'
import MarkdownText from '../features/team/MarkdownText.vue'
import { INK_FOCUS_CLASSES } from './handDrawn'

const props = defineProps<{
  fact: string
  author: string
  usage: string
}>()
const emit = defineEmits<{ forget: [] }>()

const expanded = ref(false)

const quickSummary = computed(() => {
  const firstLine = props.fact.trim().split(/\r?\n/)[0] ?? ''
  const sentenceEnd = firstLine.search(/[.!?](\s|$)/)
  if (sentenceEnd !== -1 && sentenceEnd < 140) {
    return firstLine.slice(0, sentenceEnd + 1)
  }
  return firstLine
})
</script>

<template>
  <article class="flex min-w-0 flex-col gap-1.5 rounded-md border border-line bg-surface px-3 py-2">
    <button
      type="button"
      class="w-full cursor-pointer text-left"
      :class="INK_FOCUS_CLASSES"
      :aria-expanded="expanded"
      :title="expanded ? 'Recolher memória' : 'Expandir memória'"
      @click="expanded = !expanded"
    >
      <MarkdownText v-if="expanded" :text="fact" class="text-caption text-ink" />
      <p v-else class="line-clamp-2 text-caption text-ink">{{ quickSummary }}</p>
    </button>
    <footer class="flex items-center justify-between gap-2">
      <span class="min-w-0 truncate text-tiny text-muted" :title="`${author} · ${usage}`">
        {{ author }}
      </span>
      <UiButton variant="quiet" size="sm" @press="emit('forget')">Esquecer</UiButton>
    </footer>
  </article>
</template>
