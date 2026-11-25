<script lang="ts">
const DEFAULT_LABEL = 'Selecionar agente'
const DEFAULT_PLACEHOLDER = 'Selecionar agente'
const DEFAULT_SEARCH_PLACEHOLDER = 'Buscar por nome, papel ou provedor…'
const DEFAULT_SEARCH_LABEL = 'Buscar agente'
const DEFAULT_EMPTY_TEXT = 'Nenhum agente disponível.'
</script>

<script setup lang="ts">
import { computed, ref } from 'vue'
import UiSearchPicker from '../../ui/UiSearchPicker.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import type { Agent } from '../team/events'
import type { AgentProfile } from '../team/teamApi'
import { useFeed } from '../team/useFeed'
import { useRoster } from '../team/useRoster'
import { useAgentControl } from '../panel/useAgentControl'
import { useActiveLlm } from '../providers/useActiveLlm'
import { useTemporaryOverrides } from '../providers/useTemporaryOverrides'
import {
  ALL_AGENTS_OPTION_ID,
  DEFAULT_ALL_AGENTS_LABEL,
  buildAgentPickerOptions,
  emptyAgentSearchMessage,
  presenceMetaOf,
  type AgentPickerOption,
} from './agentPicker'

const OPTION_PORTRAIT_SIZE = 28
const TRIGGER_PORTRAIT_SIZE = 18

const CHIP_CLASSES =
  'inline-flex max-w-full items-center truncate rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-tiny text-muted'
const CHIP_OVERRIDE_CLASSES =
  'inline-flex max-w-full items-center truncate rounded-sm bg-warn-soft px-1.5 py-0.5 font-mono text-tiny text-warn-text'
const COUNT_BADGE_CLASSES =
  'inline-flex min-w-5 items-center justify-center rounded-full bg-accent-soft px-1.5 py-0.5 text-tiny font-semibold tabular-nums text-accent'
const STATUS_BADGE_BASE =
  'absolute -right-0.5 -bottom-0.5 grid size-3.5 place-items-center rounded-full border'

const props = withDefaults(
  defineProps<{
    agents?: readonly AgentProfile[]
    allowAll?: boolean
    allLabel?: string
    busyFirst?: boolean
    counts?: Readonly<Record<string, number | undefined>>
    countLabel?: (count: number) => string
    label?: string
    placeholder?: string
    searchPlaceholder?: string
    disabled?: boolean
    placement?: 'bottom' | 'top'
    align?: 'start' | 'end'
    floating?: 'absolute' | 'fixed'
    popoverClass?: string
    triggerClass?: string
    triggerVariant?: 'input' | 'compact' | 'ghost'
    showCheck?: boolean
    title?: string
    testId?: string
  }>(),
  {
    agents: undefined,
    allowAll: false,
    allLabel: DEFAULT_ALL_AGENTS_LABEL,
    busyFirst: false,
    counts: undefined,
    countLabel: undefined,
    label: DEFAULT_LABEL,
    placeholder: DEFAULT_PLACEHOLDER,
    searchPlaceholder: DEFAULT_SEARCH_PLACEHOLDER,
    disabled: false,
    placement: 'bottom',
    align: 'start',
    floating: 'absolute',
    popoverClass: 'w-84',
    triggerClass: undefined,
    triggerVariant: 'input',
    showCheck: true,
    title: undefined,
    testId: 'agent-picker',
  },
)

const model = defineModel<Agent | null>({ default: null })
const open = defineModel<boolean>('open', { default: false })

const emit = defineEmits<{
  select: [agentId: Agent | null]
}>()

const roster = useRoster()
const feed = useFeed()
const control = useAgentControl()
const activeLlm = useActiveLlm()
const overrides = useTemporaryOverrides()

const query = ref('')

const sourceAgents = computed(() => props.agents ?? roster.agents)

const pickerModel = computed<string | null>({
  get() {
    if (model.value === null) {
      return props.allowAll ? ALL_AGENTS_OPTION_ID : null
    }
    return model.value
  },
  set(next) {
    if (next === null || next === ALL_AGENTS_OPTION_ID) {
      model.value = null
      return
    }
    model.value = next
  },
})

const options = computed<AgentPickerOption[]>(() =>
  buildAgentPickerOptions({
    agents: sourceAgents.value,
    query: query.value,
    states: feed.states,
    isPaused: (id) => control.isPaused(id),
    counts: props.counts,
    presets: roster.presets,
    activeLlm: activeLlm.loaded ? activeLlm.active : null,
    overrides: overrides.overrides,
    busyFirst: props.busyFirst,
    allowAll: props.allowAll,
    allLabel: props.allLabel,
  }),
)

const selectedProfile = computed<AgentProfile | null>(() => {
  if (model.value === null) return null
  return sourceAgents.value.find((agent) => agent.id === model.value) ?? roster.profileOf(model.value) ?? null
})

const selectedPresence = computed(() =>
  selectedProfile.value
    ? presenceMetaOf(feed.stateOf(selectedProfile.value.id), control.isPaused(selectedProfile.value.id))
    : null,
)

function countTitle(count: number): string {
  return props.countLabel ? props.countLabel(count) : String(count)
}

function onSelect(item: AgentPickerOption): void {
  const nextId = item.kind === 'all' ? null : item.id
  emit('select', nextId)
}
</script>

<template>
  <UiSearchPicker
    v-model="pickerModel"
    v-model:open="open"
    v-model:query="query"
    :items="options"
    external-filter
    :label="label"
    :placeholder="allowAll ? allLabel : placeholder"
    :search-placeholder="searchPlaceholder"
    :search-label="DEFAULT_SEARCH_LABEL"
    :empty-text="DEFAULT_EMPTY_TEXT"
    :empty-query-text="emptyAgentSearchMessage"
    :disabled="disabled"
    :placement="placement"
    :align="align"
    :floating="floating"
    :popover-class="popoverClass"
    :trigger-class="triggerClass"
    :trigger-variant="triggerVariant"
    :show-check="showCheck"
    :title="title"
    :test-id="testId"
    @select="onSelect"
  >
    <template #trigger="{ open: isOpen, toggle }">
      <slot
        name="trigger"
        :agent="selectedProfile"
        :presence="selectedPresence"
        :open="isOpen"
        :toggle="toggle"
      >
        <span class="flex min-w-0 items-center gap-1.5">
          <AgentPortrait
            v-if="selectedProfile"
            :who="selectedProfile.id"
            :size="TRIGGER_PORTRAIT_SIZE"
            class="shrink-0"
          />
          <svg
            v-else-if="allowAll"
            class="size-3.5 shrink-0 text-muted"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <circle cx="6" cy="5" r="2.25" />
            <path d="M2 13c0-2.2 1.8-3.6 4-3.6s4 1.4 4 3.6" />
            <circle cx="11.5" cy="5.5" r="1.75" />
            <path d="M11.5 9.5c1.5.3 2.5 1.5 2.5 3.5" />
          </svg>
          <span :class="['min-w-0 truncate', selectedProfile || allowAll ? 'text-ink' : 'text-muted']">
            {{ selectedProfile ? selectedProfile.name : allowAll ? allLabel : placeholder }}
          </span>
        </span>
        <svg
          class="size-3.5 shrink-0 text-muted transition-transform duration-150 motion-reduce:transition-none"
          :class="isOpen && 'rotate-180'"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="m6 9 6 6 6-6" />
        </svg>
      </slot>
    </template>

    <template #leading="{ item }">
      <span
        v-if="item.kind === 'all'"
        class="grid size-7 shrink-0 place-items-center rounded-full border border-line bg-surface-2 text-muted"
        aria-hidden="true"
      >
        <svg
          class="size-4"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="6" cy="5" r="2.25" />
          <path d="M2 13c0-2.2 1.8-3.6 4-3.6s4 1.4 4 3.6" />
          <circle cx="11.5" cy="5.5" r="1.75" />
          <path d="M11.5 9.5c1.5.3 2.5 1.5 2.5 3.5" />
        </svg>
      </span>

      <span v-else class="relative flex shrink-0">
        <AgentPortrait :who="item.agent.id" :size="OPTION_PORTRAIT_SIZE" />
        <span
          :class="[STATUS_BADGE_BASE, item.presence.badgeClasses]"
          :title="item.presence.label"
          role="img"
          :aria-label="item.presence.label"
        >
          <!-- Trabalhando: raio / atividade (distinguível sem cor) -->
          <svg
            v-if="item.presence.kind === 'working'"
            class="size-2.5 motion-safe:animate-pulse"
            viewBox="0 0 12 12"
            fill="currentColor"
            aria-hidden="true"
          >
            <path d="M6.8 1L2.6 6.5h3L5.2 11l4.2-5.5h-3L6.8 1z" />
          </svg>
          <!-- Aguardando: relógio -->
          <svg
            v-else-if="item.presence.kind === 'waiting'"
            class="size-2.5"
            viewBox="0 0 12 12"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
            aria-hidden="true"
          >
            <path d="M6 3.2V6l2 1.4" />
          </svg>
          <!-- Pausado: duas barras verticais -->
          <svg
            v-else-if="item.presence.kind === 'paused'"
            class="size-2.5"
            viewBox="0 0 12 12"
            fill="currentColor"
            aria-hidden="true"
          >
            <rect x="3" y="2.5" width="2" height="7" rx="0.6" />
            <rect x="7" y="2.5" width="2" height="7" rx="0.6" />
          </svg>
          <!-- Livre: círculo vazado -->
          <svg
            v-else
            class="size-2"
            viewBox="0 0 12 12"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            aria-hidden="true"
          >
            <circle cx="6" cy="6" r="3" />
          </svg>
        </span>
      </span>
    </template>

    <template #option="{ item }">
      <span v-if="item.kind === 'all'" class="flex min-w-0 flex-col">
        <span class="truncate text-control font-semibold text-ink">{{ item.label }}</span>
        <span class="truncate text-tiny text-muted">Exibir todos os membros do time</span>
      </span>

      <span v-else class="flex min-w-0 flex-col gap-0.5">
        <span class="flex min-w-0 items-center gap-1.5">
          <span class="truncate text-control font-semibold text-ink">{{ item.agent.name }}</span>
          <span
            v-if="item.presence.kind !== 'idle'"
            class="shrink-0 text-tiny font-medium text-muted"
          >
            · {{ item.presence.shortLabel }}
          </span>
        </span>
        <span class="flex min-w-0 flex-wrap items-center gap-1.5">
          <span v-if="item.agent.role" class="truncate text-meta text-muted">{{ item.agent.role }}</span>
          <span
            v-if="item.llm.chipLabel"
            :class="item.llm.isOverride ? CHIP_OVERRIDE_CLASSES : CHIP_CLASSES"
            :title="item.llm.model ? `${item.llm.providerLabel} · ${item.llm.model}` : item.llm.providerLabel"
          >
            {{ item.llm.chipLabel }}
          </span>
        </span>
      </span>
    </template>

    <template #trailing="{ item }">
      <span
        v-if="item.count > 0"
        :class="COUNT_BADGE_CLASSES"
        :title="countTitle(item.count)"
        :aria-label="countTitle(item.count)"
      >
        {{ item.count }}
      </span>
    </template>
  </UiSearchPicker>
</template>
