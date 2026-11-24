<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import type { VokiaPermission } from './events'
import { useFeed } from './useFeed'

const props = withDefaults(defineProps<{ permission: VokiaPermission; reason?: string; buttonClass?: string }>(), {
  reason: '',
  buttonClass: '',
})

const PERMISSION_REASON_MAX_CHARS = 500
const ALWAYS_TITLE = 'Permitir sempre este comando neste projeto'
const SWAY_CLASSES = 'inline-flex motion-safe:animate-permission-sway'

const feed = useFeed()

function rootTitle(rootCommand: string): string {
  return `Permitir comandos iniciados com '${rootCommand}'`
}

function deny(): void {
  const reason = Array.from(props.reason.trim()).slice(0, PERMISSION_REASON_MAX_CHARS).join('')
  void feed.decidePermission(props.permission.id, false, false, reason === '' ? undefined : reason)
}
</script>

<template>
  <span class="flex shrink-0 items-center gap-1.5">
    <UiButton :class="buttonClass" variant="approve" size="sm" @press="feed.decidePermission(permission.id, true)">
      Permitir
    </UiButton>
    <span :class="permission.suggestAlways && SWAY_CLASSES">
      <UiButton
        :class="buttonClass"
        variant="approve"
        size="sm"
        :title="ALWAYS_TITLE"
        @press="feed.decidePermission(permission.id, true, true)"
      >
        Sempre
      </UiButton>
    </span>
    <UiButton
      v-if="permission.rootCommand"
      :class="buttonClass"
      variant="approve"
      size="sm"
      :title="rootTitle(permission.rootCommand)"
      @press="feed.decidePermission(permission.id, true, true, undefined, true)"
    >
      Raiz
    </UiButton>
    <UiButton :class="buttonClass" variant="danger" size="sm" @press="deny">Negar</UiButton>
  </span>
</template>
