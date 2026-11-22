<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import { VIEW_COPY } from './orchestrationLabels'
import type { OrchTeam } from './orchestrationTypes'
import TeamDeleteDialog from './TeamDeleteDialog.vue'
import TeamFlowThumb from './TeamFlowThumb.vue'
import TeamEditDialog, { type TeamEdit } from './TeamEditDialog.vue'
import { forgetFlowDraft, peekFlowDraft } from './useFlowDraft'
import { useOrchestration } from './useOrchestration'

const ELLIPSIS_PATHS = ['M8 3h.01', 'M8 8h.01', 'M8 13h.01']
const AVATAR_SIZE = 24

type Dialog = { kind: 'edit'; team: OrchTeam | null } | { kind: 'delete'; team: OrchTeam }

const emit = defineEmits<{ open: [teamId: string] }>()

const orchestration = useOrchestration()
const roster = useRoster()
const teams = computed<OrchTeam[]>(() => orchestration.snapshot?.teams ?? [])
// Os agentes do card são os nós do fluxo do time, não o elenco inteiro do time.
function flowAgentsOf(teamId: string): Agent[] {
  return peekFlowDraft(teamId)?.agents.value ?? []
}

const dialog = ref<Dialog | null>(null)
const openMenuId = ref<string | null>(null)

function toggleMenu(teamId: string): void {
  openMenuId.value = openMenuId.value === teamId ? null : teamId
}

function openCreate(): void {
  dialog.value = { kind: 'edit', team: null }
}

function openEdit(team: OrchTeam): void {
  openMenuId.value = null
  dialog.value = { kind: 'edit', team }
}

// Só quem está posicionado no time pode liderá-lo (o Ember nunca é posicionado).
function membersOf(teamId: string): Agent[] {
  return (orchestration.snapshot?.placements ?? [])
    .filter((placement) => placement.teamId === teamId)
    .map((placement) => placement.agentId)
}

function openDelete(team: OrchTeam): void {
  openMenuId.value = null
  dialog.value = { kind: 'delete', team }
}

async function saveEdit(edit: TeamEdit): Promise<void> {
  if (dialog.value?.kind !== 'edit') return
  const draft = { id: dialog.value.team?.id ?? null, name: edit.name, instructions: edit.instructions }
  if (await orchestration.saveTeam(draft, edit.leaderAgent)) dialog.value = null
}

// Única porta de exclusão: só o modal de confirmação chama esta função.
async function confirmDelete(): Promise<void> {
  if (dialog.value?.kind !== 'delete') return
  const { id } = dialog.value.team
  if (await orchestration.deleteTeam(id)) {
    forgetFlowDraft(id)
    dialog.value = null
  }
}

function leaderLabel(agent: Agent | null): string {
  return agent === null ? VIEW_COPY.noLeader : `${VIEW_COPY.leaderBadge}: ${roster.nameOf(agent)}`
}

function agentCountLabel(count: number): string {
  return `${count} ${count === 1 ? VIEW_COPY.agentSingular : VIEW_COPY.agentPlural}`
}

function onPointerDown(event: PointerEvent): void {
  if (openMenuId.value === null) return
  if (event.target instanceof Element && event.target.closest('[data-team-menu]')) return
  openMenuId.value = null
}

onMounted(() => window.addEventListener('pointerdown', onPointerDown))
onUnmounted(() => window.removeEventListener('pointerdown', onPointerDown))
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-col gap-4">
    <div class="flex justify-end">
      <UiButton variant="primary" size="lg" @press="openCreate">{{ VIEW_COPY.newTeam }}</UiButton>
    </div>

    <UiEmpty v-if="teams.length === 0" :title="VIEW_COPY.noTeams" />
    <ul v-else class="m-0 grid list-none grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-3 p-0">
      <li v-for="team in teams" :key="team.id" class="flex min-w-0 flex-col rounded-md border border-line bg-surface transition-colors hover:bg-surface-2 motion-reduce:transition-none">
        <div class="flex min-w-0 items-center gap-2 px-4 pt-4">
          <button type="button" class="min-w-0 flex-1 cursor-pointer truncate text-left text-control font-semibold text-ink" @click="emit('open', team.id)">
            {{ team.name }}
          </button>
          <div class="relative shrink-0" data-team-menu>
          <UiButton variant="quiet" size="sm" :aria-label="VIEW_COPY.teamMenu" :title="VIEW_COPY.teamMenu" @press="toggleMenu(team.id)">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
              <path v-for="path in ELLIPSIS_PATHS" :key="path" :d="path" />
            </svg>
          </UiButton>
          <div
            v-if="openMenuId === team.id"
            role="menu"
            class="absolute right-0 top-full z-20 mt-1 flex min-w-40 flex-col gap-0.5 rounded-md border border-line bg-surface p-1 shadow-paper"
          >
            <button
              type="button"
              role="menuitem"
              class="cursor-pointer rounded-md px-3 py-2 text-left text-control text-ink hover:bg-hover"
              @click="openEdit(team)"
            >
              {{ VIEW_COPY.editTeam }}
            </button>
            <button
              type="button"
              role="menuitem"
              class="cursor-pointer rounded-md px-3 py-2 text-left text-control text-danger hover:bg-hover"
              @click="openDelete(team)"
            >
              {{ VIEW_COPY.removeTeam }}
            </button>
          </div>
          </div>
        </div>
        <button type="button" class="flex min-w-0 cursor-pointer flex-col gap-3 p-4 text-left" @click="emit('open', team.id)">
          <TeamFlowThumb :draft="peekFlowDraft(team.id)" />
          <span class="flex flex-wrap gap-x-3 gap-y-1 text-small text-muted">
            <span>{{ leaderLabel(team.leaderAgent) }}</span>
            <span>{{ agentCountLabel(flowAgentsOf(team.id).length) }}</span>
          </span>
          <span class="flex flex-wrap gap-1">
            <span v-for="agent in flowAgentsOf(team.id)" :key="agent" :title="roster.nameOf(agent)" class="inline-flex">
              <AgentPortrait :who="agent" :size="AVATAR_SIZE" />
            </span>
          </span>
        </button>
      </li>
    </ul>

    <TeamEditDialog
      v-if="dialog?.kind === 'edit'"
      :title="dialog.team === null ? VIEW_COPY.newTeam : VIEW_COPY.editTeamTitle"
      :initial-name="dialog.team?.name ?? ''"
      :initial-instructions="dialog.team?.instructions ?? ''"
      :initial-leader="dialog.team?.leaderAgent ?? null"
      :members="dialog.team === null ? [] : membersOf(dialog.team.id)"
      @save="saveEdit"
      @cancel="dialog = null"
    />
    <TeamDeleteDialog
      v-else-if="dialog?.kind === 'delete'"
      :team-name="dialog.team.name"
      @confirm="confirmDelete"
      @cancel="dialog = null"
    />
  </section>
</template>
