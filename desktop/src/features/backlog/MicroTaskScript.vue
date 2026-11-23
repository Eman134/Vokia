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
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import { useRoster } from '../team/useRoster'

const props = defineProps<{ script: ScriptBlock; who?: string; leader: string; returned?: string[]; learned?: string[] }>()

const roster = useRoster()
const whoName = computed(() => (props.who ? roster.nameOf(props.who) : ''))

const DETAILED_TAG = 'v2 · mais detalhado'
const RETURNED_NOTE = 'Mesmo responsável, roteiro mais detalhado.'
const LEARNED_TITLE = 'Já nasce com · aprendido da memória'
const DONE_JOINER = ' && '
const LIMITS_JOINER = ' · '
const authorNote = (leader: string): string => `escrito pela ${leader}`
const returnedTitle = (leader: string): string => `Voltou da revisão · a ${leader} acrescentou`
const closingNote = (leader: string): string => `Se algo fugir do roteiro, ele para e devolve para a ${leader} em vez de improvisar.`
</script>

<template>
  <div class="flex flex-col gap-2 rounded-md bg-surface-2 p-3 text-caption">
    <p class="flex flex-wrap items-center gap-2">
      <b class="text-ink">{{ whoName ? `Roteiro para ${whoName}` : 'Roteiro' }}</b>
      <UiBadge v-if="returned?.length" status="precisa de você" :label="DETAILED_TAG" />
      <span class="text-muted">{{ authorNote(leader) }}</span>
    </p>
    <section v-if="returned?.length" class="flex flex-col gap-1 rounded-sm bg-sticky p-2 text-mark-ink">
      <h5 class="font-semibold">{{ returnedTitle(leader) }}</h5>
      <p v-for="line in returned" :key="line">+ {{ line }}</p>
      <p>{{ RETURNED_NOTE }}</p>
    </section>
    <section v-if="learned?.length" class="flex flex-col gap-1 rounded-sm bg-mark-blue p-2 text-mark-ink">
      <h5 class="font-semibold">{{ LEARNED_TITLE }}</h5>
      <p v-for="line in learned" :key="line">{{ line }}</p>
    </section>
    <section>
      <h5 class="font-semibold text-ink">Recebe de contexto</h5>
      <p class="text-muted">{{ script.context }}</p>
    </section>
    <section>
      <h5 class="font-semibold text-ink">Pode mexer só em</h5>
      <div class="flex flex-wrap gap-1">
        <code v-for="file in script.files" :key="file" class="rounded-sm bg-surface px-1.5 font-mono text-meta text-muted">{{ file }}</code>
      </div>
    </section>
    <section>
      <h5 class="font-semibold text-ink">Passos</h5>
      <ol class="list-inside list-decimal text-muted">
        <li v-for="step in script.steps" :key="step">{{ step }}</li>
      </ol>
    </section>
    <section>
      <h5 class="font-semibold text-ink">Pronto quando</h5>
      <code class="font-mono text-meta text-muted">{{ script.done.join(DONE_JOINER) }}</code>
    </section>
    <section>
      <h5 class="font-semibold text-ink">Não pode</h5>
      <p class="text-muted">{{ script.limits.join(LIMITS_JOINER) }}</p>
    </section>
    <p class="border-t border-dashed border-line pt-2 text-muted">{{ closingNote(leader) }}</p>
  </div>
</template>
