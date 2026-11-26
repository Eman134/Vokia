<script setup lang="ts">
import { computed } from 'vue'
import { useTeamStatus } from '../useTeamStatus'
import { useFlow } from '../useFlow'
import { panelTitleFor } from '../../decisions/decisionCopy'
import { useDecisions } from '../../decisions/useDecisions'
import { usePanelTab } from '../../panel/usePanelTab'
import UiButton from '../../../ui/UiButton.vue'

const props = defineProps<{ expanded: boolean }>()

const statusStore = useTeamStatus()
const decisions = useDecisions()
const panelTab = usePanelTab()
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
    :title="decisionsTitle"
    :aria-expanded="decisions.open"
    :class="decisions.pendingCount > 0 ? 'gap-1.5 px-2' : 'w-7 px-0'"
    data-testid="decisions-button"
    @press="panelTab.toggle()"
  >
    <span
      v-if="decisions.pendingCount > 0"
      class="grid h-4.5 min-w-4.5 place-items-center rounded-full bg-warn px-1.25 text-tiny font-semibold text-bg"
      data-testid="decisions-count"
      >{{ decisions.pendingCount }}</span
    >
    <svg
      v-else-if="statusStore.status === 'done' && !expanded"
      class="size-3.5 text-ok"
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
    <svg
      v-if="decisions.pendingCount > 0 || expanded"
      class="size-4 shrink-0"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      stroke-width="1.6"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <path d="M2.5 3.5h11v6h-3l-1 2h-3l-1-2h-3z" />
    </svg>
  </UiButton>
</template>
