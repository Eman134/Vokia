<script setup lang="ts">
import { computed, ref, useId } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiProgress from '../../ui/UiProgress.vue'
import { HAND_SELECTED_CLASSES, INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import { planProgress, stagesOf } from '../artifacts/planProgress'
import { parseProposal } from './planProposal'
import { useBacklog } from './useBacklog'
import { useItemPlan, type LinkedPlan } from './useItemPlan'

const props = defineProps<{ itemId: string }>()
const emit = defineEmits<{ request: [] }>()

const SEARCH_LABEL = 'Buscar plano'
const SEARCH_PLACEHOLDER = 'Buscar pelo nome do plano'
const EMPTY_TEXT = 'Nenhum plano para vincular ainda.'
const EMPTY_HINT = 'Planos aceitos em outros itens aparecem aqui.'
const NO_MATCH_TEXT = 'Nenhum plano com esse nome.'
const SEARCH_CLASSES = `w-full min-w-0 rounded-md border border-line bg-surface px-3 py-1.5 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`
const ROW_CLASSES = `flex w-full cursor-pointer flex-col gap-0.5 rounded-md border-[1.5px] border-transparent px-2.5 py-2 text-left hover:bg-surface-2 ${INK_FOCUS_CLASSES}`

const backlog = useBacklog()
const itemPlan = useItemPlan()
const query = ref('')
const chosenId = ref<string | null>(null)
const searchId = useId()

const visible = computed(() => {
  const needle = query.value.trim().toLowerCase()
  return itemPlan.plans.filter((plan) => plan.name.toLowerCase().includes(needle))
})

const chosen = computed(() => visible.value.find((plan) => plan.id === chosenId.value))

const rows = computed(() =>
  visible.value.map((plan) => {
    const progress = planProgress(stagesOf(backlog.items, plan.id))
    return { plan, progress, label: progress ? stageLabel(progress.total, progress.done) : plan.meta }
  }),
)

const preview = computed(() => (chosen.value ? previewOf(chosen.value) : undefined))

function stageLabel(total: number, done: number): string {
  return `${total} ${total === 1 ? 'etapa' : 'etapas'}, ${done} ${done === 1 ? 'feita' : 'feitas'}`
}

// A proposta aceita fica no item que originou o plano; é dela que sai o resumo do preview.
function previewOf(plan: LinkedPlan) {
  const source = Object.values(itemPlan.states).find((entry) => entry.plan?.id === plan.id && entry.proposal)
  return source?.proposal ? parseProposal(source.proposal) : undefined
}

function link(): void {
  if (!chosen.value) return
  itemPlan.link(props.itemId, chosen.value.id)
  chosenId.value = null
}
</script>

<template>
  <div class="flex flex-col gap-2">
    <div v-if="!itemPlan.plans.length" class="flex flex-col items-start gap-2 py-3">
      <p class="font-hand text-annotation text-muted">{{ EMPTY_TEXT }}</p>
      <p class="text-caption text-muted">{{ EMPTY_HINT }}</p>
      <UiButton variant="primary" size="sm" @press="emit('request')">Pedir um plano</UiButton>
    </div>
    <template v-else>
      <label :for="searchId" class="sr-only">{{ SEARCH_LABEL }}</label>
      <input :id="searchId" v-model="query" type="search" :placeholder="SEARCH_PLACEHOLDER" :class="SEARCH_CLASSES" />
      <p v-if="!visible.length" class="text-caption text-muted">{{ NO_MATCH_TEXT }}</p>
      <ul v-else class="flex flex-col gap-1" data-testid="plan-link-list">
        <li v-for="{ plan, progress, label } in rows" :key="plan.id">
          <button
            type="button"
            :aria-pressed="chosenId === plan.id"
            :class="[ROW_CLASSES, chosenId === plan.id && HAND_SELECTED_CLASSES]"
            @click="chosenId = chosenId === plan.id ? null : plan.id"
          >
            <span class="flex w-full flex-wrap items-baseline justify-between gap-2">
              <span class="text-control font-semibold text-ink">{{ plan.name }}</span>
              <span class="text-meta text-muted">{{ label }}</span>
            </span>
            <UiProgress v-if="progress" :value="progress.percent" pencil class="mt-1" />
          </button>
          <div v-if="chosenId === plan.id" class="flex flex-col gap-1.5 px-2.5 pt-1.5 pb-1">
            <template v-if="preview">
              <p v-if="preview.summary" class="line-clamp-3 text-caption text-muted">{{ preview.summary }}</p>
              <ol class="list-decimal pl-5 text-caption text-ink">
                <li v-for="lot in preview.lots" :key="lot.n">{{ lot.title }}</li>
              </ol>
            </template>
            <UiButton class="self-start" variant="primary" size="sm" @press="link">Vincular este plano</UiButton>
          </div>
        </li>
      </ul>
    </template>
  </div>
</template>
