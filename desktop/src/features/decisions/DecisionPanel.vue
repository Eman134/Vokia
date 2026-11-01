<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import { EMPTY_TEXT, EMPTY_TITLE, RESOLVED_TITLE, UNDO_LABEL } from './decisionCopy'
import DecisionCard from './DecisionCard.vue'
import { useDecisions } from './useDecisions'

const decisions = useDecisions()
</script>

<template>
  <div class="flex max-h-96 flex-col gap-3 overflow-y-auto">
    <UiEmpty v-if="decisions.cards.length === 0" class="py-4" :title="EMPTY_TITLE" :text="EMPTY_TEXT" />
    <ul v-else class="flex flex-col gap-2">
      <DecisionCard v-for="card in decisions.cards" :key="card.key" :card="card" />
    </ul>
    <section v-if="decisions.resolved.length > 0" :aria-label="RESOLVED_TITLE">
      <p class="px-1 pb-1 text-caption text-muted">{{ RESOLVED_TITLE }}</p>
      <ul class="flex flex-col gap-1">
        <li
          v-for="entry in decisions.resolved"
          :key="entry.key"
          class="flex items-center justify-between gap-2 rounded-control bg-surface-2 px-3 py-1.5"
        >
          <span class="wrap-break-word min-w-0 text-label text-ink">{{ entry.message }}</span>
          <UiButton
            v-if="entry.decisionId !== null && !entry.sent"
            variant="quiet"
            size="sm"
            @press="decisions.undo(entry.decisionId)"
          >
            {{ UNDO_LABEL }}
          </UiButton>
        </li>
      </ul>
    </section>
  </div>
</template>
