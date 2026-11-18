<script setup lang="ts">
import { computed, ref } from 'vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import UiSelect from '../../ui/UiSelect.vue'
import UiSwitch from '../../ui/UiSwitch.vue'
import { placeableAgents } from '../team/agents'
import { useRoster } from '../team/useRoster'
import CountStepper from './CountStepper.vue'
import {
  HUMAN_GATE_LABELS,
  STEP_MODE_LABELS,
  VIEW_COPY,
  WORKTREE_LABELS,
} from './orchestrationLabels'
import {
  HUMAN_GATE_VALUES,
  STEP_MODE_VALUES,
  WORKTREE_POLICY_VALUES,
  type HumanGate,
  type OrchLayer,
  type OrchRule,
  type OrchestrationSettings,
  type StepMode,
  type WorktreePolicy,
} from './orchestrationTypes'
import { useOrchestration } from './useOrchestration'

const PARALLEL_MIN = 1
const PARALLEL_MAX = 8
const PAIR_TURNS_MIN = 1
const PAIR_TURNS_MAX = 50
const TASK_TURNS_MIN = 5
const TASK_TURNS_MAX = 200
const TASK_TURNS_STEP = 5

const props = defineProps<{ settings: OrchestrationSettings; layers: OrchLayer[]; rules: OrchRule[] }>()

const orchestration = useOrchestration()
const roster = useRoster()

// Recriado quando um salvamento falha, para o <select> voltar ao valor do snapshot.
const revision = ref(0)

const leader = computed(() => roster.profileOf(props.settings.leaderAgent))
const cliLabel = computed(() => {
  const cli = leader.value?.cli
  return roster.presets.find((preset) => preset.id === cli)?.label ?? cli ?? ''
})
const leaderLine = computed(() =>
  leader.value ? [leader.value.role, cliLabel.value, leader.value.model].join(' · ') : '',
)
const leaderTitle = computed(() =>
  `${roster.nameOf(props.settings.leaderAgent)} ${VIEW_COPY.leaderTitle}`,
)

const agentOptions = computed(() => placeableAgents(roster.agents).map((agent) => ({ label: agent.name, value: agent.id })))
const stepOptions = STEP_MODE_VALUES.map((value) => ({ label: STEP_MODE_LABELS[value], value }))
const gateOptions = HUMAN_GATE_VALUES.map((value) => ({ label: HUMAN_GATE_LABELS[value], value }))
const worktreeOptions = WORKTREE_POLICY_VALUES.map((value) => ({ label: WORKTREE_LABELS[value], value }))

const orderedLayers = computed(() => [...props.layers].sort((a, b) => a.position - b.position))
const gateEndLabel = computed(() =>
  props.settings.humanGate === 'nunca' ? VIEW_COPY.autoMerge : VIEW_COPY.youApprove,
)

const enforcementOn = computed({
  get: () => props.settings.enforcement === 'on',
  set: (value: boolean) => void save({ enforcement: value ? 'on' : 'off' }),
})
const showNoRulesWarning = computed(() => props.settings.enforcement === 'on' && props.rules.length === 0)

async function save(patch: Partial<OrchestrationSettings>): Promise<void> {
  const saved = await orchestration.saveSettings({ ...props.settings, ...patch })
  if (!saved) revision.value += 1
}

function onLeaderChange(value: string | undefined): void {
  if (value) void save({ leaderAgent: value })
}

function onStepChange(value: StepMode): void {
  void save({ stepMode: value })
}

function onGateChange(value: HumanGate): void {
  void save({ humanGate: value })
}

function onWorktreeChange(value: WorktreePolicy): void {
  void save({ worktreePolicy: value })
}
</script>

<template>
  <div :key="revision" class="flex min-w-0 flex-col gap-5">
    <section class="flex min-w-0 flex-col gap-4 rounded-[24px] border border-line bg-surface p-4.5">
      <header class="flex min-w-0 flex-col gap-1">
        <h3 class="text-control font-bold text-ink">{{ leaderTitle }}</h3>
        <p v-if="leaderLine" class="text-small text-muted">{{ leaderLine }}</p>
      </header>

      <label class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.leaderPick }}</span>
        <UiSelect
          :model-value="settings.leaderAgent"
          :options="agentOptions"
          :label="VIEW_COPY.leaderPick"
          @update:model-value="onLeaderChange"
        />
      </label>

      <div class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.steps }}</span>
        <UiSegmented
          :model-value="settings.stepMode"
          :options="stepOptions"
          :label="VIEW_COPY.steps"
          wrap
          @update:model-value="onStepChange"
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.humanGate }}</span>
        <UiSegmented
          :model-value="settings.humanGate"
          :options="gateOptions"
          :label="VIEW_COPY.humanGate"
          wrap
          @update:model-value="onGateChange"
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.parallel }}</span>
        <CountStepper
          :model-value="settings.maxParallel"
          :label="VIEW_COPY.parallel"
          :min="PARALLEL_MIN"
          :max="PARALLEL_MAX"
          @update:model-value="save({ maxParallel: $event })"
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.stepsOrder }}</span>
        <ol class="m-0 flex list-decimal flex-col gap-1 pl-5 text-caption text-ink">
          <li v-for="layer in orderedLayers" :key="layer.id">{{ layer.name }}</li>
          <li>{{ gateEndLabel }}</li>
        </ol>
      </div>
    </section>

    <section class="flex min-w-0 flex-col gap-4 rounded-[24px] border border-line bg-surface p-4.5">
      <h3 class="text-control font-bold text-ink">{{ VIEW_COPY.project }}</h3>

      <div class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.pairTurns }}</span>
        <CountStepper
          :model-value="settings.pairTurnLimit"
          :label="VIEW_COPY.pairTurns"
          :min="PAIR_TURNS_MIN"
          :max="PAIR_TURNS_MAX"
          @update:model-value="save({ pairTurnLimit: $event })"
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.taskTurns }}</span>
        <CountStepper
          :model-value="settings.taskTurnLimit"
          :label="VIEW_COPY.taskTurns"
          :min="TASK_TURNS_MIN"
          :max="TASK_TURNS_MAX"
          :step="TASK_TURNS_STEP"
          @update:model-value="save({ taskTurnLimit: $event })"
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.worktree }}</span>
        <UiSegmented
          :model-value="settings.worktreePolicy"
          :options="worktreeOptions"
          :label="VIEW_COPY.worktree"
          wrap
          @update:model-value="onWorktreeChange"
        />
      </div>

      <label class="flex items-center justify-between gap-3">
        <span class="text-control text-ink">{{ VIEW_COPY.enforcement }}</span>
        <UiSwitch v-model="enforcementOn" :label="VIEW_COPY.enforcement" />
      </label>
      <p v-if="showNoRulesWarning" class="text-small text-danger">{{ VIEW_COPY.enforcementWarning }}</p>
    </section>
  </div>
</template>
