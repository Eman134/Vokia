<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { useUsage } from './useUsage'
import { TONE_BG, TONE_TEXT, clampPercent, renewsLabel, stateOf, toneOf, updatedLabel } from './usagePlans'

defineProps<{ panelId: string }>()
const emit = defineEmits<{ close: [] }>()

const WINDOWS = [
  { key: 'fiveHour', label: '5 h' },
  { key: 'sevenDay', label: '7 d' },
] as const

const { claude, now, openable, startupError } = storeToRefs(useUsage())
</script>

<template>
  <div
    v-if="openable"
    :id="panelId"
    role="dialog"
    aria-label="Uso dos planos"
    class="absolute bottom-0 z-40 flex w-78 flex-col gap-3 rounded-popover border border-line bg-surface p-3.5 text-left text-ink shadow-float"
  >
    <div class="flex items-baseline gap-2">
      <b class="text-control">Uso dos planos</b>
      <span v-if="claude" class="text-meta text-muted">{{ updatedLabel(claude.updatedAt, now) }}</span>
      <button
        type="button"
        aria-label="Fechar"
        class="ml-auto cursor-pointer border-0 bg-transparent py-0.25 px-1.5 text-small text-muted"
        @click="emit('close')"
      >
        Fechar
      </button>
    </div>

    <template v-if="claude">
      <div class="flex flex-col gap-1.5">
        <span class="flex items-center gap-1.5 text-caption">
          <b class="font-semibold">Claude Code</b>
          <span class="ml-auto text-tiny font-semibold" :class="TONE_TEXT[stateOf(claude.fiveHour.percent).tone]">{{
            stateOf(claude.fiveHour.percent).label
          }}</span>
        </span>
        <span
          v-for="window in WINDOWS"
          :key="window.key"
          class="grid grid-cols-[26px_minmax(0,1fr)_36px] items-center gap-2 text-meta"
        >
          <span class="text-muted">{{ window.label }}</span>
          <span class="relative h-1.5 overflow-hidden rounded-full bg-surface-2">
            <span
              class="block h-full rounded-full"
              :class="TONE_BG[toneOf(claude[window.key].percent)]"
              :style="{ width: `${clampPercent(claude[window.key].percent)}%` }"
            />
          </span>
          <span class="text-right font-medium tabular-nums">{{ claude[window.key].percent }}%</span>
        </span>
        <span class="text-meta text-muted">{{ renewsLabel(claude.fiveHour.resetsAt, now) }}</span>
      </div>

      <div class="flex gap-1.5 border-t border-line pt-2.5">
        <a
          class="grid h-8 flex-1 place-items-center rounded-control bg-ink text-caption font-semibold text-bg no-underline"
          >Ver em Estatísticas</a
        >
        <a
          class="grid h-8.5 place-items-center rounded-control border border-line px-2.75 text-caption text-ink no-underline"
          >Limites</a
        >
      </div>
    </template>
    <span v-else class="text-meta" :class="TONE_TEXT.danger">{{ startupError }}</span>
  </div>
</template>
