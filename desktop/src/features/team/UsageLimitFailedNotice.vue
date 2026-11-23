<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiIcon from '../../ui/UiIcon.vue'
import { useUsage } from '../usage/useUsage'
import { resetCountdownLabel } from '../usage/usagePlans'
import { useUsageLimit } from '../usage/useUsageLimit'
import { agentName } from './agents'
import type { FeedUsageLimitFailed, VokiaMessage } from './events'
import type { CliPreset } from './teamApi'
import { useFeed } from './useFeed'
import { useRequests } from './useRequests'
import { useRoster } from './useRoster'

const props = defineProps<{ entry: FeedUsageLimitFailed }>()

const WARNING_ICON_PATH = 'M8 2a6 6 0 1 0 0 12A6 6 0 0 0 8 2z M8 5v3.5M8 10.8v.4'
const RETRY_LABEL = 'Tentar de novo'
const RETRY_FALLBACK_TEXT = 'Continue de onde parou'

const feed = useFeed()
const requests = useRequests()
const roster = useRoster()
const usage = useUsage()
const usageLimit = useUsageLimit()

const alternativeTarget = ref<CliPreset | undefined>(undefined)
const busy = ref(false)
const handled = ref(false)
const actionError = ref<string | null>(null)

const resetHint = computed(() => {
  if (!props.entry.resetsAt) {
    return null
  }
  return `O limite volta ${resetCountdownLabel(props.entry.resetsAt, usage.now)}.`
})

const migrateLabel = computed(() =>
  alternativeTarget.value ? `Continuar com o ${alternativeTarget.value.label}` : '',
)

onMounted(async () => {
  alternativeTarget.value = await usageLimit.alternativeProviderFor(props.entry.agent)
})

function findRetryMessage(): VokiaMessage | undefined {
  for (let i = feed.feed.length - 1; i >= 0; i -= 1) {
    const item = feed.feed[i]
    if (item.type !== 'message' || item.from !== 'user' || item.time > props.entry.time) {
      continue
    }
    if (props.entry.itemId && item.itemId === props.entry.itemId) {
      return item
    }
    if (item.to === props.entry.agent && (props.entry.channelId ?? undefined) === item.channelId) {
      return item
    }
  }
  return undefined
}

async function retryRequest(): Promise<void> {
  const source = findRetryMessage()
  const text = source?.text ?? RETRY_FALLBACK_TEXT
  await requests.send(text, props.entry.agent, {
    projectId: props.entry.projectId,
    channelId: props.entry.channelId ?? source?.channelId,
    itemId: props.entry.itemId ?? source?.itemId,
  })
}

async function onContinueWithAlternative(): Promise<void> {
  const target = alternativeTarget.value
  const profile = roster.profileOf(props.entry.agent)
  if (!target || !profile || busy.value) {
    return
  }
  busy.value = true
  actionError.value = null
  try {
    await usageLimit.migrateToProvider(profile, target, props.entry.resetsAt ?? 0)
    await retryRequest()
    handled.value = true
  } catch (error) {
    actionError.value = String(error)
  } finally {
    busy.value = false
  }
}

async function onRetry(): Promise<void> {
  if (busy.value) {
    return
  }
  busy.value = true
  actionError.value = null
  try {
    await retryRequest()
    handled.value = true
  } catch (error) {
    actionError.value = String(error)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div
    class="flex flex-col gap-2 rounded-md border border-line bg-surface-2 px-3.5 py-2.5 text-caption"
    data-testid="usage-limit-failed-notice"
  >
    <div class="flex items-start gap-2 text-warn-text">
      <UiIcon class="mt-0.5 shrink-0" :path="WARNING_ICON_PATH" :size="16" />
      <div class="flex min-w-0 flex-1 flex-col gap-0.5">
        <span class="font-semibold">
          Limite de uso atingido por {{ agentName(entry.agent) }}
        </span>
        <span class="wrap-break-word text-muted">
          {{ entry.message }}
          <template v-if="resetHint"> · {{ resetHint }}</template>
        </span>
      </div>
    </div>
    <div v-if="!handled" class="flex flex-wrap items-center gap-2 pl-6">
      <UiButton
        v-if="alternativeTarget"
        variant="primary"
        size="sm"
        :disabled="busy"
        data-testid="usage-limit-failed-migrate"
        @press="onContinueWithAlternative"
      >
        {{ migrateLabel }}
      </UiButton>
      <UiButton
        variant="quiet"
        size="sm"
        :disabled="busy"
        data-testid="usage-limit-failed-retry"
        @press="onRetry"
      >
        {{ RETRY_LABEL }}
      </UiButton>
    </div>
    <p v-if="actionError" role="alert" class="pl-6 text-small text-danger">{{ actionError }}</p>
  </div>
</template>
