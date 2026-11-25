<script setup lang="ts">
import { computed, reactive, ref, watch, type UnwrapNestedRefs } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiChip from '../../ui/UiChip.vue'
import { presenceOf } from '../team/agentStateStyle'
import AgentPortrait from '../team/AgentPortrait.vue'
import AgentProjectIcons from '../team/AgentProjectIcons.vue'
import TurnPlanCard from '../team/TurnPlanCard.vue'
import { PORTRAIT_SIZES } from '../team/portraitArt'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import { useFeed } from '../team/useFeed'
import { statusOfTurnPlan, useTurnPlans } from '../team/useTurnPlans'
import BatchProgressList from '../orchestration/BatchProgressList.vue'
import AgentNow from './AgentNow.vue'
import InterruptedRuns from './InterruptedRuns.vue'
import {
  agentTabLabel,
  ALL_TAB_LABEL,
  BADGE_BY_STATUS,
  NO_TASK_TEXT,
  PAUSE_LABEL,
  RESUME_LABEL,
  type NowTab,
} from './nowCopy'
import { useAgentActivity } from './useAgentActivity'
import { useAgentControl } from './useAgentControl'

const feed = useFeed()
const roster = useRoster()
const control = useAgentControl()
const turnPlans = useTurnPlans()
const tab = ref<NowTab>('todos')
type AgentActivity = UnwrapNestedRefs<ReturnType<typeof useAgentActivity>>

const activities = new Map<Agent, AgentActivity>()

function activityOf(agent: Agent): AgentActivity {
  const cached = activities.get(agent)
  if (cached !== undefined) {
    return cached
  }
  const created = reactive(useAgentActivity(agent))
  activities.set(agent, created)
  return created
}

const rows = computed(() => roster.agents.map((agent) => ({ agent, activity: activityOf(agent.id) })))
const workingRows = computed(() =>
  rows.value.filter(
    (row) => feed.activeAgents.some((active) => active.id === row.agent.id) || control.isPaused(row.agent.id),
  ),
)

// O chip do agente que ficou ocioso some; sem voltar para "Todos" a aba abriria um agente que não está na lista.
watch(workingRows, (active) => {
  if (tab.value !== 'todos' && !active.some((row) => row.agent.id === tab.value)) tab.value = 'todos'
})
</script>

<template>
  <div class="flex flex-col gap-2">
    <InterruptedRuns />
    <BatchProgressList />
    <div class="flex flex-wrap gap-1.5">
      <UiChip
        :label="`${ALL_TAB_LABEL} ${workingRows.length}`"
        :selected="tab === 'todos'"
        data-testid="now-subtab-todos"
        @press="tab = 'todos'"
      />
      <UiChip
        v-for="row in workingRows"
        :key="row.agent.id"
        :label="agentTabLabel(row.agent.name, row.activity.queue.length)"
        :selected="tab === row.agent.id"
        :data-testid="`now-subtab-${row.agent.id}`"
        @press="tab = row.agent.id"
      />
    </div>
    <ul v-if="tab === 'todos'" class="flex max-h-96 flex-col gap-2 overflow-y-auto">
      <li v-if="workingRows.length === 0" class="text-caption text-muted" data-testid="now-empty">{{ NO_TASK_TEXT }}</li>
      <li
        v-for="row in workingRows"
        :key="row.agent.id"
        class="flex flex-col gap-2 rounded-md border border-line bg-surface px-3 py-2"
        :data-testid="`now-row-${row.agent.id}`"
      >
        <div class="flex items-center gap-2">
          <AgentPortrait :who="row.agent.id" :size="PORTRAIT_SIZES.stack" />
          <span class="flex min-w-0 flex-1 flex-col">
            <span class="flex items-center gap-2">
              <span class="text-label font-semibold text-ink">{{ row.agent.name }}</span>
              <AgentProjectIcons :agent="row.agent.id" />
              <UiBadge :status="BADGE_BY_STATUS[row.activity.status]" :label="row.activity.status" />
            </span>
            <span class="wrap-break-word line-clamp-1 text-caption text-muted">{{ row.activity.task ?? NO_TASK_TEXT }}</span>
            <span class="flex min-w-0 items-center gap-1.5 text-caption text-ink">
              <span
                class="size-1.75 shrink-0 rounded-full"
                :class="presenceOf(feed.stateOf(row.agent.id), control.isPaused(row.agent.id)).dot"
              />
              <span class="min-w-0 truncate" :title="row.activity.step">{{ row.activity.step }}</span>
            </span>
          </span>
          <UiButton variant="quiet" size="sm" @press="feed.togglePause(row.agent.id)">
            {{ control.isPaused(row.agent.id) ? RESUME_LABEL : PAUSE_LABEL }}
          </UiButton>
        </div>
        <TurnPlanCard
          v-if="turnPlans.planOf(row.agent.id)"
          :plan="turnPlans.planOf(row.agent.id)!.plan"
          :status="statusOfTurnPlan(turnPlans.planOf(row.agent.id)!.plan, { active: turnPlans.planOf(row.agent.id)!.active })"
          compact
        />
      </li>
    </ul>
    <AgentNow v-else :key="tab" :agent="tab" />
  </div>
</template>
