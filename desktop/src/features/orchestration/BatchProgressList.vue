<script setup lang="ts">
import { computed } from 'vue'
import { useProjects } from '../projects/useProjects'
import type { VokiaBatchProgress } from '../team/events'
import { useRoster } from '../team/useRoster'
import { useOrchestration } from './useOrchestration'

const BATCH_LABEL = 'Lote de'
const RETURNED_LABEL = 'trocas voltaram:'
const RUNNING_LABEL = 'em andamento:'
const DONE_NOTE = 'Todas as trocas voltaram.'

const orchestration = useOrchestration()
const projects = useProjects()
const roster = useRoster()

const openBatches = computed(() =>
  [...orchestration.batches.values()].filter((batch) => batch.projectId === projects.selected?.id),
)

function progressText(batch: VokiaBatchProgress): string {
  const head = `${BATCH_LABEL} ${roster.nameOf(batch.leader)}: ${batch.returned} de ${batch.total} ${RETURNED_LABEL}`
  if (batch.running.length === 0) {
    return head
  }
  const names = batch.running.map((agent) => roster.nameOf(agent)).join(', ')
  return `${head} ${RUNNING_LABEL} ${names}`
}

function isComplete(batch: VokiaBatchProgress): boolean {
  return batch.returned === batch.total && batch.total > 0
}
</script>

<template>
  <section
    v-if="openBatches.length > 0"
    aria-label="Lotes em andamento"
    class="flex flex-col gap-2 rounded-card border border-line bg-surface p-3"
  >
    <div v-for="batch in openBatches" :key="`${batch.projectId}:${batch.leader}`" class="flex flex-col gap-0.5">
      <p class="text-caption text-ink">{{ progressText(batch) }}</p>
      <p v-if="isComplete(batch)" class="text-small text-muted">{{ DONE_NOTE }}</p>
    </div>
  </section>
</template>
