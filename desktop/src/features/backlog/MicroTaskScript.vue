<script lang="ts">
import type { ScriptBlock } from './backlogTypes'

const MIN_SCRIPT_STEPS = 2

export function isScriptValid(script: ScriptBlock): boolean {
  return (
    script.context.trim() !== '' &&
    script.files.length > 0 &&
    script.steps.length >= MIN_SCRIPT_STEPS &&
    script.done.length > 0 &&
    script.limits.length > 0
  )
}
</script>

<script setup lang="ts">
defineProps<{ script: ScriptBlock }>()
</script>

<template>
  <div class="flex flex-col gap-2 rounded-control-lg bg-surface-2 p-3 text-caption">
    <section>
      <h5 class="font-semibold text-ink">Recebe de contexto</h5>
      <p class="text-muted">{{ script.context }}</p>
    </section>
    <section>
      <h5 class="font-semibold text-ink">Pode mexer só em</h5>
      <ul class="text-muted">
        <li v-for="file in script.files" :key="file">{{ file }}</li>
      </ul>
    </section>
    <section>
      <h5 class="font-semibold text-ink">Passos</h5>
      <ol class="list-inside list-decimal text-muted">
        <li v-for="step in script.steps" :key="step">{{ step }}</li>
      </ol>
    </section>
    <section>
      <h5 class="font-semibold text-ink">Pronto quando</h5>
      <ul class="text-muted">
        <li v-for="item in script.done" :key="item">{{ item }}</li>
      </ul>
    </section>
    <section>
      <h5 class="font-semibold text-ink">Não pode</h5>
      <ul class="text-muted">
        <li v-for="item in script.limits" :key="item">{{ item }}</li>
      </ul>
    </section>
  </div>
</template>
