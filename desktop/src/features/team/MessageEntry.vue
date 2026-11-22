<script setup lang="ts">
import { computed } from 'vue'
import { agentName, DEFAULT_RECIPIENT } from './agents'
import { isBetweenAgents, type FeedEntry } from './timeline'
import { useRoster } from './useRoster'
import { useTemporaryOverrides } from '../providers/useTemporaryOverrides'
import TemporaryOverrideBadge from '../agents/TemporaryOverrideBadge.vue'
import type { AgentProfile } from './teamApi'
import type { PermissionResultOutcome, Speaker, VokiaTaskResult } from './events'
import ActivityRow from './ActivityRow.vue'
import ActivityRun from './ActivityRun.vue'
import { protocolMessageRow, splitVokiaConfirmations, taskStartedRow } from './activitySummary'
import MarkdownText from './MarkdownText.vue'
import { formatCostUsd, formatUsageModel, formatUsageTooltip, tokenSplit } from './messageUsageFormat'
import TokenUsageChart from './TokenUsageChart.vue'
import { splitAttachments } from '../attachments/attachmentBlock'
import AgentPortrait from './AgentPortrait.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import UserAvatar from '../profile/UserAvatar.vue'
import { useUserProfile } from '../profile/useUserProfile'
import { useBacklog } from '../backlog/useBacklog'
import { runTitleOf } from '../backlog/useRunItem'
import UiButton from '../../ui/UiButton.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import UiIcon from '../../ui/UiIcon.vue'
import { USER_BUBBLE_CLASSES } from '../../ui/handDrawn'

const props = defineProps<{ entry: FeedEntry }>()

const roster = useRoster()
const overrides = useTemporaryOverrides()
const userProfile = useUserProfile()
const backlog = useBacklog()

const userContent = computed(() => splitAttachments(props.entry.type === 'message' ? props.entry.text : ''))
const agentUsage = computed(() =>
  props.entry.type === 'message' && props.entry.from !== 'user' ? props.entry.usage : undefined,
)
const usageModel = computed(() => formatUsageModel(agentUsage.value))
const usageCost = computed(() => formatCostUsd(agentUsage.value?.costUsd))
const usageSplit = computed(() => tokenSplit(agentUsage.value))
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

const AVATAR_SIZE = PORTRAIT_SIZES.chat
const META_AVATAR_SIZE = PORTRAIT_SIZES.stack
const PROPOSAL_LABEL = 'Virar pedido'
const USAGE_REVEAL_CLASSES =
  'flex items-center gap-2 opacity-0 transition-opacity duration-150 group-hover:opacity-100 focus-within:opacity-100 motion-reduce:transition-none'

const OUTCOME_LABELS: Record<PermissionResultOutcome, string> = {
  allowed: '✓ Aprovado:',
  denied: '✕ Negado:',
  expired: '⏱ Expirou sem resposta:',
  'project-rule': '✓ Regra do projeto:',
}

const TIME_FORMAT = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' })
const CHECK_ICON_PATH = 'M3.5 8.5l3 3 6-6.5'
const WARNING_ICON_PATH = 'M8 2.5L14 13.5H2Z M8 6.5v3M8 11.2v.3'
const ERROR_ICON_PATH = 'M8 2a6 6 0 1 0 0 12A6 6 0 0 0 8 2z M8 5v3.5M8 10.8v.4'
const ICON_SIZE = 16
const USER_TEXT_CLASSES = 'w-fit max-w-full wrap-break-word px-3 py-1.5'
const ERROR_SCRIBBLE_CLASSES = 'pointer-events-none absolute inset-x-0 -bottom-1.5 h-2 w-full text-danger/70'
const AUTHOR_CLASSES = 'text-control font-semibold text-ink'

function taskResultTitle(entry: VokiaTaskResult): string {
  return entry.concluded ? `Tarefa concluída: ${entry.objective}` : `Tarefa interrompida: ${entry.objective}`
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
      <ul v-else-if="protocolRow" class="flex min-w-0 flex-col gap-1 text-caption text-muted">
        <ActivityRow :row="protocolRow" />
      </ul>
      <div
        v-else-if="userContent.text !== '' || userContent.attachments.length === 0"
        :class="[USER_TEXT_CLASSES, USER_BUBBLE_CLASSES]"
      >
        <MarkdownText :text="userContent.text" />
      </div>
      <ul
        v-if="userContent.attachments.length > 0"
        class="flex flex-wrap justify-end gap-1.5"
        data-testid="message-attachments"
      >
        <li
          v-for="attachment in userContent.attachments"
          :key="attachment.path"
          :title="attachment.path"
          :data-attachment-name="attachment.name"
          class="flex min-w-0 items-center gap-1.5 rounded-md border border-line bg-surface px-2.5 py-1.5 text-caption"
        >
          <span class="truncate font-semibold text-ink">{{ attachment.name }}</span>
          <span class="shrink-0 text-muted">{{ attachment.label }} · {{ attachment.size }}</span>
        </li>
      </ul>
    </div>
  </div>
  <div
    v-else-if="entry.type === 'message' && isBetweenAgents(entry)"
    class="flex min-w-0 flex-col gap-1"
    :title="[TIME_FORMAT.format(entry.time), entry.text, usageTooltip].filter(Boolean).join(' — ')"
    data-testid="agent-exchange"
  >
    <div class="flex min-w-0 items-center gap-x-2">
      <span class="flex shrink-0 items-center gap-1.5 text-caption font-semibold text-ink">
        <AgentPortrait :who="entry.from" :size="META_AVATAR_SIZE" />
        {{ speakerName(entry.from) }}
        <TemporaryOverrideBadge
          v-if="entry.from !== 'user' && overrides.byAgent(entry.from)"
          :override="overrides.byAgent(entry.from)!"
          :restoring="Boolean(overrides.restoring[entry.from])"
          @restore="overrides.restore(entry.from)"
        />
      </span>
      <span aria-hidden="true" class="shrink-0 text-caption text-muted">→</span>
      <span class="flex shrink-0 items-center gap-1.5 text-caption font-semibold text-ink">
        <AgentPortrait :who="entry.to" :size="META_AVATAR_SIZE" />
        {{ speakerName(entry.to) }}
      </span>
      <ul v-if="protocolRow" class="flex min-w-0 flex-1 flex-col text-caption text-muted">
        <ActivityRow :row="protocolRow" />
      </ul>
      <span v-else class="min-w-0 flex-1 truncate text-caption text-muted">{{ entry.text }}</span>
    </div>
    <ActivityRun v-if="entry.activity" :activity="entry.activity" />
  </div>
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
          v-if="entry.from !== 'user' && overrides.byAgent(entry.from)"
          :override="overrides.byAgent(entry.from)!"
          :restoring="Boolean(overrides.restoring[entry.from])"
          @restore="overrides.restore(entry.from)"
        />
        <time class="text-xs text-muted">{{ TIME_FORMAT.format(entry.time) }}</time>
        <span v-if="usageModel" data-testid="message-usage" class="text-xs text-muted">· {{ usageModel }}</span>
        <span
          v-if="usageCost || usageSplit"
          data-testid="message-usage-detail"
          :class="USAGE_REVEAL_CLASSES"
        >
          <span v-if="usageCost" class="tabular-nums text-xs text-muted">· {{ usageCost }}</span>
          <TokenUsageChart :usage="entry.usage" />
        </span>
      </div>
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
      <ul v-else-if="protocolRow" class="flex min-w-0 flex-col gap-1 text-caption text-muted">
        <ActivityRow :row="protocolRow" />
      </ul>
      <template v-else>
        <ul v-if="confirmations.rows.length > 0" class="flex min-w-0 flex-col gap-1 text-caption text-muted" data-testid="vokia-confirmations">
          <ActivityRow v-for="row in confirmations.rows" :key="row.id" :row="row" />
        </ul>
        <MarkdownText v-if="confirmations.rest !== ''" :text="confirmations.rest" />
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
  <p v-else-if="entry.type === 'notice'" class="text-caption text-muted">{{ entry.text }}</p>
  <div
    v-else-if="entry.type === 'task-result'"
    data-testid="task-result"
    class="flex items-start gap-2.5 rounded-md border border-line bg-surface px-3 py-2"
  >
    <UiIcon
      class="mt-0.5 shrink-0"
      :class="entry.concluded ? 'text-ok' : 'text-warn-text'"
      :path="entry.concluded ? CHECK_ICON_PATH : WARNING_ICON_PATH"
      :size="ICON_SIZE"
    />
    <div class="flex min-w-0 flex-col gap-0.5">
      <p class="wrap-break-word text-control font-semibold text-ink">{{ taskResultTitle(entry) }}</p>
      <p class="wrap-break-word text-caption text-muted">{{ taskResultDetail(entry) }}</p>
    </div>
  </div>
  <p
    v-else-if="entry.type === 'permission-result'"
    data-testid="permission-result"
    class="wrap-break-word whitespace-pre-wrap text-caption text-muted"
  >
    {{ OUTCOME_LABELS[entry.outcome] }} {{ entry.detail }}
  </p>
</template>
