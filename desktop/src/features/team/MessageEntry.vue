<script setup lang="ts">
import { computed } from 'vue'
import { agentName, DEFAULT_RECIPIENT } from './agents'
import { isBetweenAgents, type FeedEntry } from './timeline'
import { useRoster } from './useRoster'
import { useTemporaryOverrides } from '../providers/useTemporaryOverrides'
import TemporaryOverrideBadge from '../agents/TemporaryOverrideBadge.vue'
import type { AgentProfile } from './teamApi'
import type { Speaker, VokiaTaskResult } from './events'
import { permissionResultRow } from './permissionSummary'
import ActivityRow from './ActivityRow.vue'
import ActivityRun from './ActivityRun.vue'
import TurnPlanCard from './TurnPlanCard.vue'
import { statusOfTurnPlan, useTurnPlans } from './useTurnPlans'
import { protocolMessageRow, splitVokiaConfirmations, taskResultRow, taskStartedRow, type ActivityRowModel } from './activitySummary'
import MarkdownText from './MarkdownText.vue'
import { formatCostUsd, formatThinkingDuration, formatUsageModel, formatUsageTooltip, tokenSplit } from './messageUsageFormat'
import TokenUsageChart from './TokenUsageChart.vue'
import { extractAttachments } from '../attachments/attachmentBlock'
import AttachmentPreviewList from '../attachments/AttachmentPreviewList.vue'
import AgentPortrait from './AgentPortrait.vue'
import UsageLimitFailedNotice from './UsageLimitFailedNotice.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import UserAvatar from '../profile/UserAvatar.vue'
import { useUserProfile } from '../profile/useUserProfile'
import { useBacklog } from '../backlog/useBacklog'
import { runTitleOf } from '../backlog/useRunItem'
import UiButton from '../../ui/UiButton.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import UiIcon from '../../ui/UiIcon.vue'
import UiTooltip from '../../ui/UiTooltip.vue'
import { USER_BUBBLE_CLASSES } from '../../ui/handDrawn'
import { DEFAULT_JITTER, sketchArrow } from '../../ui/sketch/sketchPath'

const props = defineProps<{ entry: FeedEntry }>()

const roster = useRoster()
const overrides = useTemporaryOverrides()
const userProfile = useUserProfile()
const backlog = useBacklog()
const turnPlans = useTurnPlans()

const messagePlan = computed(() => {
  if (props.entry.type !== 'message' || props.entry.from === 'user') return undefined
  const plan = props.entry.activity?.plan
  return plan && plan.steps.length > 0 ? plan : undefined
})
const messagePlanStatus = computed(() => {
  const plan = messagePlan.value
  if (!plan || props.entry.type !== 'message') return 'completed' as const
  return statusOfTurnPlan(plan, {
    active: false,
    replaced: turnPlans.isMessagePlanReplaced(props.entry),
  })
})

const userContent = computed(() => extractAttachments(props.entry.type === 'message' ? props.entry.text : ''))
const agentUsage = computed(() =>
  props.entry.type === 'message' && props.entry.from !== 'user' ? props.entry.usage : undefined,
)
const usageModel = computed(() => formatUsageModel(agentUsage.value))
const usageCost = computed(() => formatCostUsd(agentUsage.value?.costUsd))
const usageSplit = computed(() => tokenSplit(agentUsage.value))
const thinkingDuration = computed(() =>
  props.entry.type === 'message' && props.entry.from !== 'user'
    ? formatThinkingDuration(props.entry.activity?.thinkingMs)
    : null,
)
const usageTooltip = computed(() =>
  props.entry.type === 'message' && props.entry.from !== 'user'
    ? formatUsageTooltip(props.entry.usage)
    : '',
)

const runCard = computed(() => {
  const { entry } = props
  if (entry.type !== 'message' || entry.from !== 'user') return null
  const { text } = userContent.value
  const title = runTitleOf(text)
  return title === undefined ? null : taskStartedRow(title, text)
})

const protocolRow = computed(() => {
  const { entry } = props
  if (entry.type !== 'message') return null
  return protocolMessageRow(entry.text)
})

const confirmations = computed(() =>
  props.entry.type === 'message' && props.entry.from !== 'user'
    ? splitVokiaConfirmations(props.entry.text)
    : { rows: [], rest: '' },
)
const agentContent = computed(() => extractAttachments(confirmations.value.rest))

const AVATAR_SIZE = PORTRAIT_SIZES.chat
const META_AVATAR_SIZE = PORTRAIT_SIZES.stack
const PROPOSAL_LABEL = 'Virar pedido'
const USAGE_REVEAL_CLASSES =
  'flex items-center gap-2 opacity-0 transition-opacity duration-150 group-hover:opacity-100 focus-within:opacity-100 motion-reduce:transition-none'

const TIME_FORMAT = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' })
const ERROR_ICON_PATH = 'M8 2a6 6 0 1 0 0 12A6 6 0 0 0 8 2z M8 5v3.5M8 10.8v.4'
const THINKING_ICON_PATH =
  'M9 4a3 3 0 0 0-3 3v1a3 3 0 0 0-2 5 3 3 0 0 0 2 5 3 3 0 0 0 3 3 3 3 0 0 0 3-3V7a3 3 0 0 0-3-3z M15 4a3 3 0 0 1 3 3v1a3 3 0 0 1 2 5 3 3 0 0 1-2 5 3 3 0 0 1-3 3 3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z'
const ICON_SIZE = 16
const EXCHANGE_ARROW_WIDTH = 40
const EXCHANGE_ARROW_HEIGHT = 14
const EXCHANGE_ARROW_MARGIN = 2
const EXCHANGE_ARROW_SEED = 9
const EXCHANGE_ARROW_STROKE = 1.4
const EXCHANGE_ARROW_PATH = sketchArrow(
  EXCHANGE_ARROW_MARGIN,
  EXCHANGE_ARROW_HEIGHT / 2,
  EXCHANGE_ARROW_WIDTH - EXCHANGE_ARROW_MARGIN,
  EXCHANGE_ARROW_HEIGHT / 2,
  EXCHANGE_ARROW_SEED,
  DEFAULT_JITTER,
)
const EXCHANGE_TEXT_CLASSES = 'max-w-full sm:max-w-xs text-caption text-muted'
// Caps line length on wide monitors now that the feed spans the whole panel.
const USER_BUBBLE_MAX_WIDTH_CLASSES = 'max-w-[85%] md:max-w-xl lg:max-w-2xl'
const USER_TEXT_CLASSES = `w-fit ${USER_BUBBLE_MAX_WIDTH_CLASSES} wrap-break-word px-3 py-1.5`
const ERROR_SCRIBBLE_CLASSES = 'pointer-events-none absolute inset-x-0 -bottom-1.5 h-2 w-full text-danger/70'
const AUTHOR_CLASSES = 'text-control font-semibold text-ink'
// Só os blocos de texto ficam na largura de leitura; código, diff e tabela usam a coluna inteira.
const READING_WIDTH_CLASSES =
  '[&>blockquote]:max-w-[80ch] [&>h3]:max-w-[80ch] [&>h4]:max-w-[80ch] [&>ol]:max-w-[80ch] [&>p]:max-w-[80ch] [&>ul]:max-w-[80ch]'

function taskResultLine(entry: VokiaTaskResult): ActivityRowModel {
  return taskResultRow(`task-result-${entry.time}`, entry.objective, entry.concluded, taskResultDetail(entry))
}

function taskResultDetail(entry: VokiaTaskResult): string {
  const turns = `${entry.turns} ${entry.turns === 1 ? 'turno' : 'turnos'}`
  const agents = entry.agents.map(agentName).join(', ')
  return [turns, agents, TIME_FORMAT.format(entry.time)].filter(Boolean).join(' · ')
}

function speakerName(speaker: Speaker): string {
  return speaker === 'user' ? userProfile.displayName : agentName(speaker)
}

function profile(speaker: Speaker): AgentProfile | undefined {
  return roster.profileOf(speaker)
}
</script>

<template>
  <div v-if="entry.type === 'message' && entry.from === 'user'" class="flex min-w-0 flex-row-reverse items-start gap-2">
    <div class="mt-0.5 shrink-0">
      <UserAvatar :size="AVATAR_SIZE" :title="userProfile.displayName" />
    </div>
    <div class="flex min-w-0 flex-1 flex-col items-end gap-0.5">
      <div class="flex flex-wrap items-center justify-end gap-2">
        <span class="text-control font-semibold text-ink">{{ speakerName(entry.from) }}</span>
        <span v-if="entry.to !== DEFAULT_RECIPIENT" class="text-small text-muted">
          → {{ speakerName(entry.to) }}
        </span>
        <time class="text-xs text-muted">{{ TIME_FORMAT.format(entry.time) }}</time>
      </div>
      <ul v-if="runCard" class="flex min-w-0 flex-col gap-1 text-caption text-muted" data-testid="task-started">
        <ActivityRow :row="runCard" />
      </ul>
      <ul v-else-if="protocolRow" class="flex min-w-0 flex-row items-center gap-1.5 text-caption text-muted">
        <ActivityRow
          :row="protocolRow.id === 'protocol-plan-ready' && entry.type === 'message' && entry.itemId && protocolRow.chip
            ? { ...protocolRow, chip: { ...protocolRow.chip, label: `${entry.itemId} · ${protocolRow.chip.label}` } }
            : protocolRow"
          class="min-w-0"
        />
        <button
          v-if="protocolRow.id === 'protocol-plan-ready' && entry.type === 'message' && entry.itemId"
          type="button"
          class="inline-flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-md text-muted hover:bg-surface-2 hover:text-ink"
          title="Ver artefato completo"
          aria-label="Ver artefato completo"
          @click="backlog.open(entry.itemId)"
        >
          <svg viewBox="0 0 20 20" class="size-3.5 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M5.5 3.5h6.2l3.8 3.8v9.2a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1v-12a1 1 0 0 1 1-1Z" />
            <path d="M11.5 3.5v3.8h4" />
            <path d="m8 13.5 4.2-4.2 1.3 1.3-4.2 4.2-1.6.3.3-1.6Z" />
          </svg>
        </button>
      </ul>
      <div
        v-else-if="userContent.text !== '' || userContent.attachments.length === 0"
        :class="[USER_TEXT_CLASSES, USER_BUBBLE_CLASSES]"
      >
        <MarkdownText :text="userContent.text" />
      </div>
      <AttachmentPreviewList
        v-if="userContent.attachments.length > 0"
        :attachments="userContent.attachments"
        align="end"
        data-testid="message-attachments"
      />
    </div>
  </div>
  <UiTooltip
    v-else-if="entry.type === 'message' && isBetweenAgents(entry)"
    class="flex min-w-0 flex-col items-center gap-1 text-center"
    :text="[TIME_FORMAT.format(entry.time), thinkingDuration ? `Pensou: ${thinkingDuration}` : '', entry.text, usageTooltip].filter(Boolean).join('\n')"
    data-testid="agent-exchange"
  >
    <div class="flex min-w-0 items-center gap-x-2">
      <span class="flex shrink-0 items-center gap-1.5 text-caption font-semibold text-ink">
        <AgentPortrait :who="entry.from" :size="META_AVATAR_SIZE" />
        {{ speakerName(entry.from) }}
        <TemporaryOverrideBadge
          v-if="entry.from !== 'user' && agentUsage?.isFallback"
          :override="overrides.byAgent(entry.from) ?? {
            cli: agentUsage.provider ?? '',
            model: agentUsage.model ?? '',
            originalCli: '',
            originalModel: '',
            reason: 'usage-limit',
          }"
          :restoring="Boolean(overrides.restoring[entry.from])"
          @restore="overrides.restore(entry.from)"
        />
      </span>
      <svg
        class="shrink-0 text-muted"
        :width="EXCHANGE_ARROW_WIDTH"
        :height="EXCHANGE_ARROW_HEIGHT"
        :viewBox="`0 0 ${EXCHANGE_ARROW_WIDTH} ${EXCHANGE_ARROW_HEIGHT}`"
        fill="none"
        stroke="currentColor"
        aria-hidden="true"
      >
        <path :d="EXCHANGE_ARROW_PATH" :stroke-width="EXCHANGE_ARROW_STROKE" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
      <span class="flex shrink-0 items-center gap-1.5 text-caption font-semibold text-ink">
        <AgentPortrait :who="entry.to" :size="META_AVATAR_SIZE" />
        {{ speakerName(entry.to) }}
      </span>
    </div>
    <ul v-if="protocolRow" :class="[EXCHANGE_TEXT_CLASSES, 'flex min-w-0 flex-col']">
      <ActivityRow :row="protocolRow" />
    </ul>
    <span v-else :class="[EXCHANGE_TEXT_CLASSES, 'truncate']">{{ entry.text }}</span>
    <div v-if="entry.activity" class="flex w-full flex-col gap-1">
      <span class="flex items-center gap-1.5 text-caption font-semibold text-ink">
        <AgentPortrait :who="entry.from" :size="META_AVATAR_SIZE" />
        {{ speakerName(entry.from) }}
      </span>
      <TurnPlanCard v-if="messagePlan" :plan="messagePlan" :status="messagePlanStatus" />
      <ActivityRun :activity="entry.activity" />
    </div>
  </UiTooltip>
  <div v-else-if="entry.type === 'message'" class="group flex min-w-0 items-start gap-2">
    <span class="mt-0.5 flex shrink-0" aria-hidden="true">
      <AgentPortrait :who="entry.from" :size="AVATAR_SIZE" />
    </span>
    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
      <div class="flex flex-wrap items-center gap-2">
        <span :class="AUTHOR_CLASSES">{{ speakerName(entry.from) }}</span>
        <span
          v-if="profile(entry.from)?.role"
          class="text-small text-muted"
        >
          {{ profile(entry.from)?.role }}
        </span>
        <TemporaryOverrideBadge
          v-if="entry.from !== 'user' && agentUsage?.isFallback"
          :override="overrides.byAgent(entry.from) ?? {
            cli: agentUsage.provider ?? '',
            model: agentUsage.model ?? '',
            originalCli: '',
            originalModel: '',
            reason: 'usage-limit',
          }"
          :restoring="Boolean(overrides.restoring[entry.from])"
          @restore="overrides.restore(entry.from)"
        />
        <time class="text-xs text-muted">{{ TIME_FORMAT.format(entry.time) }}</time>
        <span v-if="usageModel" data-testid="message-usage" class="text-xs text-muted">· {{ usageModel }}</span>
        <span
          v-if="thinkingDuration"
          data-testid="message-thinking-duration"
          class="inline-flex items-center gap-1 text-xs text-muted tabular-nums"
          :title="`Pensou por ${thinkingDuration}`"
        >
          <span aria-hidden="true">·</span>
          <svg
            viewBox="0 0 24 24"
            class="size-3 shrink-0"
            fill="none"
            stroke="currentColor"
            stroke-width="1.75"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path :d="THINKING_ICON_PATH" />
          </svg>
          <span>{{ thinkingDuration }}</span>
        </span>
        <span
          v-if="usageCost || usageSplit"
          data-testid="message-usage-detail"
          :class="USAGE_REVEAL_CLASSES"
        >
          <span v-if="usageCost" class="tabular-nums text-xs text-muted">· {{ usageCost }}</span>
          <TokenUsageChart :usage="entry.usage" />
        </span>
      </div>
      <TurnPlanCard v-if="messagePlan" :plan="messagePlan" :status="messagePlanStatus" class="my-1" />
      <ActivityRun v-if="entry.activity" :activity="entry.activity" />
      <div
        v-if="entry.from !== 'user' && (entry.isError || entry.text === 'Limite esgotado')"
        class="flex items-center gap-2 text-danger"
        data-testid="agent-error-message"
      >
        <UiIcon class="shrink-0" :path="ERROR_ICON_PATH" :size="ICON_SIZE" />
        <span class="relative wrap-break-word">
          {{ entry.text }}
          <UiHandLine variant="scribble" :class="ERROR_SCRIBBLE_CLASSES" />
        </span>
      </div>
      <ul v-else-if="protocolRow" class="flex min-w-0 flex-row items-center gap-1.5 text-caption text-muted">
        <ActivityRow
          :row="protocolRow.id === 'protocol-plan-ready' && entry.type === 'message' && entry.itemId && protocolRow.chip
            ? { ...protocolRow, chip: { ...protocolRow.chip, label: `${entry.itemId} · ${protocolRow.chip.label}` } }
            : protocolRow"
          class="min-w-0"
        />
        <button
          v-if="protocolRow.id === 'protocol-plan-ready' && entry.type === 'message' && entry.itemId"
          type="button"
          class="inline-flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-md text-muted hover:bg-surface-2 hover:text-ink"
          title="Ver artefato completo"
          aria-label="Ver artefato completo"
          @click="backlog.open(entry.itemId)"
        >
          <svg viewBox="0 0 20 20" class="size-3.5 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M5.5 3.5h6.2l3.8 3.8v9.2a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1v-12a1 1 0 0 1 1-1Z" />
            <path d="M11.5 3.5v3.8h4" />
            <path d="m8 13.5 4.2-4.2 1.3 1.3-4.2 4.2-1.6.3.3-1.6Z" />
          </svg>
        </button>
      </ul>
      <template v-else>
        <ul v-if="confirmations.rows.length > 0" class="flex min-w-0 flex-col gap-1 text-caption text-muted" data-testid="vokia-confirmations">
          <ActivityRow v-for="row in confirmations.rows" :key="row.id" :row="row" />
        </ul>
        <MarkdownText v-if="agentContent.text !== ''" :class="READING_WIDTH_CLASSES" :text="agentContent.text" />
        <AttachmentPreviewList
          v-if="agentContent.attachments.length > 0"
          :attachments="agentContent.attachments"
          class="mt-1"
        />
      </template>
      <UiButton
        v-if="entry.requestProposal"
        class="self-start"
        variant="quiet"
        size="sm"
        :disabled="entry.projectId === null || backlog.proposalCreated(entry)"
        @press="backlog.createFromProposal(entry)"
      >
        {{ PROPOSAL_LABEL }}
      </UiButton>
    </div>
  </div>
  <UsageLimitFailedNotice v-else-if="entry.type === 'usage-limit-failed'" :entry="entry" />
  <p v-else-if="entry.type === 'notice'" class="text-caption text-muted">{{ entry.text }}</p>
  <ul
    v-else-if="entry.type === 'task-result'"
    data-testid="task-result"
    class="flex min-w-0 flex-col gap-1 text-caption text-muted"
  >
    <ActivityRow :row="taskResultLine(entry)" />
  </ul>
  <ul
    v-else-if="entry.type === 'permission-result'"
    data-testid="permission-result"
    class="flex min-w-0 flex-col gap-1 text-caption text-muted"
  >
    <ActivityRow :row="permissionResultRow(entry)" />
  </ul>
</template>
