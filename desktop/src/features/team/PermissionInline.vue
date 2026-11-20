<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import { agentName } from './agents'
import { useFeed } from './useFeed'

const feed = useFeed()
</script>

<template>
  <span
    v-if="feed.permissions[0]"
    data-testid="permission-inline"
    class="flex min-w-0 flex-1 items-center gap-2"
  >
    <span class="min-w-0 flex-1 truncate text-label text-ink" :title="feed.permissions[0].detail">
      {{ agentName(feed.permissions[0].agent) }} quer: {{ feed.permissions[0].detail }}
    </span>
    <span v-if="feed.permissions.length > 1" class="shrink-0 text-caption text-muted">
      1 de {{ feed.permissions.length }}
    </span>
    <UiButton variant="approve" size="sm" @press="feed.decidePermission(feed.permissions[0].id, true)">
      Permitir
    </UiButton>
    <UiButton
      variant="approve"
      size="sm"
      title="Permitir sempre este comando neste projeto"
      @press="feed.decidePermission(feed.permissions[0].id, true, true)"
    >
      Sempre
    </UiButton>
    <UiButton variant="danger" size="sm" @press="feed.decidePermission(feed.permissions[0].id, false)">
      Negar
    </UiButton>
  </span>
</template>
