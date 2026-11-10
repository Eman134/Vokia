<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import { PLAN_DIALOG_CHIP, STATE_BADGE } from './backlogLabels'
import { trapFocus } from './dialogFocus'
import PlanPanel from './PlanPanel.vue'
import { useBacklog } from './useBacklog'
import { useItemPlan } from './useItemPlan'

const backlog = useBacklog()
const itemPlan = useItemPlan()
const titleId = useId()
const dialog = ref<HTMLElement>()
let previousFocus: HTMLElement | null = null

const item = computed(() => backlog.items.find((current) => current.id === itemPlan.dialogItemId) ?? null)
const state = computed(() => (item.value ? itemPlan.stateFor(item.value) : 'vazio'))

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
      class="flex max-h-full w-105 max-w-full flex-col gap-3 overflow-y-auto rounded-dialog border border-line bg-surface p-5 shadow-float"
      @keydown.esc.stop="itemPlan.closeDialog()"
      @keydown.tab="trapFocus($event, dialog)"
    >
      <header class="flex flex-wrap items-center gap-2">
        <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true" class="text-muted">
          <path
            d="M3 3h2M3 8h2M3 13h2M7.5 3h6M7.5 8h6M7.5 13h6"
            fill="none"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linecap="round"
          />
        </svg>
        <b :id="titleId" class="text-control text-ink">Plano</b>
        <span class="text-caption text-muted">{{ item.id }} · {{ item.title }}</span>
        <UiBadge :status="STATE_BADGE[state]" :label="PLAN_DIALOG_CHIP[state]" />
        <UiButton class="ml-auto" variant="quiet" size="sm" @press="itemPlan.closeDialog()">Fechar</UiButton>
      </header>
      <PlanPanel :item="item" />
    </section>
  </div>
</template>
