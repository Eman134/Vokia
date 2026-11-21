<script setup lang="ts">
import { ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiProgress from '../../ui/UiProgress.vue'
import { PLAN_HINT, type BacklogItem, type BacklogState } from './backlogTypes'
import PlanAdjustForm from './PlanAdjustForm.vue'
import { planSummaryLabel, type PlanBreakdown } from './planBreakdown'
import { useItemPlan } from './useItemPlan'

const props = defineProps<{
  item: BacklogItem
  state: BacklogState
  breakdown: PlanBreakdown
}>()

const CARD_CLASSES = 'flex flex-col gap-2.5 rounded-control-lg border border-line bg-surface-2 p-3'
const TITLE_CLASSES = 'text-label font-semibold text-ink'
const SUMMARY_CLASSES = 'text-control font-semibold text-ink'
const HINT_CLASSES = 'text-caption text-muted'
const PROGRESS_META_CLASSES = 'text-caption text-muted'
const DISCARD_MESSAGE = 'Apaga o plano proposto e o artefato.'

const itemPlan = useItemPlan()
const adjusting = ref(false)
const approving = ref(false)

function approve(): void {
  if (approving.value) return
  approving.value = true
  try {
    itemPlan.accept(props.item.id)
  } finally {
    approving.value = false
  }
}

function sendAdjust(text: string): void {
  void itemPlan.adjust(props.item.id, text)
  adjusting.value = false
}
</script>

<template>
  <section :class="CARD_CLASSES" data-testid="plan-summary-card">
    <h3 :class="TITLE_CLASSES">Plano</h3>
    <p :class="SUMMARY_CLASSES">
      {{ planSummaryLabel(breakdown.steps.length, breakdown.estimate) }}
    </p>

    <template v-if="state === 'proposto'">
      <UiProgress :value="100" tone="warn" />
    </template>
    <template v-else-if="breakdown.progress">
      <UiProgress :value="breakdown.progress.percent" tone="ok" />
      <p :class="PROGRESS_META_CLASSES">
        {{ breakdown.progress.done }} de {{ breakdown.progress.total }} feitas
      </p>
    </template>

    <p :class="HINT_CLASSES">{{ PLAN_HINT[state] }}</p>

    <div v-if="state === 'proposto'" class="flex flex-wrap items-center gap-2 pt-0.5">
      <UiButton
        variant="approve"
        size="lg"
        :disabled="approving"
        data-testid="plan-accept"
        @press="approve"
      >
        Aprovar
      </UiButton>
      <UiButton variant="quiet" size="lg" data-testid="plan-adjust-toggle" @press="adjusting = !adjusting">
        Ajustar
      </UiButton>
      <UiConfirm
        label="Descartar"
        :message="DISCARD_MESSAGE"
        confirm-label="Descartar plano"
        @confirm="itemPlan.discard(item.id)"
      />
    </div>

    <PlanAdjustForm v-if="state === 'proposto' && adjusting" @submit="sendAdjust" />
  </section>
</template>
