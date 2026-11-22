<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import UiSelect from '../../ui/UiSelect.vue'
import UiSwitch from '../../ui/UiSwitch.vue'
import { EVENT_OPTIONS, VIEW_COPY } from './orchestrationLabels'
import { GATEWAY_KIND_VALUES, type GatewayKind, type WhenEvent } from './orchestrationTypes'

export interface GatewayExit {
  key: string
  label: string
  whenEvent: WhenEvent
  isDefault: boolean
}

defineProps<{ kind: GatewayKind; exits: readonly GatewayExit[] }>()
const emit = defineEmits<{
  close: []
  setKind: [kind: GatewayKind]
  setEvent: [key: string, event: WhenEvent]
  setDefault: [key: string]
  remove: []
}>()

const KIND_OPTIONS = GATEWAY_KIND_VALUES.map((value) => ({
  value,
  label: value === 'paralelo' ? VIEW_COPY.gatewayKindParallel : VIEW_COPY.gatewayKindExclusive,
}))
</script>

<template>
  <section class="flex min-w-0 flex-col gap-4 rounded-md border border-line bg-surface p-4.5">
    <header class="flex items-center justify-between gap-3">
      <h3 class="truncate text-control font-bold text-ink">{{ kind === 'paralelo' ? VIEW_COPY.gatewayParallel : VIEW_COPY.gatewayExclusive }}</h3>
      <div class="flex shrink-0 items-center gap-1">
        <UiButton variant="quiet" size="sm" @press="emit('remove')">{{ VIEW_COPY.removeGateway }}</UiButton>
        <UiButton variant="quiet" size="sm" @press="emit('close')">{{ VIEW_COPY.close }}</UiButton>
      </div>
    </header>

    <div class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ VIEW_COPY.gatewayType }}</span>
      <UiSegmented
        :model-value="kind"
        :options="KIND_OPTIONS"
        :label="VIEW_COPY.gatewayType"
        @update:model-value="emit('setKind', $event)"
      />
    </div>

    <div v-if="kind === 'exclusivo'" class="flex flex-col gap-2">
      <span class="text-label font-medium text-muted">{{ VIEW_COPY.gatewayExits }}</span>
      <p v-if="exits.length === 0" class="text-small text-muted">{{ VIEW_COPY.gatewayNoExits }}</p>
      <div v-for="exit in exits" :key="exit.key" class="flex flex-col gap-1.5 rounded-md border border-line p-2.5">
        <span class="truncate text-control font-semibold text-ink">{{ exit.label }}</span>
        <div class="flex items-center justify-between gap-2">
          <span class="text-small text-muted">{{ VIEW_COPY.exitEvent }}</span>
          <UiSelect
            :model-value="exit.whenEvent"
            :options="EVENT_OPTIONS"
            :label="VIEW_COPY.exitEvent"
            @update:model-value="emit('setEvent', exit.key, $event as WhenEvent)"
          />
        </div>
        <div class="flex items-center justify-between gap-2">
          <span class="text-small text-muted">{{ VIEW_COPY.exitDefault }}</span>
          <UiSwitch :model-value="exit.isDefault" :label="VIEW_COPY.exitDefault" @update:model-value="emit('setDefault', exit.key)" />
        </div>
      </div>
    </div>
  </section>
</template>
