<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import { HAND_DRAWN_RADIUS, STICKY_INK_VARS } from '../../ui/handDrawn'
import { agentName } from './agents'
import { useFeed } from './useFeed'

// Selo carimbado à mão: raio irregular e traço da própria cor do botão.
const STAMP_CLASSES = `${HAND_DRAWN_RADIUS} border-[1.5px] border-current`
const STICKY_CLASSES = `flex min-w-0 flex-1 items-center gap-2 -rotate-1 rounded-sm bg-mark-yellow px-2.5 py-1 text-ink shadow-paper ${STICKY_INK_VARS}`
const SWAY_CLASSES = 'inline-flex motion-safe:animate-permission-sway'

const feed = useFeed()
</script>

<template>
  <span v-if="feed.permissions[0]" data-testid="permission-inline" :class="STICKY_CLASSES">
    <span class="min-w-0 flex-1 truncate text-label text-ink" :title="feed.permissions[0].detail">
      {{ agentName(feed.permissions[0].agent) }} quer: {{ feed.permissions[0].detail }}
    </span>
    <span v-if="feed.permissions.length > 1" class="shrink-0 text-caption text-muted">
      1 de {{ feed.permissions.length }}
    </span>
    <UiButton :class="STAMP_CLASSES" variant="approve" size="sm" @press="feed.decidePermission(feed.permissions[0].id, true)">
      Permitir
    </UiButton>
    <span :class="feed.permissions[0].suggestAlways && SWAY_CLASSES">
      <UiButton
        :class="STAMP_CLASSES"
        variant="approve"
        size="sm"
        title="Permitir sempre este comando neste projeto"
        @press="feed.decidePermission(feed.permissions[0].id, true, true)"
      >
        Sempre
      </UiButton>
    </span>
    <UiButton
      v-if="feed.permissions[0].rootCommand"
      :class="STAMP_CLASSES"
      variant="approve"
      size="sm"
      :title="`Permitir comandos iniciados com '${feed.permissions[0].rootCommand}'`"
      @press="feed.decidePermission(feed.permissions[0].id, true, true, undefined, true)"
    >
      Raiz
    </UiButton>
    <UiButton :class="STAMP_CLASSES" variant="danger" size="sm" @press="feed.decidePermission(feed.permissions[0].id, false)">
      Negar
    </UiButton>
  </span>
</template>
