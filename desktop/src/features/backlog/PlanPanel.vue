<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiChip from '../../ui/UiChip.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import { DEFAULT_RECIPIENT } from '../team/agents'
import { useConnection } from '../team/useConnection'
import { useFeed } from '../team/useFeed'
import { useDecisions } from '../decisions/useDecisions'
import { DEFAULT_COLUMN, UNLINKABLE_COLUMNS, type BacklogItem } from './backlogTypes'
import { useBacklog } from './useBacklog'
import { parseProposal } from './planProposal'
import PlanAdjustForm from './PlanAdjustForm.vue'
import UiMarker from '../../ui/UiMarker.vue'
import { tagLabel, tagMarkerTone } from './tags'
import {
  DEFAULT_DEPTH,
  NO_PLAN_TEXT,
  PLAN_STEPS,
  REVIEWED_PLAN_META,
  useItemPlan,
  type PlanDepth,
  type PlanTab,
} from './useItemPlan'

// simplified: modo do modal de item, sem pedido de plano; o Triar leva ao fluxo completo do PlanDialog.
const props = defineProps<{ item: BacklogItem; simplified?: boolean }>()

const DEPTH_OPTIONS: { id: PlanDepth; label: string }[] = [
  { id: 'rapido', label: 'Rápido · 1 lote' },
  { id: 'completo', label: 'Completo · com pesquisa' },
]
const RECOMMENDED_LABEL = ' (recomendada)'
const DISCARD_MESSAGE = 'Descarta a proposta do plano. Dá para pedir outra depois.'
const PENDING_QUESTION_TEXT = 'Há uma pergunta esperando você no painel de Decisões.'
const UNREVIEWED_PLAN_HINT = 'Plano ainda não revisado; ele só vai para Ready depois da revisão.'

const lotsLabel = (count: number, estimate: string): string => {
  const base = `propôs ${count} ${count === 1 ? 'lote' : 'lotes'}`
  return estimate ? `${base} · ${estimate}` : base
}

const backlog = useBacklog()
const itemPlan = useItemPlan()
const connection = useConnection()

const feed = useFeed()
const roster = useRoster()
const decisions = useDecisions()
const tab = ref<PlanTab>(itemPlan.takePreferredTab())
const agent = ref<Agent>(DEFAULT_RECIPIENT)
const depth = ref<PlanDepth>(DEFAULT_DEPTH)
const chosenPlanId = ref<string | null>(null)
const adjusting = ref(false)

const current = computed(() => itemPlan.planOf(props.item.id))
const canUnlink = computed(() => UNLINKABLE_COLUMNS.includes(props.item.column ?? DEFAULT_COLUMN))
const agentName = computed(() => roster.profileOf(current.value.agent ?? agent.value)?.name ?? '')
const requestBlocked = computed(() => connection.checking || feed.thinkingAgent !== undefined || itemPlan.isAgentAwaiting(agent.value))
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

function sendAdjust(text: string): void {
  void itemPlan.adjust(props.item.id, text)
  adjusting.value = false
}

// Mesma ação do card: o card não abre com o modal aberto, então o modal fecha antes.
function triage(): void {
  backlog.close()
  itemPlan.openDialog(props.item.id, 'pedir')
}
</script>

<template>
  <div class="flex flex-col gap-3 rounded-md border border-line p-3" data-testid="plan-panel">
    <template v-if="current.state === 'vazio'">
      <p class="text-control text-ink">{{ NO_PLAN_TEXT }}</p>
      <p v-if="current.error" class="text-small text-danger">{{ current.error }}</p>
      <div v-if="simplified" class="flex flex-wrap gap-1.5">
        <UiButton variant="primary" size="lg" @press="triage">Triar</UiButton>
        <UiButton variant="quiet" size="lg" @press="tab = 'vincular'">Vincular existente</UiButton>
      </div>
      <div v-else class="flex flex-wrap gap-1.5">
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
          class="h-9 cursor-pointer self-start rounded-md bg-ink px-3.5 text-control font-medium text-bg hover:bg-ink/85"
          :class="chosenPlanId === null && 'opacity-50'"
          :aria-disabled="chosenPlanId === null"
          @click="linkChosen"
        >
          Vincular plano
        </button>
      </template>

      <template v-else-if="!simplified">
        <div class="flex flex-col gap-1.5">
          <p class="text-label font-semibold text-ink">Quem planeja</p>
          <div class="flex flex-wrap gap-1.5">
            <UiChip
              v-for="profile in roster.agents"
              :key="profile.id"
              :label="profile.id === DEFAULT_RECIPIENT ? `${profile.name}${RECOMMENDED_LABEL}` : profile.name"
              :selected="agent === profile.id"
              @press="agent = profile.id"
            />
          </div>
        </div>
        <div class="flex flex-col gap-1.5">
          <p class="text-label font-semibold text-ink">Profundidade</p>
          <div class="flex flex-wrap gap-1.5">
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

    <template v-else-if="current.state === 'proposto' && !simplified">
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
          <UiMarker v-for="tag in parsed.tags" :key="tag" :tone="tagMarkerTone(tag)" class="text-meta">{{ tagLabel(tag) }}</UiMarker>
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
        <UiConfirm
          label="Descartar"
          :message="DISCARD_MESSAGE"
          confirm-label="Descartar proposta"
          @confirm="itemPlan.discard(item.id)"
        />
      </div>
      <PlanAdjustForm v-if="adjusting" @submit="sendAdjust" />
    </template>

    <template v-else>
      <p class="text-control font-medium text-ink">{{ current.plan?.name }}</p>
      <p class="text-caption text-muted">{{ current.plan?.meta }}</p>
      <p v-if="current.plan?.meta !== REVIEWED_PLAN_META && (props.item.column ?? DEFAULT_COLUMN) === DEFAULT_COLUMN" class="text-caption text-muted">{{ UNREVIEWED_PLAN_HINT }}</p>
      <UiButton v-if="canUnlink" class="self-start" variant="quiet" size="sm" @press="unlink">Desvincular</UiButton>
    </template>
  </div>
</template>
