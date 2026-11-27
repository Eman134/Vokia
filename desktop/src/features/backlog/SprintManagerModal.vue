<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import UiBadge from '../../ui/UiBadge.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import type { Sprint, SprintStatus } from './backlogTypes'
import { trapFocus } from './dialogFocus'
import { useBacklog } from './useBacklog'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  close: []
}>()

const OVERLAY_CLASSES = 'fixed inset-0 z-50 flex items-center justify-center bg-paper/70 p-4'
const FRAME_CLASSES = 'flex max-h-[calc(100dvh-2rem)] w-160 max-w-full flex-col overflow-hidden rounded-md border border-line bg-surface shadow-paper'
const INPUT_CLASSES = `h-9 w-full rounded-md border border-line bg-surface px-3 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`
const TEXTAREA_CLASSES = `w-full resize-none rounded-md border border-line bg-surface px-3 py-2 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`
const ACTION_BUTTON_CLASSES = `inline-flex h-7 cursor-pointer items-center rounded-sm border border-line bg-surface-2 px-2.5 text-caption text-ink hover:bg-hover disabled:cursor-not-allowed disabled:opacity-45 ${INK_FOCUS_CLASSES}`
const SPRINT_CARD_CLASSES = 'flex flex-col gap-2 rounded-md border border-line bg-bg p-3.5'

const STATUS_GROUP_ORDER: readonly SprintStatus[] = ['ativa', 'planejada', 'encerrada']
const STATUS_GROUP_LABEL: Record<SprintStatus, string> = {
  ativa: 'Ativa',
  planejada: 'Planejadas',
  encerrada: 'Encerradas',
}
const STATUS_BADGE_KIND: Record<SprintStatus, 'trabalhando' | 'backlog' | 'livre'> = {
  ativa: 'trabalhando',
  planejada: 'backlog',
  encerrada: 'livre',
}

const backlog = useBacklog()
const titleId = useId()
const dialog = ref<HTMLElement>()
let previousFocus: HTMLElement | null = null

const editingId = ref<string | null>(null)
const formOpen = ref(false)
const formName = ref('')
const formGoal = ref('')
const formStartDate = ref('')
const formEndDate = ref('')
const formError = ref<string | null>(null)
const busy = ref(false)
const closingId = ref<string | null>(null)
const moveTargetId = ref<string>('')
const deletingId = ref<string | null>(null)

const groupedSprints = computed(() =>
  STATUS_GROUP_ORDER.map((status) => ({
    status,
    label: STATUS_GROUP_LABEL[status],
    sprints: backlog.sprints.filter((sprint) => sprint.status === status),
  })).filter((group) => group.sprints.length > 0),
)

const moveTargetOptions = computed(() =>
  backlog.sprints.filter((sprint) => sprint.id !== closingId.value && sprint.status !== 'encerrada'),
)

function toDateInput(ms: number | null | undefined): string {
  if (!ms) return ''
  const date = new Date(ms)
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function fromDateInput(value: string, endOfDay = false): number | null {
  const trimmed = value.trim()
  if (!trimmed) return null
  const [year, month, day] = trimmed.split('-').map(Number)
  if (!year || !month || !day) return null
  return endOfDay
    ? new Date(year, month - 1, day, 23, 59, 59, 999).getTime()
    : new Date(year, month - 1, day, 0, 0, 0, 0).getTime()
}

function formatDateRange(sprint: Sprint): string | null {
  if (!sprint.startsAt && !sprint.endsAt) return null
  const fmt = (ms: number) => new Date(ms).toLocaleDateString('pt-BR', { day: '2-digit', month: '2-digit' })
  if (sprint.startsAt && sprint.endsAt) return `${fmt(sprint.startsAt)} – ${fmt(sprint.endsAt)}`
  if (sprint.startsAt) return `A partir de ${fmt(sprint.startsAt)}`
  return `Até ${fmt(sprint.endsAt!)}`
}

function progressPercent(sprint: Sprint): number {
  if (sprint.totalItems <= 0) return 0
  return Math.round((sprint.doneItems / sprint.totalItems) * 100)
}

function resetForm(): void {
  editingId.value = null
  formOpen.value = false
  formName.value = ''
  formGoal.value = ''
  formStartDate.value = ''
  formEndDate.value = ''
  formError.value = null
}

function openCreateForm(): void {
  resetForm()
  formOpen.value = true
}

function openEditForm(sprint: Sprint): void {
  editingId.value = sprint.id
  formName.value = sprint.name
  formGoal.value = sprint.goal
  formStartDate.value = toDateInput(sprint.startsAt)
  formEndDate.value = toDateInput(sprint.endsAt)
  formError.value = null
  formOpen.value = true
}

async function submitForm(): Promise<void> {
  const name = formName.value.trim()
  if (!name) {
    formError.value = 'Informe o nome da sprint.'
    return
  }
  const startsAt = fromDateInput(formStartDate.value, false)
  const endsAt = fromDateInput(formEndDate.value, true)
  if (startsAt !== null && endsAt !== null && endsAt < startsAt) {
    formError.value = 'A data final não pode ser anterior à data inicial.'
    return
  }
  formError.value = null
  busy.value = true
  try {
    if (editingId.value) {
      const updated = await backlog.updateSprint(editingId.value, {
        name,
        goal: formGoal.value.trim(),
        startsAt,
        endsAt,
      })
      if (updated) resetForm()
    } else {
      const created = await backlog.createSprint({
        name,
        goal: formGoal.value.trim() || undefined,
        startsAt,
        endsAt,
      })
      if (created) resetForm()
    }
  } finally {
    busy.value = false
  }
}

async function activateSprint(sprint: Sprint): Promise<void> {
  busy.value = true
  try {
    await backlog.updateSprint(sprint.id, { status: 'ativa' })
  } finally {
    busy.value = false
  }
}

function startCloseSprint(sprint: Sprint): void {
  closingId.value = sprint.id
  deletingId.value = null
  moveTargetId.value = ''
}

async function confirmCloseSprint(sprintId: string): Promise<void> {
  busy.value = true
  try {
    const target = moveTargetId.value || null
    const closed = await backlog.closeSprint(sprintId, target)
    if (closed) {
      closingId.value = null
      moveTargetId.value = ''
    }
  } finally {
    busy.value = false
  }
}

async function confirmDeleteSprint(sprintId: string): Promise<void> {
  busy.value = true
  try {
    const deleted = await backlog.deleteSprint(sprintId)
    if (deleted) deletingId.value = null
  } finally {
    busy.value = false
  }
}

function closeModal(): void {
  resetForm()
  closingId.value = null
  deletingId.value = null
  emit('close')
}

function restoreFocus(): void {
  previousFocus?.focus()
  previousFocus = null
}

watch(
  () => props.open,
  async (isOpen) => {
    if (isOpen) {
      previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
      await nextTick()
      dialog.value?.focus()
    } else {
      restoreFocus()
    }
  },
)

onBeforeUnmount(restoreFocus)
</script>

<template>
  <Teleport to="body">
    <div v-if="open" :class="OVERLAY_CLASSES" @click.self="closeModal()">
      <div
        ref="dialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="titleId"
        tabindex="-1"
        :class="FRAME_CLASSES"
        data-testid="sprint-manager-modal"
        @keydown.esc.stop="closeModal()"
        @keydown.tab="trapFocus($event, dialog)"
      >
        <header class="flex items-center justify-between gap-3 border-b border-line px-6 py-4">
          <div class="min-w-0">
            <h2 :id="titleId" class="text-section text-ink">Sprints e milestones</h2>
            <p class="text-caption text-muted">Planeje ciclos, acompanhe entregas e mova pendências ao encerrar.</p>
          </div>
          <div class="flex items-center gap-2">
            <UiButton v-if="!formOpen" variant="primary" size="sm" data-testid="sprint-new-btn" @press="openCreateForm()">
              Nova sprint
            </UiButton>
            <UiCloseButton @press="closeModal()" />
          </div>
        </header>

        <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-6 py-5">
          <form
            v-if="formOpen"
            class="flex flex-col gap-3 rounded-md border border-line bg-surface-2 p-4"
            data-testid="sprint-form"
            @submit.prevent="submitForm()"
          >
            <h3 class="text-label font-semibold text-ink">{{ editingId ? 'Editar sprint' : 'Nova sprint' }}</h3>
            <label class="flex flex-col gap-1">
              <span class="text-caption font-medium text-ink">Nome</span>
              <input
                v-model="formName"
                type="text"
                maxlength="120"
                placeholder="Ex.: Sprint 1 — Autenticação"
                :class="INPUT_CLASSES"
                data-testid="sprint-name-input"
              />
            </label>
            <label class="flex flex-col gap-1">
              <span class="text-caption font-medium text-ink">Objetivo (opcional)</span>
              <textarea
                v-model="formGoal"
                rows="2"
                maxlength="1000"
                placeholder="O que precisa estar pronto ao fim deste ciclo?"
                :class="TEXTAREA_CLASSES"
                data-testid="sprint-goal-input"
              />
            </label>
            <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <label class="flex flex-col gap-1">
                <span class="text-caption font-medium text-ink">Início</span>
                <input v-model="formStartDate" type="date" :class="INPUT_CLASSES" data-testid="sprint-start-input" />
              </label>
              <label class="flex flex-col gap-1">
                <span class="text-caption font-medium text-ink">Fim</span>
                <input v-model="formEndDate" type="date" :class="INPUT_CLASSES" data-testid="sprint-end-input" />
              </label>
            </div>
            <p v-if="formError" role="alert" class="text-caption font-medium text-danger">{{ formError }}</p>
            <div class="flex justify-end gap-2">
              <UiButton type="button" variant="quiet" size="sm" :disabled="busy" @press="resetForm()">
                Cancelar
              </UiButton>
              <UiButton type="submit" variant="primary" size="sm" :disabled="busy" data-testid="sprint-save-btn">
                {{ editingId ? 'Salvar alterações' : 'Criar sprint' }}
              </UiButton>
            </div>
          </form>

          <p v-if="backlog.sprints.length === 0 && !formOpen" class="py-6 text-center text-caption text-muted">
            Nenhuma sprint criada neste projeto ainda.
          </p>

          <section v-for="group in groupedSprints" :key="group.status" class="flex flex-col gap-2">
            <h3 class="text-label font-semibold text-muted">{{ group.label }}</h3>
            <article v-for="sprint in group.sprints" :key="sprint.id" :class="SPRINT_CARD_CLASSES" :data-testid="`sprint-row-${sprint.id}`">
              <div class="flex flex-wrap items-start justify-between gap-2">
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-2">
                    <span class="text-body font-semibold text-ink">{{ sprint.name }}</span>
                    <UiBadge :status="STATUS_BADGE_KIND[sprint.status]" :label="STATUS_GROUP_LABEL[sprint.status]" />
                    <span v-if="formatDateRange(sprint)" class="text-tiny text-muted tabular-nums">
                      {{ formatDateRange(sprint) }}
                    </span>
                  </div>
                  <p v-if="sprint.goal" class="mt-1 text-caption text-muted">{{ sprint.goal }}</p>
                </div>
                <div class="flex flex-wrap items-center gap-1.5">
                  <button
                    v-if="sprint.status === 'planejada'"
                    type="button"
                    :disabled="busy"
                    :class="ACTION_BUTTON_CLASSES"
                    data-testid="sprint-activate-btn"
                    @click="activateSprint(sprint)"
                  >
                    Ativar
                  </button>
                  <button
                    v-if="sprint.status !== 'encerrada'"
                    type="button"
                    :disabled="busy"
                    :class="ACTION_BUTTON_CLASSES"
                    @click="openEditForm(sprint)"
                  >
                    Editar
                  </button>
                  <button
                    v-if="sprint.status === 'ativa'"
                    type="button"
                    :disabled="busy"
                    :class="ACTION_BUTTON_CLASSES"
                    data-testid="sprint-close-btn"
                    @click="startCloseSprint(sprint)"
                  >
                    Encerrar
                  </button>
                  <button
                    type="button"
                    :disabled="busy"
                    class="text-danger"
                    :class="ACTION_BUTTON_CLASSES"
                    data-testid="sprint-delete-btn"
                    @click="deletingId = sprint.id; closingId = null"
                  >
                    Excluir
                  </button>
                </div>
              </div>

              <div class="flex items-center gap-3">
                <div class="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-2">
                  <div
                    class="h-full rounded-full bg-ok transition-all"
                    :style="{ width: `${progressPercent(sprint)}%` }"
                  />
                </div>
                <span class="text-tiny text-muted tabular-nums">
                  {{ sprint.doneItems }}/{{ sprint.totalItems }} ({{ progressPercent(sprint) }}%)
                </span>
              </div>

              <div v-if="closingId === sprint.id" class="mt-1 flex flex-col gap-2 rounded-sm border border-line bg-surface p-3">
                <p class="text-caption text-ink">Encerrar {{ sprint.name }} e opcionalmente mover pendências:</p>
                <select
                  v-model="moveTargetId"
                  class="h-8 cursor-pointer rounded-md border border-line bg-surface px-2 text-caption text-ink"
                  aria-label="Mover itens pendentes para"
                >
                  <option value="">Deixar pendentes sem sprint</option>
                  <option v-for="target in moveTargetOptions" :key="target.id" :value="target.id">
                    Mover para: {{ target.name }}
                  </option>
                </select>
                <div class="flex justify-end gap-2">
                  <UiButton type="button" variant="quiet" size="sm" :disabled="busy" @press="closingId = null">
                    Cancelar
                  </UiButton>
                  <UiButton type="button" variant="primary" size="sm" :disabled="busy" @press="confirmCloseSprint(sprint.id)">
                    Confirmar encerramento
                  </UiButton>
                </div>
              </div>

              <div v-if="deletingId === sprint.id" class="mt-1 flex flex-wrap items-center justify-between gap-2 rounded-sm border border-line bg-surface p-3">
                <span class="text-caption text-ink">Excluir esta sprint? Os cards vinculados ficam sem sprint.</span>
                <div class="flex gap-2">
                  <UiButton type="button" variant="quiet" size="sm" :disabled="busy" @press="deletingId = null">
                    Cancelar
                  </UiButton>
                  <UiButton type="button" variant="danger" size="sm" :disabled="busy" @press="confirmDeleteSprint(sprint.id)">
                    Confirmar exclusão
                  </UiButton>
                </div>
              </div>
            </article>
          </section>
        </div>
      </div>
    </div>
  </Teleport>
</template>
