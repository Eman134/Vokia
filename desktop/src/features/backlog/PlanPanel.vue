<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiChip from '../../ui/UiChip.vue'
import type { Agent } from '../team/events'
import { AGENTS, DEFAULT_RECIPIENT, useTeam } from '../team/useTeam'
import type { BacklogItem } from './backlogTypes'
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

const itemPlan = useItemPlan()
const team = useTeam()
const tab = ref<PlanTab>(itemPlan.takePreferredTab())
const agent = ref<Agent>(DEFAULT_RECIPIENT)
const depth = ref<PlanDepth>(DEFAULT_DEPTH)
const chosenPlanId = ref<string | null>(null)

const current = computed(() => itemPlan.planOf(props.item.id))
const agentName = computed(() => AGENTS.find((profile) => profile.id === (current.value.agent ?? agent.value))?.name ?? '')
const requestBlocked = computed(() => team.checking || team.thinkingAgent !== undefined || itemPlan.isAgentAwaiting(agent.value))

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
      <UiButton class="self-start" variant="quiet" size="sm" @press="itemPlan.cancel(item.id)">Cancelar</UiButton>
    </template>

    <template v-else-if="current.state === 'proposto'">
      <p class="text-label font-semibold text-ink">Proposta de {{ agentName }}</p>
      <p class="whitespace-pre-wrap text-control text-ink">{{ current.proposal }}</p>
      <div class="flex gap-2">
        <UiButton variant="approve" size="lg" @press="itemPlan.accept(item.id)">Aceitar e vincular</UiButton>
        <UiButton variant="quiet" size="lg" @press="itemPlan.discard(item.id)">Descartar</UiButton>
      </div>
    </template>

    <template v-else>
      <p class="text-control font-medium text-ink">{{ current.plan?.name }}</p>
      <p class="text-caption text-muted">{{ current.plan?.meta }}</p>
      <UiButton class="self-start" variant="quiet" size="sm" @press="unlink">Desvincular</UiButton>
    </template>
  </div>
</template>
