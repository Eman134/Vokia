<script setup lang="ts">
import { ref } from 'vue'
import { useTeam } from './useTeam'

const RING_BOX = 28
const RING_CENTER = RING_BOX / 2
const RING_RADIUS = 11
const RING_STROKE = 2.5
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS
const SPINNING_ARC = RING_CIRCUMFERENCE / 4
const FULL_RING_DASH = `${RING_CIRCUMFERENCE} 0`
const SPINNING_RING_DASH = `${SPINNING_ARC} ${RING_CIRCUMFERENCE - SPINNING_ARC}`
const PILL_CLASSES =
  'fixed bottom-6 inset-x-6 mx-auto flex h-12 max-w-190 items-center gap-3 rounded-full border border-line bg-surface pl-2.5 pr-1.5 shadow-float'

const team = useTeam()
const draft = ref('')

async function submit(): Promise<void> {
  const text = draft.value.trim()
  if (text === '' || team.busy || team.checking) {
    return
  }
  const pending = draft.value
  draft.value = ''
  if (!(await team.send(text))) {
    draft.value = pending
  }
}
</script>

<template>
  <form v-if="team.online" :class="PILL_CLASSES" @submit.prevent="submit">
    <svg
      class="size-6.5 shrink-0"
      :class="{ 'animate-spin': team.thinkingAgent }"
      :viewBox="`0 0 ${RING_BOX} ${RING_BOX}`"
      fill="none"
      :stroke-width="RING_STROKE"
      aria-hidden="true"
    >
      <circle :cx="RING_CENTER" :cy="RING_CENTER" :r="RING_RADIUS" class="stroke-line" />
      <circle
        :cx="RING_CENTER"
        :cy="RING_CENTER"
        :r="RING_RADIUS"
        class="stroke-ok"
        stroke-linecap="round"
        :stroke-dasharray="team.thinkingAgent ? SPINNING_RING_DASH : FULL_RING_DASH"
      />
    </svg>
    <span class="w-32 shrink-0 truncate text-label font-semibold text-muted">{{ team.statusText }}</span>
    <span class="h-5.5 w-px shrink-0 bg-line" aria-hidden="true" />
    <input
      v-model="draft"
      class="h-8.5 min-w-0 flex-1 rounded-full bg-surface-2 px-3.5 text-label text-ink outline-none placeholder:text-muted focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40"
      placeholder="Peça algo ao time"
      :disabled="team.busy || team.checking"
    />
  </form>
  <div role="status" :class="team.online ? undefined : PILL_CLASSES">
    <template v-if="!team.online">
      <svg
        class="size-4 shrink-0 text-danger"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.4"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M12 19.5h.01" />
        <path d="M8.5 15.5a5 5 0 0 1 7 0" />
        <path d="M5 12a10 10 0 0 1 14 0" />
        <path d="M2 8.8a15 15 0 0 1 20 0" />
        <path d="M3 3l18 18" />
      </svg>
      <span class="text-label font-semibold text-danger">Sem conexão</span>
    </template>
  </div>
</template>
