<script setup lang="ts">
import { computed, reactive, ref, type UnwrapNestedRefs } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiChip from '../../ui/UiChip.vue'
import { presenceOf } from '../team/agentStateStyle'
import AgentPortrait from '../team/AgentPortrait.vue'
import AgentProjectIcons from '../team/AgentProjectIcons.vue'
import { PORTRAIT_SIZES } from '../team/portraitArt'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import { useTeam } from '../team/useTeam'
import AgentNow from './AgentNow.vue'
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

const team = useTeam()
const roster = useRoster()
const control = useAgentControl()
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
</script>

<template>
  <div class="flex flex-col gap-2">
    <div class="flex flex-wrap gap-1.5">
      <UiChip
        :label="`${ALL_TAB_LABEL} ${team.activeAgents.length}`"
        :selected="tab === 'todos'"
        data-testid="now-subtab-todos"
        @press="tab = 'todos'"
      />
      <UiChip
        v-for="row in rows"
        :key="row.agent.id"
        :label="agentTabLabel(row.agent.name, row.activity.queue.length)"
        :selected="tab === row.agent.id"
        :data-testid="`now-subtab-${row.agent.id}`"
        @press="tab = row.agent.id"
      />
    </div>
    <ul v-if="tab === 'todos'" class="flex max-h-96 flex-col gap-2 overflow-y-auto">
      <li
        v-for="row in rows"
        :key="row.agent.id"
        class="flex items-center gap-2 rounded-card border border-line bg-surface px-3 py-2"
        :data-testid="`now-row-${row.agent.id}`"
      >
        <AgentPortrait :who="row.agent.id" :size="PORTRAIT_SIZES.stack" />
        <span class="flex min-w-0 flex-1 flex-col">
          <span class="flex items-center gap-2">
            <span class="text-label font-semibold text-ink">{{ row.agent.name }}</span>
            <AgentProjectIcons :agent="row.agent.id" />
            <UiBadge :status="BADGE_BY_STATUS[row.activity.status]" :label="row.activity.status" />
          </span>
          <span class="wrap-break-word line-clamp-1 text-caption text-muted">{{ row.activity.task ?? NO_TASK_TEXT }}</span>
          <span class="flex items-center gap-1.5 text-caption text-ink">
            <span
              class="size-1.75 rounded-full"
              :class="presenceOf(team.stateOf(row.agent.id), control.isPaused(row.agent.id)).dot"
            />
            {{ row.activity.step }}
          </span>
        </span>
        <UiButton variant="quiet" size="sm" @press="team.togglePause(row.agent.id)">
          {{ control.isPaused(row.agent.id) ? RESUME_LABEL : PAUSE_LABEL }}
        </UiButton>
      </li>
    </ul>
    <AgentNow v-else :key="tab" :agent="tab" />
  </div>
</template>
