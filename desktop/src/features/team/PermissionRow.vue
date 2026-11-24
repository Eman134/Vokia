<script setup lang="ts">
import { computed, ref, useId } from 'vue'
import UiField from '../../ui/UiField.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import ActivityRow from './ActivityRow.vue'
import { agentName } from './agents'
import type { VokiaPermission } from './events'
import PermissionActions from './PermissionActions.vue'
import { permissionRequestRow } from './permissionSummary'
import { usePermissionCountdown } from './usePermissionCountdown'

const props = defineProps<{ permission: VokiaPermission; pendingCount: number }>()

const REASON_LABEL = 'Motivo ao negar (opcional)'
const SUGGEST_ALWAYS_TEXT = 'Você costuma permitir isto neste projeto. Quer permitir sempre neste projeto?'
const DETAILS_LABEL = 'Detalhes do pedido de permissão'
const COUNT_CLASSES = 'shrink-0 text-caption text-muted'

const row = computed(() => permissionRequestRow(props.permission, agentName))
const name = computed(() => agentName(props.permission.agent))
const { label: countdown } = usePermissionCountdown(props.permission.expiresAt)
const approvals = computed(() => {
  const count = props.permission.approvedBefore
  if (count === 0) return 'Primeira vez neste projeto'
  return `Permitido ${count} ${count === 1 ? 'vez' : 'vezes'} neste projeto`
})

const open = ref(false)
const reason = ref('')
const detailsId = useId()
</script>

<template>
  <div role="alertdialog" data-testid="permission-card" :aria-label="`${name} pede permissão`">
    <ul class="flex min-w-0 flex-col text-caption text-muted">
      <ActivityRow :row="row">
        <span v-if="pendingCount > 1" :class="COUNT_CLASSES">1 de {{ pendingCount }}</span>
        <button
          type="button"
          class="shrink-0 cursor-pointer text-muted hover:text-ink"
          :class="INK_FOCUS_CLASSES"
          :aria-label="DETAILS_LABEL"
          :aria-expanded="open"
          :aria-controls="detailsId"
          @click="open = !open"
        >
          <span aria-hidden="true">{{ open ? '▾' : '▸' }}</span>
        </button>
        <PermissionActions :permission="permission" :reason="reason" class="ml-auto" />
        <template #details>
          <div v-if="open" :id="detailsId" class="flex flex-col gap-1 pl-5">
            <p>Expira em <span data-testid="permission-countdown">{{ countdown }}</span></p>
            <p>{{ approvals }}</p>
            <p v-if="permission.suggestAlways" class="text-ink">{{ SUGGEST_ALWAYS_TEXT }}</p>
            <div class="mt-1 max-w-md"><UiField v-model="reason" :label="REASON_LABEL" /></div>
          </div>
        </template>
      </ActivityRow>
    </ul>
  </div>
</template>
