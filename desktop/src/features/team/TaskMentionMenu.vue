<script setup lang="ts">
import { COLUMN_LABEL, type BacklogItem } from '../backlog/backlogTypes'

defineProps<{
  id: string
  tasks: readonly BacklogItem[]
  activeIndex: number
  left: number
}>()

const emit = defineEmits<{
  choose: [task: BacklogItem]
}>()
</script>

<template>
  <div
    class="absolute bottom-full z-30 mb-2 max-h-60 w-80 max-w-full overflow-y-auto rounded-md border border-line bg-surface p-1.5 shadow-paper"
    :style="{ left: `${left}px` }"
  >
    <ul v-if="tasks.length > 0" :id="id" role="listbox" aria-label="Tasks" class="flex flex-col gap-px">
      <li
        v-for="(task, index) in tasks"
        :id="`${id}-${task.id}`"
        :key="task.id"
        role="option"
        :aria-selected="index === activeIndex"
        class="flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5"
        :class="index === activeIndex ? 'bg-accent-soft' : 'hover:bg-hover'"
        @mousedown.prevent
        @click="emit('choose', task)"
      >
        <span class="shrink-0 rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-tiny font-bold text-ink">
          {{ task.publicId ?? task.id }}
        </span>
        <span class="min-w-0 flex-1 truncate text-label text-ink">{{ task.title }}</span>
        <span v-if="task.column" class="shrink-0 text-tiny text-muted">
          {{ COLUMN_LABEL[task.column] }}
        </span>
      </li>
    </ul>
    <p v-else class="px-2 py-1.5 text-caption text-muted">Nenhuma task encontrada</p>
  </div>
</template>
