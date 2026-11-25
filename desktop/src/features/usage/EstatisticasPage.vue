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
import UiSelect from '../../ui/UiSelect.vue'
import UiTable from '../../ui/UiTable.vue'
import { useProjects } from '../projects/useProjects'
import { useProviders } from '../providers/useProviders'
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

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const CARD_CLASSES = 'flex flex-col gap-1.5 rounded-md border border-line bg-surface p-4'
const SECTION_CARD_CLASSES = 'flex flex-col gap-3 rounded-md border border-line bg-surface p-4'
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
    <UiPageHeader :title="PAGE_TITLE" :subtitle="PAGE_SUBTITLE">
      <template #actions>
        <UiBadge status="lote" :label="COST_BADGE_LABEL" />
        <UiButton variant="quiet" size="sm" :disabled="loading" @press="analytics.load()">
          {{ loading ? 'Atualizando…' : 'Atualizar' }}
        </UiButton>
      </template>
    </UiPageHeader>

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

      <div class="ml-auto flex flex-wrap items-center gap-2">
        <UiSelect v-model="projectModel" :options="projectOptions" label="Filtrar por projeto" class="w-44" />
        <UiSelect v-model="agentModel" :options="agentOptions" label="Filtrar por agente" class="w-44" />
        <UiSelect v-model="providerModel" :options="providerOptions" label="Filtrar por provedor" class="w-44" />
      </div>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-2 rounded-md border border-line bg-surface-2 px-3.5 py-2 text-caption text-muted">
      <span>{{ data?.costDisclaimer ?? COST_DISCLAIMER_DEFAULT }}</span>
      <span v-if="data" class="tabular-nums text-meta">
        {{ data.range.from }} → {{ data.range.to }} ({{ data.range.timezone }}, {{ data.range.offset }}) · tabela
        {{ data.pricingVersion }}
      </span>
    </div>

    <UiNotice v-if="error !== null" :text="error" @close="error = null" />

    <UiEmpty v-if="!loading && isEmpty" :title="EMPTY_TITLE" :text="EMPTY_TEXT" />

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

      <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <section :class="SECTION_CARD_CLASSES">
          <h2 class="text-lead text-ink">Consumo por agente</h2>
          <UiTable :columns="AGENT_COLUMNS" :rows="agentRows" :min-width="460">
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
  </section>
</template>
