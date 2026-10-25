<script setup lang="ts">
import { ref } from 'vue'
import { useTeam } from '../stores/useTeam'

const team = useTeam()
const draft = ref('')

async function submit(): Promise<void> {
  const text = draft.value.trim()
  if (text === '' || team.busy) {
    return
  }
  draft.value = ''
  await team.send(text)
}
</script>

<template>
  <form
    class="fixed bottom-6 left-1/2 flex w-[min(720px,calc(100%-2rem))] -translate-x-1/2 items-center gap-3 rounded-2xl border border-line bg-white px-4 py-3 shadow-sm"
    @submit.prevent="submit"
  >
    <svg
      class="h-6 w-6 shrink-0"
      :class="{ 'animate-spin': team.thinkingAgent }"
      viewBox="0 0 24 24"
      fill="none"
      stroke-width="3"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="9" class="stroke-line" />
      <circle
        cx="12"
        cy="12"
        r="9"
        class="stroke-accent"
        stroke-linecap="round"
        :stroke-dasharray="team.thinkingAgent ? '14 57' : '57 0'"
      />
    </svg>
    <span class="w-32 shrink-0 text-sm text-ink/70">{{ team.statusText }}</span>
    <input
      v-model="draft"
      class="min-w-0 flex-1 rounded-xl border border-line bg-canvas px-3 py-2 text-sm text-ink outline-none focus:border-accent disabled:opacity-60"
      placeholder="Peça algo ao time"
      :disabled="team.busy"
    />
    <button
      type="submit"
      class="rounded-xl bg-accent px-4 py-2 text-sm font-medium text-white disabled:opacity-60"
      :disabled="team.busy || draft.trim() === ''"
    >
      Enviar
    </button>
  </form>
</template>
