<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { storeToRefs } from 'pinia'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import UiSkeleton from '../../ui/UiSkeleton.vue'
import UiSelect from '../../ui/UiSelect.vue'
import UiTable from '../../ui/UiTable.vue'
import { useProjects } from '../projects/useProjects'
import { useProviders } from '../providers/useProviders'
import AgentPortrait from '../team/AgentPortrait.vue'
import { useRoster } from '../team/useRoster'
import { formatTokenCount } from '../team/messageUsageFormat'
import {
  PERIOD_OPTIONS,
  UNPRICED_COST_LABEL,
  formatDurationMs,
  formatEstimatedUsd,
  useUsageAnalytics,
  type PeriodPreset,
} from './useUsageAnalytics'

type ChartMetric = 'tokens' | 'cost'

const PAGE_TITLE = 'Estatísticas'
const PAGE_SUBTITLE =
  'Consumo histórico desacoplado das conversas por provedor, modelo, projeto e agente. Limpar o histórico de chat preserva todas as métricas.'
const COST_BADGE_LABEL = 'Estimativa equivalente à API'
const COST_DISCLAIMER_DEFAULT =
  'Valores em USD são estimativas equivalentes à API (não representam cobrança adicional sobre a sua assinatura).'
const EMPTY_TITLE = 'Sem registros no período'
const EMPTY_TEXT =
  'Nenhum turno concluído foi encontrado para os filtros selecionados. Ajuste o período ou remova filtros para ver o histórico.'
const ALL_VALUE = '__all__'

const PAGE_CLASSES = `min-w-0 flex-1 overflow-y-auto ${PAGE_INSET_CLASSES}`
const CARD_CLASSES = 'flex min-w-0 flex-col gap-1.5 rounded-md border border-line bg-surface p-4'
const SECTION_CARD_CLASSES = 'flex min-w-0 flex-col gap-3 rounded-md border border-line bg-surface p-4'
const DATE_INPUT_CLASSES = `h-9 rounded-md border border-line bg-surface px-2.5 text-control text-ink transition-colors hover:border-faint ${INK_FOCUS_CLASSES}`

const CHART_METRIC_OPTIONS: readonly { label: string; value: ChartMetric }[] = [
  { label: 'Tokens', value: 'tokens' },
  { label: 'Custo equiv. API', value: 'cost' },
]

const PROVIDER_BAR_CLASSES: Record<string, string> = {
  claude: 'bg-feature-orange',
  codex: 'bg-feature-green',
  opencode: 'bg-feature-blue',
  antigravity: 'bg-feature-purple',
  gemini: 'bg-feature-purple',
}

const FALLBACK_BAR_PALETTE = [
  'bg-project-teal',
  'bg-project-rose',
  'bg-project-yellow',
  'bg-accent',
  'bg-project-pink',
] as const

const PIE_COLORS = [
  '#3b82f6',
  '#10b981',
  '#f59e0b',
  '#8b5cf6',
  '#ec4899',
  '#06b6d4',
  '#ef4444',
  '#84cc16',
] as const

const PIE_RADIUS = 72
const PIE_CIRCUMFERENCE = 2 * Math.PI * PIE_RADIUS

const INT_FORMAT = new Intl.NumberFormat('pt-BR')

const AGENT_COLUMNS = [
  { key: 'agent', title: 'Agente' },
  { key: 'turns', title: 'Turnos', align: 'right' as const },
  { key: 'input', title: 'Entrada', align: 'right' as const },
  { key: 'cache', title: 'Cache', align: 'right' as const },
  { key: 'output', title: 'Saída (+ pens.)', align: 'right' as const },
  { key: 'cost', title: 'Equiv. API', align: 'right' as const },
]

const MODEL_COLUMNS = [
  { key: 'model', title: 'Modelo' },
  { key: 'turns', title: 'Turnos', align: 'right' as const },
  { key: 'tokens', title: 'Tokens', align: 'right' as const },
  { key: 'thought', title: 'Pensamento', align: 'right' as const },
  { key: 'cost', title: 'Equiv. API', align: 'right' as const },
]

const DAILY_COLUMNS = [
  { key: 'date', title: 'Data' },
  { key: 'turns', title: 'Turnos', align: 'right' as const },
  { key: 'input', title: 'Entrada', align: 'right' as const },
  { key: 'cacheRead', title: 'Cache leitura', align: 'right' as const },
  { key: 'cacheWrite', title: 'Cache escrita', align: 'right' as const },
  { key: 'output', title: 'Saída', align: 'right' as const },
  { key: 'thought', title: 'Pensamento', align: 'right' as const },
  { key: 'total', title: 'Total tokens', align: 'right' as const },
  { key: 'duration', title: 'Tempo', align: 'right' as const },
  { key: 'cost', title: 'Estimativa equiv. API', align: 'right' as const },
]

const TASK_COLUMNS = [
  { key: 'task', title: 'Task' },
  { key: 'turns', title: 'Turnos', align: 'right' as const },
  { key: 'duration', title: 'Tempo', align: 'right' as const },
  { key: 'tokens', title: 'Tokens', align: 'right' as const },
  { key: 'rate', title: 'Razão (tok/s)', align: 'right' as const },
  { key: 'cost', title: 'Equiv. API', align: 'right' as const },
]

const analytics = useUsageAnalytics()
const projectsStore = useProjects()
const roster = useRoster()
const providersStore = useProviders()
const { data, loading, error, isEmpty, period, customFrom, customTo } = storeToRefs(analytics)

const chartMetric = ref<ChartMetric>('tokens')
const draftFrom = ref(customFrom.value)
const draftTo = ref(customTo.value)

const periodModel = computed<PeriodPreset>({
  get: () => period.value,
  set: (value) => {
    if (value === 'custom') {
      draftFrom.value = customFrom.value
      draftTo.value = customTo.value
    }
    void analytics.setPeriod(value)
  },
})

const projectOptions = computed(() => [
  { label: 'Todos os projetos', value: ALL_VALUE },
  ...projectsStore.projects.map((p) => ({ label: p.name, value: String(p.id) })),
])

const projectModel = computed<string>({
  get: () => (analytics.projectId === null ? ALL_VALUE : String(analytics.projectId)),
  set: (value) => {
    void analytics.setProjectFilter(value === ALL_VALUE ? null : Number(value))
  },
})

const agentOptions = computed(() => {
  const seen = new Map<string, string>()
  for (const agent of roster.agents) {
    seen.set(agent.id, agent.name)
  }
  for (const row of data.value?.byAgent ?? []) {
    if (!seen.has(row.agentId)) {
      seen.set(row.agentId, roster.nameOf(row.agentId))
    }
  }
  return [
    { label: 'Todos os agentes', value: ALL_VALUE },
    ...Array.from(seen.entries()).map(([value, label]) => ({ label, value })),
  ]
})

const agentModel = computed<string>({
  get: () => analytics.agentId ?? ALL_VALUE,
  set: (value) => {
    void analytics.setAgentFilter(value === ALL_VALUE ? null : value)
  },
})

const providerOptions = computed(() => {
  const seen = new Map<string, string>()
  for (const provider of providersStore.providers) {
    seen.set(provider.id, provider.label)
  }
  for (const row of data.value?.byProvider ?? []) {
    if (!seen.has(row.providerId)) {
      seen.set(row.providerId, row.providerId)
    }
  }
  return [
    { label: 'Todos os provedores', value: ALL_VALUE },
    ...Array.from(seen.entries()).map(([value, label]) => ({ label, value })),
  ]
})

const providerModel = computed<string>({
  get: () => analytics.providerId ?? ALL_VALUE,
  set: (value) => {
    void analytics.setProviderFilter(value === ALL_VALUE ? null : value)
  },
})

function providerLabel(id: string): string {
  return providersStore.providers.find((p) => p.id === id)?.label ?? id
}

function providerBarClass(id: string, index: number): string {
  const known = PROVIDER_BAR_CLASSES[id.toLowerCase()]
  if (known) return known
  return FALLBACK_BAR_PALETTE[index % FALLBACK_BAR_PALETTE.length]
}

const providerOrder = computed<string[]>(() => {
  const fromBreakdown = data.value?.byProvider.map((p) => p.providerId) ?? []
  if (fromBreakdown.length > 0) return fromBreakdown
  const set = new Set<string>()
  for (const entry of data.value?.dailyByProvider ?? []) {
    set.add(entry.providerId)
  }
  return Array.from(set)
})

interface StackedBarSegment {
  providerId: string
  label: string
  value: number
  heightPercent: number
  colorClass: string
  tooltip: string
}

interface StackedBarDay {
  date: string
  shortLabel: string
  totalValue: number
  segments: StackedBarSegment[]
  summaryTooltip: string
}

function shortDateLabel(isoDate: string): string {
  const [, month, day] = isoDate.split('-')
  return month && day ? `${day}/${month}` : isoDate
}

const stackedDays = computed<StackedBarDay[]>(() => {
  const response = data.value
  if (!response || response.daily.length === 0) return []
  const byDayProvider = new Map<string, Map<string, { tokens: number; cost: number | null }>>()
  for (const entry of response.dailyByProvider) {
    let inner = byDayProvider.get(entry.date)
    if (!inner) {
      inner = new Map()
      byDayProvider.set(entry.date, inner)
    }
    inner.set(entry.providerId, {
      tokens: entry.totalTokens,
      cost: entry.costUsdEstimated,
    })
  }
  const metric = chartMetric.value
  const rawDays = response.daily.map((day) => {
    const providerMap = byDayProvider.get(day.date)
    const segmentsRaw = providerOrder.value
      .map((providerId, index) => {
        const entry = providerMap?.get(providerId)
        const value = metric === 'tokens' ? (entry?.tokens ?? 0) : (entry?.cost ?? 0)
        return {
          providerId,
          label: providerLabel(providerId),
          value,
          colorClass: providerBarClass(providerId, index),
        }
      })
      .filter((seg) => seg.value > 0)
    const totalValue = segmentsRaw.reduce((acc, seg) => acc + seg.value, 0)
    return { date: day.date, totalValue, segmentsRaw }
  })
  const maxTotal = rawDays.reduce((max, item) => Math.max(max, item.totalValue), 0)
  return rawDays.map((item) => {
    const segments: StackedBarSegment[] = item.segmentsRaw.map((seg) => ({
      providerId: seg.providerId,
      label: seg.label,
      value: seg.value,
      heightPercent: maxTotal > 0 ? Math.max((seg.value / maxTotal) * 100, 3) : 0,
      colorClass: seg.colorClass,
      tooltip:
        metric === 'tokens'
          ? `${item.date} · ${seg.label}: ${INT_FORMAT.format(seg.value)} tokens`
          : `${item.date} · ${seg.label}: ${formatEstimatedUsd(seg.value)}`,
    }))
    const totalFormatted =
      metric === 'tokens'
        ? `${INT_FORMAT.format(item.totalValue)} tokens`
        : item.totalValue > 0
          ? formatEstimatedUsd(item.totalValue)
          : UNPRICED_COST_LABEL
    return {
      date: item.date,
      shortLabel: shortDateLabel(item.date),
      totalValue: item.totalValue,
      segments,
      summaryTooltip: `${item.date}: ${totalFormatted}`,
    }
  })
})

const agentRows = computed(() =>
  (data.value?.byAgent ?? []).map((row) => ({
    agentId: row.agentId,
    agent: roster.nameOf(row.agentId),
    turns: INT_FORMAT.format(row.turns),
    input: formatTokenCount(row.inputTokens),
    cache: formatTokenCount(row.cachedInputTokens + row.cacheWriteTokens),
    output:
      row.thoughtTokens > 0
        ? `${formatTokenCount(row.outputTokens)} (+${formatTokenCount(row.thoughtTokens)})`
        : formatTokenCount(row.outputTokens),
    cost: formatEstimatedUsd(row.costUsdEstimated),
    unpriced: row.unpricedTurns > 0 && row.costUsdEstimated === null,
  })),
)

interface AgentPieSlice {
  agentId: string
  name: string
  tokens: number
  percent: number
  color: string
  dashArray: string
  dashOffset: number
}

const agentPieSlices = computed<AgentPieSlice[]>(() => {
  const rows = (data.value?.byAgent ?? []).filter((r) => r.totalTokens > 0)
  const total = rows.reduce((sum, r) => sum + r.totalTokens, 0)
  if (total <= 0) return []
  let offset = 0
  return rows.map((row, index) => {
    const ratio = row.totalTokens / total
    const length = ratio * PIE_CIRCUMFERENCE
    const slice: AgentPieSlice = {
      agentId: row.agentId,
      name: roster.nameOf(row.agentId),
      tokens: row.totalTokens,
      percent: ratio * 100,
      color: PIE_COLORS[index % PIE_COLORS.length],
      dashArray: `${length} ${PIE_CIRCUMFERENCE}`,
      dashOffset: -offset,
    }
    offset += length
    return slice
  })
})

const hoveredPieAgent = ref<string | null>(null)

const modelRows = computed(() =>
  (data.value?.byModel ?? []).map((row) => ({
    model: row.model,
    provider: providerLabel(row.providerId),
    turns: INT_FORMAT.format(row.turns),
    tokens: formatTokenCount(row.totalTokens),
    thought: row.thoughtTokens > 0 ? formatTokenCount(row.thoughtTokens) : '—',
    cost: formatEstimatedUsd(row.costUsdEstimated),
    unpriced: row.unpricedTurns > 0 && row.costUsdEstimated === null,
    priceBasis: row.priceBasis,
  })),
)

const dailyRows = computed(() =>
  [...(data.value?.daily ?? [])].reverse().map((row) => ({
    date: row.date,
    turns: INT_FORMAT.format(row.turns),
    input: INT_FORMAT.format(row.inputTokens),
    cacheRead: INT_FORMAT.format(row.cachedInputTokens),
    cacheWrite: INT_FORMAT.format(row.cacheWriteTokens),
    output: INT_FORMAT.format(row.outputTokens),
    thought: INT_FORMAT.format(row.thoughtTokens),
    total: INT_FORMAT.format(row.totalTokens),
    duration: formatDurationMs(row.durationMs),
    cost: formatEstimatedUsd(row.costUsdEstimated),
    unpriced: row.unpricedTurns > 0 && row.costUsdEstimated === null,
  })),
)

const hoveredLineIndex = ref<number | null>(null)

const LINE_CHART_W = 680
const LINE_CHART_H = 190
const LINE_PAD_L = 48
const LINE_PAD_R = 16
const LINE_PAD_T = 16
const LINE_PAD_B = 28

const tokenLineChart = computed(() => {
  const days = data.value?.daily ?? []
  if (days.length === 0) return null
  const plotW = LINE_CHART_W - LINE_PAD_L - LINE_PAD_R
  const plotH = LINE_CHART_H - LINE_PAD_T - LINE_PAD_B
  const maxTokens = Math.max(1, ...days.map((d) => d.totalTokens))
  const count = days.length
  const xFor = (i: number) =>
    count === 1 ? LINE_PAD_L + plotW / 2 : LINE_PAD_L + (i / (count - 1)) * plotW
  const yFor = (val: number) => LINE_PAD_T + plotH - (val / maxTokens) * plotH

  const points = days.map((day, i) => {
    const outputTotal = day.outputTokens + day.thoughtTokens
    return {
      index: i,
      date: day.date,
      shortLabel: shortDateLabel(day.date),
      x: xFor(i),
      yTotal: yFor(day.totalTokens),
      yOutput: yFor(outputTotal),
      totalTokens: day.totalTokens,
      inputTokens: day.inputTokens,
      outputTotal,
      turns: day.turns,
      duration: formatDurationMs(day.durationMs),
      cost: formatEstimatedUsd(day.costUsdEstimated),
    }
  })

  const totalLine = points.map((p) => `${p.x.toFixed(1)},${p.yTotal.toFixed(1)}`).join(' ')
  const outputLine = points.map((p) => `${p.x.toFixed(1)},${p.yOutput.toFixed(1)}`).join(' ')
  const baseY = (LINE_PAD_T + plotH).toFixed(1)
  const firstX = points[0].x.toFixed(1)
  const lastX = points[points.length - 1].x.toFixed(1)
  const areaPath = `M ${firstX} ${baseY} L ${points.map((p) => `${p.x.toFixed(1)} ${p.yTotal.toFixed(1)}`).join(' L ')} L ${lastX} ${baseY} Z`

  const yTicks = [0, 0.5, 1].map((ratio) => ({
    y: yFor(maxTokens * ratio),
    label: formatTokenCount(Math.round(maxTokens * ratio)),
  }))

  const xLabelStep = Math.max(1, Math.ceil(points.length / 8))
  return { points, totalLine, outputLine, areaPath, yTicks, xLabelStep, plotBottom: LINE_PAD_T + plotH }
})

function formatTokensPerSec(rate: number | null): string {
  if (rate === null || !Number.isFinite(rate) || rate <= 0) return '—'
  if (rate >= 1000) return `${(rate / 1000).toFixed(1)}k tok/s`
  return `${rate.toFixed(1)} tok/s`
}

function projectNameOf(projectId: number | null): string | null {
  if (projectId === null) return null
  return projectsStore.projects.find((p) => p.id === projectId)?.name ?? null
}

const taskRows = computed(() =>
  (data.value?.byTask ?? []).map((row) => ({
    cardId: row.cardId,
    code: row.publicId ?? row.cardId,
    title: row.title ?? row.cardId,
    project: projectNameOf(row.projectId),
    turns: INT_FORMAT.format(row.turns),
    duration: formatDurationMs(row.durationMs),
    tokens: formatTokenCount(row.totalTokens),
    tokensDetail: `Ent. ${formatTokenCount(row.inputTokens)} · Saída ${formatTokenCount(row.outputTokens + row.thoughtTokens)}`,
    rate: formatTokensPerSec(row.tokensPerSec),
    cost: formatEstimatedUsd(row.costUsdEstimated),
    unpriced: row.unpricedTurns > 0 && row.costUsdEstimated === null,
  })),
)

const taskSummary = computed(() => {
  const tasks = data.value?.byTask ?? []
  const totalDurationMs = tasks.reduce((acc, row) => acc + row.durationMs, 0)
  const totalTokens = tasks.reduce((acc, row) => acc + row.totalTokens, 0)
  const avgRate = totalDurationMs > 0 ? (totalTokens / (totalDurationMs / 1000)) : null
  return {
    count: tasks.length,
    totalDuration: formatDurationMs(totalDurationMs),
    avgRate: formatTokensPerSec(avgRate),
  }
})

function applyCustom(): void {
  void analytics.applyCustomRange(draftFrom.value, draftTo.value)
}

onMounted(async () => {
  await Promise.allSettled([
    projectsStore.loaded ? Promise.resolve() : projectsStore.load(),
    roster.agents.length > 0 ? Promise.resolve() : roster.load(),
    providersStore.providers.length > 0 ? Promise.resolve() : providersStore.load(),
    analytics.load(),
  ])
})
</script>

<template>
  <section :class="PAGE_CLASSES">
    <div class="flex min-w-0 flex-col gap-5">
    <UiPageHeader :title="PAGE_TITLE" :subtitle="PAGE_SUBTITLE">
      <template #actions>
        <UiBadge status="lote" :label="COST_BADGE_LABEL" />
        <UiButton variant="quiet" size="sm" :disabled="loading" @press="analytics.load()">
          {{ loading ? 'Atualizando…' : 'Atualizar' }}
        </UiButton>
      </template>
    </UiPageHeader>

    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="flex flex-wrap items-center gap-3">
        <UiSegmented v-model="periodModel" :options="PERIOD_OPTIONS" label="Período das estatísticas" wrap />

        <div v-if="period === 'custom'" class="flex flex-wrap items-center gap-2">
          <label class="flex items-center gap-1.5 text-caption text-muted">
            <span>De</span>
            <input v-model="draftFrom" type="date" aria-label="Data inicial" :class="DATE_INPUT_CLASSES" />
          </label>
          <label class="flex items-center gap-1.5 text-caption text-muted">
            <span>Até</span>
            <input v-model="draftTo" type="date" aria-label="Data final" :class="DATE_INPUT_CLASSES" />
          </label>
          <UiButton variant="primary" size="lg" :disabled="loading || !draftFrom || !draftTo" @press="applyCustom">
            Aplicar
          </UiButton>
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <UiSelect v-model="projectModel" :options="projectOptions" label="Filtrar por projeto" class="w-44 max-w-full" />
        <UiSelect v-model="providerModel" :options="providerOptions" label="Filtrar por provedor" class="w-44 max-w-full" />
      </div>
    </div>

    <div
      v-if="agentOptions.length > 1"
      class="flex flex-wrap items-center gap-1.5"
      role="group"
      aria-label="Filtrar por agente"
    >
      <button
        v-for="option in agentOptions"
        :key="option.value"
        type="button"
        class="inline-flex cursor-pointer items-center gap-1.5 rounded-full border px-2.5 py-1 text-caption transition-colors"
        :class="
          agentModel === option.value
            ? 'border-ink bg-ink text-bg font-semibold'
            : 'border-line bg-surface text-ink hover:bg-hover'
        "
        :aria-pressed="agentModel === option.value"
        @click="agentModel = option.value"
      >
        <AgentPortrait v-if="option.value !== ALL_VALUE" :who="option.value" :size="18" />
        <span>{{ option.label }}</span>
      </button>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-2 rounded-md border border-line bg-surface-2 px-3.5 py-2 text-caption text-muted">
      <span>{{ data?.costDisclaimer ?? COST_DISCLAIMER_DEFAULT }}</span>
      <span v-if="data" class="tabular-nums text-meta">
        {{ data.range.from }} → {{ data.range.to }} ({{ data.range.timezone }}, {{ data.range.offset }}) · tabela
        {{ data.pricingVersion }}
      </span>
    </div>

    <UiNotice v-if="error !== null" :text="error" @close="error = null" />

    <div v-if="loading && !data" class="flex flex-col gap-4">
      <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <UiSkeleton v-for="n in 4" :key="n" type="card" />
      </div>
      <div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <UiSkeleton type="card" />
        <UiSkeleton type="card" />
      </div>
    </div>

    <UiEmpty v-else-if="!loading && isEmpty" :title="EMPTY_TITLE" :text="EMPTY_TEXT" />

    <template v-else-if="data">
      <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <article :class="CARD_CLASSES">
          <p class="text-caption text-muted">Total de tokens</p>
          <p class="text-section font-semibold tabular-nums text-ink">
            {{ formatTokenCount(data.totals.totalTokens) }}
          </p>
          <p class="text-meta tabular-nums text-muted">
            Ent. {{ formatTokenCount(data.totals.inputTokens) }} · Saída {{ formatTokenCount(data.totals.outputTokens) }}
            <template v-if="data.totals.thoughtTokens > 0">
              · Pens. {{ formatTokenCount(data.totals.thoughtTokens) }}
            </template>
          </p>
        </article>

        <article :class="CARD_CLASSES">
          <p class="text-caption text-muted">{{ COST_BADGE_LABEL }}</p>
          <p class="text-section font-semibold tabular-nums text-ink">
            {{ formatEstimatedUsd(data.totals.costUsdEstimated) }}
          </p>
          <p class="text-meta text-muted">
            <template v-if="data.totals.unpricedTurns > 0">
              {{ data.totals.unpricedTurns }} turno(s) sem estimativa de custo
            </template>
            <template v-else>
              Todos os {{ INT_FORMAT.format(data.totals.pricedTurns) }} turno(s) precificados
            </template>
          </p>
        </article>

        <article :class="CARD_CLASSES">
          <p class="text-caption text-muted">Economia (cache + tiers leves)</p>
          <p class="text-section font-semibold tabular-nums text-ok">
            {{ data.savings.savingsPercent.toFixed(1) }}%
          </p>
          <p class="text-meta tabular-nums text-muted">
            {{ formatEstimatedUsd(data.savings.totalSavingsUsd) }} economizados · cache
            {{ data.savings.cacheHitPercent.toFixed(1) }}%
          </p>
        </article>

        <article :class="CARD_CLASSES">
          <p class="text-caption text-muted">Turnos e tempo</p>
          <p class="text-section font-semibold tabular-nums text-ink">
            {{ INT_FORMAT.format(data.totals.turns) }}
          </p>
          <p class="text-meta tabular-nums text-muted">
            Duração {{ formatDurationMs(data.totals.durationMs) }}
            <template v-if="data.totals.fallbackTurns > 0">
              · {{ data.totals.fallbackTurns }} fallback(s)
            </template>
          </p>
        </article>
      </div>

      <div
        v-if="data.totals.unpricedTurns > 0"
        class="rounded-md border border-line bg-warn-soft px-3.5 py-2 text-caption text-warn-text"
      >
        {{ data.totals.unpricedTurns }} turno(s) usaram modelo sem tarifa conhecida na tabela local: os tokens completos
        são contabilizados normalmente e o custo aparece como <strong>{{ UNPRICED_COST_LABEL }}</strong> (nunca $0.00).
      </div>

      <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <section :class="SECTION_CARD_CLASSES">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <div>
              <h2 class="text-lead text-ink">Uso diário por provedor</h2>
              <p class="text-meta text-muted">
                Agrupado por dia no fuso {{ data.range.timezone }} ({{ data.range.offset }}).
              </p>
            </div>
            <UiSegmented v-model="chartMetric" :options="CHART_METRIC_OPTIONS" label="Métrica do gráfico" />
          </div>

          <div class="flex flex-wrap items-center gap-3 text-meta text-muted">
            <span
              v-for="(providerId, index) in providerOrder"
              :key="providerId"
              class="inline-flex items-center gap-1.5"
            >
              <span class="size-2.5 rounded-xs" :class="providerBarClass(providerId, index)" aria-hidden="true" />
              <span class="text-ink">{{ providerLabel(providerId) }}</span>
            </span>
          </div>

          <div
            v-if="stackedDays.length > 0"
            class="flex h-48 items-end gap-1.5 overflow-x-auto border-b border-line pt-4 pb-1"
            role="img"
            aria-label="Gráfico empilhado de uso diário por provedor"
          >
            <div
              v-for="day in stackedDays"
              :key="day.date"
              class="flex h-full min-w-8 flex-1 flex-col items-center justify-end gap-1"
              :title="day.summaryTooltip"
            >
              <div class="flex h-36 w-full max-w-10 flex-col-reverse justify-start gap-px overflow-hidden rounded-t-sm bg-surface-2">
                <div
                  v-for="segment in day.segments"
                  :key="segment.providerId"
                  class="w-full transition-all"
                  :class="segment.colorClass"
                  :style="{ height: `${segment.heightPercent}%` }"
                  :title="segment.tooltip"
                />
              </div>
              <span class="text-tiny tabular-nums text-muted">{{ day.shortLabel }}</span>
            </div>
          </div>
        </section>

        <section :class="SECTION_CARD_CLASSES">
          <div>
            <h2 class="text-lead text-ink">Consumo de tokens por agente</h2>
            <p class="text-meta text-muted">Distribuição do total de tokens entre os agentes no período.</p>
          </div>
          <div v-if="agentPieSlices.length > 0" class="flex flex-col items-center gap-4 sm:flex-row sm:items-center">
            <div class="relative grid size-48 shrink-0 place-items-center">
              <svg viewBox="0 0 200 200" class="size-48 -rotate-90" role="img" aria-label="Gráfico de pizza de consumo de tokens por agente">
                <circle
                  v-for="slice in agentPieSlices"
                  :key="slice.agentId"
                  cx="100"
                  cy="100"
                  :r="PIE_RADIUS"
                  fill="none"
                  :stroke="slice.color"
                  :stroke-width="hoveredPieAgent === slice.agentId ? 32 : 26"
                  :stroke-dasharray="slice.dashArray"
                  :stroke-dashoffset="slice.dashOffset"
                  class="cursor-pointer transition-all duration-150"
                  :opacity="hoveredPieAgent && hoveredPieAgent !== slice.agentId ? 0.45 : 1"
                  @mouseenter="hoveredPieAgent = slice.agentId"
                  @mouseleave="hoveredPieAgent = null"
                  @click="agentModel = agentModel === slice.agentId ? ALL_VALUE : slice.agentId"
                >
                  <title>{{ slice.name }}: {{ formatTokenCount(slice.tokens) }} ({{ slice.percent.toFixed(1) }}%)</title>
                </circle>
              </svg>
              <div class="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center">
                <span class="text-tiny text-muted">Total</span>
                <span class="text-card-title font-semibold tabular-nums text-ink">
                  {{ formatTokenCount(data.totals.totalTokens) }}
                </span>
              </div>
            </div>
            <ul class="flex min-w-0 flex-1 flex-col gap-1.5 self-stretch justify-center">
              <li
                v-for="slice in agentPieSlices"
                :key="slice.agentId"
                class="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-caption transition-colors hover:bg-surface-2"
                :class="hoveredPieAgent === slice.agentId && 'bg-surface-2'"
                @mouseenter="hoveredPieAgent = slice.agentId"
                @mouseleave="hoveredPieAgent = null"
                @click="agentModel = agentModel === slice.agentId ? ALL_VALUE : slice.agentId"
              >
                <span class="size-2.5 shrink-0 rounded-full" :style="{ backgroundColor: slice.color }" aria-hidden="true" />
                <AgentPortrait :who="slice.agentId" :size="20" class="shrink-0" />
                <span class="min-w-0 flex-1 truncate font-medium text-ink">{{ slice.name }}</span>
                <span class="shrink-0 font-mono text-meta tabular-nums text-muted">{{ formatTokenCount(slice.tokens) }}</span>
                <span class="w-12 shrink-0 text-right text-meta font-semibold tabular-nums text-ink">{{ slice.percent.toFixed(1) }}%</span>
              </li>
            </ul>
          </div>
          <p v-else class="text-caption text-muted">Sem consumo de tokens por agente no período.</p>
        </section>
      </div>

      <section :class="SECTION_CARD_CLASSES">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div>
            <h2 class="text-lead text-ink">Consumo de tokens ao longo do tempo</h2>
            <p class="text-meta text-muted">
              Evolução diária do total de tokens e de saída + pensamento no período.
            </p>
          </div>
          <div class="flex flex-wrap items-center gap-3 text-meta text-muted">
            <span class="inline-flex items-center gap-1.5">
              <span class="h-0.5 w-4 rounded-full bg-accent" aria-hidden="true" />
              <span class="text-ink">Total de tokens</span>
            </span>
            <span class="inline-flex items-center gap-1.5">
              <span class="h-0.5 w-4 border-t-2 border-dashed border-ok" aria-hidden="true" />
              <span class="text-ink">Saída + pens.</span>
            </span>
          </div>
        </div>

        <div v-if="tokenLineChart" class="flex flex-col gap-2">
          <div class="w-full overflow-x-auto">
            <svg
              :viewBox="`0 0 ${LINE_CHART_W} ${LINE_CHART_H}`"
              class="h-48 w-full min-w-[460px]"
              role="img"
              aria-label="Gráfico de linha do consumo de tokens ao longo do tempo"
            >
              <defs>
                <linearGradient id="vokia-tokens-area" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="var(--color-accent)" stop-opacity="0.22" />
                  <stop offset="100%" stop-color="var(--color-accent)" stop-opacity="0.01" />
                </linearGradient>
              </defs>

              <g v-for="tick in tokenLineChart.yTicks" :key="tick.label">
                <line
                  :x1="LINE_PAD_L"
                  :x2="LINE_CHART_W - LINE_PAD_R"
                  :y1="tick.y"
                  :y2="tick.y"
                  stroke="var(--color-line)"
                  stroke-dasharray="3 3"
                />
                <text
                  :x="LINE_PAD_L - 8"
                  :y="tick.y + 3.5"
                  text-anchor="end"
                  fill="var(--color-muted)"
                  font-size="10"
                >
                  {{ tick.label }}
                </text>
              </g>

              <path :d="tokenLineChart.areaPath" fill="url(#vokia-tokens-area)" />
              <polyline
                :points="tokenLineChart.totalLine"
                fill="none"
                stroke="var(--color-accent)"
                stroke-width="2.2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
              <polyline
                :points="tokenLineChart.outputLine"
                fill="none"
                stroke="var(--color-ok)"
                stroke-width="1.6"
                stroke-dasharray="4 3"
                stroke-linecap="round"
                stroke-linejoin="round"
              />

              <g v-for="pt in tokenLineChart.points" :key="pt.date">
                <text
                  v-if="pt.index % tokenLineChart.xLabelStep === 0 || pt.index === tokenLineChart.points.length - 1"
                  :x="pt.x"
                  :y="tokenLineChart.plotBottom + 16"
                  text-anchor="middle"
                  fill="var(--color-muted)"
                  font-size="10"
                >
                  {{ pt.shortLabel }}
                </text>
                <circle
                  :cx="pt.x"
                  :cy="pt.yTotal"
                  :r="hoveredLineIndex === pt.index ? 5 : 3.2"
                  fill="var(--color-accent)"
                  stroke="var(--color-surface)"
                  stroke-width="1.5"
                  class="cursor-pointer transition-all duration-100"
                  @mouseenter="hoveredLineIndex = pt.index"
                  @mouseleave="hoveredLineIndex = null"
                >
                  <title>{{ pt.date }} · {{ formatTokenCount(pt.totalTokens) }} tokens (saída+pens: {{ formatTokenCount(pt.outputTotal) }}) · {{ pt.turns }} turnos · {{ pt.duration }} · {{ pt.cost }}</title>
                </circle>
              </g>
            </svg>
          </div>
          <p v-if="hoveredLineIndex !== null && tokenLineChart.points[hoveredLineIndex]" class="text-caption tabular-nums text-muted">
            <strong class="text-ink">{{ tokenLineChart.points[hoveredLineIndex].date }}</strong>
            · Total {{ formatTokenCount(tokenLineChart.points[hoveredLineIndex].totalTokens) }}
            · Saída+pens {{ formatTokenCount(tokenLineChart.points[hoveredLineIndex].outputTotal) }}
            · {{ tokenLineChart.points[hoveredLineIndex].turns }} turno(s)
            · {{ tokenLineChart.points[hoveredLineIndex].duration }}
            · {{ tokenLineChart.points[hoveredLineIndex].cost }}
          </p>
        </div>
        <p v-else class="text-caption text-muted">Sem dados diários para exibir a linha de consumo.</p>
      </section>

      <section :class="SECTION_CARD_CLASSES">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div>
            <h2 class="text-lead text-ink">Estatísticas por task</h2>
            <p class="text-meta text-muted">
              Consumo de tokens, tempo gasto e razão de vazão (tokens/s) por card executado.
            </p>
          </div>
          <div v-if="taskSummary.count > 0" class="flex flex-wrap items-center gap-2 text-meta tabular-nums text-muted">
            <span>{{ taskSummary.count }} task(s)</span>
            <span>· Tempo {{ taskSummary.totalDuration }}</span>
            <span>· Média {{ taskSummary.avgRate }}</span>
          </div>
        </div>
        <UiTable v-if="taskRows.length > 0" :columns="TASK_COLUMNS" :rows="taskRows" :min-width="640">
          <template #cell-task="{ row }">
            <div class="flex min-w-0 flex-col">
              <div class="flex items-center gap-1.5">
                <span class="shrink-0 rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-tiny font-semibold text-muted">{{ row.code }}</span>
                <span class="truncate font-medium text-ink">{{ row.title }}</span>
              </div>
              <span v-if="row.project" class="text-tiny text-muted">{{ row.project }}</span>
            </div>
          </template>
          <template #cell-tokens="{ row }">
            <div class="flex flex-col items-end">
              <span class="font-medium text-ink">{{ row.tokens }}</span>
              <span class="text-tiny text-muted">{{ row.tokensDetail }}</span>
            </div>
          </template>
          <template #cell-cost="{ row }">
            <span :class="row.unpriced ? 'text-muted italic' : ''">{{ row.cost }}</span>
          </template>
        </UiTable>
        <p v-else class="text-caption text-muted">Nenhuma task vinculada aos turnos deste período.</p>
      </section>

      <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <section :class="SECTION_CARD_CLASSES">
          <h2 class="text-lead text-ink">Consumo por agente</h2>
          <UiTable :columns="AGENT_COLUMNS" :rows="agentRows" :min-width="460">
            <template #cell-agent="{ row }">
              <span class="inline-flex items-center gap-2 font-medium text-ink">
                <AgentPortrait :who="String(row.agentId)" :size="20" class="shrink-0" />
                <span>{{ row.agent }}</span>
              </span>
            </template>
            <template #cell-cost="{ row }">
              <span :class="row.unpriced ? 'text-muted italic' : ''">{{ row.cost }}</span>
            </template>
          </UiTable>
        </section>

        <section :class="SECTION_CARD_CLASSES">
          <h2 class="text-lead text-ink">Consumo por modelo</h2>
          <UiTable :columns="MODEL_COLUMNS" :rows="modelRows" :min-width="460">
            <template #cell-model="{ row }">
              <div class="flex flex-col">
                <span class="font-medium text-ink">{{ row.model }}</span>
                <span class="text-tiny text-muted">{{ row.provider }}</span>
              </div>
            </template>
            <template #cell-cost="{ row }">
              <span :class="row.unpriced ? 'text-muted italic' : ''">{{ row.cost }}</span>
            </template>
          </UiTable>
        </section>
      </div>

      <section :class="SECTION_CARD_CLASSES">
        <div class="flex flex-wrap items-baseline justify-between gap-2">
          <h2 class="text-lead text-ink">Tabela diária</h2>
          <span class="text-meta text-muted">
            Valores numéricos exatos · {{ COST_BADGE_LABEL }}
          </span>
        </div>
        <UiTable :columns="DAILY_COLUMNS" :rows="dailyRows" :min-width="760">
          <template #cell-cost="{ row }">
            <span :class="row.unpriced ? 'text-muted italic' : ''">{{ row.cost }}</span>
          </template>
        </UiTable>
      </section>
    </template>
    </div>
  </section>
</template>
