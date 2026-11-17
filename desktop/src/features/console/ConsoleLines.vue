<script setup lang="ts">
import type { ConsoleEntry, ConsoleKind } from '../team/events'

const KIND_PREFIX: Record<ConsoleKind, string> = {
  start: '▶', text: '', tool: '$', output: '', error: '✕', permission: '?', done: '✓', failed: '✕',
}

defineProps<{ entries: readonly ConsoleEntry[] }>()

const KIND_CLASSES: Record<ConsoleKind, string> = {
  start: 'text-term-muted', text: 'text-term-ink', tool: 'text-term-accent', output: 'text-term-muted',
  error: 'text-term-danger', permission: 'text-term-warn', done: 'text-term-ok', failed: 'text-term-danger',
}
</script>

<template>
  <ol class="flex flex-col gap-0.5 font-mono text-small">
    <li v-for="entry in entries" :key="entry.seq" :data-kind="entry.kind" class="flex gap-2" :class="KIND_CLASSES[entry.kind]">
      <span class="w-3 shrink-0 select-none" aria-hidden="true">{{ KIND_PREFIX[entry.kind] }}</span>
      <span class="min-w-0 wrap-break-word whitespace-pre-wrap">{{ entry.text }}</span>
    </li>
  </ol>
</template>
