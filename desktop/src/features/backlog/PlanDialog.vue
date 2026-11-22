<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import UiIcon from '../../ui/UiIcon.vue'
import { BACKLOG_ICON_PATH } from '../../shell/railNav'
import { PLAN_DIALOG_CHIP, STATE_BADGE } from './backlogLabels'
import type { BacklogState } from './backlogTypes'
import { trapFocus } from './dialogFocus'
import PlanPanel from './PlanPanel.vue'
import { useBacklog } from './useBacklog'
import { useItemPlan } from './useItemPlan'

// Sem plano ou com a proposta esperando o usuário, o selo só repetiria o que o conteúdo do modal já mostra.
const HIDDEN_BADGE_STATES: readonly BacklogState[] = ['vazio', 'proposto']

const backlog = useBacklog()
const itemPlan = useItemPlan()
const titleId = useId()
const dialog = ref<HTMLElement>()
let previousFocus: HTMLElement | null = null

const item = computed(() => backlog.items.find((current) => current.id === itemPlan.dialogItemId) ?? null)
const state = computed(() => (item.value ? itemPlan.stateFor(item.value) : 'vazio'))
// Sem plano, o título diz qual dos dois modais está aberto; depois do pedido, os dois viram o acompanhamento do plano.
const title = computed(() => {
  const name = item.value?.title ?? ''
  if (state.value !== 'vazio') return `Plano para ${name}`
  return itemPlan.dialogTab === 'vincular' ? `Vincular plano a ${name}` : `Pedir plano para ${name}`
})

watch(item, (current) => {
  if (itemPlan.dialogItemId !== null && current === null) itemPlan.closeDialog()
})

watch(
  () => itemPlan.dialogItemId,
  async (id) => {
    if (id === null) {
      previousFocus?.focus()
      previousFocus = null
      return
    }
    previousFocus ??= document.activeElement instanceof HTMLElement ? document.activeElement : null
    await nextTick()
    dialog.value?.focus()
  },
)

// Quando o estado troca, o botão focado sai do DOM e o foco cai no body; sem ele na janela, Esc e Tab não a alcançam.
watch(
  state,
  () => {
    if (dialog.value && !dialog.value.contains(document.activeElement)) dialog.value.focus()
  },
  { flush: 'post' },
)
</script>

<template>
  <div
    v-if="item"
    class="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4"
    @click.self="itemPlan.closeDialog()"
  >
    <section
      ref="dialog"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      tabindex="-1"
      class="flex max-h-full w-130 max-w-full flex-col gap-3 overflow-y-auto rounded-md border border-line bg-surface p-5 shadow-paper"
      @keydown.esc.stop="itemPlan.closeDialog()"
      @keydown.tab="trapFocus($event, dialog)"
    >
      <header class="flex flex-wrap items-center gap-2">
        <UiIcon :path="BACKLOG_ICON_PATH" :size="14" class="text-muted" />
        <b :id="titleId" class="min-w-0 text-control text-ink">{{ title }}</b>
        <span class="font-mono text-meta text-muted">{{ item.id }}</span>
        <UiBadge v-if="!HIDDEN_BADGE_STATES.includes(state)" :status="STATE_BADGE[state]" :label="PLAN_DIALOG_CHIP[state]" />
        <UiCloseButton class="ml-auto" @press="itemPlan.closeDialog()" />
      </header>
      <PlanPanel :key="item.id" :item="item" />
    </section>
  </div>
</template>
