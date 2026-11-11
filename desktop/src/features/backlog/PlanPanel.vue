<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiChip from '../../ui/UiChip.vue'
import type { Agent } from '../team/events'
import { AGENTS, DEFAULT_RECIPIENT, useTeam } from '../team/useTeam'
import { useDecisions } from '../decisions/useDecisions'
import type { BacklogItem } from './backlogTypes'
import { parseProposal } from './planProposal'
import { tagLabel } from './tags'
import {
  DEFAULT_DEPTH,
  NO_PLAN_TEXT,
  PLAN_STEPS,
  useItemPlan,
  type PlanDepth,
  type PlanTab,
} from './useItemPlan'

const props = defineProps<{ item: BacklogItem }>()

const DEPTH_OPTIONS: { id: PlanDepth; label: string }[] = [
  { id: 'rapido', label: 'Rápido · 1 lote' },
  { id: 'completo', label: 'Completo · com pesquisa' },
]
const RECOMMENDED_LABEL = ' (recomendada)'
const PENDING_QUESTION_TEXT = 'Há uma pergunta esperando você no painel de Decisões.'
const ADJUST_LABEL = 'O que ajustar'

const lotsLabel = (count: number, estimate: string): string => {
  const base = `propôs ${count} ${count === 1 ? 'lote' : 'lotes'}`
  return estimate ? `${base} · ${estimate}` : base
}

const itemPlan = useItemPlan()
const team = useTeam()
const decisions = useDecisions()
const tab = ref<PlanTab>(itemPlan.takePreferredTab())
const agent = ref<Agent>(DEFAULT_RECIPIENT)
const depth = ref<PlanDepth>(DEFAULT_DEPTH)
const chosenPlanId = ref<string | null>(null)
const adjusting = ref(false)
const adjustText = ref('')

const current = computed(() => itemPlan.planOf(props.item.id))
const agentName = computed(() => AGENTS.find((profile) => profile.id === (current.value.agent ?? agent.value))?.name ?? '')
const requestBlocked = computed(() => team.checking || team.thinkingAgent !== undefined || itemPlan.isAgentAwaiting(agent.value))
const parsed = computed(() => parseProposal(current.value.proposal ?? ''))
const pendingQuestion = computed(() => itemPlan.hasPendingQuestion(props.item.id))

function requestPlan(): void {
  if (requestBlocked.value) return
  void itemPlan.request(props.item.id, agent.value, depth.value)
}

function linkChosen(): void {
  if (chosenPlanId.value === null) return
  itemPlan.link(props.item.id, chosenPlanId.value)
  chosenPlanId.value = null
}

function unlink(): void {
  itemPlan.unlink(props.item.id)
  chosenPlanId.value = null
}

function sendAdjust(): void {
  void itemPlan.adjust(props.item.id, adjustText.value)
  adjustText.value = ''
  adjusting.value = false
}
</script>

<template>
  <div class="flex flex-col gap-3 rounded-control-lg border border-line p-3" data-testid="plan-panel">
    <template v-if="current.state === 'vazio'">
      <p class="text-control text-ink">{{ NO_PLAN_TEXT }}</p>
      <p v-if="current.error" class="text-small text-danger">{{ current.error }}</p>
      <div class="flex gap-1.5">
        <UiChip label="Vincular existente" :selected="tab === 'vincular'" @press="tab = 'vincular'" />
        <UiChip label="Pedir a um agente" :selected="tab === 'pedir'" @press="tab = 'pedir'" />
      </div>

      <template v-if="tab === 'vincular'">
        <p v-if="!itemPlan.plans.length" class="text-caption text-muted">Nenhum plano ainda</p>
        <ul v-else class="flex flex-col gap-1">
          <li v-for="plan in itemPlan.plans" :key="plan.id">
            <label class="flex items-center gap-2 text-control text-ink">
              <input v-model="chosenPlanId" type="radio" name="linked-plan" :value="plan.id" />
              <span>{{ plan.name }}</span>
              <span class="text-caption text-muted">{{ plan.meta }}</span>
            </label>
          </li>
        </ul>
        <button
          type="button"
          class="h-9 self-start rounded-control bg-ink px-3.5 text-control font-medium text-bg"
          :class="chosenPlanId === null && 'opacity-50'"
          :aria-disabled="chosenPlanId === null"
          @click="linkChosen"
        >
          Vincular plano
        </button>
      </template>

      <template v-else>
        <div class="flex flex-col gap-1.5">
          <p class="text-label font-semibold text-ink">Quem planeja</p>
          <div class="flex gap-1.5">
            <UiChip
              v-for="profile in AGENTS"
              :key="profile.id"
              :label="profile.id === DEFAULT_RECIPIENT ? `${profile.name}${RECOMMENDED_LABEL}` : profile.name"
              :selected="agent === profile.id"
              @press="agent = profile.id"
            />
          </div>
        </div>
        <div class="flex flex-col gap-1.5">
          <p class="text-label font-semibold text-ink">Profundidade</p>
          <div class="flex gap-1.5">
            <UiChip
              v-for="option in DEPTH_OPTIONS"
              :key="option.id"
              :label="option.label"
              :selected="depth === option.id"
              @press="depth = option.id"
            />
          </div>
        </div>
        <UiButton class="self-start" variant="primary" size="lg" :disabled="requestBlocked" @press="requestPlan">
          Pedir plano à {{ agentName }}
        </UiButton>
      </template>
    </template>

    <template v-else-if="current.state === 'gerando'">
      <p class="text-control font-medium text-ink">{{ agentName }} está escrevendo o plano</p>
      <ol class="flex flex-col gap-1 text-caption text-muted">
        <li v-for="(step, index) in PLAN_STEPS" :key="step">
          {{ index < PLAN_STEPS.length - 1 ? '✓' : '…' }} {{ step }}
        </li>
      </ol>
      <p v-if="current.interim" class="whitespace-pre-wrap text-caption text-muted">{{ current.interim }}</p>
      <template v-if="pendingQuestion">
        <p class="text-control text-ink">{{ PENDING_QUESTION_TEXT }}</p>
        <UiButton class="self-start" variant="approve" size="sm" @press="decisions.setOpen(true)">Responder</UiButton>
      </template>
      <UiButton
        v-else-if="current.interim"
        class="self-start"
        variant="quiet"
        size="sm"
        @press="itemPlan.promoteInterim(item.id)"
      >
        Usar a última resposta como proposta
      </UiButton>
      <UiButton class="self-start" variant="quiet" size="sm" @press="itemPlan.cancel(item.id)">Cancelar</UiButton>
    </template>

    <template v-else-if="current.state === 'proposto'">
      <template v-if="parsed.lots.length">
        <p class="text-label font-semibold text-ink">
          {{ agentName }} {{ lotsLabel(parsed.lots.length, parsed.estimate) }}
        </p>
        <ol class="flex flex-col gap-1.5">
          <li v-for="lot in parsed.lots" :key="lot.n" class="text-control text-ink">
            <b>{{ lot.n }}. {{ lot.title }}</b>
            <span v-if="lot.meta" class="text-caption text-muted"> · {{ lot.meta }}</span>
            <p v-if="lot.tasks" class="text-caption text-muted">{{ lot.tasks }}</p>
          </li>
        </ol>
        <p v-if="parsed.risk" class="text-caption text-ink">Risco: {{ parsed.risk }}</p>
        <div v-if="parsed.tags.length" class="flex flex-wrap items-center gap-1.5">
          <span class="text-caption text-muted">Etiquetas sugeridas</span>
          <span v-for="tag in parsed.tags" :key="tag" class="rounded-chip bg-surface-2 px-2 text-meta text-ink">{{ tagLabel(tag) }}</span>
        </div>
        <p v-if="parsed.summary" class="whitespace-pre-wrap text-caption text-muted">{{ parsed.summary }}</p>
      </template>
      <template v-else>
        <p class="text-label font-semibold text-ink">Proposta de {{ agentName }}</p>
        <p class="whitespace-pre-wrap text-control text-ink">{{ current.proposal }}</p>
      </template>
      <div class="flex flex-wrap gap-2">
        <UiButton variant="approve" size="lg" @press="itemPlan.accept(item.id)">Aceitar e vincular</UiButton>
        <UiButton variant="quiet" size="lg" @press="adjusting = !adjusting">Ajustar</UiButton>
        <UiButton variant="quiet" size="lg" @press="itemPlan.discard(item.id)">Descartar</UiButton>
      </div>
      <div v-if="adjusting" class="flex flex-col gap-1.5">
        <textarea
          v-model="adjustText"
          :aria-label="ADJUST_LABEL"
          :placeholder="ADJUST_LABEL"
          rows="3"
          class="w-full rounded-control border border-line bg-surface px-3 py-2 text-control text-ink placeholder:text-faint"
        />
        <UiButton class="self-start" variant="primary" size="sm" :disabled="adjustText.trim() === ''" @press="sendAdjust">Enviar ajuste</UiButton>
      </div>
    </template>

    <template v-else>
      <p class="text-control font-medium text-ink">{{ current.plan?.name }}</p>
      <p class="text-caption text-muted">{{ current.plan?.meta }}</p>
      <UiButton class="self-start" variant="quiet" size="sm" @press="unlink">Desvincular</UiButton>
    </template>
  </div>
</template>
