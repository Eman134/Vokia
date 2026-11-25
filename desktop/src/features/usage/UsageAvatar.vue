<script setup lang="ts">
import { onBeforeUnmount, ref, useId, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useUsage } from './useUsage'
import UserAvatar from '../profile/UserAvatar.vue'
import { useUserProfile } from '../profile/useUserProfile'
import UsagePanel from './UsagePanel.vue'
import {
  OUTER_RING_RADIUS,
  TONE_TEXT,
  clampPercent,
  isUsingCredits,
  mostRestrictiveWindow,
  providerShortName,
  ringDash,
  toneOf,
  usageBgClass,
  usageStrokeClass,
  windowKindLabel,
} from './usagePlans'

defineProps<{ expanded: boolean }>()

const RING_BOX = 44
const RING_CENTER = RING_BOX / 2
const RING_STROKE = 2.4
const PANEL_GAP = 10
const PANEL_LEFT = RING_BOX + PANEL_GAP
const USER_AVATAR_SIZE = 28

const store = useUsage()
const profile = useUserProfile()
const { providerList, mostCritical, visible, alert, ariaLabel, openable, now } = storeToRefs(store)
const open = ref(false)
const panelId = useId()
const rootRef = ref<HTMLElement | null>(null)
const triggerRef = ref<HTMLButtonElement | null>(null)

function onDocumentPointerDown(event: PointerEvent): void {
  if (!open.value) return
  const target = event.target as Node | null
  if (target && rootRef.value?.contains(target)) return
  open.value = false
}

watch(open, (isOpen) => {
  if (isOpen) {
    document.addEventListener('pointerdown', onDocumentPointerDown)
  } else {
    document.removeEventListener('pointerdown', onDocumentPointerDown)
  }
})

watch(openable, (isOpenable) => {
  if (!isOpenable) open.value = false
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onDocumentPointerDown)
})

function toggle(): void {
  open.value = !open.value
  if (open.value) store.refreshClock()
}

function closeAndRefocus(): void {
  if (!open.value) return
  open.value = false
  triggerRef.value?.focus()
}

function emphasisClass(providerId: string, percent: number, usingCredits: boolean) {
  const thresholds = store.thresholdsOf(providerId)
  if (usingCredits) return ['font-semibold', TONE_TEXT.accent]
  return percent >= thresholds.warn ? ['font-semibold', TONE_TEXT[toneOf(percent, thresholds)]] : []
}
</script>

<template>
  <div ref="rootRef" class="relative flex flex-col" @keydown.esc="closeAndRefocus">
    <button
      ref="triggerRef"
      type="button"
      :class="[
        'mt-1 flex items-center gap-2.5 rounded-md border-0 bg-transparent text-left',
        openable ? 'cursor-pointer' : 'disabled:cursor-not-allowed',
        expanded ? 'p-0 pr-1' : 'p-0',
      ]"
      :style="{ height: `${RING_BOX}px` }"
      :disabled="!openable"
      :aria-label="ariaLabel"
      :title="ariaLabel"
      :aria-expanded="open"
      :aria-controls="open ? panelId : undefined"
      @click="toggle"
    >
      <span class="relative grid size-11 flex-none place-items-center">
        <svg
          v-if="mostCritical && visible"
          class="absolute inset-0 size-11"
          :viewBox="`0 0 ${RING_BOX} ${RING_BOX}`"
          aria-hidden="true"
        >
          <circle
            :cx="RING_CENTER"
            :cy="RING_CENTER"
            :r="OUTER_RING_RADIUS"
            fill="none"
            :stroke-width="RING_STROKE"
            class="stroke-line"
          />
          <circle
            :cx="RING_CENTER"
            :cy="RING_CENTER"
            :r="OUTER_RING_RADIUS"
            fill="none"
            :stroke-width="RING_STROKE"
            stroke-linecap="round"
            :class="usageStrokeClass(mostCritical.window.percent, mostCritical.thresholds, mostCritical.usingCredits)"
            :stroke-dasharray="ringDash(mostCritical.window.percent, OUTER_RING_RADIUS)"
            :transform="`rotate(-90 ${RING_CENTER} ${RING_CENTER})`"
          />
        </svg>
        <UserAvatar :size="USER_AVATAR_SIZE" :title="profile.displayName" class="relative" />
        <span
          v-if="mostCritical && alert"
          class="absolute top-px right-px size-2.25 rounded-full shadow-[0_0_0_2px_var(--color-surface)]"
          :class="usageBgClass(mostCritical.window.percent, mostCritical.thresholds, mostCritical.usingCredits)"
        />
      </span>
      <span v-if="expanded" class="flex min-w-0 flex-1 flex-col gap-1">
        <b class="truncate text-caption font-medium text-ink" :title="profile.displayName">{{ profile.displayName }}</b>
        <template v-if="visible">
          <template v-for="provider in providerList" :key="provider.providerId">
            <span
              v-if="mostRestrictiveWindow(provider, now)"
              class="flex items-center gap-1.5 text-tiny text-muted"
            >
              <span class="w-14 flex-none truncate">
                {{ providerShortName(provider) }} {{ windowKindLabel(mostRestrictiveWindow(provider, now)!.kind) }}
              </span>
              <span class="h-1 flex-1 overflow-hidden rounded-full bg-line">
                <span
                  class="block h-full rounded-full"
                  :class="
                    usageBgClass(
                      mostRestrictiveWindow(provider, now)!.percent,
                      store.thresholdsOf(provider.providerId),
                      isUsingCredits(provider),
                    )
                  "
                  :style="{ width: `${clampPercent(mostRestrictiveWindow(provider, now)!.percent)}%` }"
                />
              </span>
              <span
                class="w-7 flex-none text-right tabular-nums"
                :class="
                  emphasisClass(
                    provider.providerId,
                    mostRestrictiveWindow(provider, now)!.percent,
                    isUsingCredits(provider),
                  )
                "
              >
                {{ mostRestrictiveWindow(provider, now)!.percent }}%
              </span>
            </span>
          </template>
        </template>
      </span>
    </button>
    <UsagePanel
      v-if="open"
      :panel-id="panelId"
      :style="{ left: `${PANEL_LEFT}px` }"
      @close="closeAndRefocus"
    />
  </div>
</template>
