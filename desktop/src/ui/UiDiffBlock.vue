<script setup lang="ts">
type DiffLineKind = 'add' | 'del' | 'ctx'

defineProps<{
  file: string
  lines: { kind: DiffLineKind; text: string }[]
}>()

const LINE_CLASSES: Record<DiffLineKind, string> = {
  add: 'bg-ok-soft text-ink',
  del: 'bg-danger/10 text-ink',
  ctx: 'text-muted',
}
// Contexto usa espaço para manter o texto alinhado com as linhas marcadas.
const PREFIXES: Record<DiffLineKind, string> = {
  add: '+',
  del: '−',
  ctx: ' ',
}
</script>

<template>
  <div class="min-w-0 overflow-hidden rounded-card border border-line bg-surface font-mono text-small">
    <p class="border-b border-line px-3 py-2 text-caption text-muted">{{ file }}</p>
    <div class="overflow-x-auto py-2">
      <div
        v-for="(line, index) in lines"
        :key="index"
        :class="LINE_CLASSES[line.kind]"
        class="whitespace-pre px-3"
      >
        <span class="select-none">{{ PREFIXES[line.kind] }}</span>{{ line.text }}
      </div>
    </div>
  </div>
</template>
