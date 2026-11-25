<script setup lang="ts">
import { storeToRefs } from 'pinia'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import type { ProviderPlanUsage, ProviderUsageWindow } from '../team/events'
import { useUsage } from './useUsage'
import {
  TONE_TEXT,
  clampPercent,
  isUsingCredits,
  mostRestrictiveWindow,
  paceInfo,
  primaryWindowsOf,
  providerDisplayName,
  renewsLabel,
  stateOf,
  updatedLabel,
  usageBgClass,
  windowKindLabel,
} from './usagePlans'

defineProps<{ panelId: string }>()
const emit = defineEmits<{ close: [] }>()

const PANEL_TITLE = 'Uso dos planos'
const CREDITS_BADGE_LABEL = 'Usando créditos'
const READ_FAILED_LABEL = 'última leitura falhou'
const ABOVE_PACE_LABEL = 'ritmo alto'
const REFRESH_LABEL = 'Atualizar'

const store = useUsage()
const { providerList, latestUpdatedAtMs, now, openable, startupError, refreshing } = storeToRefs(store)

function providerStatus(provider: ProviderPlanUsage) {
  const worst = mostRestrictiveWindow(provider, now.value)
  const thresholds = store.thresholdsOf(provider.providerId)
  return stateOf(worst?.percent ?? 0, thresholds, isUsingCredits(provider))
}

function windowBarClass(provider: ProviderPlanUsage, window: ProviderUsageWindow): string {
  return usageBgClass(window.percent, store.thresholdsOf(provider.providerId), isUsingCredits(provider))
}

function paceTooltip(window: ProviderUsageWindow): string | undefined {
  const pace = paceInfo(window, now.value)
  if (!pace) return undefined
  const timeLeft = Math.round(pace.remainingTimePercent)
  const quotaLeft = Math.round(pace.remainingQuotaPercent)
  return pace.status === 'above-pace'
    ? `Ritmo acima do sustentável: ${quotaLeft}% cota restante vs. ${timeLeft}% tempo restante`
    : `Ritmo sustentável: ${quotaLeft}% cota restante vs. ${timeLeft}% tempo restante`
}

function paceMarkerClass(window: ProviderUsageWindow): string {
  const pace = paceInfo(window, now.value)
  if (!pace) return 'bg-graphite'
  return pace.status === 'above-pace' ? 'bg-danger' : 'bg-ok'
}

function isAbovePace(window: ProviderUsageWindow): boolean {
  return paceInfo(window, now.value)?.status === 'above-pace'
}
</script>

<template>
  <div
    v-if="openable"
    :id="panelId"
    role="dialog"
    :aria-label="PANEL_TITLE"
    class="absolute bottom-0 z-40 flex max-h-[75vh] w-84 flex-col gap-3 overflow-y-auto rounded-md border border-line bg-surface p-3.5 text-left text-ink shadow-paper"
  >
    <div class="flex items-center gap-2">
      <b class="text-control">{{ PANEL_TITLE }}</b>
      <span v-if="latestUpdatedAtMs !== null" class="text-meta text-muted">{{
        updatedLabel(latestUpdatedAtMs, now)
      }}</span>
      <button
        type="button"
        class="ml-auto cursor-pointer rounded-sm border border-line bg-surface-2 px-1.5 py-0.5 text-tiny text-ink hover:bg-hover disabled:cursor-not-allowed disabled:opacity-60"
        :disabled="refreshing"
        :aria-label="REFRESH_LABEL"
        :title="REFRESH_LABEL"
        @click="store.refresh()"
      >
        {{ refreshing ? '…' : REFRESH_LABEL }}
      </button>
      <UiCloseButton @press="emit('close')" />
    </div>

    <template v-if="providerList.length > 0">
      <div
        v-for="(provider, index) in providerList"
        :key="provider.providerId"
        class="flex flex-col gap-2"
        :class="index > 0 ? 'border-t border-line pt-2.5' : ''"
      >
        <div class="flex flex-wrap items-center gap-1.5 text-caption">
          <b class="font-semibold">{{ providerDisplayName(provider) }}</b>
          <span
            v-if="provider.creditsAvailable === true"
            class="rounded-sm bg-accent-soft px-1.5 py-0.5 text-tiny font-semibold text-accent"
          >
            {{ CREDITS_BADGE_LABEL }}
          </span>
          <span
            v-else
            class="ml-auto text-tiny font-semibold"
            :class="TONE_TEXT[providerStatus(provider).tone]"
          >
            {{ providerStatus(provider).label }}
          </span>
        </div>

        <span v-if="provider.readFailed" class="text-tiny text-warn-text">
          {{ READ_FAILED_LABEL }}
        </span>

        <div
          v-for="(window, wIdx) in primaryWindowsOf(provider)"
          :key="`${provider.providerId}-w-${wIdx}`"
          class="flex flex-col gap-0.5"
        >
          <span class="grid grid-cols-[54px_minmax(0,1fr)_36px] items-center gap-2 text-meta">
            <span class="truncate text-muted">{{ windowKindLabel(window.kind, window.label) }}</span>
            <span
              class="relative h-1.5 rounded-full bg-surface-2"
              :title="paceTooltip(window)"
            >
              <span
                class="block h-full rounded-full"
                :class="windowBarClass(provider, window)"
                :style="{ width: `${clampPercent(window.percent)}%` }"
              />
              <span
                v-if="paceInfo(window, now)"
                class="pointer-events-none absolute -top-0.5 -bottom-0.5 w-0.5 -translate-x-1/2 rounded-full"
                :class="paceMarkerClass(window)"
                :style="{ left: `${paceInfo(window, now)!.expectedUsedPercent}%` }"
                aria-hidden="true"
              />
            </span>
            <span class="text-right font-medium tabular-nums">{{ window.percent }}%</span>
          </span>
          <span class="flex items-center justify-between pl-[62px] text-tiny text-muted">
            <span>{{ renewsLabel(window.resetsAt, now) }}</span>
            <span v-if="isAbovePace(window)" class="font-medium text-warn-text">{{ ABOVE_PACE_LABEL }}</span>
          </span>
        </div>

        <div
          v-if="provider.groups && provider.groups.length > 0"
          class="mt-0.5 flex flex-col gap-1.5 border-t border-line pt-1.5"
        >
          <div
            v-for="group in provider.groups"
            :key="`${provider.providerId}-g-${group.id}`"
            class="flex flex-col gap-0.5"
          >
            <span class="grid grid-cols-[88px_minmax(0,1fr)_36px] items-center gap-2 text-meta">
              <span
                class="truncate text-muted"
                :title="group.models?.length ? group.models.join(', ') : group.label"
              >
                {{ group.label }}
              </span>
              <span
                class="relative h-1.5 rounded-full bg-surface-2"
                :title="paceTooltip(group.window)"
              >
                <span
                  class="block h-full rounded-full"
                  :class="windowBarClass(provider, group.window)"
                  :style="{ width: `${clampPercent(group.window.percent)}%` }"
                />
                <span
                  v-if="paceInfo(group.window, now)"
                  class="pointer-events-none absolute -top-0.5 -bottom-0.5 w-0.5 -translate-x-1/2 rounded-full"
                  :class="paceMarkerClass(group.window)"
                  :style="{ left: `${paceInfo(group.window, now)!.expectedUsedPercent}%` }"
                  aria-hidden="true"
                />
              </span>
              <span class="text-right font-medium tabular-nums">{{ group.window.percent }}%</span>
            </span>
            <span class="flex items-center justify-between pl-[96px] text-tiny text-muted">
              <span>{{ renewsLabel(group.window.resetsAt, now) }}</span>
              <span v-if="isAbovePace(group.window)" class="font-medium text-warn-text">{{ ABOVE_PACE_LABEL }}</span>
            </span>
          </div>
        </div>
      </div>
    </template>
    <span v-else class="text-meta" :class="TONE_TEXT.danger">{{ startupError }}</span>
  </div>
</template>
