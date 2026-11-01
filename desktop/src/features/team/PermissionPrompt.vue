<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import type { VokiaPermission } from './events'
import { agentName } from './useTeam'
import { usePermissionCountdown } from './usePermissionCountdown'

const props = defineProps<{ permission: VokiaPermission }>()
const emit = defineEmits<{ decide: [allow: boolean] }>()

const name = agentName(props.permission.agent)
const { label } = usePermissionCountdown(props.permission.expiresAt)

const approvals =
  props.permission.approvedBefore === 0
    ? 'Primeira vez neste projeto'
    : `Permitido ${props.permission.approvedBefore} ${props.permission.approvedBefore === 1 ? 'vez' : 'vezes'} neste projeto`
</script>

<template>
  <div
    role="alertdialog"
    data-testid="permission-card"
    :aria-label="`${name} pede permissão`"
    class="flex items-start gap-2.5"
  >
    <span
      class="grid size-7 shrink-0 place-items-center rounded-full border border-line bg-surface-2 text-small font-semibold text-ink"
    >
      {{ name.charAt(0) }}
    </span>
    <div class="min-w-0 flex-1">
      <header class="flex items-baseline gap-2">
        <span class="text-caption font-semibold text-ink">{{ name }}</span>
        <span class="text-caption text-muted">aguardando você</span>
      </header>
      <div class="mt-1.5 rounded-card border border-line bg-surface px-4 py-3">
        <p class="text-caption text-muted">{{ name }} quer executar ({{ permission.tool }}):</p>
        <code class="mt-1.5 block wrap-break-word whitespace-pre-wrap text-control text-ink">{{ permission.detail }}</code>
        <p class="mt-2 text-caption text-muted">
          Expira em <span data-testid="permission-countdown">{{ label }}</span> · também dá para decidir na barra
        </p>
        <p class="text-caption text-muted">{{ approvals }}</p>
        <div class="mt-3 flex gap-2">
          <UiButton variant="approve" size="sm" @press="emit('decide', true)">Permitir</UiButton>
          <UiButton variant="danger" size="sm" @press="emit('decide', false)">Negar</UiButton>
        </div>
      </div>
    </div>
  </div>
</template>
