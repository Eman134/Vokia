<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import { agentName, useTeam } from './useTeam'

const team = useTeam()
</script>

<template>
  <span
    v-if="team.permissions[0]"
    data-testid="permission-inline"
    class="flex min-w-0 shrink items-center gap-2"
  >
    <span class="min-w-0 truncate text-label text-ink" :title="team.permissions[0].detail">
      {{ agentName(team.permissions[0].agent) }} quer: {{ team.permissions[0].detail }}
    </span>
    <span v-if="team.permissions.length > 1" class="shrink-0 text-caption text-muted">
      1 de {{ team.permissions.length }}
    </span>
    <UiButton variant="approve" size="sm" @press="team.decidePermission(team.permissions[0].id, true)">
      Permitir
    </UiButton>
    <UiButton variant="danger" size="sm" @press="team.decidePermission(team.permissions[0].id, false)">
      Negar
    </UiButton>
  </span>
</template>
