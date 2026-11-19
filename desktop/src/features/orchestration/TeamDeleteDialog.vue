<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import { CANCEL_LABEL } from '../agents/agentFormCopy'
import ModalPanel from './ModalPanel.vue'
import { VIEW_COPY } from './orchestrationLabels'

defineProps<{ teamName: string }>()
const emit = defineEmits<{ confirm: []; cancel: [] }>()
</script>

<template>
  <ModalPanel :title="VIEW_COPY.deleteTeamTitle" @close="emit('cancel')">
    <p class="text-body text-muted">{{ VIEW_COPY.removeTeamConfirm }}</p>
    <p class="text-control font-semibold text-ink">{{ teamName }}</p>
    <div class="flex justify-end gap-2">
      <UiButton variant="quiet" size="lg" @press="emit('cancel')">{{ CANCEL_LABEL }}</UiButton>
      <UiButton variant="danger" size="lg" @press="emit('confirm')">{{ VIEW_COPY.removeTeam }}</UiButton>
    </div>
  </ModalPanel>
</template>
