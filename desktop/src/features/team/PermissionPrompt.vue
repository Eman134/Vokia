<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import type { VokiaPermission } from './events'
import { AGENTS } from './useTeam'

const props = defineProps<{ permission: VokiaPermission }>()
const emit = defineEmits<{ decide: [allow: boolean] }>()

const agentName = AGENTS.find((agent) => agent.id === props.permission.agent)?.name ?? props.permission.agent
</script>

<template>
  <div role="alertdialog" :aria-label="`${agentName} pede permissão`" class="rounded-card border border-line bg-surface px-4 py-3">
    <p class="text-caption text-muted">{{ agentName }} quer executar ({{ permission.tool }}):</p>
    <code class="mt-1.5 block wrap-break-word whitespace-pre-wrap text-control text-ink">{{ permission.detail }}</code>
    <div class="mt-3 flex gap-2">
      <UiButton variant="approve" size="sm" @press="emit('decide', true)">Aprovar</UiButton>
      <UiButton variant="danger" size="sm" @press="emit('decide', false)">Negar</UiButton>
    </div>
  </div>
</template>
