<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiMarker from '../../ui/UiMarker.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import MarkdownText from '../team/MarkdownText.vue'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import { DEFAULT_RECIPIENT } from '../team/agents'
import { useConnection } from '../team/useConnection'
import { useDecisions } from '../decisions/useDecisions'
import { COLUMN_LABEL, DEFAULT_COLUMN, UNLINKABLE_COLUMNS, type TaskCardAction, type BacklogItem } from './backlogTypes'
import BacklogPlanActions from './BacklogPlanActions.vue'
import { useBacklog } from './useBacklog'
import { parseProposal } from './planProposal'
import PlanAdjustForm from './PlanAdjustForm.vue'
import PlanAgentCast from './PlanAgentCast.vue'
import PlanDepthCards, { DEPTH_LABELS } from './PlanDepthCards.vue'
import PlanDraftSheet from './PlanDraftSheet.vue'
import PlanLinkPicker from './PlanLinkPicker.vue'
import PlanNoteField from './PlanNoteField.vue'
import PlanSlotPicker from './PlanSlotPicker.vue'
import { tagLabel, tagMarkerTone } from './tags'
import {
  DEFAULT_DEPTH,
  NO_PLAN_TEXT,
  PLAN_TAB_BY_ACTION,
  PLAN_STEPS,
  REVIEWED_PLAN_META,
  useItemPlan,
  type PlanDepth,
} from './useItemPlan'

// simplified: modo do modal de item, sem pedido de plano; o Triar leva ao fluxo completo do PlanDialog.
const props = defineProps<{ item: BacklogItem; simplified?: boolean }>()

const SIMPLIFIED_FRAME = 'rounded-md border border-line p-3'
const DISCARD_MESSAGE = 'Descarta a proposta do plano. Dá para pedir outra depois.'
const PENDING_QUESTION_TEXT = 'Há uma pergunta esperando você no painel de Decisões.'
const UNREVIEWED_PLAN_HINT = `Plano ainda não revisado; ele só vai para ${COLUMN_LABEL.Ready} depois da revisão.`
const TRIAGE_ACTION = { label: 'Triar', id: 'triar' } as const
const AGENT_SLOT_LABEL = 'Escolher quem planeja'
const DEPTH_SLOT_LABEL = 'Escolher a profundidade'
const SLOT_PORTRAIT_SIZE = 20
const CHECKING_REASON = 'Verificando a conexão com os agentes.'
const ADJUST_HINT = 'o que ajustar?'
const NOTE_CONTEXT_PREFIX = 'Vai junto a nota salva:'

const lotsLabel = (count: number, estimate: string): string => {
  const base = `propôs ${count} ${count === 1 ? 'lote' : 'lotes'}`
  return estimate ? `${base}, ${estimate}` : base
}

const backlog = useBacklog()
const itemPlan = useItemPlan()
const connection = useConnection()

const roster = useRoster()
const decisions = useDecisions()
const agent = ref<Agent>(DEFAULT_RECIPIENT)
const depth = ref<PlanDepth>(DEFAULT_DEPTH)
const note = ref(props.item.planNote ?? '')
const adjusting = ref(false)

const current = computed(() => itemPlan.planOf(props.item.id))
const canUnlink = computed(() => UNLINKABLE_COLUMNS.includes(props.item.column ?? DEFAULT_COLUMN))
const agentName = computed(() => roster.profileOf(current.value.agent ?? agent.value)?.name ?? '')
const chosenAgentName = computed(() => roster.profileOf(agent.value)?.name ?? agent.value)
const blockedReason = computed(() => (connection.checking ? CHECKING_REASON : null))
const requestBlocked = computed(() => blockedReason.value !== null)
const parsed = computed(() => parseProposal(current.value.proposal ?? ''))
const pendingQuestion = computed(() => itemPlan.hasPendingQuestion(props.item.id))

function requestPlan(): void {
  if (requestBlocked.value) return
  void itemPlan.request(props.item.id, agent.value, depth.value, note.value)
}


function sendAdjust(text: string): void {
  void itemPlan.adjust(props.item.id, text)
  adjusting.value = false
}

// Mesmas ações do card: o card não abre com o modal do item aberto, então ele fecha antes.
function onCardAction(action: TaskCardAction): void {
  if (action === 'iniciar') return
  backlog.close()
  itemPlan.openDialog(props.item.id, PLAN_TAB_BY_ACTION[action])
}
</script>

<template>
  <div class="flex flex-col gap-3" :class="simplified && SIMPLIFIED_FRAME" data-testid="plan-panel">
    <template v-if="current.state === 'vazio' && simplified">
      <p class="text-control text-ink">{{ NO_PLAN_TEXT }}</p>
      <p v-if="current.error" class="text-small text-danger">{{ current.error }}</p>
      <BacklogPlanActions :action="TRIAGE_ACTION" @action="onCardAction" />
    </template>

    <template v-else-if="current.state === 'vazio'">
      <p v-if="current.error" class="text-small text-danger">{{ current.error }}</p>

      <PlanLinkPicker v-if="itemPlan.dialogTab === 'vincular'" :item-id="item.id" @request="itemPlan.openDialog(item.id, 'pedir')" />

      <template v-else>
        <p class="text-section leading-[2] text-ink" data-testid="plan-sentence">
          Peça a
          <PlanSlotPicker :label="AGENT_SLOT_LABEL" data-testid="plan-agent-slot">
            <template #value>
              <AgentPortrait :who="agent" :size="SLOT_PORTRAIT_SIZE" />
              {{ chosenAgentName }}
            </template>
            <template #default="{ close }">
              <PlanAgentCast v-model="agent" @chosen="close" />
            </template>
          </PlanSlotPicker>
          um plano
          <PlanSlotPicker :label="DEPTH_SLOT_LABEL" data-testid="plan-depth-slot">
            <template #value>{{ DEPTH_LABELS[depth] }}</template>
            <template #default="{ close }">
              <PlanDepthCards v-model="depth" @chosen="close" />
            </template>
          </PlanSlotPicker>.
        </p>
        <PlanNoteField v-model="note" :agent-name="chosenAgentName" @submit="requestPlan" />
        <UiButton class="self-start" variant="primary" size="lg" :disabled="requestBlocked" @press="requestPlan">
          Pedir plano
        </UiButton>
        <p v-if="blockedReason" class="text-caption text-muted" role="status">{{ blockedReason }}</p>
      </template>
    </template>

    <template v-else-if="current.state === 'gerando'">
      <PlanDraftSheet
        v-if="!simplified"
        :agent-name="agentName"
        :interim="current.interim"
        :pending-question="pendingQuestion"
        @answer="decisions.setOpen(true)"
      />
      <template v-else>
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
      </template>
      <div class="flex flex-wrap gap-2">
        <UiButton
          v-if="current.interim && !pendingQuestion"
          variant="quiet"
          size="sm"
          @press="itemPlan.promoteInterim(item.id)"
        >
          Usar a última resposta como proposta
        </UiButton>
        <UiButton variant="quiet" size="sm" @press="itemPlan.cancel(item.id)">Cancelar</UiButton>
      </div>
    </template>

    <template v-else-if="current.state === 'proposto' && !simplified">
      <template v-if="parsed.lots.length">
        <p class="text-label font-semibold text-ink">
          {{ agentName }} {{ lotsLabel(parsed.lots.length, parsed.estimate) }}
        </p>
        <ol class="flex flex-col gap-1.5">
          <li v-for="lot in parsed.lots" :key="lot.n" class="text-control text-ink">
            <b>{{ lot.n }}. {{ lot.title }}</b>
            <span v-if="lot.meta" class="block text-caption text-muted">{{ lot.meta }}</span>
            <p v-if="lot.tasks" class="text-caption text-muted">{{ lot.tasks }}</p>
          </li>
        </ol>
        <p v-if="parsed.risk" class="text-caption text-ink">Risco: {{ parsed.risk }}</p>
        <div v-if="parsed.tags.length" class="flex flex-wrap items-center gap-1.5">
          <span class="text-caption text-muted">Etiquetas sugeridas</span>
          <UiMarker v-for="tag in parsed.tags" :key="tag" :tone="tagMarkerTone(tag)" class="text-meta">{{ tagLabel(tag) }}</UiMarker>
        </div>
        <MarkdownText v-if="parsed.summary" :text="parsed.summary" class="text-caption! text-muted!" />
      </template>
      <template v-else>
        <p class="text-label font-semibold text-ink">Proposta de {{ agentName }}</p>
        <MarkdownText v-if="current.proposal" :text="current.proposal" />
      </template>
      <div class="flex flex-wrap gap-2">
        <UiButton variant="approve" size="lg" @press="itemPlan.accept(item.id)">Aceitar e vincular</UiButton>
        <UiButton variant="quiet" size="lg" :aria-expanded="adjusting" @press="adjusting = !adjusting">Ajustar</UiButton>
        <UiConfirm
          label="Descartar"
          :message="DISCARD_MESSAGE"
          confirm-label="Descartar proposta"
          @confirm="itemPlan.discard(item.id)"
        />
      </div>
      <div v-if="adjusting" class="flex flex-col gap-1.5 border-l-[1.5px] border-ink-pen pl-3">
        <span class="font-hand text-annotation text-ink-pen">{{ ADJUST_HINT }}</span>
        <PlanAdjustForm @submit="sendAdjust" />
        <p v-if="item.planNote" class="line-clamp-2 text-meta text-muted">{{ NOTE_CONTEXT_PREFIX }} {{ item.planNote }}</p>
      </div>
    </template>

    <template v-else>
      <p class="text-control font-medium text-ink">{{ current.plan?.name }}</p>
      <p class="text-caption text-muted">{{ current.plan?.meta }}</p>
      <p v-if="current.plan?.meta !== REVIEWED_PLAN_META && (props.item.column ?? DEFAULT_COLUMN) === DEFAULT_COLUMN" class="text-caption text-muted">{{ UNREVIEWED_PLAN_HINT }}</p>
      <UiButton v-if="canUnlink" class="self-start" variant="quiet" size="sm" @press="itemPlan.unlink(item.id)">Desvincular</UiButton>
    </template>
  </div>
</template>
