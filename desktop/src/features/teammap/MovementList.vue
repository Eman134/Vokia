<script setup lang="ts">
import UiEmpty from '../../ui/UiEmpty.vue'
import { formatMovementTime, movementVerb } from '../orchestration/orchestrationLabels'
import type { OrchMovement, OrchRule } from '../orchestration/orchestrationTypes'
import { useRoster } from '../team/useRoster'
import { MOVEMENTS_TITLE, NO_MOVEMENTS_TITLE } from './teamMapLayers'

const props = defineProps<{ movements: OrchMovement[]; rules: OrchRule[] }>()
const roster = useRoster()

function line(movement: OrchMovement): string {
  const actor = roster.nameOf(movement.fromAgent)
  return `${actor} ${movementVerb(movement, props.rules, (id) => roster.nameOf(id))} · ${formatMovementTime(movement.createdAt)}`
}
</script>

<template>
  <section class="flex flex-col gap-3 rounded-[24px] border border-line bg-surface p-4.5">
    <h2 class="text-control font-bold text-ink">{{ MOVEMENTS_TITLE }}</h2>
    <ul v-if="movements.length > 0" class="m-0 flex list-none flex-col gap-1.5 p-0">
      <li v-for="movement in movements" :key="movement.id" class="text-caption text-ink">{{ line(movement) }}</li>
    </ul>
    <UiEmpty v-else :title="NO_MOVEMENTS_TITLE" />
  </section>
</template>
