<script setup lang="ts">
import { computed, ref } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import { isUnassigned, type BacklogItem } from '../backlog/backlogTypes'
import { STATE_BADGE, STATE_CHIP } from '../backlog/backlogLabels'
import { useBacklog } from '../backlog/useBacklog'
import { useItemPlan } from '../backlog/useItemPlan'
import { useTriage } from '../backlog/useTriage'
import { agentName } from './agents'

const backlog = useBacklog()
const itemPlan = useItemPlan()
const triage = useTriage()

const adjustingId = ref<string | null>(null)
const adjustDrafts = ref<Record<string, string>>({})

const triageItems = computed<BacklogItem[]>(() =>
  backlog.items.filter((item) => backlog.triagingIds.has(item.id)),
)

function displayId(item: BacklogItem): string {
  return item.publicId ?? item.id
}

function stateOf(item: BacklogItem) {
  return itemPlan.stateFor(item)
}

function isSelected(item: BacklogItem): boolean {
  const current = triage.selectedTriageId ?? triageItems.value[0]?.id ?? null
  return current === item.id
}

function toggleAdjust(itemId: string): void {
  adjustingId.value = adjustingId.value === itemId ? null : itemId
}

function sendAdjustment(itemId: string): void {
  const text = (adjustDrafts.value[itemId] ?? '').trim()
  if (!text) return
  itemPlan.adjust(itemId, text)
  adjustDrafts.value = { ...adjustDrafts.value, [itemId]: '' }
  adjustingId.value = null
}
</script>

<template>
  <section
    v-if="triageItems.length > 0"
    class="mb-3 flex flex-col gap-2.5 rounded-md border border-line bg-surface p-3"
    aria-label="Triagens em andamento"
    data-testid="triage-channel-panel"
  >
    <header class="flex items-center justify-between gap-2">
      <b class="text-caption font-semibold text-ink">Triagens e pré-aprovações ({{ triageItems.length }})</b>
    </header>

    <div
      v-for="item in triageItems"
      :key="item.id"
      class="flex flex-col gap-2 rounded-sm border bg-paper p-2.5"
      :class="isSelected(item) ? 'border-ink/40' : 'border-line'"
      data-testid="triage-card"
    >
      <div class="flex flex-wrap items-center gap-2">
        <span class="rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-tiny font-bold text-ink">
          {{ displayId(item) }}
        </span>
        <button
          type="button"
          class="min-w-0 flex-1 cursor-pointer truncate text-left text-caption font-semibold text-ink hover:underline"
          :title="item.title"
          @click="backlog.open(item.id)"
        >
          {{ item.title }}
        </button>
        <UiBadge :status="STATE_BADGE[stateOf(item)]" :label="STATE_CHIP[stateOf(item)]" />
        <span v-if="!isUnassigned(item)" class="text-tiny text-muted">
          {{ agentName(item.who) }}
        </span>
        <UiButton
          variant="quiet"
          size="sm"
          :class="isSelected(item) && 'bg-surface-2 text-ink'"
          @click="triage.selectedTriageId = item.id"
        >
          {{ isSelected(item) ? 'Falando aqui' : 'Conversar' }}
        </UiButton>
      </div>

      <div
        v-if="itemPlan.proposalFor(item)"
        class="flex flex-col gap-1.5 rounded-sm bg-sticky px-2.5 py-2 text-mark-ink"
      >
        <p v-if="itemPlan.proposalFor(item)!.summary" class="text-caption leading-[1.45]">
          {{ itemPlan.proposalFor(item)!.summary }}
        </p>
        <p class="text-meta">
          {{ itemPlan.proposalFor(item)!.lots.length }} etapas<template v-if="itemPlan.proposalFor(item)!.estimate"> · {{ itemPlan.proposalFor(item)!.estimate }}</template>
        </p>
        <ul v-if="itemPlan.proposalFor(item)!.lots.length" class="flex list-disc flex-col gap-0.5 pl-4 text-meta">
          <li v-for="lot in itemPlan.proposalFor(item)!.lots" :key="lot.n">{{ lot.title }}</li>
        </ul>
      </div>

      <div v-if="stateOf(item) === 'proposto'" class="flex flex-wrap items-center gap-1.5">
        <UiButton variant="primary" size="sm" @press="itemPlan.accept(item.id)">
          Aprovar plano
        </UiButton>
        <UiButton variant="quiet" size="sm" @press="toggleAdjust(item.id)">
          Pedir alterações
        </UiButton>
        <UiButton variant="quiet" size="sm" @press="itemPlan.discard(item.id)">
          Recusar
        </UiButton>
      </div>

      <div v-if="adjustingId === item.id" class="flex flex-col gap-1.5">
        <textarea
          v-model="adjustDrafts[item.id]"
          rows="2"
          class="w-full resize-none rounded-sm border border-line bg-surface px-2.5 py-1.5 text-caption text-ink placeholder:text-muted focus:border-ink focus:outline-none"
          placeholder="Descreva as alterações desejadas no plano…"
        />
        <div class="flex justify-end gap-1.5">
          <UiButton variant="quiet" size="sm" @press="adjustingId = null">Cancelar</UiButton>
          <UiButton
            variant="primary"
            size="sm"
            :disabled="!(adjustDrafts[item.id] ?? '').trim()"
            @press="sendAdjustment(item.id)"
          >
            Enviar alterações
          </UiButton>
        </div>
      </div>
    </div>
  </section>
</template>
