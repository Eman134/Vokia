<script setup lang="ts">
import { computed } from 'vue'
import { useTeamStatus } from '../useTeamStatus'
import { useFlow } from '../useFlow'
import { panelTitleFor } from '../../decisions/decisionCopy'
import { useDecisions } from '../../decisions/useDecisions'
import UiButton from '../../../ui/UiButton.vue'

const props = defineProps<{ expanded: boolean }>()

const statusStore = useTeamStatus()
const decisions = useDecisions()
const flow = useFlow()
const decisionsTitle = computed(() => panelTitleFor(decisions.pendingCount))
// Minimizada e sem pendência nem check, o botão ficaria vazio e deixaria um vão à direita da pílula.
const showDecisionsButton = computed(
  () => !flow.active && (props.expanded || decisions.pendingCount > 0 || statusStore.status === 'done'),
)
</script>

<template>
  <UiButton
    v-if="showDecisionsButton"
    variant="quiet"
    size="sm"
    :aria-label="decisionsTitle"
    :aria-expanded="decisions.open"
    class="gap-1.75"
    data-testid="decisions-button"
    @press="decisions.toggle()"
  >
    <span
      v-if="decisions.pendingCount > 0"
      class="grid h-4.5 min-w-4.5 place-items-center rounded-full bg-warn px-1.25 text-tiny font-semibold text-bg"
      data-testid="decisions-count"
      >{{ decisions.pendingCount }}</span
    >
    <svg
      v-else-if="statusStore.status === 'done' && !expanded"
      class="size-3 text-ok"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <path d="M2.5 8.5L6 12l7.5-8" />
    </svg>
    <span v-if="expanded">{{ decisionsTitle }}</span>
  </UiButton>
</template>
