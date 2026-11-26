<script setup lang="ts">
import { HAND_DRAWN_RADIUS, STICKY_INK_VARS } from '../../ui/handDrawn'
import { agentName } from './agents'
import AgentPortrait from './AgentPortrait.vue'
import PermissionActions from './PermissionActions.vue'
import { useFeed } from './useFeed'

// Selo carimbado à mão: raio irregular e traço da própria cor do botão.
const STAMP_CLASSES = `${HAND_DRAWN_RADIUS} border-[1.5px] border-current`
const STICKY_CLASSES = `flex min-w-0 flex-1 items-center gap-2 -rotate-1 rounded-sm bg-mark-yellow px-2.5 py-1 text-ink shadow-paper ${STICKY_INK_VARS}`

const PORTRAIT_SIZE = 20

const feed = useFeed()
</script>

<template>
  <span v-if="feed.projectPermissions[0]" data-testid="permission-inline" :class="STICKY_CLASSES">
    <AgentPortrait :who="feed.projectPermissions[0].agent" :size="PORTRAIT_SIZE" class="shrink-0" />
    <span class="min-w-0 flex-1 truncate text-label text-ink" :title="feed.projectPermissions[0].detail">
      {{ agentName(feed.projectPermissions[0].agent) }} quer: {{ feed.projectPermissions[0].detail }}
    </span>
    <span v-if="feed.projectPermissions.length > 1" class="shrink-0 text-caption text-muted">
      1 de {{ feed.projectPermissions.length }}
    </span>
    <PermissionActions :permission="feed.projectPermissions[0]" :button-class="STAMP_CLASSES" />
  </span>
</template>
