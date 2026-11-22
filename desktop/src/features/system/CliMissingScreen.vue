<script lang="ts">
export const CLI_MISSING_TITLE = 'Claude Code não encontrado'
export const CLI_MISSING_HELP = 'Instale a CLI e faça login nela. A Vokia não guarda seu login.'
export const CLI_RECHECK_LABEL = 'Verificar de novo'
</script>

<script setup lang="ts">
import type { CliProblem } from '../team/events'
import VokiaMascot from '../brand/VokiaMascot.vue'
import UiButton from '../../ui/UiButton.vue'
import { STICKY_INK_VARS } from '../../ui/handDrawn'

const SHEET_CLASSES = `flex max-w-xl flex-col items-center gap-4 -rotate-1 rounded-sm bg-mark-yellow p-8 text-center text-ink shadow-paper ${STICKY_INK_VARS}`

defineProps<{ problem: CliProblem }>()
const emit = defineEmits<{ recheck: [] }>()
</script>

<template>
  <main class="flex h-screen items-center justify-center bg-bg p-8">
    <section :class="SHEET_CLASSES">
      <VokiaMascot look="erro" />
      <h1 class="text-section">{{ CLI_MISSING_TITLE }}</h1>
      <p class="max-w-lg text-body text-muted">{{ CLI_MISSING_HELP }}</p>
      <p class="max-w-lg text-body text-muted">{{ problem.reason }}</p>
      <code class="rounded-sm bg-surface-2 px-2.5 py-2 font-mono text-caption text-ink">
        {{ problem.fixCommand }}
      </code>
      <UiButton variant="primary" size="lg" @press="emit('recheck')">{{ CLI_RECHECK_LABEL }}</UiButton>
    </section>
  </main>
</template>
